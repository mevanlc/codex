//! Installs complete mevanlc/codex packages into `~/.local/share/mevanlc-codex`
//! with commands in `~/.local/bin`.
//!
//! Every release publishes this installer for each target. First installs,
//! `codex update`, and the daemon updater all run the newest one, so fixes to
//! installation logic reach existing installs without a new `codex` binary.

mod cancel;
mod digest;
mod fsutil;
mod github;
mod install;
mod layout;
mod lock;
mod package;
mod probe;
mod selection;
#[cfg(test)]
mod test_support;

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;

pub use cancel::Cancel;
pub use cancel::Cancelled;
pub use install::Guards;

use github::GitHub;
use install::InstallOptions;
use install::Installer;
use install::Output;
use install::Source;
use install::SourceId;
use layout::Layout;
use layout::REPO;
use probe::ProcessProbe;

/// Where the package comes from.
#[derive(Clone, Debug)]
pub enum Mode {
    /// The newest published non-prerelease carrying this platform's package.
    Release,
    /// The newest non-expired Actions artifact for this platform (needs a token).
    Artifact,
    /// A local package archive.
    Archive(PathBuf),
}

#[derive(Clone, Debug)]
pub struct Request {
    pub mode: Mode,
    /// Exact release tag or fork version: a deliberate pin that permits downgrades.
    pub version: Option<String>,
    pub list_available: bool,
    pub force: bool,
}

/// Process environment the installer depends on, gathered in one place.
#[derive(Clone, Debug)]
pub struct Environment {
    pub user_home: PathBuf,
    /// `CODEX_ARTIFACT_NAME`: `codex-<target>`, overriding this build's target.
    pub artifact_name: Option<String>,
    /// `CODEX_RELEASE_ASSET_NAME`: the package asset to install.
    pub release_asset_name: Option<String>,
    /// `CODEX_INSTALL_DAEMON_ONLY=1`: install the daemon's package root only.
    pub daemon_only: bool,
    pub guards: Guards,
    /// `GH_TOKEN` or `GITHUB_TOKEN`.
    pub token: Option<String>,
    pub temp_root: Option<PathBuf>,
    pub api_url: String,
}

impl Environment {
    pub fn from_process() -> Result<Self> {
        let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        let flag = |name: &str| var(name).as_deref() == Some("1");
        // Termux has no /tmp; Python's tempfile used $PREFIX/tmp there as well.
        let temp_root = if cfg!(unix) && var("TMPDIR").is_none() {
            var("PREFIX").map(|prefix| Path::new(&prefix).join("tmp"))
        } else {
            None
        };
        Ok(Self {
            user_home: codex_utils_home_dir::find_fork_user_home()?,
            artifact_name: var("CODEX_ARTIFACT_NAME"),
            release_asset_name: var("CODEX_RELEASE_ASSET_NAME"),
            daemon_only: flag("CODEX_INSTALL_DAEMON_ONLY"),
            guards: Guards {
                if_latest: flag("CODEX_INSTALL_IF_LATEST"),
                if_current: flag("CODEX_INSTALL_IF_CURRENT"),
                from_release: var("CODEX_UPDATE_FROM_RELEASE"),
            },
            token: var("GH_TOKEN").or_else(|| var("GITHUB_TOKEN")),
            temp_root,
            api_url: github::API_URL.to_string(),
        })
    }
}

pub fn run(request: &Request, environment: &Environment, cancel: &Cancel) -> Result<()> {
    let installer = Installer {
        layout: Layout::new(
            &environment.user_home,
            environment.artifact_name.as_deref(),
            environment.release_asset_name.as_deref(),
            environment.daemon_only,
        )?,
        guards: environment.guards.clone(),
        probe: Box::new(ProcessProbe {
            timeout: Duration::from_secs(30),
            cancel: cancel.clone(),
        }),
        cancel: cancel.clone(),
        output: Output::stdout(),
        lock_timeout: Duration::from_secs(60),
    };
    installer.run(request, environment)
}

/// A resolved package source and how to obtain its archive.
struct Plan {
    source: Source,
    description: String,
    fetch: Fetch,
}

enum Fetch {
    Local(PathBuf),
    Asset {
        url: String,
        digest: Option<String>,
        size: Option<u64>,
    },
    Artifact {
        url: String,
        digest: Option<String>,
    },
}

impl Installer {
    fn run(&self, request: &Request, environment: &Environment) -> Result<()> {
        let layout = &self.layout;
        let client = match &request.mode {
            Mode::Archive(_) => None,
            Mode::Artifact => {
                let token = environment
                    .token
                    .clone()
                    .or_else(|| gh_auth_token(&self.cancel))
                    .context(
                        "--artifact needs a GitHub token: set GH_TOKEN or run `gh auth login`",
                    )?;
                Some(GitHub::new(
                    &environment.api_url,
                    Some(token),
                    self.cancel.clone(),
                )?)
            }
            Mode::Release => Some(GitHub::new(
                &environment.api_url,
                environment.token.clone(),
                self.cancel.clone(),
            )?),
        };
        let github = || client.as_ref().context("a local archive needs no download");
        let plan = match &request.mode {
            Mode::Archive(path) => {
                let modified = std::fs::metadata(path)
                    .and_then(|metadata| metadata.modified())
                    .with_context(|| format!("cannot read package archive {}", path.display()))?;
                Plan {
                    source: Source {
                        id: SourceId::Archive(
                            path.file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                        ),
                        source_epoch: chrono::DateTime::<chrono::Utc>::from(modified).timestamp(),
                        published_epoch: None,
                    },
                    description: format!("local archive {}", path.display()),
                    fetch: Fetch::Local(path.clone()),
                }
            }
            Mode::Release => {
                let github = github()?;
                let release = match &request.version {
                    Some(version) => {
                        let mut tags = vec![version.clone()];
                        for tag in [format!("fork-v{version}"), format!("v{version}")] {
                            if !tags.contains(&tag) {
                                tags.push(tag);
                            }
                        }
                        let mut found = None;
                        for tag in tags {
                            if let Some(release) = github.release_by_tag(&tag)? {
                                found = Some(release);
                                break;
                            }
                        }
                        found
                            .with_context(|| format!("no release matching {version:?} in {REPO}"))?
                    }
                    None => {
                        let releases = github.releases()?;
                        // GitHub does not order releases by publication time,
                        // and a newer partial release must not hide a complete one.
                        let mut rows: Vec<_> = releases
                            .iter()
                            .filter(|release| {
                                !release.draft && release.asset(&layout.asset).is_some()
                            })
                            .collect();
                        rows.sort_by_key(|release| {
                            std::cmp::Reverse(release.published().unwrap_or(""))
                        });
                        let default = rows.iter().find(|release| !release.prerelease);
                        if request.list_available {
                            self.output.line("TAG  PUBLISHED  LAYOUT".to_string());
                            for release in &rows {
                                let is_default =
                                    default.is_some_and(|default| std::ptr::eq(*default, *release));
                                self.output.line(format!(
                                    "{}  {}  package{}",
                                    release.tag_name,
                                    release.published_at.as_deref().unwrap_or(""),
                                    if is_default { " (default)" } else { "" }
                                ));
                            }
                            return Ok(());
                        }
                        (*default.with_context(|| {
                            format!("no published non-prerelease carries {}", layout.asset)
                        })?)
                        .clone()
                    }
                };
                let asset = release
                    .asset(&layout.asset)
                    .filter(|_| !release.draft)
                    .with_context(|| {
                        format!(
                            "requested release in {REPO} does not include asset {}",
                            layout.asset
                        )
                    })?;
                Plan {
                    source: Source {
                        id: SourceId::Tag(release.tag_name.clone()),
                        source_epoch: epoch(asset.updated_at.as_deref().or(release.published()))?,
                        published_epoch: Some(epoch(release.published())?),
                    },
                    description: format!("release assets from tag {}", release.tag_name),
                    fetch: Fetch::Asset {
                        url: asset.browser_download_url.clone(),
                        digest: asset.digest.clone(),
                        size: asset.size,
                    },
                }
            }
            Mode::Artifact => {
                let github = github()?;
                let artifact = github
                    .artifacts(&layout.artifact)?
                    .into_iter()
                    .max_by(|left, right| left.created_at.cmp(&right.created_at))
                    .with_context(|| {
                        format!(
                            "no non-expired artifact named {} found in {REPO}",
                            layout.artifact
                        )
                    })?;
                let created = epoch(Some(&artifact.created_at))?;
                Plan {
                    source: Source {
                        id: SourceId::Artifact(artifact.id),
                        source_epoch: created,
                        published_epoch: Some(created),
                    },
                    description: format!(
                        "artifact {} from run {}",
                        artifact.id,
                        artifact
                            .workflow_run
                            .as_ref()
                            .map_or_else(|| "unknown".to_string(), |run| run.id.to_string())
                    ),
                    fetch: Fetch::Artifact {
                        url: artifact.archive_download_url,
                        digest: artifact.digest,
                    },
                }
            }
        };

        let explicit = request.version.is_some() || matches!(request.mode, Mode::Archive(_));
        let step = |message: String| self.output.step(message);
        step(format!(
            "Installer: {} {}",
            codex_install_context::fork::INSTALLER_NAME,
            env!("CARGO_PKG_VERSION")
        ));
        step(format!("Repository: {REPO}"));
        step(format!(
            "Mode: {}",
            match request.mode {
                Mode::Release => "release",
                Mode::Artifact => "artifact",
                Mode::Archive(_) => "archive",
            }
        ));
        if let Some(version) = &request.version {
            step(format!("Requested version: {version}"));
        }
        step(format!("Artifact name: {}", layout.artifact));
        step(format!("Release asset: {}", layout.asset));
        step(format!("Package root: {}", layout.root.display()));
        self.report_existing();
        step(format!("Requested source: {}", plan.description));
        step(format!(
            "Source timestamp: {}",
            timestamp(plan.source.source_epoch)
        ));
        if request.version.is_some() {
            step("Explicit version requested; skipping package currency check".to_string());
        } else if matches!(request.mode, Mode::Archive(_)) {
            step("Local archive requested; skipping package currency check".to_string());
        } else if request.force {
            step("Forced install requested; skipping package currency check".to_string());
        }

        let temp = match &environment.temp_root {
            Some(root) => {
                std::fs::create_dir_all(root)?;
                tempfile::Builder::new()
                    .prefix("codex-fork-installer-")
                    .tempdir_in(root)?
            }
            None => tempfile::Builder::new()
                .prefix("codex-fork-installer-")
                .tempdir()?,
        };
        let fetch = || match plan.fetch {
            Fetch::Local(path) => Ok(path),
            Fetch::Asset { url, digest, size } => {
                step(format!("Downloading {}", plan.description));
                let archive = temp.path().join(&layout.asset);
                github()?.download(&url, &archive, digest.as_deref(), size)?;
                Ok(archive)
            }
            Fetch::Artifact { url, digest } => {
                step(format!("Downloading {}", plan.description));
                let container = temp.path().join("artifact.zip");
                github()?.download(&url, &container, digest.as_deref(), None)?;
                let archive = temp.path().join(&layout.asset);
                unpack_artifact(&container, &layout.asset, &archive)?;
                Ok(archive)
            }
        };
        self.install_with(
            &plan.source,
            InstallOptions {
                check_currency: !explicit && !request.force,
                follows_latest: matches!(request.mode, Mode::Release) && request.version.is_none(),
            },
            fetch,
        )
    }

    fn report_existing(&self) {
        let layout = &self.layout;
        for (command, label) in [
            ("codex", "Codex"),
            ("codex-code-mode-host", "code-mode host"),
        ] {
            let name = layout.target.exe(command);
            let selected = layout.current().join("bin").join(&name);
            let binary = if layout.daemon_only {
                selected
            } else {
                let visible = layout.bin.join(&name);
                if cfg!(windows) && !visible.exists() {
                    selected
                } else {
                    visible
                }
            };
            let Ok(metadata) = std::fs::metadata(&binary) else {
                self.output.step(if command == "codex" {
                    "Existing version: not installed"
                } else {
                    "Existing code-mode host: not installed"
                });
                continue;
            };
            if command == "codex" {
                let version = self
                    .probe
                    .version(&binary)
                    .unwrap_or_else(|_| "unavailable (could not run --version)".to_string());
                self.output.step(format!("Existing version: {version}"));
            }
            if let Ok(modified) = metadata.modified() {
                let modified = chrono::DateTime::<chrono::Utc>::from(modified).timestamp();
                self.output
                    .step(format!("Existing {label} mtime: {}", timestamp(modified)));
            }
        }
    }
}

/// An Actions artifact is a zip wrapping exactly the package archive.
fn unpack_artifact(container: &Path, asset: &str, destination: &Path) -> Result<()> {
    let mut artifact = zip::ZipArchive::new(std::fs::File::open(container)?)
        .context("downloaded artifact is not a zip archive")?;
    anyhow::ensure!(
        artifact.len() == 1 && artifact.by_index(0)?.name()? == asset,
        "download must contain exactly one complete package archive: {asset}"
    );
    let mut member = artifact.by_index(0)?;
    let mut out = std::fs::File::create(destination)?;
    std::io::copy(&mut member, &mut out)?;
    Ok(())
}

/// The token `gh` already holds, so `--artifact` works without extra setup.
fn gh_auth_token(cancel: &Cancel) -> Option<String> {
    let mut command = Command::new(if cfg!(windows) { "gh.exe" } else { "gh" });
    command.args(["auth", "token"]);
    let output = probe::output_with_timeout(command, Duration::from_secs(15), cancel).ok()?;
    let token = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (output.status.success() && !token.is_empty()).then_some(token)
}

fn epoch(value: Option<&str>) -> Result<i64> {
    let value = value.context("GitHub metadata has no timestamp")?;
    Ok(chrono::DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("invalid GitHub timestamp {value:?}"))?
        .timestamp())
}

fn timestamp(epoch: i64) -> String {
    let time = chrono::DateTime::from_timestamp(epoch, 0)
        .map(|time| time.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!("{time} (epoch: {epoch})")
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
