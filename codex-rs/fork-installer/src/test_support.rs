//! Package fixtures shared by the installer tests.

use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;

use crate::cancel::Cancel;
use crate::install::Guards;
use crate::install::InstallOptions;
use crate::install::Installer;
use crate::install::Output;
use crate::install::Source;
use crate::install::SourceId;
use crate::layout::Layout;
use crate::layout::Target;
use crate::probe::Probe;

pub(crate) fn host_target() -> Target {
    Target::new(
        codex_install_context::fork::release_target().unwrap_or(if cfg!(windows) {
            "x86_64-pc-windows-msvc"
        } else {
            "x86_64-unknown-linux-musl"
        }),
    )
}

/// Reads the version a fake executable would print, so tests run anywhere.
pub(crate) struct FakeProbe;

impl Probe for FakeProbe {
    fn version(&self, executable: &Path) -> Result<String> {
        let text = std::fs::read_to_string(executable)?;
        anyhow::ensure!(
            !text.contains("exit 9"),
            "{} exited with 9",
            executable.display()
        );
        let start = text.find("echo '").context("not a fake executable")? + "echo '".len();
        let end = start + text[start..].find('\'').context("not a fake executable")?;
        Ok(text[start..end].to_string())
    }
}

pub(crate) struct PackageSpec {
    pub(crate) version: String,
    pub(crate) target: Target,
    pub(crate) missing: Option<String>,
    /// Permission bits of the code-mode host on Unix.
    pub(crate) host_mode: u32,
    pub(crate) manifest_changes: Vec<(&'static str, Value)>,
    pub(crate) script: Option<String>,
}

impl PackageSpec {
    pub(crate) fn new(version: &str) -> Self {
        Self {
            version: version.to_string(),
            target: host_target(),
            missing: None,
            host_mode: 0o755,
            manifest_changes: Vec::new(),
            script: None,
        }
    }
}

pub(crate) struct Fixture {
    pub(crate) dir: TempDir,
    pub(crate) home: PathBuf,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("home");
        std::fs::create_dir(&home).expect("home");
        Self { dir, home }
    }

    pub(crate) fn layout(&self, daemon_only: bool) -> Layout {
        Layout::new(&self.home, None, None, daemon_only).expect("layout")
    }

    pub(crate) fn installer(&self, daemon_only: bool, guards: Guards) -> Installer {
        installer_at(&self.home, daemon_only, guards)
    }

    pub(crate) fn cli_root(&self) -> PathBuf {
        self.layout(false).root
    }

    pub(crate) fn daemon_root(&self) -> PathBuf {
        self.layout(true).root
    }

    pub(crate) fn bin(&self) -> PathBuf {
        self.home.join(".local").join("bin")
    }

    /// Build a package archive in the format its target's releases use.
    pub(crate) fn package(&self, spec: &PackageSpec) -> PathBuf {
        let target = &spec.target;
        let mut names = target.required_executables();
        names.push("codex-resources/data/file".to_string());
        names.push("codex-path/README".to_string());
        let mut files: Vec<(String, Vec<u8>, u32)> = Vec::new();
        for name in names {
            if spec.missing.as_deref() == Some(name.as_str()) {
                continue;
            }
            let script = spec
                .script
                .clone()
                .unwrap_or_else(|| format!("#!/bin/sh\necho 'codex {}'\n", spec.version));
            let mode = if name == format!("bin/{}", target.exe("codex-code-mode-host")) {
                spec.host_mode
            } else {
                0o755
            };
            files.push((name, script.into_bytes(), mode));
        }
        let mut manifest = json!({
            "layoutVersion": 1,
            "version": spec.version,
            "target": target.triple(),
            "variant": "codex",
            "entrypoint": format!("bin/{}", target.exe("codex")),
            "resourcesDir": "codex-resources",
            "pathDir": "codex-path",
        });
        for (key, value) in &spec.manifest_changes {
            manifest[*key] = value.clone();
        }
        files.push((
            "codex-package.json".to_string(),
            serde_json::to_vec(&manifest).expect("manifest"),
            0o644,
        ));
        let archive = self.dir.path().join(format!(
            "package-{}{}",
            crate::fsutil::random_hex(8),
            if target.windows() { ".zip" } else { ".tar.zst" }
        ));
        write_archive(&archive, &files);
        archive
    }
}

pub(crate) fn installer_at(home: &Path, daemon_only: bool, guards: Guards) -> Installer {
    Installer {
        layout: Layout::new(home, None, None, daemon_only).expect("layout"),
        guards,
        probe: Box::new(FakeProbe),
        cancel: Cancel::default(),
        output: Output::default(),
        lock_timeout: Duration::from_secs(20),
    }
}

pub(crate) fn write_archive(archive: &Path, files: &[(String, Vec<u8>, u32)]) {
    let file = std::fs::File::create(archive).expect("archive");
    if archive.to_string_lossy().ends_with(".zip") {
        let mut zip = zip::ZipWriter::new(file);
        for (name, contents, mode) in files {
            // Encode the whole mode, as release archives built on Unix do;
            // unix_permissions() would drop the file type bits.
            let mode = if mode & !0o777 == 0 {
                0o100000 | mode
            } else {
                *mode
            };
            let options = zip::write::SimpleFileOptions::default()
                .system(zip::System::Unix)
                .external_attributes(mode << 16);
            zip.start_file(name.as_str(), options).expect("zip entry");
            zip.write_all(contents).expect("zip write");
        }
        zip.finish().expect("zip finish");
    } else {
        let encoder = zstd::stream::write::Encoder::new(file, 3).expect("zstd");
        let mut tar = tar::Builder::new(encoder);
        for (name, contents, mode) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(*mode);
            header.set_cksum();
            tar.append_data(&mut header, name, contents.as_slice())
                .expect("tar entry");
        }
        tar.into_inner()
            .expect("tar finish")
            .finish()
            .expect("zstd finish");
    }
}

pub(crate) fn release_source(version: &str, published: i64) -> Source {
    Source {
        id: SourceId::Tag(format!("fork-v{version}")),
        source_epoch: published,
        published_epoch: Some(published),
    }
}

pub(crate) const LATEST: InstallOptions = InstallOptions {
    check_currency: true,
    follows_latest: true,
};

pub(crate) const FORCED: InstallOptions = InstallOptions {
    check_currency: false,
    follows_latest: true,
};

pub(crate) const PINNED: InstallOptions = InstallOptions {
    check_currency: false,
    follows_latest: false,
};

/// The release `current` selects.
pub(crate) fn selected(root: &Path) -> PathBuf {
    std::fs::canonicalize(root.join("current")).expect("current selection")
}
