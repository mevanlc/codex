import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("configure-android-toolchain.sh")


class AndroidToolchainTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="android toolchain ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.host_resource = self.root / "host clang 21"
        (self.host_resource / "include").mkdir(parents=True)
        (self.host_resource / "include/arm_neon.h").touch()
        self.ndk = self.root / "sdk/ndk/27.2.12479018/toolchains/llvm/prebuilt"
        self.runtime = self.make_ndk("linux-x86_64")
        self.env = dict(os.environ)
        self.env.update(
            ANDROID_HOME=str(self.root / "sdk"),
            RUNNER_TEMP=str(self.root / "runner temp"),
            GITHUB_ENV=str(self.root / "github_env"),
            GITHUB_PATH=str(self.root / "github_path"),
            PATH=f"{self.bin}{os.pathsep}{os.environ['PATH']}",
            MOCK_RESOURCE=str(self.host_resource),
            MOCK_LOG=str(self.root / "compiler args"),
        )
        Path(self.env["RUNNER_TEMP"]).mkdir()
        for name in ("clang", "clang++"):
            self.write_tool(
                self.bin / name,
                'if [ "$1" = -print-resource-dir ]; then\n'
                '  printf "%s\\n" "$MOCK_RESOURCE"\n'
                'else\n'
                '  printf "%s\\n" "$@" > "$MOCK_LOG"\n'
                'fi\n',
            )
        for name in ("llvm-ar", "llvm-ranlib"):
            self.write_tool(self.bin / name, "exit 0\n")

    @staticmethod
    def write_tool(path, body):
        path.write_text("#!/usr/bin/env bash\nset -eu\n" + body)
        path.chmod(0o755)

    def make_ndk(self, host):
        toolchain = self.ndk / host
        runtime = (
            toolchain / "lib/clang/18/lib/linux/libclang_rt.builtins-aarch64-android.a"
        )
        runtime.parent.mkdir(parents=True)
        runtime.touch()
        (toolchain / "sysroot").mkdir()
        return runtime

    def configure(self, check=True):
        return subprocess.run(
            ["bash", str(SCRIPT)],
            env=self.env,
            text=True,
            capture_output=True,
            check=check,
        )

    def exported(self):
        return dict(
            line.split("=", 1)
            for line in Path(self.env["GITHUB_ENV"]).read_text().splitlines()
        )

    def test_fallback_pairs_host_headers_with_ndk_runtime(self):
        self.configure()
        exported = self.exported()
        resource = (
            Path(self.env["RUNNER_TEMP"]) / "android-a1a-toolchain/clang-resource"
        )
        self.assertEqual(
            (resource / "include").resolve(), self.host_resource / "include"
        )
        self.assertEqual((resource / "lib").resolve(), self.runtime.parents[1])
        self.assertEqual(exported["RUSTFLAGS"], f"-C link-arg={self.runtime}")
        for key in ("CC_aarch64_linux_android", "CXX_aarch64_linux_android"):
            subprocess.run(
                [exported[key], "source with spaces.c", "-c"], env=self.env, check=True
            )
            args = Path(self.env["MOCK_LOG"]).read_text().splitlines()
            self.assertEqual(args[args.index("-resource-dir") + 1], str(resource))
            self.assertIn("source with spaces.c", args)
            self.assertIn("--rtlib=compiler-rt", args)
        self.configure()  # Reconfiguration must safely refresh the symlinks.
        self.assertTrue((resource / "include/arm_neon.h").is_file())

    def test_native_ndk_branch_is_preserved(self):
        runtime = self.make_ndk("linux-aarch64")
        native_bin = self.ndk / "linux-aarch64/bin"
        native_bin.mkdir()
        for name in (
            "aarch64-linux-android35-clang",
            "aarch64-linux-android35-clang++",
            "llvm-ar",
            "llvm-ranlib",
        ):
            self.write_tool(native_bin / name, "exit 0\n")
        self.configure()
        exported = self.exported()
        self.assertEqual(
            exported["CC_aarch64_linux_android"],
            str(native_bin / "aarch64-linux-android35-clang"),
        )
        self.assertEqual(exported["RUSTFLAGS"], f"-C link-arg={runtime}")
        self.assertFalse(
            (Path(self.env["RUNNER_TEMP"]) / "android-a1a-toolchain").exists()
        )

    def test_mismatched_c_and_cxx_headers_fail_early(self):
        self.write_tool(self.bin / "clang++", "echo /different/resource\n")
        result = self.configure(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("same resource headers", result.stderr)
        self.assertFalse(Path(self.env["GITHUB_ENV"]).exists())

    def test_missing_ndk_builtins_fail_early(self):
        self.runtime.unlink()
        self.assertNotEqual(self.configure(check=False).returncode, 0)
        self.assertFalse(Path(self.env["GITHUB_ENV"]).exists())


if __name__ == "__main__":
    unittest.main()
