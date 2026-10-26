//! Owns scheduled and manual installs, daemon restarts, and updater replacement.

use std::collections::VecDeque;
use std::path::Path;
#[cfg(unix)]
use std::process::Command as StdCommand;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::RouteAwareClientPool;
use futures::FutureExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
#[cfg(unix)]
use tokio::signal::unix::Signal;
#[cfg(unix)]
use tokio::signal::unix::SignalKind;
#[cfg(unix)]
use tokio::signal::unix::signal;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::sleep_until;

use crate::Daemon;
use crate::RestartIfRunningOutcome;
use crate::RestartMode;
use crate::fork_installer::InstallerHttp;
#[cfg(test)]
use crate::fork_installer::InstallerResponse;
use crate::managed_install::ExecutableIdentity;
use crate::managed_install::executable_identity;
use crate::managed_install::resolved_managed_codex_bin;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use crate::settings::DaemonSettings;
use crate::settings::UpdaterSettings;

#[path = "manual_update.rs"]
mod manual_update;

pub(crate) async fn request_manual_update(
    daemon: &Daemon,
    _http_client_factory: HttpClientFactory,
) -> Result<crate::UpdateOutput> {
    manual_update::request(daemon).await
}

const INITIAL_UPDATE_DELAY: Duration = Duration::from_secs(5 * 60);
const RESTART_RETRY_INTERVAL: Duration = Duration::from_millis(50);
// Manual update responses are capped after JSON serialization. This leaves room for the status,
// envelope, and the worst-case six-byte JSON escape for every captured byte.
const INSTALLER_STDERR_TAIL_BYTES: usize = 2 * 1024;
const INSTALLER_STDERR_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

pub(crate) async fn run(
    http_client_factory: HttpClientFactory,
    restore_release: Option<String>,
) -> Result<()> {
    let http = RouteAwareClientPool::new_without_request_logging(
        http_client_factory,
        ClientRouteClass::Other,
    );
    run_with_http(
        &http,
        &Daemon::from_environment()?,
        &current_updater_identity().await?,
        restore_release,
    )
    .await
}

async fn run_with_http(
    http: &impl InstallerHttp,
    daemon: &Daemon,
    running_updater_identity: &ExecutableIdentity,
    mut restore_release: Option<String>,
) -> Result<()> {
    #[cfg(unix)]
    let mut terminate =
        signal(SignalKind::terminate()).context("failed to install updater shutdown handler")?;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let paths = daemon.backend_paths(&DaemonSettings::default());
        for backend in [
            crate::backend::pid_backend(paths.clone()),
            crate::backend::pid_update_loop_backend(paths),
        ] {
            // Inspection failures must preserve legacy records without blocking updates.
            let _ = backend.promote_legacy_identity().await;
        }
    }
    #[cfg(windows)]
    let updater = {
        // Updater ownership needs only paths, not settings that may be mid-edit.
        crate::backend::pid_update_loop_backend(daemon.backend_paths(&DaemonSettings::default()))
    };
    #[cfg(windows)]
    updater.wait_for_ownership().await?;
    #[cfg(windows)]
    let mut terminate = Signal;
    #[cfg(windows)]
    let _installer_job = crate::backend::windows::updater_job()?;
    let socket_path = daemon.manual_update_socket_path();
    codex_uds::prepare_private_socket_directory(
        socket_path
            .parent()
            .context("updater socket has no parent")?,
    )
    .await?;
    #[cfg(unix)]
    let (listener, _socket_guard) =
        codex_app_server_transport::bind_private_daemon_socket(&socket_path).await?;
    #[cfg(windows)]
    let listener = {
        if socket_path.exists() {
            tokio::fs::remove_file(&socket_path).await?;
        }
        codex_uds::UnixListener::bind(&socket_path).await?
    };
    let mut listener = Some(listener);
    #[cfg(windows)]
    updater.mark_ready().await?;
    let needs_managed_handoff =
        match resolved_managed_codex_bin(&daemon.current_managed_codex_bin()?).await {
            Ok(managed_bin) => executable_identity(&managed_bin)
                .await
                .map_or(true, |identity| identity != *running_updater_identity),
            Err(_) => true,
        };
    let auto_update_enabled = UpdaterSettings::load(&daemon.settings_file)
        .await
        .map(|settings| settings.auto_update_enabled)
        .unwrap_or(true);
    let mut next_check = Instant::now()
        + if restore_release.is_some() || needs_managed_handoff || !auto_update_enabled {
            Duration::from_secs(15)
        } else {
            INITIAL_UPDATE_DELAY
        };
    let mut manual_handoff_pending = needs_managed_handoff;
    loop {
        tokio::select! {
            biased;
            _ = terminate.recv() => return Ok(()),
            connection = listener.as_mut().context("updater listener closed")?.accept() => {
                let connection = connection.context("failed to accept updater request")?;
                // Only a manual CLI launch under the operation lock supplies this
                // single-use authority. Ordinary socket requests cannot undo a pin.
                let restoration = restore_release.take();
                let trigger = restoration.as_deref().map_or(UpdateTrigger::Manual, UpdateTrigger::RestoreProduction);
                let disposition = match manual_update::handle_request(connection, http, daemon, running_updater_identity, &mut terminate, trigger).await {
                    Ok(manual_update::RequestDisposition::Stop) => return Ok(()),
                    Ok(disposition) => disposition,
                    Err(_) => manual_update::RequestDisposition::Continue,
                };
                if UpdaterSettings::load(&daemon.settings_file)
                    .await
                    .is_ok_and(|settings| !settings.auto_update_enabled)
                {
                    // Drain requests queued during this one-shot update before exiting.
                    next_check = Instant::now() + Duration::from_millis(100);
                    continue;
                }
                if matches!(disposition, manual_update::RequestDisposition::Unchanged) {
                    continue;
                }
                manual_handoff_pending = true;
                next_check = Instant::now();
            }
            _ = sleep_until(next_check) => {
                // The authorizing CLI may have exited before sending its request.
                if restore_release.is_some() {
                    return Ok(());
                }
                match UpdaterSettings::load(&daemon.settings_file).await {
                    Ok(settings) if !settings.auto_update_enabled => return Ok(()),
                    Err(_) => {
                        next_check = Instant::now() + Duration::from_secs(60);
                        continue;
                    }
                    Ok(_) => {}
                }
                // Failed successor cleanup leaves its PID published. The predecessor
                // must stop instead of installing again without ownership.
                #[cfg(windows)]
                updater.wait_for_ownership().await?;
                if manual_handoff_pending {
                    if !daemon.is_stable_standalone_release()? {
                        if !daemon.has_latest_selection_marker() {
                            return Ok(());
                        }
                        next_check = Instant::now() + Duration::from_secs(30);
                        continue;
                    }
                    match adopt_managed_updater(daemon, running_updater_identity, &mut listener).await {
                        Ok(UpdateLoopControl::Stop) => return Ok(()),
                        Ok(UpdateLoopControl::Continue) => {
                            manual_handoff_pending = false;
                            let Some(delay) = next_update_delay(daemon).await else {
                                return Ok(());
                            };
                            next_check = Instant::now() + delay;
                        }
                        Err(err) => {
                            if listener.is_none() {
                                return Err(err);
                            }
                            eprintln!("warning: failed to refresh managed updater: {err:#}");
                            next_check = Instant::now() + Duration::from_secs(30);
                        }
                    }
                    continue;
                }
                let started = std::time::Instant::now();
                match update_once(http, daemon, running_updater_identity, &mut terminate, UpdateTrigger::Scheduled).await {
                    Ok((UpdateLoopControl::Continue, Some(_))) => {
                        manual_handoff_pending = true;
                        next_check = Instant::now();
                        continue;
                    }
                    Ok((UpdateLoopControl::Continue, None)) => {}
                    Err(err) => {
                        let _ = crate::diagnostics::result::<()>("scheduled_update", started, Err(err));
                    }
                    Ok((UpdateLoopControl::Stop, _)) => return Ok(()),
                }
                let Some(delay) = next_update_delay(daemon).await else {
                    return Ok(());
                };
                next_check = Instant::now() + delay;
            }
        }
    }
}

async fn next_update_delay(daemon: &Daemon) -> Option<Duration> {
    match UpdaterSettings::load(&daemon.settings_file).await {
        Ok(settings) if !settings.auto_update_enabled => None,
        Ok(settings) => Some(settings.update_interval(Duration::from_secs(60))),
        Err(_) => Some(Duration::from_secs(60)),
    }
}

async fn adopt_managed_updater(
    daemon: &Daemon,
    running_identity: &ExecutableIdentity,
    listener: &mut Option<codex_uds::UnixListener>,
) -> Result<UpdateLoopControl> {
    let managed_bin = resolved_managed_codex_bin(&daemon.current_managed_codex_bin()?).await?;
    if executable_identity(&managed_bin).await? == *running_identity {
        return Ok(UpdateLoopControl::Continue);
    }
    if !crate::managed_install::supports_daemon_update_loop(&managed_bin).await {
        return Ok(UpdateLoopControl::Stop);
    }
    #[cfg(unix)]
    {
        let _ = listener;
        reexec_managed_updater(
            &managed_bin,
            daemon
                .update_pid_file
                .parent()
                .context("updater pid path has no parent")?,
        )
        .map(|_| UpdateLoopControl::Stop)
    }
    #[cfg(windows)]
    {
        let replacement = crate::backend::pid_update_loop_backend(
            daemon.backend_paths_with_bin(&daemon.load_settings().await?, &managed_bin),
        );
        listener.take();
        if let Err(err) = replacement.replace_current_updater().await {
            // A failed replacement may still own the PID if its cleanup could
            // not terminate the successor. Never reopen our request socket then.
            replacement.wait_for_ownership().await?;
            let socket_path = daemon.manual_update_socket_path();
            if socket_path.exists() {
                tokio::fs::remove_file(&socket_path).await?;
            }
            *listener = Some(codex_uds::UnixListener::bind(&socket_path).await?);
            return Err(err);
        }
        Ok(UpdateLoopControl::Stop)
    }
}

async fn sleep_or_terminate(duration: Duration, terminate: &mut Signal) -> bool {
    tokio::select! {
        _ = sleep(duration) => false,
        _ = terminate.recv() => true,
    }
}

enum UpdateLoopControl {
    Continue,
    Stop,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpdateTrigger<'a> {
    Scheduled,
    Manual,
    RestoreProduction(&'a str),
}

async fn update_once(
    http: &impl InstallerHttp,
    daemon: &Daemon,
    running_updater_identity: &ExecutableIdentity,
    terminate: &mut Signal,
    trigger: UpdateTrigger<'_>,
) -> Result<(UpdateLoopControl, Option<RestartIfRunningOutcome>)> {
    if trigger == UpdateTrigger::Scheduled
        && !UpdaterSettings::load(&daemon.settings_file)
            .await?
            .auto_update_enabled
    {
        return Ok((UpdateLoopControl::Stop, None));
    }
    if release_selection_unstable(daemon, trigger)? {
        // An installer can be between changing current and publishing its
        // latest-channel marker. Retry after the interval instead of exiting.
        return Ok((UpdateLoopControl::Continue, None));
    }
    let (package_root, previous_release_dir, previous_release) = selected_release(daemon)?;
    let root_identity = package_root.canonicalize()?;
    let installer_mode = if let UpdateTrigger::RestoreProduction(expected) = trigger {
        anyhow::ensure!(
            previous_release == expected,
            "daemon selection changed; retry the update"
        );
        InstallerMode::RestoreProduction(&previous_release)
    } else {
        InstallerMode::Update(&previous_release)
    };
    let started = std::time::Instant::now();
    let releases = tokio::select! {
        result = crate::fork_installer::fetch_releases(http) => crate::diagnostics::result("installer_fetch", started, result)?,
        _ = terminate.recv() => return Ok((UpdateLoopControl::Stop, None)),
    };
    // A scheduled check downloads and runs the installer only when a newer
    // release exists; restart handling below still applies either way.
    let install = trigger != UpdateTrigger::Scheduled
        || !crate::fork_installer::newest_release_installed(&releases, &previous_release_dir);
    if install {
        let installer = tokio::select! {
            result = crate::fork_installer::fetch_installer(http, &releases) => crate::diagnostics::result("installer_fetch", started, result)?,
            _ = terminate.recv() => return Ok((UpdateLoopControl::Stop, None)),
        };
        anyhow::ensure!(
            daemon.package_root == package_root && package_root.canonicalize()? == root_identity,
            "daemon package root changed during the update; retry the command"
        );
        if trigger == UpdateTrigger::Scheduled
            && !UpdaterSettings::load(&daemon.settings_file)
                .await?
                .auto_update_enabled
        {
            return Ok((UpdateLoopControl::Stop, None));
        }
        if release_selection_unstable(daemon, trigger)? {
            return Ok((UpdateLoopControl::Continue, None));
        }
        let directory = tempfile::Builder::new()
            .prefix("codex-fork-installer-")
            .tempdir()
            .context("failed to stage the standalone Codex updater")?;
        let installer = crate::fork_installer::write_installer(directory.path(), &installer)?;
        let started = std::time::Instant::now();
        #[cfg(unix)]
        if matches!(
            crate::diagnostics::result(
                "installer_run",
                started,
                run_installer(&installer, installer_mode, &package_root, terminate.recv()).await
            )?,
            UpdateLoopControl::Stop
        ) {
            return Ok((UpdateLoopControl::Stop, None));
        }
        #[cfg(windows)]
        tokio::select! {
            result = run_installer(&installer, installer_mode, &package_root) => { crate::diagnostics::result("installer_run", started, result)?; },
            _ = terminate.recv() => return Ok((UpdateLoopControl::Stop, None)),
        }
    }
    anyhow::ensure!(
        trigger == UpdateTrigger::Scheduled || daemon.is_stable_standalone_release()?,
        "installer did not select a stable latest release; retry the update"
    );
    if release_selection_unstable(daemon, trigger)? {
        return Ok((UpdateLoopControl::Continue, None));
    }

    let managed_codex_bin =
        resolved_managed_codex_bin(&daemon.current_managed_codex_bin()?).await?;
    crate::diagnostics::event(
        "package_selected",
        serde_json::json!({
            "release": selected_release(daemon).ok().map(|(_, _, release)| release),
            "trigger": if trigger == UpdateTrigger::Scheduled { "scheduled" } else { "manual" },
        }),
    );
    let restart_mode = match trigger {
        UpdateTrigger::Manual | UpdateTrigger::RestoreProduction(_) => {
            RestartMode::IfBinaryOrVersionChanged
        }
        // Updater adoption does not imply daemon replacement.
        UpdateTrigger::Scheduled
            if executable_identity(&managed_codex_bin).await? != *running_updater_identity =>
        {
            RestartMode::IfBinaryOrVersionChanged
        }
        UpdateTrigger::Scheduled => RestartMode::IfVersionChanged,
    };

    loop {
        if terminate.recv().now_or_never().flatten().is_some() {
            return Ok((UpdateLoopControl::Stop, None));
        }
        match daemon
            .try_restart_if_running(restart_mode, &managed_codex_bin, &package_root)
            .await?
        {
            RestartIfRunningOutcome::Busy => {
                if sleep_or_terminate(RESTART_RETRY_INTERVAL, terminate).await {
                    return Ok((UpdateLoopControl::Stop, None));
                }
            }
            RestartIfRunningOutcome::Restarted => {
                return Ok((
                    UpdateLoopControl::Continue,
                    Some(RestartIfRunningOutcome::Restarted),
                ));
            }
            RestartIfRunningOutcome::NotRunning => {
                return Ok((
                    UpdateLoopControl::Continue,
                    Some(RestartIfRunningOutcome::NotRunning),
                ));
            }
            RestartIfRunningOutcome::AlreadyCurrent
                if restart_mode == RestartMode::IfBinaryOrVersionChanged =>
            {
                anyhow::ensure!(
                    daemon.is_stable_standalone_release()?
                        && resolved_managed_codex_bin(&daemon.current_managed_codex_bin()?).await?
                            == managed_codex_bin,
                    "managed daemon changed during the update; retry"
                );
                return Ok((
                    UpdateLoopControl::Continue,
                    Some(RestartIfRunningOutcome::AlreadyCurrent),
                ));
            }
            RestartIfRunningOutcome::NotReady
                if trigger == UpdateTrigger::Scheduled
                    && restart_mode == RestartMode::IfBinaryOrVersionChanged =>
            {
                return Ok((
                    UpdateLoopControl::Continue,
                    Some(RestartIfRunningOutcome::NotReady),
                ));
            }
            RestartIfRunningOutcome::NotReady | RestartIfRunningOutcome::AlreadyCurrent => {
                anyhow::ensure!(
                    trigger == UpdateTrigger::Scheduled,
                    "managed daemon could not restart; retry when it is ready"
                );
                return Ok((
                    if daemon.is_stable_standalone_release()? {
                        UpdateLoopControl::Continue
                    } else {
                        UpdateLoopControl::Stop
                    },
                    None,
                ));
            }
        }
    }
}

fn release_selection_unstable(daemon: &Daemon, trigger: UpdateTrigger<'_>) -> Result<bool> {
    if daemon.is_stable_standalone_release()?
        || matches!(trigger, UpdateTrigger::RestoreProduction(_))
            && manual_update::supported(daemon)?
    {
        return Ok(false);
    }
    anyhow::ensure!(
        trigger == UpdateTrigger::Scheduled,
        "standalone install selection changed during the update"
    );
    Ok(true)
}

fn selected_release(daemon: &Daemon) -> Result<(std::path::PathBuf, std::path::PathBuf, String)> {
    let root = daemon.package_root.clone();
    let release = std::fs::canonicalize(root.join("current"))?;
    let name = release
        .file_name()
        .context("managed release has no name")?
        .to_string_lossy()
        .into_owned();
    Ok((root, release, name))
}

async fn current_updater_identity() -> Result<ExecutableIdentity> {
    let current_exe =
        std::env::current_exe().context("failed to resolve current updater executable")?;
    executable_identity(&current_exe).await
}

#[cfg(unix)]
pub(crate) fn reexec_managed_updater(managed_codex_bin: &Path, state_dir: &Path) -> Result<()> {
    let mut command = StdCommand::new(managed_codex_bin);
    command.args(["app-server", "daemon", "pid-update-loop"]);
    crate::background_command::set_working_directory(&mut command, state_dir)?;
    let err = command.exec();
    Err(err).with_context(|| {
        format!(
            "failed to replace updater with managed Codex binary {}",
            managed_codex_bin.display()
        )
    })
}

enum InstallerMode<'a> {
    Update(&'a str),
    RestoreProduction(&'a str),
}

/// Run the downloaded installer with the guards that keep it from replacing a
/// selection that changed or was pinned since this update began.
async fn run_installer(
    installer: &Path,
    mode: InstallerMode<'_>,
    package_root: &Path,
    #[cfg(unix)] terminate: impl std::future::Future<Output = Option<()>>,
) -> Result<UpdateLoopControl> {
    let mut command = Command::new(installer);
    #[cfg(unix)]
    command.process_group(0);
    #[cfg(windows)]
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    let (latest_guard, current_guard, previous_release) = match mode {
        InstallerMode::Update(release) => ("1", "0", release),
        InstallerMode::RestoreProduction(release) => ("0", "1", release),
    };
    command
        .env(
            "CODEX_INSTALL_DAEMON_ONLY",
            if package_root.ends_with("app-server-daemon") {
                "1"
            } else {
                "0"
            },
        )
        .env("CODEX_INSTALL_IF_LATEST", latest_guard)
        .env("CODEX_INSTALL_IF_CURRENT", current_guard)
        .env("CODEX_UPDATE_FROM_RELEASE", previous_release)
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = spawn_installer(&mut command)
        .await
        .context("failed to invoke standalone Codex updater")?;
    let mut stderr = child
        .stderr
        .take()
        .context("standalone Codex updater stderr was unavailable")?;
    let stderr_tail = Arc::new(Mutex::new(VecDeque::with_capacity(
        INSTALLER_STDERR_TAIL_BYTES,
    )));
    let captured_tail = Arc::clone(&stderr_tail);
    let mut stderr_task = tokio::spawn(async move {
        let mut buf = [0; 4096];
        loop {
            let read = stderr.read(&mut buf).await?;
            if read == 0 {
                return Ok::<_, std::io::Error>(());
            }
            let Ok(mut tail) = captured_tail.lock() else {
                return Ok(());
            };
            if read >= INSTALLER_STDERR_TAIL_BYTES {
                tail.clear();
                tail.extend(
                    buf[read - INSTALLER_STDERR_TAIL_BYTES..read]
                        .iter()
                        .copied(),
                );
            } else {
                let retained = INSTALLER_STDERR_TAIL_BYTES - read;
                if tail.len() > retained {
                    let remove = tail.len() - retained;
                    tail.drain(..remove);
                }
                tail.extend(buf[..read].iter().copied());
            }
        }
    });
    #[cfg(windows)]
    let _installer_job = crate::backend::windows::installer_job(&child)?;
    #[cfg(unix)]
    let mut terminate = std::pin::pin!(terminate);
    #[cfg(unix)]
    let status = tokio::select! {
        result = child.wait() => result,
        _ = &mut terminate => {
            cancel_installer(&mut child).await;
            stderr_task.abort();
            return Ok(UpdateLoopControl::Stop);
        }
    };
    #[cfg(windows)]
    let status = child.wait().await;
    let status = match status {
        Ok(status) => status,
        Err(err) => {
            stderr_task.abort();
            return Err(err).context("failed to wait for standalone Codex updater");
        }
    };

    if status.success() {
        stderr_task.abort();
        Ok(UpdateLoopControl::Continue)
    } else {
        let _ = tokio::time::timeout(INSTALLER_STDERR_DRAIN_TIMEOUT, &mut stderr_task).await;
        stderr_task.abort();
        let bytes = stderr_tail
            .lock()
            .map(|tail| tail.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let detail = String::from_utf8_lossy(&bytes);
        let detail = detail.trim();
        if detail.is_empty() {
            anyhow::bail!("standalone Codex updater exited with status {status}")
        }
        anyhow::bail!("standalone Codex updater exited with status {status}:\n{detail}")
    }
}

async fn spawn_installer(command: &mut Command) -> std::io::Result<tokio::process::Child> {
    // Another thread forking while the installer was being written can hold
    // its write descriptor open for a moment.
    #[cfg(unix)]
    for _ in 0..20 {
        match command.spawn() {
            Err(error) if error.raw_os_error() == Some(libc::ETXTBSY) => {
                sleep(Duration::from_millis(50)).await;
            }
            result => return result,
        }
    }
    command.spawn()
}

#[cfg(unix)]
async fn cancel_installer(child: &mut tokio::process::Child) {
    let Some(pid) = child.id().and_then(|pid| libc::pid_t::try_from(pid).ok()) else {
        return;
    };
    // TERM lets the installer roll back; its OS lock is released when it exits.
    unsafe { libc::kill(-pid, libc::SIGTERM) };
    sleep(Duration::from_secs(2)).await;
    // Keep the installer unreaped until after the group kill, so its PID cannot
    // be reused while descendants that ignored TERM are still running.
    unsafe { libc::kill(-pid, libc::SIGKILL) };
    let _ = child.wait().await;
}

#[cfg(test)]
#[path = "update_loop_tests.rs"]
mod tests;

#[cfg(windows)]
struct Signal;

#[cfg(windows)]
impl Signal {
    async fn recv(&mut self) -> Option<()> {
        // An unreadable control path must stop the updater rather than disable shutdown.
        let _ = codex_app_server_transport::daemon_shutdown_signal().await;
        Some(())
    }
}
