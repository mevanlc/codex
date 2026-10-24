use pretty_assertions::assert_eq;

#[test]
fn fork_package_root_never_adopts_upstream_packages_or_state() {
    let home = tempfile::TempDir::new().unwrap();
    for file in [
        "packages/standalone/current/codex",
        "app-server-daemon/app-server.pid",
        "packages/app-server-daemon/current/bin/codex",
    ] {
        let path = home.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "upstream").unwrap();
    }
    let root = super::package_root(home.path());
    assert_eq!(
        root,
        home.path()
            .join(".local/share/mevanlc-codex/packages/app-server-daemon")
    );
    assert_eq!(
        super::managed_codex_bin(&root),
        root.join("current/bin")
            .join(super::managed_codex_file_name())
    );
}

#[cfg(unix)]
#[test]
fn latest_channel_includes_snapshots_and_preserves_pins() {
    let home = tempfile::TempDir::new().unwrap();
    let root = super::package_root(home.path());
    let name = "0.164.0-123abcd-aarch64-linux-android-digest";
    let release = root.join("releases").join(name);
    let bin = release.join("bin/codex");
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::write(&bin, "codex").unwrap();
    std::fs::write(release.join("codex-package.json"), "{}").unwrap();
    std::os::unix::fs::symlink(&release, root.join("current")).unwrap();
    assert!(!super::is_latest_release(&root, &bin));
    std::fs::write(root.join("auto-update-version"), name).unwrap();
    assert!(super::is_latest_release(&root, &bin));
    std::fs::write(root.join("auto-update-version"), "another-release").unwrap();
    assert!(!super::is_latest_release(&root, &bin));
    std::fs::remove_file(root.join("auto-update-version")).unwrap();
    assert!(!super::is_latest_release(&root, &bin));
}

#[cfg(unix)]
#[tokio::test]
async fn older_managed_binary_does_not_claim_updater_support() {
    let temp = tempfile::TempDir::new().unwrap();
    let binary = temp.path().join("codex");
    codex_utils_cargo_bin::write_executable(&binary, "#!/bin/sh\nexit 2\n").unwrap();
    assert!(!super::supports_daemon_update_loop(&binary).await);
    codex_utils_cargo_bin::write_executable(&binary, "#!/bin/sh\nexit 0\n").unwrap();
    assert!(super::supports_daemon_update_loop(&binary).await);
}
