#!/usr/bin/env bash
# Standalone fork installer. Keep the Python transaction inline so updater
# handoffs only need this one file (including in Git for Windows Bash).
set -euo pipefail

python_bin="$(command -v python3 || command -v python || true)"
[[ -n "$python_bin" ]] || { echo 'error: python3 or python is required' >&2; exit 1; }

kernel="$(uname -s)"
machine="$(uname -m)"
case "${MSYSTEM_CARCH:-}:$kernel:$machine" in
  aarch64:MINGW*:*|arm64:MINGW*:*|aarch64:MSYS*:*|arm64:MSYS*:*|*:MINGW*-ARM64:*|*:MSYS*-ARM64:*) target=aarch64-pc-windows-msvc ;;
  *:MINGW*:x86_64|*:MSYS*:x86_64|*:CYGWIN*:x86_64) target=x86_64-pc-windows-msvc ;;
  *:Darwin:arm64) target=aarch64-apple-darwin ;;
  *:Darwin:x86_64) target=x86_64-apple-darwin ;;
  *:Linux:aarch64)
    if [[ -n "${PREFIX:-}" ]]; then target=aarch64-linux-android; else target=aarch64-unknown-linux-musl; fi ;;
  *:Linux:x86_64) target=x86_64-unknown-linux-musl ;;
  *) target="" ;;
esac
case "${PROCESSOR_ARCHITEW6432:-${PROCESSOR_ARCHITECTURE:-}}:$kernel" in
  ARM64:MINGW*|ARM64:MSYS*|ARM64:CYGWIN*) target=aarch64-pc-windows-msvc ;;
esac
artifact="${CODEX_ARTIFACT_NAME:-codex-$target}"
target="${artifact#codex-}"
[[ -n "$target" ]] || { echo 'error: unsupported platform; set CODEX_ARTIFACT_NAME' >&2; exit 1; }
install_user_home="$HOME"
case "$kernel" in
  MINGW*|MSYS*|CYGWIN*) install_user_home="$(cygpath -w "$HOME")" ;;
esac
export CODEX_INSTALL_USER_HOME="$install_user_home"
# Python receives metadata through files, never a potentially multi-MB env var.
exec "$python_bin" - "$target" "$@" <<'PY'
import argparse
import contextlib
import datetime as dt
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shlex
import shutil
import signal
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import time
import uuid
import zipfile

if os.name == "nt":
    # Git for Windows Bash consumes UTF-8, including non-ASCII home paths.
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

REPO = "mevanlc/codex"
GH = shutil.which("gh") or "gh"
TARGET = sys.argv[1]
WINDOWS = TARGET.endswith("-pc-windows-msvc")
SUFFIX = ".exe" if WINDOWS else ""
ASSET = os.environ.get("CODEX_RELEASE_ASSET_NAME") or f"codex-{TARGET}" + (".zip" if WINDOWS else ".tar.zst")
ARTIFACT = f"codex-{TARGET}"
USER_HOME = Path(os.environ["CODEX_INSTALL_USER_HOME"])
INSTALL_ROOT = USER_HOME / ".local/share/mevanlc-codex"
DAEMON_ONLY = os.environ.get("CODEX_INSTALL_DAEMON_ONLY") == "1"
ROOT = INSTALL_ROOT / "packages" / ("app-server-daemon" if DAEMON_ONLY else "cli")
BIN = USER_HOME / ".local/bin"
SEMVER = re.compile(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?\Z")


def step(message):
    print(f"==> {message}", flush=True)


def run(args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def epoch(value):
    return int(dt.datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp())


def timestamp(value):
    return f"{dt.datetime.fromtimestamp(value, dt.timezone.utc):%Y-%m-%d %H:%M:%S UTC} (epoch: {value})"


def gh_json(endpoint, directory, paginate=False):
    path = directory / "metadata.json"
    with path.open("w") as stream:
        run([GH, "api", f"repos/{REPO}/{endpoint}"] + (["--paginate", "--slurp"] if paginate else []), stdout=stream)
    with path.open() as stream:
        return json.load(stream)


def release_rows(pages):
    # GitHub releases are not reliably ordered by publication time. A newer
    # desktop-only release must not hide the most recent Android package.
    releases = [item for page in pages for item in page]
    return sorted((r for r in releases if not r.get("draft") and any(a.get("name") == ASSET for a in r.get("assets", []))),
                  key=lambda r: r.get("published_at") or r.get("created_at") or "", reverse=True)


def read_text(path):
    try:
        return path.read_text().strip()
    except FileNotFoundError:
        return ""


def selected():
    current = ROOT / "current"
    if not os.path.lexists(current):
        return None
    release = current.resolve(strict=False)
    if release.parent != (ROOT / "releases").resolve():
        raise RuntimeError(f"refusing unmanaged current selection: {current}")
    if os.name == "nt":
        if current.lstat().st_reparse_tag != 0xA0000003:
            raise RuntimeError("current must be an installer-owned directory junction")
    elif not current.is_symlink():
        raise RuntimeError("current must be an installer-owned symlink")
    return release


@contextlib.contextmanager
def install_lock():
    ROOT.mkdir(parents=True, exist_ok=True)
    lock = ROOT / "install.lock"
    deadline = time.monotonic() + 60
    if os.name == "nt":
        # The Rust lifecycle code opens this same file with share_mode(0).
        import ctypes
        from ctypes import wintypes as w
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.CreateFileW.argtypes = [w.LPCWSTR, w.DWORD, w.DWORD, w.LPVOID, w.DWORD, w.DWORD, w.HANDLE]
        kernel.CreateFileW.restype = w.HANDLE
        kernel.CloseHandle.argtypes = [w.HANDLE]
        while True:
            handle = kernel.CreateFileW(str(lock), 0x40000000, 0, None, 4, 0x80, None)
            if handle != w.HANDLE(-1).value:
                break
            if ctypes.get_last_error() not in (5, 32) or time.monotonic() >= deadline:
                raise ctypes.WinError(ctypes.get_last_error())
            time.sleep(0.05)
        try:
            yield
        finally:
            kernel.CloseHandle(handle)
    else:
        import fcntl
        with lock.open("a") as stream:
            while True:
                try:
                    fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except BlockingIOError:
                    if time.monotonic() >= deadline:
                        raise RuntimeError("timed out waiting for installer lock")
                    time.sleep(0.05)
            yield
    # Keep the lock inode: unlinking it would split concurrent installers.


def set_current(release):
    current = ROOT / "current"
    if os.name != "nt":
        temporary = ROOT / (".current-" + uuid.uuid4().hex)
        try:
            temporary.symlink_to(release)
            os.replace(temporary, current)
        finally:
            temporary.unlink(missing_ok=True)
        return
    # Retarget the NTFS junction in place, without administrator symlink rights
    # or a missing-current interval. Same FSCTL used by the Windows installer.
    import ctypes
    from ctypes import wintypes as w
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateFileW.argtypes = [w.LPCWSTR, w.DWORD, w.DWORD, w.LPVOID, w.DWORD, w.DWORD, w.HANDLE]
    kernel.CreateFileW.restype = w.HANDLE
    kernel.DeviceIoControl.argtypes = [w.HANDLE, w.DWORD, w.LPVOID, w.DWORD, w.LPVOID, w.DWORD, ctypes.POINTER(w.DWORD), w.LPVOID]
    kernel.CloseHandle.argtypes = [w.HANDLE]
    if not current.exists():
        current.mkdir()
    substitute = ("\\??\\" + str(release.resolve())).encode("utf-16-le")
    buffer = struct.pack("<IHHHHHH", 0xA0000003, len(substitute) + 12, 0, 0, len(substitute), len(substitute) + 2, 0) + substitute + b"\0\0\0\0"
    handle = kernel.CreateFileW(str(current), 0x40000000, 7, None, 3, 0x02200000, None)
    if handle == w.HANDLE(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        returned = w.DWORD()
        if not kernel.DeviceIoControl(handle, 0x900A4, buffer, len(buffer), None, 0, ctypes.byref(returned), None):
            raise ctypes.WinError(ctypes.get_last_error())
    finally:
        kernel.CloseHandle(handle)


def remove_current():
    current = ROOT / "current"
    if os.path.lexists(current):
        if os.name == "nt":
            current.rmdir()  # Remove the junction, never its target.
        else:
            current.unlink()


def extract(archive, destination):
    def safe(name):
        p = PurePosixPath(name)
        if "\\" in name or ":" in name or p.is_absolute() or ".." in p.parts:
            raise RuntimeError(f"unsafe package member: {name}")
        return destination.joinpath(*p.parts)

    if archive.name.endswith(".zip"):
        with zipfile.ZipFile(archive) as package:
            for member in package.infolist():
                path = safe(member.filename)
                mode = member.external_attr >> 16
                if stat.S_ISLNK(mode):
                    raise RuntimeError("package links are not supported")
                if member.is_dir():
                    path.mkdir(parents=True, exist_ok=True)
                else:
                    path.parent.mkdir(parents=True, exist_ok=True)
                    with package.open(member) as source, path.open("xb") as out:
                        shutil.copyfileobj(source, out)
                    path.chmod((mode & 0o777) or 0o644)
    elif archive.name.endswith(".tar.zst"):
        with subprocess.Popen(["zstd", "-d", "-c", str(archive)], stdout=subprocess.PIPE) as decompress:
            with tarfile.open(fileobj=decompress.stdout, mode="r|") as package:
                for member in package:
                    path = safe(member.name)
                    if member.isdir():
                        path.mkdir(parents=True, exist_ok=True)
                    elif member.isfile():
                        path.parent.mkdir(parents=True, exist_ok=True)
                        with package.extractfile(member) as source, path.open("xb") as out:
                            shutil.copyfileobj(source, out)
                        path.chmod(member.mode & 0o777)
                    else:
                        raise RuntimeError(f"unsupported package member: {member.name}")
            if decompress.wait():
                raise RuntimeError("package decompression failed")
    else:
        raise RuntimeError("expected a complete .tar.zst or .zip package; loose binaries are no longer supported")


def validate(package, expected_version=None, probe=True):
    with (package / "codex-package.json").open() as stream:
        manifest = json.load(stream)
    expected = {"layoutVersion": 1, "variant": "codex", "target": TARGET, "entrypoint": f"bin/codex{SUFFIX}",
                "pathDir": "codex-path", "resourcesDir": "codex-resources"}
    for key, value in expected.items():
        if manifest.get(key) != value:
            raise RuntimeError(f"invalid package {key}: expected {value!r}, got {manifest.get(key)!r}")
    version = manifest.get("version", "")
    if not SEMVER.fullmatch(version) or (expected_version and expected_version != version):
        raise RuntimeError(f"invalid or mismatched package version: {version!r}")
    for directory in ("bin", "codex-path", "codex-resources"):
        if not (package / directory).is_dir():
            raise RuntimeError(f"package is missing directory {directory}")
    names = [f"bin/codex{SUFFIX}", f"bin/codex-code-mode-host{SUFFIX}", f"codex-path/rg{SUFFIX}"]
    if WINDOWS:
        names += ["codex-resources/codex-command-runner.exe", "codex-resources/codex-windows-sandbox-setup.exe"]
    elif "-unknown-linux-" in TARGET:
        names += ["codex-resources/bwrap"]
    for name in names:
        path = package / name
        if not path.is_file() or path.is_symlink() or not path.stat().st_size:
            raise RuntimeError(f"package is missing required executable {name}")
        if not WINDOWS and not path.stat().st_mode & 0o111:
            raise RuntimeError(f"package file {name} is not executable")
    if probe:
        try:
            output = run([str(package / names[0]), "--version"], capture_output=True, text=True, timeout=30).stdout.strip()
        except subprocess.CalledProcessError as error:
            raise RuntimeError(f"packaged Codex failed --version: {error.stderr.strip() or error.returncode}") from error
        if output.split()[-1:] != [version]:
            raise RuntimeError(f"package version {version} does not match executable: {output}")
        run([str(package / names[2]), "--version"], stdout=subprocess.DEVNULL, timeout=30)
    return version


def tree_digest(package):
    digest = hashlib.sha256()
    for path in sorted(package.rglob("*")):
        if path.is_file() and path != package / "install-receipt.json":
            digest.update(path.relative_to(package).as_posix().encode() + b"\0")
            digest.update(str(path.stat().st_mode & 0o777).encode() + b"\0")
            with path.open("rb") as stream:
                for block in iter(lambda: stream.read(1024 * 1024), b""):
                    digest.update(block)
    return digest.hexdigest()


def atomic_text(path, value):
    temporary = path.with_name("." + path.name + "-" + uuid.uuid4().hex)
    try:
        with temporary.open("w", encoding="utf-8", newline="\n") as stream:
            stream.write(value)
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def launchers(command):
    executable = ROOT / "current/bin" / (command + SUFFIX)
    if WINDOWS:
        cmd_path = "%~dp0..\\share\\mevanlc-codex\\packages\\cli\\current\\bin\\" + command + ".exe"
        return {
            BIN / command: "#!/usr/bin/env bash\nexec " + shlex.quote(executable.as_posix()) + ' "$@"\n',
            BIN / (command + ".cmd"): '@echo off\nsetlocal DisableDelayedExpansion\n"' + cmd_path + '" %*\n',
        }
    return {BIN / command: str(executable)}


def launcher_matches(path, value):
    if WINDOWS:
        return path.is_file() and not path.is_symlink() and read_text(path) == value.strip()
    return path.is_symlink() and os.readlink(path) == value


def install(archive, source, args):
    with install_lock():
        if not DAEMON_ONLY:
            visible = BIN.resolve()
            components = [part.casefold() for part in visible.parts]
            legacy_package = ("packages", "standalone") in zip(components, components[1:])
            if legacy_package or any((directory / "codex-package.json").is_file() for directory in (visible, *visible.parents)):
                raise RuntimeError("~/.local/bin points inside a Codex package; use a command directory outside that package so its files remain untouched")
        previous = selected()
        previous_name = previous.name if previous else ""
        marker = ROOT / "auto-update-version"
        old_marker = read_text(marker)
        for guard in ("CODEX_INSTALL_IF_CURRENT", "CODEX_INSTALL_IF_LATEST"):
            enabled = os.environ.get(guard) == "1"
            expected = os.environ.get("CODEX_UPDATE_FROM_RELEASE", "")
            if enabled and (not expected or expected != previous_name or (guard.endswith("LATEST") and old_marker != previous_name)):
                step(f"Selection changed or pinned; skipped guarded install ({guard})")
                return
        if os.environ.get("CODEX_UPDATE_FROM_RELEASE") and os.environ["CODEX_UPDATE_FROM_RELEASE"] != previous_name:
            step("Selection changed; skipped guarded install")
            return
        if previous and not args.force and not args.version:
            try:
                validate(previous)
                receipt = json.loads((previous / "install-receipt.json").read_text())
                commands_valid = DAEMON_ONLY or all(
                    launcher_matches(path, value) for command in ("codex", "codex-code-mode-host")
                    for path, value in launchers(command).items())
                if WINDOWS and not DAEMON_ONLY:
                    commands_valid = commands_valid and not any((BIN / (command + ext)).exists()
                        for command in ("codex", "codex-code-mode-host") for ext in (".exe", ".ps1", ".bat"))
                same_source = receipt.get("tag", receipt.get("artifactId")) == source.get("tag", source.get("artifactId"))
                newer = receipt["publishedEpoch"] > source["publishedEpoch"]
                current = same_source and receipt["sourceEpoch"] >= source["sourceEpoch"]
                if receipt["repository"] == REPO and (newer or current) and commands_valid and tree_digest(previous) == receipt["packageDigest"]:
                    if args.mode == "release" and not args.version:
                        atomic_text(marker, previous_name)
                    step("Installed Codex package is already as new or newer than requested source")
                    step("Use --force to install anyway")
                    return
            except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError):
                step("Existing package is incomplete or changed; repairing")
        releases = ROOT / "releases"
        releases.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix=".staging-", dir=releases) as staging:
            stage = Path(staging) / "package"
            stage.mkdir()
            extract(archive, stage)
            expected_version = source.get("tag", "").removeprefix("fork-v").removeprefix("v")
            expected_version = expected_version if SEMVER.fullmatch(expected_version) else None
            version = validate(stage, expected_version)
            digest = tree_digest(stage)
            name = f"{version}-{TARGET}-{digest[:16]}"
            release = releases / name
            receipt = {**source, "repository": REPO, "version": version, "target": TARGET, "packageDigest": digest}
            (stage / "install-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
            if release.exists():
                if tree_digest(release) != digest:
                    # Retain a damaged immutable release for diagnosis; publish a repair.
                    release = releases / (name + "-repair-" + uuid.uuid4().hex[:8])
                    os.rename(stage, release)
            else:
                os.rename(stage, release)
        name = release.name
        # Keep old commands for recovery, including loose .exe files which would
        # otherwise shadow the .cmd launcher in PowerShell/cmd.
        backup = INSTALL_ROOT / "backups" / (dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%S") + "-" + uuid.uuid4().hex[:8])
        changed = []
        published = False
        old_marker_exists = marker.exists()
        try:
            set_current(release)
            published = True
            if not DAEMON_ONLY:
                BIN.mkdir(parents=True, exist_ok=True)
                for command in ("codex", "codex-code-mode-host"):
                    desired = launchers(command)
                    unchanged = {path for path, value in desired.items() if launcher_matches(path, value)}
                    paths = [BIN / command]
                    if WINDOWS:
                        paths += [BIN / (command + extension) for extension in (".cmd", ".exe", ".ps1", ".bat")]
                    for path in paths:
                        if path in unchanged:
                            continue
                        saved = None
                        if os.path.lexists(path):
                            if path.is_dir():
                                raise RuntimeError(f"refusing to replace command directory {path}")
                            backup.mkdir(parents=True, exist_ok=True)
                            saved = backup / path.name
                            os.replace(path, saved)
                        changed.append((path, saved))
                    for path, value in desired.items():
                        if path in unchanged:
                            continue
                        if WINDOWS:
                            atomic_text(path, value)
                            if path.suffix != ".cmd":
                                path.chmod(0o755)
                        else:
                            path.symlink_to(value)
            # A pin/artifact never silently joins the latest channel.
            if args.mode == "release" and not args.version:
                atomic_text(marker, name)
            else:
                marker.unlink(missing_ok=True)
            validate(ROOT / "current", version)
        except BaseException:
            for path, saved in reversed(changed):
                path.unlink(missing_ok=True)
                if saved:
                    os.replace(saved, path)
            if published:
                if previous:
                    set_current(previous)
                else:
                    remove_current()
            if old_marker_exists:
                atomic_text(marker, old_marker)
            else:
                marker.unlink(missing_ok=True)
            raise
        step(f"Selected package: {release}")
        if backup.exists():
            step(f"Previous commands saved in {backup}")
        step(f"Installed version: {version}")
        if DAEMON_ONLY:
            step("Daemon package selected; the daemon updater owns any required restart")


def main():
    parser = argparse.ArgumentParser(description="Install complete mevanlc/codex packages into ~/.local/share/mevanlc-codex; commands go in ~/.local/bin. Requires gh, Python 3.9+, and zstd for tar archives. On Windows run in Git for Windows Bash.")
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--artifact", dest="mode", action="store_const", const="artifact", help="latest Actions artifact for this platform")
    modes.add_argument("--release", dest="mode", action="store_const", const="release", help="newest published non-prerelease carrying this platform's package (default)")
    parser.set_defaults(mode="release")
    parser.add_argument("--force", action="store_true", help="bypass currency checks")
    pins = parser.add_mutually_exclusive_group()
    pins.add_argument("--version", help="exact tag or bare fork version; deliberate pin, permits downgrades")
    pins.add_argument("--list-available", action="store_true")
    args = parser.parse_args(sys.argv[2:])
    if args.version is not None and not args.version:
        parser.error("--version requires a nonempty tag or version")
    if args.mode == "artifact" and (args.version or args.list_available):
        parser.error("--version and --list-available cannot be combined with --artifact")
    if not shutil.which("gh"):
        raise RuntimeError("gh is required (authenticate with gh auth login)")
    temp_root = os.environ.get("TMPDIR") or (str(Path(os.environ["PREFIX"]) / "tmp") if os.environ.get("PREFIX") else None)
    if temp_root:
        Path(temp_root).mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="codex-gh-", dir=temp_root) as tmp:
        directory = Path(tmp)
        if args.mode == "release":
            if args.version:
                release = None
                for tag in dict.fromkeys([args.version, "fork-v" + args.version, "v" + args.version]):
                    try:
                        release = gh_json("releases/tags/" + tag, directory)
                        break
                    except subprocess.CalledProcessError:
                        continue
                if release is None:
                    raise RuntimeError(f"no release matching {args.version!r} in {REPO}")
            else:
                rows = release_rows(gh_json("releases?per_page=100", directory, paginate=True))
                default = next((r for r in rows if not r.get("prerelease")), None)
                if args.list_available:
                    print("TAG  PUBLISHED  LAYOUT")
                    for r in rows:
                        print(f'{r["tag_name"]}  {r.get("published_at", "")}  package' + (" (default)" if r is default else ""))
                    return
                if default is None:
                    raise RuntimeError(f"no published non-prerelease carries {ASSET}")
                release = default
            asset = next((a for a in release.get("assets", []) if a.get("name") == ASSET), None)
            if asset is None or release.get("draft"):
                raise RuntimeError(f"requested release in {REPO} does not include asset {ASSET}")
            source = {"tag": release["tag_name"], "sourceEpoch": epoch(asset["updated_at"]),
                      "publishedEpoch": epoch(release.get("published_at") or release["created_at"])}
            download = [GH, "release", "download", release["tag_name"], "-R", REPO, "-p", ASSET]
            description = "release assets from tag " + release["tag_name"]
        else:
            pages = gh_json("actions/artifacts?per_page=100", directory, paginate=True)
            artifacts = [a for page in pages for a in page.get("artifacts", []) if a.get("name") == ARTIFACT and not a.get("expired")]
            if not artifacts:
                raise RuntimeError(f"no non-expired artifact named {ARTIFACT} found in {REPO}")
            artifact = max(artifacts, key=lambda a: a["created_at"])
            source = {"artifactId": artifact["id"], "sourceEpoch": epoch(artifact["created_at"]), "publishedEpoch": epoch(artifact["created_at"])}
            download = [GH, "run", "download", str(artifact["workflow_run"]["id"]), "-R", REPO, "-n", ARTIFACT]
            description = f'artifact {artifact["id"]} from run {artifact["workflow_run"]["id"]}'
        step(f"Repository: {REPO}")
        step(f"Mode: {args.mode}")
        if args.version:
            step(f"Requested version: {args.version}")
        step(f"Artifact name: {ARTIFACT}")
        step(f"Release asset: {ASSET}")
        step(f"Package root: {ROOT}")
        for command, label in (("codex", "Codex"), ("codex-code-mode-host", "code-mode host")):
            binary = ROOT / "current/bin" / (command + SUFFIX) if DAEMON_ONLY else BIN / (command + SUFFIX)
            if WINDOWS and not DAEMON_ONLY and not binary.exists():
                binary = ROOT / "current/bin" / (command + SUFFIX)
            if binary.exists():
                if command == "codex":
                    try:
                        output = run([str(binary), "--version"], capture_output=True, text=True, timeout=15).stdout.strip()
                    except (OSError, subprocess.SubprocessError):
                        output = "unavailable (could not run --version)"
                    step(f"Existing version: {output}")
                step(f"Existing {label} mtime: {timestamp(int(binary.stat().st_mtime))}")
            else:
                step("Existing version: not installed" if command == "codex" else "Existing code-mode host: not installed")
        step(f"Requested source: {description}")
        step(f'Source timestamp: {timestamp(source["sourceEpoch"])}')
        if args.version:
            step("Explicit version requested; skipping package currency check")
        elif args.force:
            step("Forced install requested; skipping package currency check")
        download_dir = directory / "download"
        download_dir.mkdir()
        step(f"Downloading {description}")
        run(download + ["-D", str(download_dir)])
        archive = download_dir / ASSET
        if not archive.is_file() or len(list(download_dir.iterdir())) != 1:
            raise RuntimeError(f"download must contain exactly one complete package archive: {ASSET}")
        install(archive, source, args)


try:
    # SIGTERM unwinds the transaction and releases the OS lock.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    main()
except subprocess.CalledProcessError as error:
    sys.exit(error.returncode)
except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, tarfile.TarError, zipfile.BadZipFile) as error:
    print(f"error: {error}", file=sys.stderr)
    sys.exit(1)
PY
