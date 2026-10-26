//! The installation transaction. Everything that changes the installation
//! happens under the install lock, and any failure before the final
//! validation restores the previous selection, commands, and marker.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::cancel::Cancel;
use crate::digest;
use crate::digest::DIGEST_FORMAT;
use crate::digest::RECEIPT;
use crate::fsutil::atomic_write;
use crate::fsutil::lexists;
use crate::fsutil::random_hex;
use crate::fsutil::read_marker;
use crate::fsutil::remove_file_if_exists;
use crate::fsutil::rename_with_retry;
use crate::fsutil::resolve_lenient;
use crate::layout::COMMANDS;
use crate::layout::Layout;
use crate::layout::REPO;
use crate::lock;
use crate::package;
use crate::probe::Probe;
use crate::selection;

/// Conditions from the daemon updater under which an install may proceed.
#[derive(Clone, Debug, Default)]
pub struct Guards {
    /// Only replace the release named by `from_release`, and only while it follows latest.
    pub if_latest: bool,
    /// Only replace the release named by `from_release`, pinned or not.
    pub if_current: bool,
    pub from_release: Option<String>,
}

/// Where a package came from, as recorded in its receipt.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub(crate) id: SourceId,
    pub(crate) source_epoch: i64,
    pub(crate) published_epoch: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SourceId {
    Tag(String),
    Artifact(u64),
    Archive(String),
}

impl Source {
    /// Release tags name their package version; other sources are unchecked.
    fn expected_version(&self) -> Option<String> {
        let SourceId::Tag(tag) = &self.id else {
            return None;
        };
        let version = tag.strip_prefix("fork-v").unwrap_or(tag);
        let version = version.strip_prefix('v').unwrap_or(version);
        package::is_package_version(version).then(|| version.to_string())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    artifact_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    archive: Option<String>,
    source_epoch: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    published_epoch: Option<i64>,
    repository: String,
    version: String,
    target: String,
    package_digest: String,
    #[serde(default)]
    digest_format: u32,
}

impl Receipt {
    fn source_id(&self) -> Option<SourceId> {
        if let Some(tag) = &self.tag {
            Some(SourceId::Tag(tag.clone()))
        } else if let Some(id) = self.artifact_id {
            Some(SourceId::Artifact(id))
        } else {
            self.archive.clone().map(SourceId::Archive)
        }
    }
}

/// How an install relates to the currency check and the latest channel.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InstallOptions {
    /// Skip the install when the selected package is already as new or newer.
    pub(crate) check_currency: bool,
    /// Record the selection in `auto-update-version` so updaters may replace it.
    pub(crate) follows_latest: bool,
}

/// Progress lines, printed and kept for tests.
#[derive(Debug, Default)]
pub(crate) struct Output {
    echo: bool,
    lines: Mutex<Vec<String>>,
}

impl Output {
    pub(crate) fn stdout() -> Self {
        Self {
            echo: true,
            lines: Mutex::default(),
        }
    }

    pub(crate) fn step(&self, message: impl std::fmt::Display) {
        self.line(format!("==> {message}"));
    }

    pub(crate) fn line(&self, line: String) {
        if self.echo {
            println!("{line}");
        }
        if let Ok(mut lines) = self.lines.lock() {
            lines.push(line);
        }
    }

    #[cfg(test)]
    pub(crate) fn text(&self) -> String {
        self.lines
            .lock()
            .map(|lines| lines.join("\n"))
            .unwrap_or_default()
    }
}

pub(crate) struct Installer {
    pub(crate) layout: Layout,
    pub(crate) guards: Guards,
    pub(crate) probe: Box<dyn Probe>,
    pub(crate) cancel: Cancel,
    pub(crate) output: Output,
    pub(crate) lock_timeout: Duration,
}

enum Currency {
    /// As new or newer than the request, intact, and with valid commands.
    Current,
    Outdated,
    /// Recorded with a digest this installer cannot verify.
    OlderFormat,
}

/// What a failed commit must undo, in reverse order.
#[derive(Default)]
struct Changes {
    commands: Vec<(PathBuf, Option<PathBuf>)>,
    selected: bool,
}

impl Installer {
    #[cfg(test)]
    pub(crate) fn install(
        &self,
        archive: &Path,
        source: &Source,
        options: InstallOptions,
    ) -> Result<()> {
        self.install_with(source, options, || Ok(archive.to_path_buf()))
    }

    /// Install the archive `fetch` provides. It runs before the lock is taken,
    /// so a download never holds it, unless an unlocked look shows the selected
    /// package is already current; then nothing is downloaded at all.
    pub(crate) fn install_with(
        &self,
        source: &Source,
        options: InstallOptions,
        fetch: impl FnOnce() -> Result<PathBuf>,
    ) -> Result<()> {
        let mut fetch = Some(fetch);
        let mut download = || {
            fetch
                .take()
                .context("package archive was already fetched")?()
        };
        let mut archive = None;
        if !(options.check_currency && self.looks_current(source)) {
            archive = Some(download()?);
        }
        let _lock = lock::acquire(&self.layout.root, self.lock_timeout, &self.cancel)?;
        if !self.layout.daemon_only {
            self.refuse_command_directory_in_package()?;
        }
        let previous = selection::selected(&self.layout.root)?;
        let previous_name = previous
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let marker = self.layout.marker();
        let old_marker = read_marker(&marker)?;
        if self.guarded_skip(&previous_name, old_marker.as_deref().unwrap_or("").trim()) {
            return Ok(());
        }
        if let Some(previous) = previous.as_deref()
            && options.check_currency
        {
            match self.currency(previous, source) {
                Ok(Currency::Current) => {
                    if options.follows_latest {
                        atomic_write(&marker, previous_name.as_bytes())?;
                    }
                    self.output.step(
                        "Installed Codex package is already as new or newer than requested source",
                    );
                    self.output.step("Use --force to install anyway");
                    return Ok(());
                }
                Ok(Currency::Outdated) => {}
                Ok(Currency::OlderFormat) => self
                    .output
                    .step("Installed package was recorded by an older installer; reinstalling"),
                Err(error) => self.output.step(format!(
                    "Existing package is incomplete or changed; repairing ({error:#})"
                )),
            }
        }
        // Rarely, the selection changed after the unlocked look.
        let archive = match archive {
            Some(archive) => archive,
            None => download()?,
        };
        let (release, version) = self.stage(&archive, source)?;
        self.commit(
            &release,
            previous.as_deref(),
            &version,
            old_marker,
            options.follows_latest,
        )
    }

    /// Never let launcher replacement write into a package's own files.
    fn refuse_command_directory_in_package(&self) -> Result<()> {
        let visible = resolve_lenient(&self.layout.bin)?;
        let components: Vec<String> = visible
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_lowercase())
            .collect();
        let legacy = components
            .windows(2)
            .any(|pair| pair[0] == "packages" && pair[1] == "standalone");
        anyhow::ensure!(
            !legacy
                && !visible
                    .ancestors()
                    .any(|directory| directory.join("codex-package.json").is_file()),
            "~/.local/bin points inside a Codex package; use a command directory outside that package so its files remain untouched"
        );
        Ok(())
    }

    fn guarded_skip(&self, previous_name: &str, marker: &str) -> bool {
        let expected = self.guards.from_release.as_deref().unwrap_or("");
        for (enabled, guard, latest) in [
            (self.guards.if_current, "CODEX_INSTALL_IF_CURRENT", false),
            (self.guards.if_latest, "CODEX_INSTALL_IF_LATEST", true),
        ] {
            if enabled
                && (expected.is_empty()
                    || expected != previous_name
                    || latest && marker != previous_name)
            {
                self.output.step(format!(
                    "Selection changed or pinned; skipped guarded install ({guard})"
                ));
                return true;
            }
        }
        if !expected.is_empty() && expected != previous_name {
            self.output
                .step("Selection changed; skipped guarded install");
            return true;
        }
        false
    }

    fn looks_current(&self, source: &Source) -> bool {
        matches!(
            selection::selected(&self.layout.root),
            Ok(Some(previous)) if matches!(self.currency(&previous, source), Ok(Currency::Current))
        )
    }

    fn currency(&self, previous: &Path, source: &Source) -> Result<Currency> {
        package::validate(
            &self.layout.target,
            previous,
            /*expected_version*/ None,
            Some(self.probe.as_ref()),
        )?;
        let receipt: Receipt = serde_json::from_slice(&std::fs::read(previous.join(RECEIPT))?)
            .context("invalid install receipt")?;
        if receipt.digest_format != DIGEST_FORMAT {
            return Ok(Currency::OlderFormat);
        }
        let commands_valid = self.layout.daemon_only
            || COMMANDS.iter().all(|command| {
                self.layout
                    .launchers(command)
                    .iter()
                    .all(|(path, value)| launcher_matches(path, value))
                    && self
                        .layout
                        .stale_commands(command)
                        .iter()
                        .all(|path| !matches!(lexists(path), Ok(true)))
            });
        let newer = matches!(
            (receipt.published_epoch, source.published_epoch),
            (Some(installed), Some(requested)) if installed > requested
        );
        let current = receipt.source_id().as_ref() == Some(&source.id)
            && receipt.source_epoch >= source.source_epoch;
        Ok(
            if receipt.repository == REPO
                && (newer || current)
                && commands_valid
                && digest::tree_digest(previous)? == receipt.package_digest
            {
                Currency::Current
            } else {
                Currency::Outdated
            },
        )
    }

    /// Extract and verify the package beside the releases, then publish it as
    /// an immutable release directory. Returns the release and its version.
    fn stage(&self, archive: &Path, source: &Source) -> Result<(PathBuf, String)> {
        let releases = self.layout.releases();
        std::fs::create_dir_all(&releases)?;
        // Holding the lock, any staging directory left here was abandoned.
        for entry in std::fs::read_dir(&releases)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with(".staging-") {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
        let staging = tempfile::Builder::new()
            .prefix(".staging-")
            .tempdir_in(&releases)?;
        let stage = staging.path().join("package");
        std::fs::create_dir(&stage)?;
        package::extract(archive, &stage, &self.cancel)?;
        let target = &self.layout.target;
        let version = package::validate(
            target,
            &stage,
            source.expected_version().as_deref(),
            Some(self.probe.as_ref()),
        )?;
        self.cancel.check()?;
        let digest = digest::tree_digest(&stage)?;
        let name = format!("{version}-{}-{}", target.triple(), &digest[..16]);
        let receipt = Receipt {
            tag: match &source.id {
                SourceId::Tag(tag) => Some(tag.clone()),
                _ => None,
            },
            artifact_id: match source.id {
                SourceId::Artifact(id) => Some(id),
                _ => None,
            },
            archive: match &source.id {
                SourceId::Archive(name) => Some(name.clone()),
                _ => None,
            },
            source_epoch: source.source_epoch,
            published_epoch: source.published_epoch,
            repository: REPO.to_string(),
            version: version.clone(),
            target: target.triple().to_string(),
            package_digest: digest.clone(),
            digest_format: DIGEST_FORMAT,
        };
        let mut receipt = serde_json::to_vec_pretty(&receipt)?;
        receipt.push(b'\n');
        std::fs::write(stage.join(RECEIPT), &receipt)?;
        let mut release = releases.join(&name);
        if lexists(&release)? {
            if digest::tree_digest(&release).ok().as_deref() == Some(digest.as_str()) {
                // Same contents: reuse the directory, recording this source.
                atomic_write(&release.join(RECEIPT), &receipt)?;
                return Ok((release, version));
            }
            // Keep a damaged release for diagnosis and publish a repair beside it.
            release = releases.join(format!("{name}-repair-{}", random_hex(8)));
        }
        rename_with_retry(&stage, &release)
            .with_context(|| format!("failed to publish {}", release.display()))?;
        Ok((release, version))
    }

    fn commit(
        &self,
        release: &Path,
        previous: Option<&Path>,
        version: &str,
        old_marker: Option<String>,
        follows_latest: bool,
    ) -> Result<()> {
        let backup = self.layout.install_root.join("backups").join(format!(
            "{}-{}",
            chrono::Utc::now().format("%Y%m%dT%H%M%S"),
            random_hex(8)
        ));
        let mut changes = Changes::default();
        if let Err(error) = self.publish(release, version, &backup, follows_latest, &mut changes) {
            self.rollback(changes, previous, old_marker);
            return Err(error);
        }
        self.output
            .step(format!("Selected package: {}", release.display()));
        if backup.exists() {
            self.output
                .step(format!("Previous commands saved in {}", backup.display()));
        }
        self.output.step(format!("Installed version: {version}"));
        if self.layout.daemon_only {
            self.output
                .step("Daemon package selected; the daemon updater owns any required restart");
        }
        Ok(())
    }

    fn publish(
        &self,
        release: &Path,
        version: &str,
        backup: &Path,
        follows_latest: bool,
        changes: &mut Changes,
    ) -> Result<()> {
        self.cancel.check()?;
        selection::set_current(&self.layout.root, release)?;
        changes.selected = true;
        if !self.layout.daemon_only {
            std::fs::create_dir_all(&self.layout.bin)?;
            for command in COMMANDS {
                self.cancel.check()?;
                let desired = self.layout.launchers(command);
                let unchanged: Vec<&Path> = desired
                    .iter()
                    .filter(|(path, value)| launcher_matches(path, value))
                    .map(|(path, _)| path.as_path())
                    .collect();
                // Move aside everything that would shadow or conflict with the
                // launchers, including loose executables from older installs.
                let paths = desired
                    .iter()
                    .map(|(path, _)| path.clone())
                    .chain(self.layout.stale_commands(command));
                for path in paths {
                    if unchanged.contains(&path.as_path()) {
                        continue;
                    }
                    let mut saved = None;
                    if lexists(&path)? {
                        anyhow::ensure!(
                            !path.is_dir(),
                            "refusing to replace command directory {}",
                            path.display()
                        );
                        std::fs::create_dir_all(backup)?;
                        let destination =
                            backup.join(path.file_name().context("command has no file name")?);
                        std::fs::rename(&path, &destination)
                            .with_context(|| format!("failed to move {} aside", path.display()))?;
                        saved = Some(destination);
                    }
                    changes.commands.push((path, saved));
                }
                for (path, value) in &desired {
                    if !unchanged.contains(&path.as_path()) {
                        write_launcher(path, value)?;
                    }
                }
            }
        }
        let marker = self.layout.marker();
        if follows_latest {
            let name = release
                .file_name()
                .context("release has no name")?
                .to_string_lossy();
            atomic_write(&marker, name.as_bytes())?;
        } else {
            remove_file_if_exists(&marker)?;
        }
        self.cancel.check()?;
        package::validate(
            &self.layout.target,
            &self.layout.current(),
            Some(version),
            Some(self.probe.as_ref()),
        )
        .context("the selected package failed validation")?;
        Ok(())
    }

    fn rollback(&self, changes: Changes, previous: Option<&Path>, old_marker: Option<String>) {
        let warn = |what: &str, result: Result<()>| {
            if let Err(error) = result {
                eprintln!("warning: rollback could not restore {what}: {error:#}");
            }
        };
        for (path, saved) in changes.commands.into_iter().rev() {
            let what = path.display().to_string();
            warn(
                &what,
                remove_file_if_exists(&path)
                    .and_then(|()| saved.map_or(Ok(()), |saved| std::fs::rename(saved, &path)))
                    .map_err(Into::into),
            );
        }
        if changes.selected {
            warn(
                "the previous selection",
                match previous {
                    Some(previous) => selection::set_current(&self.layout.root, previous),
                    None => selection::remove_current(&self.layout.root).map_err(Into::into),
                },
            );
        }
        let marker = self.layout.marker();
        warn(
            "auto-update-version",
            match old_marker {
                Some(text) => atomic_write(&marker, text.as_bytes()),
                None => remove_file_if_exists(&marker).map_err(Into::into),
            },
        );
    }
}

/// Windows launchers are files compared by text; Unix launchers are symlinks.
fn launcher_matches(path: &Path, value: &str) -> bool {
    if cfg!(windows) {
        let normalize = |text: &str| text.replace("\r\n", "\n").trim().to_string();
        std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
            && std::fs::read_to_string(path).is_ok_and(|text| normalize(&text) == normalize(value))
    } else {
        std::fs::read_link(path).is_ok_and(|target| target == Path::new(value))
    }
}

fn write_launcher(path: &Path, value: &str) -> Result<()> {
    #[cfg(windows)]
    {
        atomic_write(path, value.as_bytes())
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(value, path)
            .with_context(|| format!("failed to create {}", path.display()))
    }
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
