#!/usr/bin/env python3

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


INSTALL_SCRIPT = Path(__file__).with_name("install-from-gh.sh")
ASSET = "codex-aarch64-linux-android.tar.zst"
HOST_ASSET = "codex-code-mode-host-aarch64-linux-android.zst"
DOWNLOAD_STOP = 75


def release(tag, published, *, main=True, host=False, **flags):
    names = ([ASSET] if main else []) + ([HOST_ASSET] if host else [])
    return {
        "tag_name": tag,
        "published_at": published,
        "created_at": published,
        "assets": [{"name": name, "updated_at": published} for name in names],
        **flags,
    }


class InstallerFixture(unittest.TestCase):
    def setUp(self):
        tmp_root = os.environ.get("TMPDIR")
        if not tmp_root and os.environ.get("PREFIX"):
            tmp_root = str(Path(os.environ["PREFIX"]) / "tmp")
        self.temp_dir = tempfile.TemporaryDirectory(dir=tmp_root)
        self.addCleanup(self.temp_dir.cleanup)
        self.root = Path(self.temp_dir.name)
        stub_bin = self.root / "bin"
        stub_bin.mkdir()
        gh = stub_bin / "gh"
        gh.write_text(
            f"#!{sys.executable}\n" + """
import json
import os
from pathlib import Path
import sys

root = Path(os.environ["GH_TEST_ROOT"])
fixture = json.loads((root / "fixture.json").read_text())
args = sys.argv[1:]
if args[0] == "api":
    if args[1] == "repos/mevanlc/codex/releases?per_page=100":
        if fixture.get("api_error"):
            print("GitHub metadata request failed", file=sys.stderr)
            sys.exit(fixture["api_error"])
        assert "--paginate" in args and "--slurp" in args, args
        print(json.dumps(fixture["pages"]))
    elif args[1].startswith("repos/mevanlc/codex/releases/tags/"):
        tag = args[1].split("/tags/", 1)[1]
        pinned = fixture.get("pinned", {}).get(tag)
        if pinned is None:
            sys.exit(1)
        print(json.dumps(pinned))
    elif args[1] == "repos/mevanlc/codex/actions/artifacts?per_page=100":
        print(json.dumps(fixture["pages"]))
    else:
        raise AssertionError(args)
elif args[:2] in (["release", "download"], ["run", "download"]):
    (root / "download.json").write_text(json.dumps(args))
    # Selection tests stop before installation; transaction tests supply a package.
    if not fixture.get("archive"):
        sys.exit(75)
    import shutil
    destination = Path(args[args.index("-D") + 1])
    shutil.copyfile(fixture["archive"], destination / fixture.get("asset", "codex-aarch64-linux-android.tar.zst"))
else:
    raise AssertionError(args)
""",
            encoding="utf-8",
        )
        gh.chmod(0o700)
        if os.name == "nt":
            stub = stub_bin / "gh-stub.py"
            gh.rename(stub)
            (stub_bin / "gh.cmd").write_text(f'@"{sys.executable}" "{stub}" %*\n')
        self.env = os.environ.copy()
        self.env.update({
            "PATH": str(stub_bin) + os.pathsep + self.env["PATH"],
            "HOME": str(self.root / "home"),
            "TMPDIR": str(self.root),
            "GH_TEST_ROOT": str(self.root),
            "CODEX_ARTIFACT_NAME": "codex-aarch64-linux-android",
        })
        self.env.pop("CODEX_RELEASE_ASSET_NAME", None)
        self.env.pop("CODEX_CODE_MODE_HOST_RELEASE_ASSET_NAME", None)

    def run_installer(self, pages, *args, **fixture):
        (self.root / "fixture.json").write_text(
            json.dumps({"pages": pages, **fixture}), encoding="utf-8"
        )
        result = subprocess.run(
            [os.environ.get("CODEX_TEST_BASH") or shutil.which("bash"), INSTALL_SCRIPT.resolve().as_posix(), *args],
            env=self.env,
            capture_output=True,
            text=True,
            timeout=getattr(self, "installer_timeout", 30),
        )
        self.assertEqual(list(self.root.glob("codex-gh-*")), [])
        return result

    def assert_download(self, result, tag):
        self.assertEqual(result.returncode, DOWNLOAD_STOP, result.stderr)
        args = json.loads((self.root / "download.json").read_text())
        expected = ["release", "download", tag, "-R", "mevanlc/codex", "-p", ASSET]
        self.assertEqual(args[:-2], expected)
        self.assertEqual(args[-2], "-D")


@unittest.skipUnless(os.name == "posix" and shutil.which("bash"), "requires Bash")
class InstallFromGhTest(InstallerFixture):
    def test_partial_release_falls_back_across_pages_by_publication_time(self):
        pages = [
            [
                release("partial", "2026-10-04T00:00:00Z", main=False),
                release("older", "2026-10-01T00:00:00Z"),
                release("draft", "2026-10-06T00:00:00Z", draft=True),
                release("preview", "2026-10-05T00:00:00Z", prerelease=True),
            ],
            [release("newest-android", "2026-10-02T00:00:00Z")],
        ]
        self.assert_download(self.run_installer(pages), "newest-android")

    def test_package_is_the_only_downloaded_asset(self):
        pages = [[
            release("older-dual", "2026-10-01T00:00:00Z", host=True),
            release("newest-single", "2026-10-02T00:00:00Z", host=False),
        ]]
        self.assert_download(self.run_installer(pages), "newest-single")

    def test_list_marks_the_same_default_and_excludes_partial_and_draft_releases(self):
        pages = [[
            release("partial", "2026-10-04T00:00:00Z", main=False),
            release("preview", "2026-10-05T00:00:00Z", prerelease=True),
            release("draft", "2026-10-06T00:00:00Z", draft=True),
        ], [
            release("older", "2026-10-01T00:00:00Z"),
            release("newest-android", "2026-10-02T00:00:00Z"),
        ]]
        result = self.run_installer(pages, "--list-available")
        self.assertEqual(result.returncode, 0, result.stderr)
        lines = result.stdout.splitlines()[1:]
        self.assertEqual([line.split()[0] for line in lines], [
            "preview", "newest-android", "older",
        ])
        self.assertEqual([line.split()[0] for line in lines if "(default)" in line], [
            "newest-android",
        ])
        self.assertFalse((self.root / "download.json").exists())

    def test_missing_platform_asset_fails_without_downloading(self):
        result = self.run_installer([[release("partial", "2026-10-04T00:00:00Z", main=False)]])
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(f"no published non-prerelease carries {ASSET}", result.stderr)
        self.assertFalse((self.root / "download.json").exists())

    def test_default_does_not_automatically_install_prereleases(self):
        result = self.run_installer([[release("preview", "2026-10-05T00:00:00Z", prerelease=True)]])
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(f"no published non-prerelease carries {ASSET}", result.stderr)
        self.assertFalse((self.root / "download.json").exists())

    def test_explicit_version_can_select_a_prerelease(self):
        tag = "fork-v0.161.0-test"
        result = self.run_installer(
            [], "--version", "0.161.0-test",
            pinned={tag: release(tag, "2026-10-05T00:00:00Z", prerelease=True)},
        )
        self.assert_download(result, tag)

    def test_explicit_version_missing_asset_does_not_fall_back(self):
        result = self.run_installer(
            [[release("available", "2026-10-02T00:00:00Z")]],
            "--version", "partial",
            pinned={"partial": release("partial", "2026-10-04T00:00:00Z", main=False)},
        )
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(f"requested release in mevanlc/codex does not include asset {ASSET}", result.stderr)
        self.assertFalse((self.root / "download.json").exists())

    def test_metadata_fetch_failure_is_not_reported_as_missing_assets(self):
        result = self.run_installer([], api_error=17)
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertIn("GitHub metadata request failed", result.stderr)
        self.assertNotIn("no releases carry", result.stderr)
        self.assertFalse((self.root / "download.json").exists())



@unittest.skipUnless(os.name == "posix" and shutil.which("bash") and shutil.which("zstd"), "requires Unix and zstd")
class PackageTransactionTest(InstallerFixture):
    """Exercise the real standalone installer with only GitHub mocked."""
    def package(self, version="1.2.3-aaaaaaa", target="aarch64-linux-android", *, missing=None, mode=0o755, manifest_changes=None):
        import tarfile
        import zipfile
        directory = Path(tempfile.mkdtemp(dir=self.root))
        suffix = ".exe" if target.endswith("-msvc") else ""
        names = [f"bin/codex{suffix}", f"bin/codex-code-mode-host{suffix}", f"codex-path/rg{suffix}"]
        if "-unknown-linux-" in target:
            names += ["codex-resources/bwrap"]
        if suffix:
            names += ["codex-resources/codex-command-runner.exe", "codex-resources/codex-windows-sandbox-setup.exe"]
        for name in names + ["codex-resources/data/file"]:
            if name == missing:
                continue
            file = directory / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text(f"#!{shutil.which('sh')}\necho 'codex {version}'\n")
            file.chmod(mode if name == names[1] else 0o755)
        manifest = dict(layoutVersion=1, version=version, target=target, variant="codex", entrypoint=f"bin/codex{suffix}", resourcesDir="codex-resources", pathDir="codex-path")
        manifest.update(manifest_changes or {})
        (directory / "codex-package.json").write_text(json.dumps(manifest))
        archive = directory.with_suffix(".zip" if suffix else ".tar.zst")
        if suffix:
            with zipfile.ZipFile(archive, "w") as package:
                for file in directory.rglob("*"):
                    package.write(file, file.relative_to(directory))
        else:
            tar_path = directory.with_suffix(".tar")
            with tarfile.open(tar_path, "w") as package:
                for file in directory.iterdir():
                    package.add(file, file.name)
            subprocess.run(["zstd", "-q", "-f", str(tar_path), "-o", str(archive)], check=True)
        return archive

    @property
    def installed_root(self):
        return Path(self.env["HOME"]) / ".local/share/mevanlc-codex/packages/cli"

    def install(self, package=None, version="1.2.3-aaaaaaa", date="2026-10-02T00:00:00Z", *args, **kwargs):
        package = package or self.package(version)
        source = release("fork-v" + version, date)
        source["assets"][0]["name"] = kwargs.get("asset", ASSET)
        return self.run_installer([[source]], *args, archive=str(package), pinned={source["tag_name"]: source}, **kwargs)

    def test_full_install_is_independent_of_config_and_xdg(self):
        config = self.root / "other-config"
        config.mkdir()
        (config / "config.toml").write_text("[features]\ndaemon_auto_start = false\n")
        self.env.update(CODEX_HOME=str(config), XDG_DATA_HOME=str(self.root / "xdg"))
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        current = self.installed_root / "current"
        self.assertTrue(current.is_symlink())
        self.assertTrue((current / "codex-path/rg").is_file())
        self.assertTrue((current / "codex-resources/data/file").is_file())
        self.assertTrue((Path(self.env["HOME"]) / ".local/bin/codex").is_symlink())
        self.assertEqual((config / "config.toml").read_text(), "[features]\ndaemon_auto_start = false\n")
        self.assertFalse((config / "packages").exists())
        self.assertIn("Source timestamp:", result.stdout)
        self.assertIn("epoch:", result.stdout)

    def test_currency_force_pin_repair_and_retention(self):
        first = self.package()
        self.assertEqual(self.install(first).returncode, 0)
        old = (self.installed_root / "current").resolve()
        result = self.install(first)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("already as new or newer", result.stdout)
        self.assertIn("Existing version: codex 1.2.3-aaaaaaa", result.stdout)
        self.assertIn("Existing Codex mtime:", result.stdout)
        self.assertIn("Existing code-mode host mtime:", result.stdout)
        (old / "codex-path/rg").unlink()
        result = self.install(first)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.installed_root / "current/codex-path/rg").exists())
        self.assertTrue(old.is_dir())
        result = self.install(first, "1.2.3-aaaaaaa", "2026-10-02T00:00:00Z", "--force")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Forced install requested", result.stdout)
        package = self.package("1.0.0")
        result = self.install(package, "1.0.0", "2026-10-01T00:00:00Z", "--version", "1.0.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Explicit version requested", result.stdout)
        self.assertFalse((self.installed_root / "auto-update-version").exists())
        self.assertTrue(old.is_dir())

    def test_invalid_packages_preserve_the_previous_selection(self):
        self.assertEqual(self.install().returncode, 0)
        old = (self.installed_root / "current").resolve()
        for kwargs in [dict(missing="bin/codex-code-mode-host"), dict(mode=0o644), dict(manifest_changes={"target":"wrong"}), dict(manifest_changes={"layoutVersion":2}), dict(manifest_changes={"version":"../../escape"}), dict(manifest_changes={"entrypoint":"codex"})]:
            with self.subTest(kwargs=kwargs):
                result = self.install(self.package(**kwargs), "1.2.3-aaaaaaa", "2026-10-02T00:00:00Z", "--force")
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertEqual((self.installed_root / "current").resolve(), old)

    def test_rollback_restores_selection_and_replaced_commands(self):
        self.assertEqual(self.install().returncode, 0)
        old = (self.installed_root / "current").resolve()
        marker = (self.installed_root / "auto-update-version").read_text()
        binary = Path(self.env["HOME"]) / ".local/bin/codex"
        binary.unlink()
        binary.write_text("#!/bin/sh\necho 'codex original'\n")
        binary.chmod(0o755)
        host = binary.with_name("codex-code-mode-host")
        host.unlink()
        host.mkdir()  # A real directory makes command replacement fail after selection.
        result = self.install(self.package("1.3.0"), "1.3.0")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(binary.read_text(), "#!/bin/sh\necho 'codex original'\n")
        self.assertEqual((self.installed_root / "current").resolve(), old)
        self.assertEqual((self.installed_root / "auto-update-version").read_text(), marker)
        self.assertTrue(host.is_dir())

    def test_command_directory_alias_cannot_modify_an_upstream_package(self):
        upstream = self.root / "upstream-package"
        (upstream / "bin").mkdir(parents=True)
        (upstream / "codex-package.json").write_text('{"version":"0.1.0"}')
        binary = upstream / "bin/codex"
        original = f"#!{shutil.which('sh')}\necho 'codex 0.1.0'\n"
        binary.write_text(original)
        binary.chmod(0o755)
        visible = Path(self.env["HOME"]) / ".local/bin"
        visible.parent.mkdir(parents=True)
        visible.symlink_to(upstream / "bin", target_is_directory=True)
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("points inside a Codex package", result.stderr)
        self.assertEqual(binary.read_text(), original)
        self.assertFalse((self.installed_root / "current").exists())

    def test_failed_download_changes_nothing(self):
        self.assertEqual(self.install().returncode, 0)
        old = (self.installed_root / "current").resolve()
        result = self.run_installer([[release("new", "2026-10-03T00:00:00Z")]])
        self.assertEqual(result.returncode, 75)
        self.assertEqual((self.installed_root / "current").resolve(), old)

    def test_daemon_install_is_separate_and_guarded(self):
        self.assertEqual(self.install().returncode, 0)
        cli = (self.installed_root / "current").resolve()
        self.env["CODEX_INSTALL_DAEMON_ONLY"] = "1"
        self.assertEqual(self.install().returncode, 0)
        daemon = self.installed_root.with_name("app-server-daemon")
        old = (daemon / "current").resolve()
        (daemon / "auto-update-version").unlink()  # Explicit pin wins over a scheduled update.
        self.env.update(CODEX_INSTALL_IF_LATEST="1", CODEX_UPDATE_FROM_RELEASE=old.name)
        result = self.install(self.package("1.3.0"), "1.3.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("skipped guarded install", result.stdout)
        self.assertEqual((daemon / "current").resolve(), old)
        self.assertEqual((self.installed_root / "current").resolve(), cli)
        self.env.update(CODEX_INSTALL_IF_LATEST="0", CODEX_INSTALL_IF_CURRENT="1")
        result = self.install(self.package("1.3.0"), "1.3.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotEqual((daemon / "current").resolve(), old)
        self.assertEqual((self.installed_root / "current").resolve(), cli)

    def test_linux_and_windows_package_requirements(self):
        for target in ["x86_64-unknown-linux-musl", "aarch64-apple-darwin", "x86_64-pc-windows-msvc"]:
            with self.subTest(target=target):
                self.env["CODEX_ARTIFACT_NAME"] = "codex-" + target
                asset = "codex-" + target + (".zip" if target.endswith("-msvc") else ".tar.zst")
                result = self.install(self.package(target=target), "1.2.3-aaaaaaa", "2026-10-02T00:00:00Z", "--force", asset=asset)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_artifact_contains_one_package(self):
        artifact = {"id":42, "name":"codex-aarch64-linux-android", "created_at":"2026-10-02T00:00:00Z", "workflow_run":{"id":7}}
        result = self.run_installer([{"artifacts":[artifact]}], "--artifact", archive=str(self.package()))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.installed_root / "auto-update-version").exists())

    def test_concurrent_installs_serialize_selection(self):
        archive = self.package()
        pages = [[release("fork-v1.2.3-aaaaaaa", "2026-10-02T00:00:00Z")]]
        (self.root / "fixture.json").write_text(json.dumps(dict(pages=pages, archive=str(archive))))
        commands = [subprocess.Popen([shutil.which("bash"), str(INSTALL_SCRIPT)], env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) for _ in range(3)]
        for command in commands:
            out, err = command.communicate(timeout=30)
            self.assertEqual(command.returncode, 0, out + err)
        self.assertEqual(len(list((self.installed_root / "releases").iterdir())), 1)
        self.assertEqual((self.installed_root / "auto-update-version").read_text(), (self.installed_root / "current").resolve().name)



@unittest.skipUnless(os.name == "nt" and os.environ.get("CODEX_TEST_NATIVE_PACKAGE"), "native Windows package supplied by CI")
class NativePackageInstallTest(InstallerFixture):
    def test_junction_launchers_and_daemon_selection(self):
        import zipfile
        self.installer_timeout = 180
        self.env["HOME"] = str(self.root / "home & 100% ready! café 測試")
        archive = Path(os.environ["CODEX_TEST_NATIVE_PACKAGE"]).resolve()
        with zipfile.ZipFile(archive) as package:
            manifest = json.loads(package.read("codex-package.json"))
        target, version = manifest["target"], manifest["version"]
        asset = f"codex-{target}.zip"
        self.env["CODEX_ARTIFACT_NAME"] = f"codex-{target}"
        config = self.root / "separate-config"
        config.mkdir()
        (config / "config.toml").write_text("[features]\ndaemon_auto_start = false\n")
        self.env["CODEX_HOME"] = str(config)
        source = release("fork-v" + version, "2026-10-02T00:00:00Z")
        source["assets"][0]["name"] = asset
        def install(*args):
            result = self.run_installer([[source]], *args, archive=str(archive), asset=asset)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        old_bin = Path(self.env["HOME"]) / ".local/bin"
        old_bin.mkdir(parents=True)
        (old_bin / "codex.exe").write_bytes(b"legacy executable")
        (old_bin / "codex.ps1").write_text("Write-Output 'stale'; exit 7\n")
        install()
        root = Path(self.env["HOME"]) / ".local/share/mevanlc-codex/packages"
        current = root / "cli/current"
        self.assertEqual(current.lstat().st_reparse_tag, 0xA0000003)
        selected = current.resolve()
        binary = Path(self.env["HOME"]) / ".local/bin/codex.cmd"
        result = subprocess.run([str(binary), "--version"], capture_output=True, text=True, check=True)
        self.assertIn(version, result.stdout)
        bash = os.environ["CODEX_TEST_BASH"]
        result = subprocess.run([bash, binary.with_suffix("").as_posix(), "--version"], capture_output=True, text=True, check=True)
        self.assertIn(version, result.stdout)
        self.assertFalse(binary.with_suffix(".exe").exists())
        self.assertFalse(binary.with_suffix(".ps1").exists())
        powershell = Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"
        shell_env = self.env.copy()
        shell_env["PATH"] = str(binary.parent) + os.pathsep + shell_env["PATH"]
        result = subprocess.run([str(powershell), "-NoProfile", "-NonInteractive", "-Command", "codex --version; exit $LASTEXITCODE"], env=shell_env, capture_output=True, text=True, check=True)
        self.assertIn(version, result.stdout)
        # A cmd.exe parent may still have the launcher open during codex update.
        with binary.open("rb"):
            install("--force")
        self.assertEqual(current.resolve(), selected)
        self.env["CODEX_INSTALL_DAEMON_ONLY"] = "1"
        install()
        self.assertTrue((root / "app-server-daemon/current/bin/codex.exe").is_file())
        self.assertEqual(current.resolve(), selected)
        self.assertFalse((config / "packages").exists())
        self.assertEqual((config / "config.toml").read_text(), "[features]\ndaemon_auto_start = false\n")


if __name__ == "__main__":
    unittest.main()
