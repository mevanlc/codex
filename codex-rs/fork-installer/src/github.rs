//! Reads fork releases and Actions artifacts from the GitHub REST API.
//!
//! Releases are public, so no credentials are needed; a token from the
//! environment raises the anonymous rate limit. Downloads are verified
//! against the SHA-256 digest GitHub records for each asset.

use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::blocking::Response;
use reqwest::header::ACCEPT;
use reqwest::header::AUTHORIZATION;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use sha2::Digest;
use sha2::Sha256;

use crate::cancel::Cancel;
use crate::layout::REPO;

pub(crate) const API_URL: &str = "https://api.github.com";
const PAGE_SIZE: usize = 100;
const MAX_PAGES: usize = 50;
const API_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Release {
    pub(crate) tag_name: String,
    pub(crate) published_at: Option<String>,
    pub(crate) created_at: Option<String>,
    #[serde(default)]
    pub(crate) draft: bool,
    #[serde(default)]
    pub(crate) prerelease: bool,
    #[serde(default)]
    pub(crate) assets: Vec<Asset>,
}

impl Release {
    pub(crate) fn published(&self) -> Option<&str> {
        self.published_at.as_deref().or(self.created_at.as_deref())
    }

    pub(crate) fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.name == name)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Asset {
    pub(crate) name: String,
    pub(crate) updated_at: Option<String>,
    pub(crate) browser_download_url: String,
    pub(crate) size: Option<u64>,
    pub(crate) digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ArtifactPage {
    #[serde(default)]
    artifacts: Vec<Artifact>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Artifact {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) created_at: String,
    #[serde(default)]
    pub(crate) expired: bool,
    pub(crate) archive_download_url: String,
    pub(crate) digest: Option<String>,
    pub(crate) workflow_run: Option<WorkflowRun>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct WorkflowRun {
    pub(crate) id: u64,
}

pub(crate) struct GitHub {
    client: Client,
    api: String,
    token: Option<String>,
    cancel: Cancel,
}

impl GitHub {
    pub(crate) fn new(api: &str, token: Option<String>, cancel: Cancel) -> Result<Self> {
        codex_utils_rustls_provider::ensure_rustls_crypto_provider();
        let client = Client::builder()
            .use_rustls_tls()
            .user_agent(concat!("codex-fork-installer/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(30))
            // Package downloads are large; API requests set their own timeout.
            .timeout(None)
            .build()
            .context("failed to create HTTP client")?;
        Ok(Self {
            client,
            api: api.trim_end_matches('/').to_string(),
            token,
            cancel,
        })
    }

    pub(crate) fn has_token(&self) -> bool {
        self.token.is_some()
    }

    fn get(&self, url: &str, accept: &str) -> reqwest::blocking::RequestBuilder {
        let request = self.client.get(url).header(ACCEPT, accept);
        match &self.token {
            Some(token) => request.header(AUTHORIZATION, format!("Bearer {token}")),
            None => request,
        }
    }

    /// GET an API path, returning None for 404.
    fn api_json<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>> {
        self.cancel.check()?;
        let url = format!("{}/{path}", self.api);
        let response = self
            .get(&url, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .timeout(API_TIMEOUT)
            .send()
            .with_context(|| format!("GitHub metadata request failed: {url}"))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = check_status(response, &url, self.has_token())?;
        response
            .json()
            .map(Some)
            .with_context(|| format!("GitHub returned invalid metadata for {url}"))
    }

    /// All releases, newest pages first as GitHub returns them.
    pub(crate) fn releases(&self) -> Result<Vec<Release>> {
        let mut releases = Vec::new();
        for page in 1..=MAX_PAGES {
            let batch: Vec<Release> = self
                .api_json(&format!(
                    "repos/{REPO}/releases?per_page={PAGE_SIZE}&page={page}"
                ))?
                .with_context(|| format!("GitHub repository {REPO} was not found"))?;
            let last = batch.len() < PAGE_SIZE;
            releases.extend(batch);
            if last {
                break;
            }
        }
        Ok(releases)
    }

    pub(crate) fn release_by_tag(&self, tag: &str) -> Result<Option<Release>> {
        self.api_json(&format!("repos/{REPO}/releases/tags/{tag}"))
    }

    /// Non-expired artifacts with this exact name, from every page.
    pub(crate) fn artifacts(&self, name: &str) -> Result<Vec<Artifact>> {
        let mut artifacts = Vec::new();
        for page in 1..=MAX_PAGES {
            let batch: ArtifactPage = self
                .api_json(&format!(
                    "repos/{REPO}/actions/artifacts?per_page={PAGE_SIZE}&page={page}&name={name}"
                ))?
                .with_context(|| format!("GitHub repository {REPO} was not found"))?;
            let last = batch.artifacts.len() < PAGE_SIZE;
            artifacts.extend(
                batch
                    .artifacts
                    .into_iter()
                    .filter(|artifact| artifact.name == name && !artifact.expired),
            );
            if last {
                break;
            }
        }
        Ok(artifacts)
    }

    /// Stream `url` into `destination`, requiring the published SHA-256 digest.
    pub(crate) fn download(
        &self,
        url: &str,
        destination: &Path,
        digest: Option<&str>,
        size: Option<u64>,
    ) -> Result<()> {
        let expected = digest
            .and_then(|digest| digest.strip_prefix("sha256:"))
            .with_context(|| format!("GitHub published no SHA-256 digest for {url}"))?
            .to_ascii_lowercase();
        self.cancel.check()?;
        let response = self
            .get(url, "application/octet-stream")
            .send()
            .with_context(|| format!("download failed: {url}"))?;
        let mut response = check_status(response, url, self.has_token())?;
        let mut file = std::fs::File::create(destination)
            .with_context(|| format!("failed to create {}", destination.display()))?;
        let mut hasher = Sha256::new();
        let mut written = 0u64;
        let mut buffer = vec![0; 1 << 16];
        loop {
            self.cancel.check()?;
            let read = response
                .read(&mut buffer)
                .with_context(|| format!("download failed: {url}"))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            file.write_all(&buffer[..read])?;
            written += read as u64;
        }
        file.sync_all()?;
        if let Some(size) = size {
            anyhow::ensure!(
                written == size,
                "download of {url} was truncated: {written} of {size} bytes"
            );
        }
        let actual = format!("{:x}", hasher.finalize());
        anyhow::ensure!(
            actual == expected,
            "download of {url} does not match its published digest (expected sha256:{expected}, got sha256:{actual})"
        );
        Ok(())
    }
}

fn check_status(response: Response, url: &str, has_token: bool) -> Result<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let rate_limited = (status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS)
        && response
            .headers()
            .get("x-ratelimit-remaining")
            .is_some_and(|remaining| remaining == "0");
    let body = response.text().unwrap_or_default();
    let detail = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|body| body["message"].as_str().map(str::to_string))
        .unwrap_or(body);
    let hint = if rate_limited && !has_token {
        "; set GH_TOKEN to raise the anonymous rate limit"
    } else if status == StatusCode::UNAUTHORIZED && !has_token {
        "; set GH_TOKEN or run `gh auth login`"
    } else {
        ""
    };
    anyhow::bail!(
        "GitHub request failed with status {status}: {url}: {}{hint}",
        detail.trim()
    )
}
