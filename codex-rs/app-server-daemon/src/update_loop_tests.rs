use std::sync::Mutex;
#[cfg(unix)]
use std::time::Duration;

use pretty_assertions::assert_eq;
#[cfg(unix)]
use tempfile::TempDir;

use super::InstallerHttp;
use super::InstallerResponse;
#[cfg(unix)]
use super::manual_update::run as manual_update_once;
#[cfg(unix)]
use crate::Daemon;
#[cfg(unix)]
use crate::RestartIfRunningOutcome;
#[cfg(unix)]
use crate::UpdateOutput;
#[cfg(unix)]
use crate::UpdateStatus;
use crate::fork_installer::fetch_installer;
use crate::fork_installer::fetch_releases;
#[cfg(unix)]
use crate::managed_install::executable_identity;
#[cfg(unix)]
use crate::managed_install::executable_identity_from_reader;
use codex_install_context::fork;
use sha2::Digest;

const FAKE_TAG: &str = "fork-v9.9.9-test";
const FAKE_INSTALLER_URL: &str = "https://example.invalid/codex-fork-installer";

fn releases_page_url() -> String {
    format!("{}?per_page=100&page=1", fork::RELEASES_URL)
}

/// One complete release whose installer for this platform has `installer`'s digest.
fn fake_releases(installer: &[u8]) -> Vec<u8> {
    serde_json::json!([{
        "tag_name": FAKE_TAG,
        "published_at": "2026-10-01T00:00:00Z",
        "assets": [
            {"name": fork::release_asset_name().expect("supported platform")},
            {
                "name": fork::installer_asset_name().expect("supported platform"),
                "browser_download_url": FAKE_INSTALLER_URL,
                "digest": format!("sha256:{:x}", sha2::Sha256::digest(installer)),
            },
        ],
    }])
    .to_string()
    .into_bytes()
}

#[tokio::test]
async fn installer_fetch_reads_releases_and_returns_the_verified_installer() {
    let installer = b"#!/bin/sh\nprintf 'update bytes'\n".to_vec();
    let http = FakeInstallerHttp::new(InstallerResponse::Success(installer.clone()));

    let releases = fetch_releases(&http).await.expect("release metadata");
    assert_eq!(
        fetch_installer(&http, &releases)
            .await
            .expect("installer fetch should succeed"),
        installer
    );
    assert_eq!(
        http.requested_urls(),
        vec![releases_page_url(), FAKE_INSTALLER_URL.to_string()]
    );
}

#[tokio::test]
async fn installer_fetch_rejects_a_digest_mismatch() {
    struct Tampered;
    impl InstallerHttp for Tampered {
        async fn get(&self, url: &str) -> anyhow::Result<InstallerResponse> {
            Ok(InstallerResponse::Success(if url == FAKE_INSTALLER_URL {
                b"tampered".to_vec()
            } else {
                fake_releases(b"published")
            }))
        }
    }
    let releases = fetch_releases(&Tampered).await.expect("release metadata");
    let error = fetch_installer(&Tampered, &releases)
        .await
        .expect_err("tampered installer should fail");
    assert!(
        error
            .to_string()
            .contains("does not match its published digest")
    );
}

#[tokio::test]
async fn installer_fetch_rejects_non_success_status() {
    let http = FakeInstallerHttp::new(InstallerResponse::Unsuccessful { status: 503 });

    let error = fetch_releases(&http)
        .await
        .expect_err("non-success response should fail");

    assert!(error.to_string().contains("503"));
    assert_eq!(http.requested_urls(), vec![releases_page_url()]);
}

#[cfg(unix)]
fn test_installer(script: &str) -> (TempDir, std::path::PathBuf) {
    let directory = TempDir::new().expect("installer directory");
    let installer = crate::fork_installer::write_installer(directory.path(), script.as_bytes())
        .expect("write installer");
    (directory, installer)
}

#[cfg(unix)]
#[test]
fn updater_reexec_preserves_paths_and_recovers_deleted_working_directory() {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::PermissionsExt;

    for (delete_cwd, home_name) in [
        (false, "home"),
        (true, "home"),
        (false, "alias"),
        (true, "alias"),
        // macOS filesystems reject non-Unicode names.
        #[cfg(target_os = "linux")]
        (false, "link/../home"),
    ] {
        let home = TempDir::new().expect("home");
        let project = home.path().join("project");
        let state = home.path().join("state");
        let codex_home = home.path().join("home");
        std::fs::create_dir(&project).expect("project");
        std::fs::create_dir(&state).expect("state");
        std::fs::create_dir(&codex_home).expect("codex home");
        std::os::unix::fs::symlink(&codex_home, home.path().join("alias")).expect("home alias");
        if home_name == "link/../home" {
            let non_unicode = home.path().join(std::ffi::OsStr::from_bytes(b"home-\xff"));
            std::fs::create_dir_all(non_unicode.join("child")).expect("non-Unicode parent");
            std::fs::create_dir(non_unicode.join("home")).expect("non-Unicode home");
            std::os::unix::fs::symlink(non_unicode.join("child"), home.path().join("link"))
                .expect("Unicode alias");
        }
        let executable = home.path().join("updater");
        std::fs::write(
        &executable,
        b"#!/bin/sh\ntest -d \"$CODEX_HOME\" || exit 1\n{ pwd -P; printf '%s\\n' \"$CODEX_HOME\" \"$CODEX_SQLITE_HOME\" \"$AWS_CONFIG_FILE\" \"$SSL_CERT_DIR\" \"$NPM_CONFIG_CAFILE\"; } > \"$CODEX_TEST_UPDATER_OUTPUT\"\n",
    )
    .expect("updater shim");
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
            .expect("executable");
        let output = state.join("output");
        let project_path = project.canonicalize().expect("project path");
        let ca_file = if delete_cwd {
            project_path.join("ca.pem").into_os_string()
        } else {
            " ca.pem ".into()
        };
        let cert_dir = if delete_cwd {
            project_path.join("certs")
        } else {
            std::path::PathBuf::from("certs")
        };
        let result = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args(["--exact", "update_loop::tests::updater_reexec_child"])
            .current_dir(&project)
            .env("CODEX_TEST_UPDATER_EXECUTABLE", &executable)
            .env("CODEX_TEST_UPDATER_STATE", &state)
            .env("CODEX_TEST_UPDATER_OUTPUT", &output)
            .env("CODEX_TEST_UPDATER_DELETE_CWD", delete_cwd.to_string())
            .env("CODEX_HOME", format!("../{home_name}"))
            .env("CODEX_SQLITE_HOME", " sqlite ")
            .env("AWS_CONFIG_FILE", "~/config")
            .env("NPM_CONFIG_CAFILE", ca_file)
            .env(
                "SSL_CERT_DIR",
                std::env::join_paths([&cert_dir, &state]).unwrap(),
            )
            .output()
            .expect("spawn updater test");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let state = state.canonicalize().expect("state path");
        let expected = [
            if delete_cwd { &state } else { &project_path }
                .display()
                .to_string(),
            if home_name == "link/../home" {
                project_path.join(format!("../{home_name}"))
            } else {
                home.path()
                    .canonicalize()
                    .expect("parent path")
                    .join(home_name)
            }
            .display()
            .to_string(),
            " sqlite ".to_string(),
            "~/config".to_string(),
            std::env::join_paths([project_path.join("certs"), home.path().join("state")])
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            project_path.join("ca.pem").display().to_string(),
        ]
        .join("\n")
            + "\n";
        assert_eq!(
            std::fs::read_to_string(output).expect("updater output"),
            expected
        );
    }
}

#[cfg(unix)]
#[test]
fn updater_reexec_child() {
    use std::os::unix::process::CommandExt;

    let Some(executable) = std::env::var_os("CODEX_TEST_UPDATER_EXECUTABLE") else {
        return;
    };
    let state = std::env::var_os("CODEX_TEST_UPDATER_STATE").expect("state path");
    if std::env::var("CODEX_TEST_UPDATER_DELETE_CWD").as_deref() == Ok("true") {
        // Model the initial background launch while the relative home is resolvable.
        let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
        command
            .args(["--exact", "update_loop::tests::updater_reexec_child"])
            .env("CODEX_TEST_UPDATER_DELETE_CWD", "prepared");
        crate::background_command::set_working_directory(
            &mut command,
            std::path::Path::new(&state),
        )
        .expect("prepare background launch");
        panic!("failed to launch prepared updater: {}", command.exec());
    }
    if std::env::var("CODEX_TEST_UPDATER_DELETE_CWD").as_deref() == Ok("prepared") {
        std::fs::remove_dir(std::env::current_dir().expect("project cwd"))
            .expect("delete updater cwd");
    }
    super::reexec_managed_updater(
        std::path::Path::new(&executable),
        std::path::Path::new(&state),
    )
    .expect("reexec updater");
    panic!("successful exec does not return");
}

struct FakeInstallerHttp {
    response: InstallerResponse,
    requested_urls: Mutex<Vec<String>>,
}

impl FakeInstallerHttp {
    fn new(response: InstallerResponse) -> Self {
        Self {
            response,
            requested_urls: Mutex::new(Vec::new()),
        }
    }

    fn requested_urls(&self) -> Vec<String> {
        self.requested_urls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// Serves one release whose installer is the configured response.
impl InstallerHttp for FakeInstallerHttp {
    async fn get(&self, url: &str) -> anyhow::Result<InstallerResponse> {
        self.requested_urls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(url.to_string());
        Ok(match &self.response {
            InstallerResponse::Success(installer) if url.starts_with(fork::RELEASES_URL) => {
                InstallerResponse::Success(fake_releases(installer))
            }
            response => response.clone(),
        })
    }
}

#[cfg(unix)]
#[tokio::test]
async fn cancelling_installer_stops_its_children() {
    let home = tempfile::TempDir::new().expect("home");
    let ready = home.path().join("ready");
    let delayed = home.path().join("delayed");
    let (_directory, installer) = test_installer(&format!(
        "#!/bin/sh\n(trap '' TERM; echo ready > '{ready}'; sleep 4; echo late > '{delayed}') &\nwait\n",
        ready = ready.display(),
        delayed = delayed.display(),
    ));
    let (cancel, cancelled) = tokio::sync::oneshot::channel();
    let ready_for_signal = ready.clone();
    let signal_sender = tokio::spawn(async move {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !ready_for_signal.exists() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "installer did not start"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        cancel.send(()).expect("cancel installer");
    });
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        super::run_installer(
            &installer,
            super::InstallerMode::Update("0.150.0-test"),
            &home
                .path()
                .join(".local/share/mevanlc-codex/packages/app-server-daemon"),
            async { cancelled.await.ok() },
        ),
    )
    .await
    .expect("installer cancellation timed out")
    .expect("installer cancellation failed");
    signal_sender.await.expect("signal sender");
    assert!(matches!(result, super::UpdateLoopControl::Stop));
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(!delayed.exists());
}

#[cfg(unix)]
fn manual_update_daemon(home: &TempDir) -> (Daemon, String) {
    let target = if cfg!(target_os = "macos") {
        format!("{}-apple-darwin", std::env::consts::ARCH)
    } else {
        format!("{}-unknown-linux-musl", std::env::consts::ARCH)
    };
    let release = format!("1.0.0-{target}");
    let standalone = home
        .path()
        .join(".local/share/mevanlc-codex/packages/app-server-daemon");
    let bin = standalone.join("releases").join(&release).join("bin/codex");
    std::fs::create_dir_all(bin.parent().expect("binary parent")).expect("release directory");
    codex_utils_cargo_bin::write_executable(
        &bin,
        "#!/bin/sh\nif [ \"$1\" = '--version' ]; then echo codex 1.0.0; else exec sleep 30; fi\n",
    )
    .expect("managed binary");
    std::os::unix::fs::symlink(format!("releases/{release}"), standalone.join("current"))
        .expect("current release");
    std::fs::write(standalone.join("auto-update-version"), &release).expect("latest marker");
    std::fs::write(
        bin.parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("codex-package.json"),
        "{}",
    )
    .unwrap();
    let state = home.path().join("app-server-daemon");
    std::fs::create_dir(&state).unwrap();
    std::fs::write(state.join("app-server.stderr.log"), b"").unwrap();
    (
        Daemon {
            codex_home: home.path().to_path_buf(),
            package_root: standalone.clone(),
            log_diagnostics: false,
            socket_path: home.path().join("app-server-control/server.sock"),
            pid_file: state.join("app-server.pid"),
            update_pid_file: state.join("app-server-updater.pid"),
            operation_lock_file: state.join("daemon.lock"),
            settings_file: state.join("settings.json"),
            managed_codex_bin: standalone.join("current/bin/codex"),
        },
        release,
    )
}

#[cfg(unix)]
fn test_terminate() -> tokio::signal::unix::Signal {
    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("install test signal handler")
}

#[cfg(unix)]
#[tokio::test]
async fn manual_request_retries_after_updater_replacement() {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let home = TempDir::new().expect("home");
    let (daemon, _) = manual_update_daemon(&home);
    let socket_path = daemon.manual_update_socket_path();
    codex_uds::prepare_private_socket_directory(socket_path.parent().expect("socket parent"))
        .await
        .expect("socket directory");
    let mut listener = codex_uds::UnixListener::bind(&socket_path)
        .await
        .expect("old updater socket");
    let expected = UpdateOutput {
        status: UpdateStatus::NoUpdate,
        managed_codex_path: daemon.managed_codex_bin.clone(),
        installed_version: None,
        running_version: None,
        message: "already current".to_string(),
    };
    let reply = expected.clone();
    let server = tokio::spawn(async move {
        let mut old = listener.accept().await.expect("first connection");
        let mut request = [0; 7];
        old.read_exact(&mut request).await.expect("first request");
        drop(old);
        drop(listener);
        tokio::fs::remove_file(&socket_path)
            .await
            .expect("remove old socket");
        let mut successor = codex_uds::UnixListener::bind(&socket_path)
            .await
            .expect("successor socket");
        let mut connection = successor.accept().await.expect("retried connection");
        connection
            .read_exact(&mut request)
            .await
            .expect("retried request");
        connection
            .write_all(&serde_json::to_vec(&Ok::<_, String>(reply)).expect("serialize response"))
            .await
            .expect("send response");
    });
    assert_eq!(
        super::manual_update::request(&daemon)
            .await
            .expect("request survives handoff"),
        expected
    );
    server.await.expect("replacement task");
}

#[cfg(unix)]
#[tokio::test]
async fn manual_request_accepts_maximum_installer_stderr_detail() {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let home = TempDir::new().expect("home");
    let (daemon, _) = manual_update_daemon(&home);
    let socket_path = daemon.manual_update_socket_path();
    codex_uds::prepare_private_socket_directory(socket_path.parent().expect("socket parent"))
        .await
        .expect("socket directory");
    let mut listener = codex_uds::UnixListener::bind(&socket_path)
        .await
        .expect("updater socket");
    let expected_suffix = "installer failed";
    let message = format!(
        "standalone updater failed:\n{}{}",
        "\0".repeat(super::INSTALLER_STDERR_TAIL_BYTES),
        expected_suffix,
    );
    let response = serde_json::to_vec(&Err::<UpdateOutput, _>(message)).expect("serialize error");
    assert!(response.len() < super::manual_update::MAX_RESPONSE_BYTES as usize);
    let server = tokio::spawn(async move {
        let mut connection = listener.accept().await.expect("request connection");
        let mut request = [0; 7];
        connection.read_exact(&mut request).await.expect("request");
        connection
            .write_all(&response)
            .await
            .expect("send response");
    });

    let error = super::manual_update::request(&daemon)
        .await
        .expect_err("manual update should fail");
    assert!(error.to_string().ends_with(expected_suffix));
    server.await.expect("server task");
}

#[cfg(unix)]
#[tokio::test]
async fn manual_request_recovers_when_one_shot_updater_exits() {
    use tokio::io::AsyncReadExt;

    let home = TempDir::new().expect("home");
    let (daemon, _) = manual_update_daemon(&home);
    let socket_path = daemon.manual_update_socket_path();
    codex_uds::prepare_private_socket_directory(socket_path.parent().expect("socket parent"))
        .await
        .expect("socket directory");
    let mut listener = codex_uds::UnixListener::bind(&socket_path)
        .await
        .expect("one-shot updater socket");
    let server = tokio::spawn(async move {
        let mut connection = listener.accept().await.expect("request connection");
        let mut request = [0; 7];
        connection.read_exact(&mut request).await.expect("request");
        drop(connection);
        drop(listener);
        tokio::fs::remove_file(socket_path)
            .await
            .expect("remove exited updater socket");
    });
    // Without the selected executable, the startup path reports unsupported. A
    // retry that only waits for a successor would time out instead.
    std::fs::remove_file(&daemon.managed_codex_bin).expect("remove selected binary");
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        super::manual_update::request(&daemon),
    )
    .await
    .expect("retry should return to normal startup")
    .expect("unsupported response");
    assert_eq!(result.status, UpdateStatus::Unsupported);
    server.await.expect("updater task");
}

#[cfg(unix)]
#[tokio::test]
async fn unsupported_request_preserves_updater_schedule() {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let home = TempDir::new().expect("home");
    let (daemon, _) = manual_update_daemon(&home);
    let daemon = std::sync::Arc::new(daemon);
    let identity = executable_identity(&daemon.managed_codex_bin)
        .await
        .expect("updater identity");
    let socket_path = daemon.manual_update_socket_path();
    let http = FakeInstallerHttp::new(InstallerResponse::Success(Vec::new()));
    let updater_daemon = std::sync::Arc::clone(&daemon);
    let worker = tokio::spawn(async move {
        super::run_with_http(
            &http,
            &updater_daemon,
            &identity,
            /*restore_release*/ None,
        )
        .await
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !socket_path.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "updater did not listen"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // An external installer can pin while this ordinary worker still listens.
    // Raw IPC must not gain the manual CLI's authority to restore production.
    let marker = super::selected_release(&daemon)
        .unwrap()
        .0
        .join("auto-update-version");
    let previous_marker = std::fs::read(&marker).unwrap();
    std::fs::remove_file(&marker).unwrap();
    let mut pinned = codex_uds::UnixStream::connect(&socket_path).await.unwrap();
    pinned.write_all(b"update\n").await.unwrap();
    let mut response = Vec::new();
    pinned.read_to_end(&mut response).await.unwrap();
    let response: Result<UpdateOutput, String> = serde_json::from_slice(&response).unwrap();
    assert_eq!(response.unwrap().status, UpdateStatus::Unsupported);
    assert!(!marker.exists());
    std::fs::write(&marker, previous_marker).unwrap();
    std::fs::remove_file(&daemon.managed_codex_bin).expect("remove selected binary");
    let mut malformed = codex_uds::UnixStream::connect(&socket_path)
        .await
        .expect("connect malformed request");
    malformed
        .write_all(b"upd")
        .await
        .expect("send partial request");
    malformed.shutdown().await.expect("disconnect request");
    let mut discarded = Vec::new();
    malformed
        .read_to_end(&mut discarded)
        .await
        .expect("rejected request");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!worker.is_finished(), "malformed request stopped updater");
    assert_eq!(
        super::manual_update::request(&daemon)
            .await
            .expect("unsupported response")
            .status,
        UpdateStatus::Unsupported
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!worker.is_finished(), "unsupported request stopped updater");
    worker.abort();
}

#[cfg(unix)]
async fn test_control_server(
    daemon: &Daemon,
    home: &std::path::Path,
) -> tokio::task::JoinHandle<()> {
    use futures::SinkExt;
    use futures::StreamExt;
    std::fs::create_dir_all(daemon.socket_path.parent().expect("socket parent"))
        .expect("socket directory");
    let mut listener = codex_uds::UnixListener::bind(&daemon.socket_path)
        .await
        .expect("control listener");
    let codex_home = home.to_path_buf();
    tokio::spawn(async move {
        loop {
            let connection = listener.accept().await.expect("control connection");
            let mut websocket = tokio_tungstenite::accept_async(connection)
                .await
                .expect("websocket handshake");
            websocket
                .next()
                .await
                .expect("initialize request")
                .expect("frame");
            let version = if std::fs::read_to_string(
                crate::managed_install::package_root(&codex_home).join("auto-update-version"),
            )
            .unwrap_or_default()
            .starts_with("1.1.0")
            {
                "1.1.0"
            } else {
                "1.0.0"
            };
            websocket.send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::json!({"id": 1, "result": {
                    "userAgent": format!("codex_app_server_daemon/{version}"),
                    "codexHome": codex_home, "platformFamily": "unix", "platformOs": std::env::consts::OS,
                }}).to_string().into(),
            )).await.expect("initialize response");
            websocket
                .next()
                .await
                .expect("initialized notification")
                .expect("frame");
        }
    })
}

#[cfg(unix)]
#[tokio::test]
async fn scheduled_no_op_preserves_daemon_and_hands_off_stale_updater() {
    let home = TempDir::new().expect("home");
    let (mut daemon, release) = manual_update_daemon(&home);
    let root = daemon.package_root.clone();
    daemon = daemon.current_installation().unwrap();
    let stale = root
        .join("releases")
        .join(release.replace("1.0.0", "0.8.0"));
    std::fs::create_dir_all(stale.join("bin")).unwrap();
    std::fs::copy(&daemon.managed_codex_bin, stale.join("bin/codex")).unwrap();
    std::fs::write(
        stale.join("codex-package.json"),
        serde_json::json!({
            "version": "0.8.0",
            "target": release.strip_prefix("1.0.0-").unwrap(),
            "entrypoint": "bin/codex"
        })
        .to_string(),
    )
    .unwrap();
    let settings = crate::settings::DaemonSettings::default();
    let backend = crate::backend::pid_backend(daemon.backend_paths(&settings));
    backend.start().await.expect("start daemon");
    let server = test_control_server(&daemon, home.path()).await;
    let pid_record = std::fs::read(&daemon.pid_file).expect("daemon PID record");
    let no_op = FakeInstallerHttp::new(InstallerResponse::Success(b"#!/bin/sh\nexit 0\n".to_vec()));

    let outcome = super::update_once(
        &no_op,
        &daemon,
        &executable_identity_from_reader(&b"stale updater"[..]).expect("updater identity"),
        &mut test_terminate(),
        super::UpdateTrigger::Scheduled,
    )
    .await
    .expect("scheduled update");

    assert!(matches!(
        outcome,
        (
            super::UpdateLoopControl::Continue,
            Some(RestartIfRunningOutcome::AlreadyCurrent)
        )
    ));
    assert_eq!(
        std::fs::read(&daemon.pid_file).expect("daemon PID record"),
        pid_record
    );
    assert!(daemon.managed_codex_bin.is_file());
    assert!(stale.exists());
    backend.stop().await.expect("stop daemon");
    server.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn scheduled_resource_only_update_restarts_daemon() {
    let home = TempDir::new().expect("home");
    let (daemon, release) = manual_update_daemon(&home);
    let settings = crate::settings::DaemonSettings::default();
    let backend = crate::backend::pid_backend(daemon.backend_paths(&settings));
    backend.start().await.expect("start daemon");
    let server = test_control_server(&daemon, home.path()).await;
    let previous_pid_record = std::fs::read(&daemon.pid_file).expect("daemon PID record");
    let root = home
        .path()
        .join(".local/share/mevanlc-codex/packages/app-server-daemon");
    let next_release = release.replacen("1.0.0", "1.0.1", 1);
    let installer = FakeInstallerHttp::new(InstallerResponse::Success(
        format!(
            "#!/bin/sh\nmkdir -p '{root}/releases/{next_release}/bin'\ncp '{root}/releases/{release}/bin/codex' '{root}/releases/{next_release}/bin/codex'\ncp '{root}/releases/{release}/codex-package.json' '{root}/releases/{next_release}/codex-package.json'\nln -sfn 'releases/{next_release}' '{root}/current'\nprintf '{next_release}' > '{root}/auto-update-version'\n",
            root = root.display(),
        )
        .into_bytes(),
    ));

    let outcome = super::update_once(
        &installer,
        &daemon,
        &executable_identity(&daemon.managed_codex_bin)
            .await
            .expect("updater identity"),
        &mut test_terminate(),
        super::UpdateTrigger::Scheduled,
    )
    .await
    .expect("scheduled update");

    assert!(matches!(
        outcome,
        (
            super::UpdateLoopControl::Continue,
            Some(RestartIfRunningOutcome::Restarted)
        )
    ));
    assert_ne!(
        std::fs::read(&daemon.pid_file).expect("daemon PID record"),
        previous_pid_record
    );
    backend.stop().await.expect("stop daemon");
    server.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn manual_update_restarts_managed_daemon_with_automatic_updates_disabled() {
    check_manual_update_restart("standalone").await;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[tokio::test]
async fn readiness_waits_for_a_slow_managed_daemon() {
    let home = TempDir::new().unwrap();
    let (daemon, _) = manual_update_daemon(&home);
    let settings = crate::settings::DaemonSettings::default();
    let backend = crate::backend::pid_backend(daemon.backend_paths(&settings));
    backend.start().await.unwrap();

    tokio::time::pause();
    let (ready, listener) = tokio::join!(daemon.wait_until_ready(), async {
        // Model a cold start that takes longer than the old Linux readiness deadline.
        tokio::time::sleep(Duration::from_secs(15)).await;
        tokio::time::resume();
        test_control_server(&daemon, home.path())
            .await
            .abort_handle()
    });
    backend.stop().await.unwrap();
    listener.abort();
    assert_eq!(ready.unwrap().app_server_version, "1.0.0");
}

#[cfg(unix)]
#[tokio::test]
async fn daemon_start_and_restart_preserve_launch_features() {
    for features in [
        std::collections::BTreeMap::new(),
        std::collections::BTreeMap::from([
            ("api_key_model_discovery".to_string(), true),
            ("code_mode_host".to_string(), false),
        ]),
    ] {
        let home = TempDir::new().unwrap();
        let (daemon, _) = manual_update_daemon(&home);
        let args_path = home.path().join("launch-args");
        std::fs::write(
        &daemon.settings_file,
        r#"{"featureOverrides":{"auth_elicitation":true},"updater":{"autoUpdateEnabled":false}}"#,
    )
    .unwrap();
        codex_utils_cargo_bin::write_executable(&daemon.managed_codex_bin, &format!(
        "#!/bin/sh\nif [ \"$1\" = --version ]; then echo codex 1.0.0; exit; fi\nif [ \"$3\" = --help ]; then exit; fi\nprintf '%s\\n' \"$@\" > '{}'\nexec sleep 30\n",
        args_path.display(),
    )).unwrap();
        let control = async {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(/*secs*/ 10);
            while !args_path.exists() {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "daemon did not launch"
                );
                tokio::time::sleep(Duration::from_millis(/*millis*/ 20)).await;
            }
            test_control_server(&daemon, home.path())
                .await
                .abort_handle()
        };
        let (started, server) = tokio::join!(daemon.start(&features), control);
        assert_eq!(started.unwrap().status, crate::LifecycleStatus::Started);
        assert_eq!(
            daemon.load_settings().await.unwrap().feature_overrides,
            features
        );
        let expected = if features.is_empty() {
            "app-server\n--listen\nunix://\n--analytics-default-enabled\n--managed-daemon\n"
        } else {
            "app-server\n--listen\nunix://\n--analytics-default-enabled\n-c\nfeatures.api_key_model_discovery=true\n-c\nfeatures.code_mode_host=false\n--managed-daemon\n"
        };
        assert_eq!(std::fs::read_to_string(&args_path).unwrap(), expected);
        let reused = daemon
            .start(&std::collections::BTreeMap::from([(
                "api_key_model_discovery".to_string(),
                false,
            )]))
            .await
            .unwrap();
        assert_eq!(reused.status, crate::LifecycleStatus::AlreadyRunning);
        assert_eq!(
            daemon.load_settings().await.unwrap().feature_overrides,
            features
        );
        assert_eq!(std::fs::read_to_string(&args_path).unwrap(), expected);
        std::fs::remove_file(&args_path).unwrap();
        let restarted = daemon.restart(&std::collections::BTreeMap::new()).await;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(/*secs*/ 10);
        while std::fs::read_to_string(&args_path).ok().as_deref() != Some(expected)
            && tokio::time::Instant::now() < deadline
        {
            tokio::time::sleep(Duration::from_millis(/*millis*/ 20)).await;
        }
        let args = std::fs::read_to_string(args_path);
        daemon.stop().await.unwrap();
        server.abort();
        assert_eq!(restarted.unwrap().status, crate::LifecycleStatus::Restarted);
        assert_eq!(args.unwrap(), expected);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn confirmed_feature_restart_preserves_ownership_and_skips_matching_settings() {
    use crate::LifecycleStatus;
    use std::collections::BTreeMap;

    for managed in [true, false] {
        let home = TempDir::new().unwrap();
        let (daemon, _) = manual_update_daemon(&home);
        std::fs::write(&daemon.settings_file,
            r#"{"featureOverrides":{"auth_elicitation":true,"api_key_model_discovery":true},"updater":{"autoUpdateEnabled":false},"shutdownGraceSeconds":0}"#
        ).unwrap();
        let original = daemon.load_settings().await.unwrap();
        if managed {
            daemon.start_managed_backend(&original).await.unwrap();
        }
        let server = test_control_server(&daemon, home.path()).await;
        let _lock = daemon.acquire_operation_lock().await.unwrap();
        let requested = BTreeMap::from([
            ("api_key_model_discovery".to_string(), false),
            ("mcp_oauth_refresh_coordination".to_string(), true),
        ]);
        if managed {
            // Hide the selection without removing the script the spawned shell still needs.
            let selected_package = daemon.managed_codex_bin.parent().unwrap();
            let saved_package = selected_package.with_extension("saved");
            std::fs::rename(selected_package, &saved_package).unwrap();
            let error = daemon
                .restart_with_features_locked(&requested)
                .await
                .unwrap_err();
            std::fs::rename(saved_package, selected_package).unwrap();
            assert!(
                error.to_string().contains("daemon executable not found"),
                "{error:#}"
            );
            assert_eq!(daemon.load_settings().await.unwrap(), original);
        }
        let result = daemon.restart_with_features_locked(&requested).await;
        if managed {
            assert_eq!(result.unwrap().status, LifecycleStatus::Restarted);
            let pid = std::fs::read(&daemon.pid_file).unwrap();
            let mut expected = original;
            expected.feature_overrides.extend(requested.clone());
            assert_eq!(daemon.load_settings().await.unwrap(), expected);
            assert_eq!(
                daemon
                    .restart_with_features_locked(&requested)
                    .await
                    .unwrap()
                    .status,
                LifecycleStatus::AlreadyRunning
            );
            assert_eq!(std::fs::read(&daemon.pid_file).unwrap(), pid);
            daemon.stop().await.unwrap();
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("no running managed daemon")
            );
            assert_eq!(daemon.load_settings().await.unwrap(), original);
        }
        server.abort();
    }
}

#[cfg(unix)]
#[tokio::test]
async fn manual_update_restarts_local_daemon_with_automatic_updates_disabled() {
    check_manual_update_restart("app-server-daemon").await;
}

#[cfg(unix)]
async fn check_manual_update_restart(package_directory: &str) {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let local_package = package_directory == "app-server-daemon";
    let home = TempDir::new().expect("home");
    let (mut daemon, mut release) = manual_update_daemon(&home);
    let standalone = daemon.package_root.clone();
    if local_package {
        let local = format!("local-development-{release}");
        std::fs::rename(
            standalone.join("releases").join(&release),
            standalone.join("releases").join(&local),
        )
        .unwrap();
        std::fs::remove_file(standalone.join("current")).unwrap();
        std::os::unix::fs::symlink(format!("releases/{local}"), standalone.join("current"))
            .unwrap();
        std::fs::remove_file(standalone.join("auto-update-version")).unwrap();
        daemon.managed_codex_bin = standalone.join("current/bin/codex");
        release = local;
    }
    let daemon = std::sync::Arc::new(daemon);
    std::fs::create_dir_all(daemon.settings_file.parent().expect("state directory"))
        .expect("state directory");
    std::fs::write(
        &daemon.settings_file,
        r#"{"updater":{"autoUpdateEnabled":false}}"#,
    )
    .expect("disabled updater");
    let server = test_control_server(&daemon, home.path()).await;
    let settings = crate::settings::DaemonSettings::default();
    let backend = crate::backend::pid_backend(daemon.backend_paths(&settings));
    backend.start().await.expect("start daemon");
    let current_pid = || {
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(&daemon.pid_file).expect("daemon PID record"),
        )
        .expect("PID JSON")["pid"]
            .as_u64()
            .expect("PID")
    };
    let before = current_pid();
    let version = if local_package { "1.0.0" } else { "1.1.0" };
    let next = release
        .trim_start_matches("local-development-")
        .replacen("1.0.0", version, 1);
    let guard = if local_package {
        "CODEX_INSTALL_IF_CURRENT"
    } else {
        "CODEX_INSTALL_IF_LATEST"
    };
    let install_binary = if local_package {
        // Same binary and version, but a different package: it must still restart.
        format!(
            "cp '{root}/releases/{release}/bin/codex' '{root}/releases/{next}/bin/codex'",
            root = standalone.display()
        )
    } else {
        format!(
            r#"printf '#!/bin/sh\nif [ "$1" = --version ]; then echo codex 1.1.0; else exec sleep 30; fi\n' > '{root}/releases/{next}/bin/codex'"#,
            root = standalone.display()
        )
    };
    let ready = home.path().join("installer-ready");
    let proceed = home.path().join("installer-proceed");
    let script = format!(
        "#!/bin/sh\nif [ \"$CODEX_UPDATE_FROM_RELEASE\" = '{next}' ]; then exit 0; fi\ntest \"${guard}\" = 1 || exit 4\ntest \"$CODEX_UPDATE_FROM_RELEASE\" = '{release}' || exit 5\ntouch '{ready}'\nwhile [ ! -e '{proceed}' ]; do sleep .05; done\nmkdir -p '{root}/releases/{next}/bin'\n{install_binary}\nchmod +x '{root}/releases/{next}/bin/codex'\nprintf '{{}}' > '{root}/releases/{next}/codex-package.json'\nln -sfn 'releases/{next}' '{root}/current'\nprintf '{next}' > '{root}/auto-update-version'\n",
        root = standalone.display(),
        ready = ready.display(),
        proceed = proceed.display(),
    );
    let http = FakeInstallerHttp::new(InstallerResponse::Success(script.into_bytes()));
    let updater_daemon = std::sync::Arc::clone(&daemon);
    let restore_release = local_package.then(|| release.clone());
    let worker = tokio::spawn(async move {
        super::run_with_http(
            &http,
            &updater_daemon,
            &executable_identity_from_reader(&b"updater"[..]).expect("updater identity"),
            restore_release,
        )
        .await
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !daemon.manual_update_socket_path().exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "updater did not listen"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let request_daemon = std::sync::Arc::clone(&daemon);
    let request = tokio::spawn(async move { super::manual_update::request(&request_daemon).await });
    while !ready.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "installer did not start"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let mut queued = codex_uds::UnixStream::connect(&daemon.manual_update_socket_path())
        .await
        .expect("queue a second update");
    queued
        .write_all(b"update\n")
        .await
        .expect("send second update");
    std::fs::write(proceed, b"go").expect("release installer");
    let output = request
        .await
        .expect("first request task")
        .expect("manual update");
    assert_eq!(output.status, UpdateStatus::Updated);
    assert_eq!(output.installed_version.as_deref(), Some(version));
    assert_eq!(output.running_version.as_deref(), Some(version));
    assert_eq!(
        output.managed_codex_path,
        standalone.join("current/bin/codex")
    );
    let restarted = current_pid();
    assert_ne!(restarted, before);
    let mut response = Vec::new();
    queued
        .read_to_end(&mut response)
        .await
        .expect("second response");
    let second: Result<crate::UpdateOutput, String> =
        serde_json::from_slice(&response).expect("valid second response");
    assert_eq!(
        second.expect("second update").status,
        UpdateStatus::NoUpdate
    );
    assert_eq!(current_pid(), restarted);
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .expect("one-shot updater did not exit")
        .expect("updater task")
        .expect("updater loop");
    let no_op = FakeInstallerHttp::new(InstallerResponse::Success(b"#!/bin/sh\nexit 0\n".to_vec()));
    let bin = daemon
        .current_managed_codex_bin()
        .expect("current executable");
    let contents = std::fs::read_to_string(&bin).expect("managed executable");
    codex_utils_cargo_bin::write_executable(
        &bin,
        &format!("{contents}\n# same-version replacement\n"),
    )
    .expect("replace binary bytes");
    let output = manual_update_once(
        &no_op,
        &daemon,
        &executable_identity_from_reader(&b"updater"[..]).expect("updater identity"),
        &mut test_terminate(),
        super::UpdateTrigger::Manual,
    )
    .await
    .expect("retry with stale running binary");
    assert_eq!(output.status, UpdateStatus::NoUpdate);
    assert_ne!(current_pid(), restarted);
    backend.stop().await.expect("stop daemon");
    server.abort();
}

#[cfg(windows)]
#[tokio::test]
async fn windows_installer_receives_guards_and_reports_failure() {
    let home = tempfile::TempDir::new().unwrap();
    let installer = home.path().join("installer.cmd");
    let package_root = home.path().join("packages/app-server-daemon");
    std::fs::write(
        &installer,
        "@echo off\r\nif not \"%CODEX_INSTALL_DAEMON_ONLY%\"==\"1\" exit /b 3\r\nif not \"%CODEX_INSTALL_IF_CURRENT%\"==\"1\" exit /b 4\r\nif not \"%CODEX_UPDATE_FROM_RELEASE%\"==\"release\" exit /b 5\r\n",
    )
    .unwrap();
    super::run_installer(
        &installer,
        super::InstallerMode::RestoreProduction("release"),
        &package_root,
    )
    .await
    .unwrap();
    std::fs::write(
        &installer,
        "@echo off\r\necho installer failed 1>&2\r\nexit /b 1\r\n",
    )
    .unwrap();
    let error = super::run_installer(
        &installer,
        super::InstallerMode::RestoreProduction("release"),
        &package_root,
    )
    .await
    .err()
    .unwrap();
    assert!(error.to_string().contains("installer failed"), "{error:#}");
}

#[cfg(unix)]
#[tokio::test]
async fn installer_failure_includes_stderr() {
    let (_directory, installer) = test_installer(&format!(
        "#!/bin/sh\nprintf 'discard me{}' >&2\nprintf 'installer failed' >&2\nexit 1\n",
        "x".repeat(super::INSTALLER_STDERR_TAIL_BYTES),
    ));
    let error = super::run_installer(
        &installer,
        super::InstallerMode::Update("0.150.0-x86_64-unknown-linux-gnu"),
        std::path::Path::new("packages/app-server-daemon"),
        futures::future::pending(),
    )
    .await
    .err()
    .expect("installer should fail");
    let error = error.to_string();
    assert!(error.contains("installer failed"));
    assert!(!error.contains("discard me"));
}

#[cfg(unix)]
#[tokio::test]
async fn installer_failure_preserves_stderr_before_drain_timeout() {
    let (_directory, installer) =
        test_installer("#!/bin/sh\nprintf 'installer failed' >&2\nsleep 5 &\nexit 1\n");
    let error = tokio::time::timeout(
        Duration::from_secs(3),
        super::run_installer(
            &installer,
            super::InstallerMode::Update("0.150.0-x86_64-unknown-linux-gnu"),
            std::path::Path::new("packages/app-server-daemon"),
            futures::future::pending(),
        ),
    )
    .await
    .expect("stderr drain should time out")
    .err()
    .expect("installer should fail");
    assert!(error.to_string().contains("installer failed"));
}

#[cfg(unix)]
#[tokio::test]
async fn update_rejects_a_package_root_change_during_download() {
    struct ChangingRoot<'a>(&'a std::path::Path, FakeInstallerHttp);
    impl InstallerHttp for ChangingRoot<'_> {
        async fn get(&self, url: &str) -> anyhow::Result<InstallerResponse> {
            if url.starts_with(fork::RELEASES_URL) {
                let root = crate::managed_install::package_root(self.0);
                std::fs::rename(&root, root.with_extension("previous"))?;
                let replacement = root.with_extension("replacement");
                std::fs::create_dir(&replacement)?;
                std::os::unix::fs::symlink(&replacement, &root)?;
            }
            self.1.get(url).await
        }
    }
    let home = TempDir::new().unwrap();
    let (daemon, _) = manual_update_daemon(&home);
    let identity = executable_identity(&daemon.managed_codex_bin)
        .await
        .unwrap();
    let error = manual_update_once(
        &ChangingRoot(
            home.path(),
            FakeInstallerHttp::new(InstallerResponse::Success(b"#!/bin/sh\nexit 99\n".to_vec())),
        ),
        &daemon,
        &identity,
        &mut test_terminate(),
        super::UpdateTrigger::Manual,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("package root changed"));
}

#[cfg(unix)]
#[tokio::test]
async fn daemon_owned_updates_request_an_isolated_installer() {
    let home = TempDir::new().unwrap();
    let (mut daemon, release) = manual_update_daemon(&home);
    let root = daemon.package_root.clone();
    daemon.managed_codex_bin = root.join("current/bin/codex");
    let identity = executable_identity(&daemon.managed_codex_bin)
        .await
        .unwrap();
    let current = FakeInstallerHttp::new(InstallerResponse::Success(
        b"#!/bin/sh\ntest \"$CODEX_INSTALL_IF_LATEST\" = 1 && test \"$CODEX_INSTALL_DAEMON_ONLY\" = 1\n".to_vec(),
    ));
    let output = manual_update_once(
        &current,
        &daemon,
        &identity,
        &mut test_terminate(),
        super::UpdateTrigger::Manual,
    )
    .await
    .unwrap();
    assert_eq!(output.status, UpdateStatus::NoUpdate);
    assert_eq!(daemon.package_root, root);

    // Reuse the stopped daemon to verify explicit unpinning with either preference.
    let marker = root.join("auto-update-version");
    let no_op = FakeInstallerHttp::new(InstallerResponse::Success(b"#!/bin/sh\nexit 0\n".to_vec()));
    let script = format!(
        r#"#!/bin/sh
set -eu
test "$CODEX_INSTALL_IF_CURRENT" = 1
test "$CODEX_INSTALL_IF_LATEST" = 0
test "$CODEX_INSTALL_DAEMON_ONLY" = 1
test "$CODEX_UPDATE_FROM_RELEASE" = '{release}'
printf '%s' '{release}' > '{marker}'
"#,
        marker = marker.display()
    );
    let restore = FakeInstallerHttp::new(InstallerResponse::Success(script.into_bytes()));
    for auto_update_enabled in [false, true] {
        let settings = format!(r#"{{"updater":{{"autoUpdateEnabled":{auto_update_enabled}}}}}"#);
        std::fs::write(&daemon.settings_file, &settings).unwrap();
        std::fs::remove_file(&marker).unwrap();
        let error = manual_update_once(
            &no_op,
            &daemon,
            &identity,
            &mut test_terminate(),
            super::UpdateTrigger::RestoreProduction(&release),
        )
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("did not select a stable latest release")
        );
        assert!(!marker.exists());
        let restored = manual_update_once(
            &restore,
            &daemon,
            &identity,
            &mut test_terminate(),
            super::UpdateTrigger::RestoreProduction(&release),
        )
        .await
        .unwrap();
        // Restoring eligibility for the same package still reports noUpdate.
        assert_eq!(restored, output);
        assert!(!daemon.pid_file.exists());
        assert!(daemon.is_stable_standalone_release().unwrap());
        assert_eq!(
            std::fs::read_to_string(&daemon.settings_file).unwrap(),
            settings
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn scheduled_check_skips_the_installer_when_the_newest_release_is_installed() {
    let home = TempDir::new().unwrap();
    let (daemon, _) = manual_update_daemon(&home);
    let identity = executable_identity(&daemon.managed_codex_bin)
        .await
        .unwrap();
    let release = super::selected_release(&daemon).unwrap().1;
    // An installer that fails proves it never ran.
    let http = FakeInstallerHttp::new(InstallerResponse::Success(b"#!/bin/sh\nexit 1\n".to_vec()));
    std::fs::write(
        release.join("install-receipt.json"),
        serde_json::json!({"repository": "mevanlc/codex", "tag": FAKE_TAG}).to_string(),
    )
    .unwrap();
    let (control, _) = super::update_once(
        &http,
        &daemon,
        &identity,
        &mut test_terminate(),
        super::UpdateTrigger::Scheduled,
    )
    .await
    .expect("scheduled check");
    assert!(matches!(control, super::UpdateLoopControl::Continue));
    assert_eq!(http.requested_urls(), vec![releases_page_url()]);

    // A newer release than the receipt records is installed.
    std::fs::write(
        release.join("install-receipt.json"),
        serde_json::json!({"repository": "mevanlc/codex", "tag": "fork-v1.0.0"}).to_string(),
    )
    .unwrap();
    let Err(error) = super::update_once(
        &http,
        &daemon,
        &identity,
        &mut test_terminate(),
        super::UpdateTrigger::Scheduled,
    )
    .await
    else {
        panic!("the failing installer should run");
    };
    assert!(
        error.to_string().contains("exited with status"),
        "{error:#}"
    );
}
