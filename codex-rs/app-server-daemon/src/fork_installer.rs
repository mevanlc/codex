//! Downloads the fork installer that performs every package update.
//!
//! Each update runs the installer from the newest release, so fixes to the
//! installation logic reach existing installs without a new `codex` binary.

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::RouteAwareClientPool;
use codex_install_context::fork;
use sha2::Digest;

const PAGE_SIZE: usize = 100;
const MAX_PAGES: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InstallerResponse {
    Success(Vec<u8>),
    Unsuccessful { status: u16 },
}

/// HTTP boundary used to read release metadata and download the installer.
///
/// Implementations must issue a GET for the supplied URL, return exact response bytes for a
/// successful status, and report a non-success status without buffering its response body.
pub(crate) trait InstallerHttp: Send + Sync {
    fn get<'a>(
        &'a self,
        url: &'a str,
    ) -> impl std::future::Future<Output = Result<InstallerResponse>> + Send + 'a;
}

impl InstallerHttp for RouteAwareClientPool {
    async fn get(&self, url: &str) -> Result<InstallerResponse> {
        let response = RouteAwareClientPool::get(self, url)
            // The GitHub API rejects requests without a user agent.
            .header(
                "user-agent",
                concat!("codex-app-server-daemon/", env!("CARGO_PKG_VERSION")),
            )
            .send()
            .await
            .with_context(|| format!("failed to fetch {url}"))?;
        if !response.status().is_success() {
            return Ok(InstallerResponse::Unsuccessful {
                status: response.status().as_u16(),
            });
        }
        let body = response
            .bytes()
            .await
            .with_context(|| format!("failed to read {url}"))?
            .to_vec();
        Ok(InstallerResponse::Success(body))
    }
}

async fn get(http: &impl InstallerHttp, url: &str) -> Result<Vec<u8>> {
    match http.get(url).await? {
        InstallerResponse::Success(body) => Ok(body),
        InstallerResponse::Unsuccessful { status } => {
            anyhow::bail!("standalone Codex updater request failed with status {status}: {url}")
        }
    }
}

pub(crate) async fn fetch_releases(http: &impl InstallerHttp) -> Result<Vec<fork::Release>> {
    let mut releases = Vec::new();
    for page in 1..=MAX_PAGES {
        let url = format!("{}?per_page={PAGE_SIZE}&page={page}", fork::RELEASES_URL);
        let batch: Vec<fork::Release> = serde_json::from_slice(&get(http, &url).await?)
            .context("GitHub returned invalid release metadata")?;
        let last = batch.len() < PAGE_SIZE;
        releases.extend(batch);
        if last {
            break;
        }
    }
    Ok(releases)
}

/// Download the installer of the newest release that publishes one for this
/// platform, verified against the digest GitHub records for it.
pub(crate) async fn fetch_installer(
    http: &impl InstallerHttp,
    releases: &[fork::Release],
) -> Result<Vec<u8>> {
    let name = fork::installer_asset_name().context("unsupported fork platform")?;
    let asset = fork::newest_release(releases, &name)
        .and_then(|release| release.assets.iter().find(|asset| asset.name == name))
        .with_context(|| format!("no published fork release carries {name}"))?;
    let url = asset
        .browser_download_url
        .as_deref()
        .with_context(|| format!("GitHub published no download URL for {name}"))?;
    let expected = asset
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .with_context(|| format!("GitHub published no SHA-256 digest for {name}"))?;
    let bytes = get(http, url).await?;
    let actual = format!("{:x}", sha2::Sha256::digest(&bytes));
    anyhow::ensure!(
        actual.eq_ignore_ascii_case(expected),
        "{name} does not match its published digest"
    );
    Ok(bytes)
}

/// Write the installer as an executable inside `directory`.
pub(crate) fn write_installer(directory: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let path = directory.join(format!(
        "{}{}",
        fork::INSTALLER_NAME,
        std::env::consts::EXE_SUFFIX
    ));
    std::fs::write(&path, bytes).with_context(|| format!("failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(path)
}

/// Whether `release` was installed from the newest release carrying this
/// platform's package, so a scheduled check has nothing to install.
pub(crate) fn newest_release_installed(releases: &[fork::Release], release: &Path) -> bool {
    let Some(asset) = fork::release_asset_name() else {
        return true;
    };
    let Some(newest) = fork::newest_release(releases, &asset) else {
        return true;
    };
    fork::installed_release_tag(release).as_deref() == Some(newest.tag_name.as_str())
}

/// Download the newest installer into `directory` for `codex update`.
pub async fn download_installer(
    http_client_factory: HttpClientFactory,
    directory: &Path,
) -> Result<PathBuf> {
    let http = RouteAwareClientPool::new_without_request_logging(
        http_client_factory,
        ClientRouteClass::Other,
    );
    let releases = fetch_releases(&http).await?;
    let bytes = fetch_installer(&http, &releases).await?;
    write_installer(directory, &bytes)
}

/// The release the CLI package's `current` selection resolves to. The
/// installer leaves it unchanged when the installed package is current.
pub fn selected_cli_release() -> Option<PathBuf> {
    let root = codex_utils_home_dir::find_fork_install_root().ok()?;
    std::fs::canonicalize(root.join("packages").join("cli").join("current")).ok()
}
