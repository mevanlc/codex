use super::*;
use crate::test_support::FORCED;
use crate::test_support::Fixture;
use crate::test_support::LATEST;
use crate::test_support::PINNED;
use crate::test_support::PackageSpec;
use crate::test_support::release_source;
use crate::test_support::selected;
use pretty_assertions::assert_eq;
use serde_json::json;

fn install(
    fixture: &Fixture,
    archive: &Path,
    version: &str,
    options: InstallOptions,
) -> Result<String> {
    let installer = fixture.installer(false, Guards::default());
    installer
        .install(archive, &release_source(version, 1_000), options)
        .map(|()| installer.output.text())
}

fn receipt(release: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(release.join(RECEIPT)).unwrap()).unwrap()
}

#[test]
fn full_install_selects_the_package_and_publishes_launchers() {
    let fixture = Fixture::new();
    let archive = fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    let output = install(&fixture, &archive, "1.2.3-aaaaaaa", LATEST).unwrap();
    assert!(
        output.contains("Installed version: 1.2.3-aaaaaaa"),
        "{output}"
    );

    let root = fixture.cli_root();
    let release = selected(&root);
    let target = crate::test_support::host_target();
    assert!(release.join("codex-resources/data/file").is_file());
    assert!(
        release
            .join(format!("codex-path/{}", target.exe("rg")))
            .is_file()
    );
    assert_eq!(
        std::fs::read_to_string(root.join("auto-update-version")).unwrap(),
        release.file_name().unwrap().to_string_lossy()
    );
    let receipt = receipt(&release);
    assert_eq!(receipt["tag"], "fork-v1.2.3-aaaaaaa");
    assert_eq!(receipt["repository"], "mevanlc/codex");
    assert_eq!(receipt["publishedEpoch"], 1_000);
    assert_eq!(receipt["digestFormat"], 2);
    assert_eq!(
        receipt["packageDigest"],
        digest::tree_digest(&release).unwrap()
    );
    assert!(
        release
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(&format!("1.2.3-aaaaaaa-{}-", target.triple()))
    );
    let bin = fixture.bin();
    if cfg!(windows) {
        let cmd = std::fs::read_to_string(bin.join("codex.cmd")).unwrap();
        assert!(cmd.contains(r"%~dp0..\share\mevanlc-codex\packages\cli\current\bin\codex.exe"));
        let bash = std::fs::read_to_string(bin.join("codex")).unwrap();
        assert!(bash.starts_with("#!/usr/bin/env bash\nexec "), "{bash}");
        assert!(bin.join("codex-code-mode-host.cmd").is_file());
    } else {
        assert_eq!(
            std::fs::read_link(bin.join("codex")).unwrap(),
            root.join("current/bin/codex")
        );
        assert!(bin.join("codex-code-mode-host").exists());
    }
}

#[test]
fn currency_force_pin_repair_and_retention() {
    let fixture = Fixture::new();
    let root = fixture.cli_root();
    let first = fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    install(&fixture, &first, "1.2.3-aaaaaaa", LATEST).unwrap();
    let old = selected(&root);

    let output = install(&fixture, &first, "1.2.3-aaaaaaa", LATEST).unwrap();
    assert!(output.contains("already as new or newer"), "{output}");
    assert_eq!(selected(&root), old);

    // A damaged release is kept for diagnosis and replaced by a repair.
    let rg = crate::test_support::host_target().exe("rg");
    std::fs::remove_file(old.join("codex-path").join(&rg)).unwrap();
    let output = install(&fixture, &first, "1.2.3-aaaaaaa", LATEST).unwrap();
    assert!(
        output.contains("incomplete or changed; repairing"),
        "{output}"
    );
    let repaired = selected(&root);
    assert_ne!(repaired, old);
    assert!(repaired.join("codex-path").join(&rg).is_file());
    assert!(old.is_dir());

    install(&fixture, &first, "1.2.3-aaaaaaa", FORCED).unwrap();
    assert!(root.join("auto-update-version").is_file());

    let pinned = fixture.package(&PackageSpec::new("1.0.0"));
    install(&fixture, &pinned, "1.0.0", PINNED).unwrap();
    assert!(!root.join("auto-update-version").exists());
    assert!(
        selected(&root)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("1.0.0-")
    );
    assert!(old.is_dir());
}

#[test]
fn newer_installed_publication_is_not_downgraded() {
    let fixture = Fixture::new();
    let root = fixture.cli_root();
    let installer = fixture.installer(false, Guards::default());
    let newer = fixture.package(&PackageSpec::new("1.3.0"));
    installer
        .install(&newer, &release_source("1.3.0", 2_000), LATEST)
        .unwrap();
    let installed = selected(&root);
    let older = fixture.package(&PackageSpec::new("1.2.0"));
    installer
        .install(&older, &release_source("1.2.0", 1_000), LATEST)
        .unwrap();
    assert_eq!(selected(&root), installed);
    assert!(installer.output.text().contains("already as new or newer"));
}

#[test]
fn receipts_from_the_python_installer_trigger_one_reinstall() {
    let fixture = Fixture::new();
    let root = fixture.cli_root();
    let archive = fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    install(&fixture, &archive, "1.2.3-aaaaaaa", LATEST).unwrap();
    let release = selected(&root);
    let mut legacy = receipt(&release);
    legacy.as_object_mut().unwrap().remove("digestFormat");
    std::fs::write(release.join(RECEIPT), serde_json::to_vec(&legacy).unwrap()).unwrap();

    let output = install(&fixture, &archive, "1.2.3-aaaaaaa", LATEST).unwrap();
    assert!(
        output.contains("recorded by an older installer; reinstalling"),
        "{output}"
    );
    assert_eq!(receipt(&selected(&root))["digestFormat"], 2);
    let output = install(&fixture, &archive, "1.2.3-aaaaaaa", LATEST).unwrap();
    assert!(output.contains("already as new or newer"), "{output}");
}

#[test]
fn invalid_packages_preserve_the_previous_selection() {
    let fixture = Fixture::new();
    let root = fixture.cli_root();
    let archive = fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    install(&fixture, &archive, "1.2.3-aaaaaaa", LATEST).unwrap();
    let old = selected(&root);
    let host = format!(
        "bin/{}",
        crate::test_support::host_target().exe("codex-code-mode-host")
    );
    let mut cases = vec![
        PackageSpec {
            missing: Some(host),
            ..PackageSpec::new("1.2.3-aaaaaaa")
        },
        PackageSpec {
            manifest_changes: vec![("target", json!("wrong"))],
            ..PackageSpec::new("1.2.3-aaaaaaa")
        },
        PackageSpec {
            manifest_changes: vec![("layoutVersion", json!(2))],
            ..PackageSpec::new("1.2.3-aaaaaaa")
        },
        PackageSpec {
            manifest_changes: vec![("version", json!("../../escape"))],
            ..PackageSpec::new("1.2.3-aaaaaaa")
        },
        PackageSpec {
            manifest_changes: vec![("entrypoint", json!("codex"))],
            ..PackageSpec::new("1.2.3-aaaaaaa")
        },
        // The executable must report the package's version.
        PackageSpec {
            script: Some("#!/bin/sh\necho 'codex 9.9.9'\n".to_string()),
            ..PackageSpec::new("1.2.3-aaaaaaa")
        },
        PackageSpec {
            script: Some("#!/bin/sh\nexit 9\n".to_string()),
            ..PackageSpec::new("1.2.3-aaaaaaa")
        },
    ];
    if cfg!(unix) {
        cases.push(PackageSpec {
            host_mode: 0o644,
            ..PackageSpec::new("1.2.3-aaaaaaa")
        });
    }
    for spec in cases {
        let archive = fixture.package(&spec);
        let result = install(&fixture, &archive, "1.2.3-aaaaaaa", FORCED);
        assert!(
            result.is_err(),
            "{:?} {:?}",
            spec.manifest_changes,
            spec.missing
        );
        assert_eq!(selected(&root), old);
    }
}

#[test]
fn rollback_restores_selection_marker_and_replaced_commands() {
    let fixture = Fixture::new();
    let root = fixture.cli_root();
    let archive = fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    install(&fixture, &archive, "1.2.3-aaaaaaa", LATEST).unwrap();
    let old = selected(&root);
    let marker = std::fs::read_to_string(root.join("auto-update-version")).unwrap();

    let bin = fixture.bin();
    let original = "#!/bin/sh\necho 'codex original'\n";
    std::fs::remove_file(bin.join("codex")).unwrap();
    std::fs::write(bin.join("codex"), original).unwrap();
    let host = bin.join("codex-code-mode-host");
    std::fs::remove_file(&host).unwrap();
    // A real directory makes command replacement fail after selection.
    std::fs::create_dir(&host).unwrap();

    let next = fixture.package(&PackageSpec::new("1.3.0"));
    let error = install(&fixture, &next, "1.3.0", LATEST).unwrap_err();
    assert!(format!("{error:#}").contains("refusing to replace command directory"));
    assert_eq!(
        std::fs::read_to_string(bin.join("codex")).unwrap(),
        original
    );
    assert_eq!(selected(&root), old);
    assert_eq!(
        std::fs::read_to_string(root.join("auto-update-version")).unwrap(),
        marker
    );
    assert!(host.is_dir());
}

#[test]
fn cancellation_during_commit_rolls_back() {
    struct CancellingProbe {
        cancel: Cancel,
        calls: std::sync::atomic::AtomicUsize,
        trigger: usize,
    }
    impl Probe for CancellingProbe {
        fn version(&self, executable: &Path) -> Result<String> {
            let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if call == self.trigger {
                self.cancel.request(143);
                self.cancel.check()?;
            }
            crate::test_support::FakeProbe.version(executable)
        }
    }

    let fixture = Fixture::new();
    let root = fixture.cli_root();
    let archive = fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    install(&fixture, &archive, "1.2.3-aaaaaaa", LATEST).unwrap();
    let old = selected(&root);
    let launcher = std::fs::read_link(fixture.bin().join("codex")).ok();

    let mut installer = fixture.installer(false, Guards::default());
    // Staging probes codex and rg; the third probe is the final validation.
    installer.probe = Box::new(CancellingProbe {
        cancel: installer.cancel.clone(),
        calls: Default::default(),
        trigger: 3,
    });
    let next = fixture.package(&PackageSpec::new("1.3.0"));
    let error = installer
        .install(&next, &release_source("1.3.0", 2_000), FORCED)
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<crate::Cancelled>().unwrap().exit_code,
        143
    );
    assert_eq!(selected(&root), old);
    assert_eq!(
        std::fs::read_link(fixture.bin().join("codex")).ok(),
        launcher
    );
}

#[test]
fn command_directory_inside_a_package_is_refused() {
    let fixture = Fixture::new();
    // ~/.local/bin is inside something that looks like a package root.
    std::fs::create_dir_all(fixture.bin()).unwrap();
    std::fs::write(fixture.home.join(".local/codex-package.json"), "{}").unwrap();
    let archive = fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    let error = install(&fixture, &archive, "1.2.3-aaaaaaa", LATEST).unwrap_err();
    assert!(format!("{error:#}").contains("points inside a Codex package"));
    assert!(!fixture.cli_root().join("current").exists());
}

#[cfg(unix)]
#[test]
fn command_directory_alias_cannot_modify_an_upstream_package() {
    let fixture = Fixture::new();
    let upstream = fixture.dir.path().join("upstream-package");
    std::fs::create_dir_all(upstream.join("bin")).unwrap();
    std::fs::write(
        upstream.join("codex-package.json"),
        r#"{"version":"0.1.0"}"#,
    )
    .unwrap();
    let binary = upstream.join("bin/codex");
    std::fs::write(&binary, "original").unwrap();
    std::fs::create_dir_all(fixture.home.join(".local")).unwrap();
    std::os::unix::fs::symlink(upstream.join("bin"), fixture.bin()).unwrap();
    let archive = fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    let error = install(&fixture, &archive, "1.2.3-aaaaaaa", LATEST).unwrap_err();
    assert!(format!("{error:#}").contains("points inside a Codex package"));
    assert_eq!(std::fs::read_to_string(binary).unwrap(), "original");
}

#[test]
fn daemon_install_is_separate_and_guarded() {
    let fixture = Fixture::new();
    let archive = fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    install(&fixture, &archive, "1.2.3-aaaaaaa", LATEST).unwrap();
    let cli = selected(&fixture.cli_root());

    let daemon_root = fixture.daemon_root();
    fixture
        .installer(true, Guards::default())
        .install(&archive, &release_source("1.2.3-aaaaaaa", 1_000), LATEST)
        .unwrap();
    let old = selected(&daemon_root);
    // An explicit pin wins over a scheduled update.
    std::fs::remove_file(daemon_root.join("auto-update-version")).unwrap();

    let next = fixture.package(&PackageSpec::new("1.3.0"));
    let old_name = old.file_name().unwrap().to_string_lossy().into_owned();
    let scheduled = fixture.installer(
        true,
        Guards {
            if_latest: true,
            if_current: false,
            from_release: Some(old_name.clone()),
        },
    );
    scheduled
        .install(&next, &release_source("1.3.0", 2_000), LATEST)
        .unwrap();
    assert!(scheduled.output.text().contains("skipped guarded install"));
    assert_eq!(selected(&daemon_root), old);

    let manual = fixture.installer(
        true,
        Guards {
            if_latest: false,
            if_current: true,
            from_release: Some(old_name),
        },
    );
    manual
        .install(&next, &release_source("1.3.0", 2_000), LATEST)
        .unwrap();
    assert_ne!(selected(&daemon_root), old);
    assert_eq!(selected(&fixture.cli_root()), cli);

    // A stale expectation never replaces whatever is selected now.
    let stale = fixture.installer(
        true,
        Guards {
            if_latest: false,
            if_current: false,
            from_release: Some("something-else".to_string()),
        },
    );
    let current = selected(&daemon_root);
    stale
        .install(&archive, &release_source("1.2.3-aaaaaaa", 3_000), FORCED)
        .unwrap();
    assert!(
        stale
            .output
            .text()
            .contains("Selection changed; skipped guarded install")
    );
    assert_eq!(selected(&daemon_root), current);
}

#[test]
fn concurrent_installs_serialize_selection() {
    let fixture = Fixture::new();
    let root = fixture.cli_root();
    let archive = fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    std::thread::scope(|scope| {
        let installs: Vec<_> = (0..3)
            .map(|_| scope.spawn(|| install(&fixture, &archive, "1.2.3-aaaaaaa", LATEST)))
            .collect();
        for handle in installs {
            handle.join().unwrap().unwrap();
        }
    });
    let releases: Vec<_> = std::fs::read_dir(root.join("releases"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(releases.len(), 1, "{releases:?}");
    assert_eq!(
        std::fs::read_to_string(root.join("auto-update-version")).unwrap(),
        releases[0].to_string_lossy()
    );
}

#[cfg(windows)]
#[test]
fn windows_launchers_replace_loose_executables() {
    let fixture = Fixture::new();
    let bin = fixture.bin();
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("codex.exe"), "legacy executable").unwrap();
    std::fs::write(bin.join("codex.ps1"), "Write-Output 'stale'; exit 7\n").unwrap();
    let archive = fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    let output = install(&fixture, &archive, "1.2.3-aaaaaaa", LATEST).unwrap();
    assert!(!bin.join("codex.exe").exists());
    assert!(!bin.join("codex.ps1").exists());
    assert!(output.contains("Previous commands saved in"), "{output}");
    let backups = fixture.home.join(".local/share/mevanlc-codex/backups");
    let backup = std::fs::read_dir(backups)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        std::fs::read_to_string(backup.join("codex.exe")).unwrap(),
        "legacy executable"
    );
    assert!(
        crate::selection::selected(&fixture.cli_root())
            .unwrap()
            .is_some()
    );

    // A launcher held open by a running cmd.exe is left alone when unchanged.
    let held = std::fs::File::open(bin.join("codex.cmd")).unwrap();
    install(&fixture, &archive, "1.2.3-aaaaaaa", FORCED).unwrap();
    drop(held);
}

#[cfg(unix)]
#[test]
fn real_executables_are_probed() {
    let fixture = Fixture::new();
    let mut installer = fixture.installer(false, Guards::default());
    installer.probe = Box::new(crate::probe::ProcessProbe {
        timeout: Duration::from_secs(30),
        cancel: Cancel::default(),
    });
    let archive = fixture.package(&PackageSpec::new("1.2.3-aaaaaaa"));
    installer
        .install(&archive, &release_source("1.2.3-aaaaaaa", 1_000), LATEST)
        .unwrap();
    let failing = fixture.package(&PackageSpec {
        script: Some("#!/bin/sh\necho broken >&2\nexit 9\n".to_string()),
        ..PackageSpec::new("1.3.0")
    });
    let error = installer
        .install(&failing, &release_source("1.3.0", 2_000), LATEST)
        .unwrap_err();
    assert!(format!("{error:#}").contains("broken"), "{error:#}");
}
