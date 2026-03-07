use std::fs::File;
use std::fs::TryLockError;
use std::future::Future;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use codex_apply_patch::CODEX_CORE_APPLY_PATCH_ARG1;
use codex_utils_home_dir::find_codex_home;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use tempfile::TempDir;

const LINUX_SANDBOX_ARG0: &str = "codex-linux-sandbox";
const APPLY_PATCH_ARG0: &str = "apply_patch";
const MISSPELLED_APPLY_PATCH_ARG0: &str = "applypatch";
#[cfg(unix)]
const EXECVE_WRAPPER_ARG0: &str = "codex-execve-wrapper";
const LOCK_FILENAME: &str = ".lock";
const PROCESS_METADATA_FILENAME: &str = ".process";
const TOKIO_WORKER_STACK_SIZE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Arg0DispatchPaths {
    pub codex_linux_sandbox_exe: Option<PathBuf>,
    pub main_execve_wrapper_exe: Option<PathBuf>,
}

/// Keeps the per-session PATH entry alive for the process lifetime.
pub struct Arg0PathEntryGuard {
    _temp_dir: TempDir,
    _lock_file: Option<File>,
    paths: Arg0DispatchPaths,
}

impl Arg0PathEntryGuard {
    fn new(temp_dir: TempDir, lock_file: Option<File>, paths: Arg0DispatchPaths) -> Self {
        Self {
            _temp_dir: temp_dir,
            _lock_file: lock_file,
            paths,
        }
    }

    pub fn paths(&self) -> &Arg0DispatchPaths {
        &self.paths
    }
}

pub fn arg0_dispatch() -> Option<Arg0PathEntryGuard> {
    // Determine if we were invoked via the special alias.
    let mut args = std::env::args_os();
    let argv0 = args.next().unwrap_or_default();
    let exe_name = Path::new(&argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    #[cfg(unix)]
    if exe_name == EXECVE_WRAPPER_ARG0 {
        let mut args = std::env::args();
        let _ = args.next();
        let file = match args.next() {
            Some(file) => file,
            None => std::process::exit(1),
        };
        let argv = args.collect::<Vec<_>>();

        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => std::process::exit(1),
        };
        let exit_code = runtime.block_on(
            codex_shell_escalation::run_shell_escalation_execve_wrapper(file, argv),
        );
        match exit_code {
            Ok(exit_code) => std::process::exit(exit_code),
            Err(_) => std::process::exit(1),
        }
    }

    if exe_name == LINUX_SANDBOX_ARG0 {
        // Safety: [`run_main`] never returns.
        codex_linux_sandbox::run_main();
    } else if exe_name == APPLY_PATCH_ARG0 || exe_name == MISSPELLED_APPLY_PATCH_ARG0 {
        codex_apply_patch::main();
    }

    let argv1 = args.next().unwrap_or_default();
    if argv1 == CODEX_CORE_APPLY_PATCH_ARG1 {
        let patch_arg = args.next().and_then(|s| s.to_str().map(str::to_owned));
        let exit_code = match patch_arg {
            Some(patch_arg) => {
                let mut stdout = std::io::stdout();
                let mut stderr = std::io::stderr();
                match codex_apply_patch::apply_patch(&patch_arg, &mut stdout, &mut stderr) {
                    Ok(()) => 0,
                    Err(_) => 1,
                }
            }
            None => {
                eprintln!("Error: {CODEX_CORE_APPLY_PATCH_ARG1} requires a UTF-8 PATCH argument.");
                1
            }
        };
        std::process::exit(exit_code);
    }

    // This modifies the environment, which is not thread-safe, so do this
    // before creating any threads/the Tokio runtime.
    load_dotenv();

    match prepend_path_entry_for_codex_aliases() {
        Ok(path_entry) => Some(path_entry),
        Err(err) => {
            // It is possible that Codex will proceed successfully even if
            // updating the PATH fails, so warn the user and move on.
            eprintln!("WARNING: proceeding, even though we could not update PATH: {err}");
            None
        }
    }
}

/// While we want to deploy the Codex CLI as a single executable for simplicity,
/// we also want to expose some of its functionality as distinct CLIs, so we use
/// the "arg0 trick" to determine which CLI to dispatch. This effectively allows
/// us to simulate deploying multiple executables as a single binary on Mac and
/// Linux (but not Windows).
///
/// When the current executable is invoked through the hard-link or alias named
/// `codex-linux-sandbox` we *directly* execute
/// [`codex_linux_sandbox::run_main`] (which never returns). Otherwise we:
///
/// 1.  Load `.env` values from `~/.codex/.env` before creating any threads.
/// 2.  Construct a Tokio multi-thread runtime.
/// 3.  Derive the path to the current executable (so children can re-invoke the
///     sandbox) when running on Linux.
/// 4.  Execute the provided async `main_fn` inside that runtime, forwarding any
///     error. Note that `main_fn` receives [`Arg0DispatchPaths`], which
///     contains the helper executable paths needed to construct
///     [`codex_core::config::Config`].
///
/// This function should be used to wrap any `main()` function in binary crates
/// in this workspace that depends on these helper CLIs.
pub fn arg0_dispatch_or_else<F, Fut>(main_fn: F) -> anyhow::Result<()>
where
    F: FnOnce(Arg0DispatchPaths) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    // Retain the TempDir so it exists for the lifetime of the invocation of
    // this executable. Admittedly, we could invoke `keep()` on it, but it
    // would be nice to avoid leaving temporary directories behind, if possible.
    let path_entry = arg0_dispatch();

    // Regular invocation – create a Tokio runtime and execute the provided
    // async entry-point.
    let runtime = build_runtime()?;
    runtime.block_on(async move {
        let current_exe = std::env::current_exe().ok();
        let paths = Arg0DispatchPaths {
            codex_linux_sandbox_exe: if cfg!(target_os = "linux") {
                current_exe.or_else(|| {
                    path_entry
                        .as_ref()
                        .and_then(|path_entry| path_entry.paths().codex_linux_sandbox_exe.clone())
                })
            } else {
                None
            },
            main_execve_wrapper_exe: path_entry
                .as_ref()
                .and_then(|path_entry| path_entry.paths().main_execve_wrapper_exe.clone()),
        };

        main_fn(paths).await
    })
}

fn build_runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    builder.thread_stack_size(TOKIO_WORKER_STACK_SIZE_BYTES);
    Ok(builder.build()?)
}

const ILLEGAL_ENV_VAR_PREFIX: &str = "CODEX_";

/// Load env vars from ~/.codex/.env.
///
/// Security: Do not allow `.env` files to create or modify any variables
/// with names starting with `CODEX_`.
fn load_dotenv() {
    if let Ok(codex_home) = find_codex_home()
        && let Ok(iter) = dotenvy::from_path_iter(codex_home.join(".env"))
    {
        set_filtered(iter);
    }
}

/// Helper to set vars from a dotenvy iterator while filtering out `CODEX_` keys.
fn set_filtered<I>(iter: I)
where
    I: IntoIterator<Item = Result<(String, String), dotenvy::Error>>,
{
    for (key, value) in iter.into_iter().flatten() {
        if !key.to_ascii_uppercase().starts_with(ILLEGAL_ENV_VAR_PREFIX) {
            // It is safe to call set_var() because our process is
            // single-threaded at this point in its execution.
            unsafe { std::env::set_var(&key, &value) };
        }
    }
}

/// Creates a temporary directory with either:
///
/// - UNIX: `apply_patch` symlink to the current executable
/// - WINDOWS: `apply_patch.bat` batch script to invoke the current executable
///   with the "secret" --codex-run-as-apply-patch flag.
///
/// This temporary directory is prepended to the PATH environment variable so
/// that `apply_patch` can be on the PATH without requiring the user to
/// install a separate `apply_patch` executable, simplifying the deployment of
/// Codex CLI.
/// Note: In debug builds the temp-dir guard is disabled to ease local testing.
///
/// IMPORTANT: This function modifies the PATH environment variable, so it MUST
/// be called before multiple threads are spawned.
pub fn prepend_path_entry_for_codex_aliases() -> std::io::Result<Arg0PathEntryGuard> {
    let codex_home = find_codex_home()?;
    #[cfg(not(debug_assertions))]
    {
        // Guard against placing helpers in system temp directories outside debug builds.
        let temp_root = std::env::temp_dir();
        if codex_home.starts_with(&temp_root) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Refusing to create helper binaries under temporary dir {temp_root:?} (codex_home: {codex_home:?})"
                ),
            ));
        }
    }

    std::fs::create_dir_all(&codex_home)?;
    // Use a CODEX_HOME-scoped temp root to avoid cluttering the top-level directory.
    let temp_root = codex_home.join("tmp").join("arg0");
    std::fs::create_dir_all(&temp_root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Ensure only the current user can access the temp directory.
        std::fs::set_permissions(&temp_root, std::fs::Permissions::from_mode(0o700))?;
    }

    // Best-effort cleanup of stale per-session dirs. Ignore failures so startup proceeds.
    if let Err(err) = janitor_cleanup(&temp_root) {
        eprintln!("WARNING: failed to clean up stale arg0 temp dirs: {err}");
    }

    let temp_dir = tempfile::Builder::new()
        .prefix("codex-arg0")
        .tempdir_in(&temp_root)?;
    let path = temp_dir.path();

    let lock_path = path.join(LOCK_FILENAME);
    let lock_file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    let lock_file = match try_lock_file(lock_file)? {
        FileLockStatus::Acquired(lock_file) => Some(lock_file),
        FileLockStatus::WouldBlock => {
            return Err(std::io::Error::new(
                ErrorKind::WouldBlock,
                format!("lock file unexpectedly busy: {lock_path:?}"),
            ));
        }
        FileLockStatus::Unsupported(_lock_file) => {
            write_current_process_metadata(path)?;
            None
        }
    };

    for filename in &[
        APPLY_PATCH_ARG0,
        MISSPELLED_APPLY_PATCH_ARG0,
        #[cfg(target_os = "linux")]
        LINUX_SANDBOX_ARG0,
        #[cfg(unix)]
        EXECVE_WRAPPER_ARG0,
    ] {
        let exe = std::env::current_exe()?;

        #[cfg(unix)]
        {
            let link = path.join(filename);
            symlink(&exe, &link)?;
        }

        #[cfg(windows)]
        {
            let batch_script = path.join(format!("{filename}.bat"));
            std::fs::write(
                &batch_script,
                format!(
                    r#"@echo off
"{}" {CODEX_CORE_APPLY_PATCH_ARG1} %*
"#,
                    exe.display()
                ),
            )?;
        }
    }

    #[cfg(unix)]
    const PATH_SEPARATOR: &str = ":";

    #[cfg(windows)]
    const PATH_SEPARATOR: &str = ";";

    let path_element = path.display();
    let updated_path_env_var = match std::env::var("PATH") {
        Ok(existing_path) => {
            format!("{path_element}{PATH_SEPARATOR}{existing_path}")
        }
        Err(_) => {
            format!("{path_element}")
        }
    };

    unsafe {
        std::env::set_var("PATH", updated_path_env_var);
    }

    let paths = Arg0DispatchPaths {
        codex_linux_sandbox_exe: {
            #[cfg(target_os = "linux")]
            {
                Some(path.join(LINUX_SANDBOX_ARG0))
            }
            #[cfg(not(target_os = "linux"))]
            {
                None
            }
        },
        main_execve_wrapper_exe: {
            #[cfg(unix)]
            {
                Some(path.join(EXECVE_WRAPPER_ARG0))
            }
            #[cfg(not(unix))]
            {
                None
            }
        },
    };

    Ok(Arg0PathEntryGuard::new(temp_dir, lock_file, paths))
}

fn janitor_cleanup(temp_root: &Path) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(temp_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let cleanup_claim = match try_claim_stale_dir(&path)? {
            Some(cleanup_claim) => cleanup_claim,
            None => continue,
        };

        match std::fs::remove_dir_all(&path) {
            Ok(()) => {}
            // Expected TOCTOU race: directory can disappear after read_dir/lock checks.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        }
        drop(cleanup_claim);
    }

    Ok(())
}

fn try_claim_stale_dir(dir: &Path) -> std::io::Result<Option<CleanupClaim>> {
    if let Some(is_live) = dir_has_live_process_metadata(dir)? {
        return if is_live {
            Ok(None)
        } else {
            Ok(Some(CleanupClaim::ProcessMetadata))
        };
    }

    let lock_path = dir.join(LOCK_FILENAME);
    let lock_file = match File::options().read(true).write(true).open(&lock_path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    match try_lock_file(lock_file)? {
        FileLockStatus::Acquired(lock_file) => Ok(Some(CleanupClaim::FileLock {
            _lock_file: lock_file,
        })),
        FileLockStatus::WouldBlock | FileLockStatus::Unsupported(_) => Ok(None),
    }
}

enum FileLockStatus {
    Acquired(File),
    WouldBlock,
    Unsupported(File),
}

enum CleanupClaim {
    FileLock { _lock_file: File },
    ProcessMetadata,
}

fn try_lock_file(lock_file: File) -> std::io::Result<FileLockStatus> {
    match lock_file.try_lock() {
        Ok(()) => Ok(FileLockStatus::Acquired(lock_file)),
        Err(TryLockError::WouldBlock) => Ok(FileLockStatus::WouldBlock),
        Err(TryLockError::Error(err)) if err.kind() == ErrorKind::Unsupported => {
            Ok(FileLockStatus::Unsupported(lock_file))
        }
        Err(TryLockError::Error(err)) => Err(err),
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcessMetadata {
    pid: u32,
    starttime_ticks: u64,
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn write_current_process_metadata(dir: &Path) -> std::io::Result<()> {
    let metadata = current_process_metadata()?;
    let metadata_path = dir.join(PROCESS_METADATA_FILENAME);
    std::fs::write(
        metadata_path,
        format!("{} {}\n", metadata.pid, metadata.starttime_ticks),
    )
}

#[cfg(not(any(target_os = "android", target_os = "linux")))]
fn write_current_process_metadata(_dir: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "process metadata fallback requires /proc support",
    ))
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn dir_has_live_process_metadata(dir: &Path) -> std::io::Result<Option<bool>> {
    let Some(metadata) = read_process_metadata(dir)? else {
        return Ok(None);
    };
    match read_process_starttime_ticks(metadata.pid) {
        Ok(starttime_ticks) => Ok(Some(starttime_ticks == metadata.starttime_ticks)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(Some(false)),
        Err(err) => Err(err),
    }
}

#[cfg(not(any(target_os = "android", target_os = "linux")))]
fn dir_has_live_process_metadata(_dir: &Path) -> std::io::Result<Option<bool>> {
    Ok(None)
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn current_process_metadata() -> std::io::Result<ProcessMetadata> {
    let pid = std::process::id();
    let starttime_ticks = read_process_starttime_ticks(pid)?;
    Ok(ProcessMetadata {
        pid,
        starttime_ticks,
    })
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn read_process_metadata(dir: &Path) -> std::io::Result<Option<ProcessMetadata>> {
    let metadata_path = dir.join(PROCESS_METADATA_FILENAME);
    let metadata = match std::fs::read_to_string(&metadata_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let mut fields = metadata.split_ascii_whitespace();
    let Some(pid) = fields.next() else {
        return Ok(None);
    };
    let Some(starttime_ticks) = fields.next() else {
        return Ok(None);
    };
    let Ok(pid) = pid.parse::<u32>() else {
        return Ok(None);
    };
    let Ok(starttime_ticks) = starttime_ticks.parse::<u64>() else {
        return Ok(None);
    };
    Ok(Some(ProcessMetadata {
        pid,
        starttime_ticks,
    }))
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn read_process_starttime_ticks(pid: u32) -> std::io::Result<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    parse_process_starttime_ticks(&stat)
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn parse_process_starttime_ticks(stat: &str) -> std::io::Result<u64> {
    let Some(comm_end) = stat.rfind(')') else {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "missing closing ')' in /proc stat entry",
        ));
    };
    let fields = stat[comm_end + 1..]
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    let Some(starttime_ticks) = fields.get(19) else {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "missing starttime field in /proc stat entry",
        ));
    };
    starttime_ticks.parse::<u64>().map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("invalid starttime field in /proc stat entry: {err}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::FileLockStatus;
    use super::LOCK_FILENAME;
    use super::PROCESS_METADATA_FILENAME;
    #[cfg(any(target_os = "android", target_os = "linux"))]
    use super::current_process_metadata;
    use super::janitor_cleanup;
    #[cfg(any(target_os = "android", target_os = "linux"))]
    use super::parse_process_starttime_ticks;
    use super::try_lock_file;
    #[cfg(any(target_os = "android", target_os = "linux"))]
    use super::write_current_process_metadata;
    use std::fs;
    use std::fs::File;
    use std::path::Path;

    fn create_lock(dir: &Path) -> std::io::Result<File> {
        let lock_path = dir.join(LOCK_FILENAME);
        File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
    }

    fn locking_is_supported() -> std::io::Result<bool> {
        let root = tempfile::tempdir()?;
        let lock_file = create_lock(root.path())?;
        match try_lock_file(lock_file)? {
            FileLockStatus::Acquired(_lock_file) => Ok(true),
            FileLockStatus::WouldBlock => Ok(true),
            FileLockStatus::Unsupported(_lock_file) => Ok(false),
        }
    }

    #[test]
    fn janitor_skips_dirs_without_lock_file() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("no-lock");
        fs::create_dir(&dir)?;

        janitor_cleanup(root.path())?;

        assert!(dir.exists());
        Ok(())
    }

    #[test]
    fn janitor_skips_dirs_with_held_lock() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("locked");
        fs::create_dir(&dir)?;
        if locking_is_supported()? {
            let lock_file = create_lock(&dir)?;
            lock_file.try_lock()?;
        } else {
            write_current_process_metadata(&dir)?;
        }

        janitor_cleanup(root.path())?;

        assert!(dir.exists());
        Ok(())
    }

    #[test]
    fn janitor_removes_dirs_with_unlocked_lock() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("stale");
        fs::create_dir(&dir)?;
        if locking_is_supported()? {
            create_lock(&dir)?;
        } else {
            let metadata = current_process_metadata()?;
            fs::write(
                dir.join(PROCESS_METADATA_FILENAME),
                format!("{} {}\n", metadata.pid, metadata.starttime_ticks + 1),
            )?;
        }

        janitor_cleanup(root.path())?;

        assert!(!dir.exists());
        Ok(())
    }

    #[cfg(any(target_os = "android", target_os = "linux"))]
    #[test]
    fn parse_process_starttime_ticks_handles_proc_stat_format() -> std::io::Result<()> {
        let stat = "1234 (codex worker) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 424242 21";
        let starttime_ticks = parse_process_starttime_ticks(stat)?;

        assert_eq!(starttime_ticks, 424242);
        Ok(())
    }

    #[cfg(any(target_os = "android", target_os = "linux"))]
    #[test]
    fn janitor_skips_dirs_with_malformed_process_metadata() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("malformed");
        fs::create_dir(&dir)?;
        fs::write(dir.join(PROCESS_METADATA_FILENAME), "not-valid\n")?;

        janitor_cleanup(root.path())?;

        assert!(dir.exists());
        Ok(())
    }
}
