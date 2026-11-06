#!/usr/bin/env python3

import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest


INSTALL_SCRIPT = Path(__file__).resolve().parents[2] / "install-fork.sh"
ASSET = "codex-fork-installer-aarch64-linux-android"
INSTALLER = b'#!/usr/bin/env sh\nprintf "installer:%s\\n" "$@"\n'
DIGEST = "sha256:" + hashlib.sha256(INSTALLER).hexdigest()


def release(tag: str, published: str | None) -> dict:
    return {
        "tag_name": tag,
        "draft": False,
        "prerelease": False,
        "published_at": published,
        "assets": [
            {
                "browser_download_url": f"https://example.test/{tag}/{ASSET}",
                "digest": DIGEST,
                "name": ASSET,
            }
        ],
    }


def run_bootstrap(metadata: str) -> tuple[subprocess.CompletedProcess, list[str]]:
    temp_root = os.environ.get("TMPDIR")
    if not temp_root and os.environ.get("PREFIX"):
        temp_root = str(Path(os.environ["PREFIX"]) / "tmp")
    with tempfile.TemporaryDirectory(dir=temp_root) as temp_dir:
        root = Path(temp_dir)
        (root / "releases.json").write_text(metadata)
        (root / "installer").write_bytes(INSTALLER)
        curl = root / "curl"
        curl.write_text(
            textwrap.dedent(
                """\
                #!/usr/bin/env sh
                set -eu
                output=""
                previous=""
                for arg in "$@"; do
                  if [ "$previous" = "-o" ]; then output="$arg"; fi
                  previous="$arg"
                done
                printf '%s\\n' "$arg" >> "$CODEX_TEST_ROOT/requests"
                case "$arg" in
                  'https://api.example.test/repos/mevanlc/codex/releases?per_page=100')
                    cp "$CODEX_TEST_ROOT/releases.json" "$output"
                    ;;
                  https://example.test/*)
                    cp "$CODEX_TEST_ROOT/installer" "$output"
                    ;;
                  *) exit 1 ;;
                esac
                """
            )
        )
        curl.chmod(0o755)
        env = {
            **os.environ,
            "PATH": str(root) + os.pathsep + os.environ["PATH"],
            "TMPDIR": str(root),
            "GH_TOKEN": "",
            "GITHUB_TOKEN": "",
            "CODEX_FORK_REPO": "mevanlc/codex",
            "CODEX_GITHUB_API_URL": "https://api.example.test",
            "CODEX_ARTIFACT_NAME": "codex-aarch64-linux-android",
            "CODEX_TEST_ROOT": str(root),
        }
        result = subprocess.run(
            ["sh", str(INSTALL_SCRIPT), "--version", "value with spaces"],
            env=env,
            capture_output=True,
            text=True,
            timeout=10,
        )
        return result, (root / "requests").read_text().splitlines()


class InstallForkShTest(unittest.TestCase):
    def test_large_compact_metadata_selects_newest_published_fork(self) -> None:
        releases = []
        for i in range(100):
            item = release(f"fork-v0.1.{i}", "2026-10-01T00:00:00Z")
            item["assets"].extend(
                {
                    "name": f"other-platform-{j}",
                    "browser_download_url": "https://example.test/" + "x" * 200,
                    "digest": DIGEST,
                    "uploader": {"name": ASSET, "digest": "decoy"},
                }
                for j in range(30)
            )
            releases.append(item)
        chosen = release("fork-v0.2.0", "2026-11-01T00:00:00Z")
        releases.insert(10, chosen)
        for flag in ("draft", "prerelease"):
            item = release("fork-v0.3.0", "2026-11-02T00:00:00Z")
            item[flag] = True
            releases.append(item)
        releases.extend(
            [
                release("rust-v0.4.0", "2026-11-03T00:00:00Z"),
                release("fork-v0.5.0", None),
            ]
        )
        missing = release("fork-v0.6.0", "2026-11-04T00:00:00Z")
        missing["assets"][0]["name"] = "another-platform"
        releases.append(missing)
        metadata = json.dumps(releases, separators=(",", ":"))
        self.assertGreater(len(metadata), 1_000_000)

        result, requests = run_bootstrap(metadata)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(requests[-1], chosen["assets"][0]["browser_download_url"])
        self.assertEqual(len(requests), 2)
        self.assertEqual(
            result.stdout, "installer:--version\ninstaller:value with spaces\n"
        )

    def test_escaped_strings_and_tokens_cross_record_boundaries(self) -> None:
        item = release("fork-v0.2.0", "2026-11-01T00:00:00Z")
        item["assets"][0]["uploader"] = {
            "name": ASSET,
            "digest": "decoy",
            "browser_download_url": "https://example.test/decoy",
        }
        # Place the first byte of an escaped backslash at a fold boundary.
        body = "x" * (4095 - len('[{"body":"')) + '\\"雪\n{"assets":[]}'
        for indent in (None, 2):
            with self.subTest(indent=indent):
                metadata = json.dumps(
                    [{"body": body, **item}],
                    separators=(",", ":"),
                    ensure_ascii=False,
                    indent=indent,
                )
                result, requests = run_bootstrap(metadata)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(
                    requests[-1], item["assets"][0]["browser_download_url"]
                )

    def test_digest_mismatch_prevents_execution(self) -> None:
        item = release("fork-v0.2.0", "2026-11-01T00:00:00Z")
        item["assets"][0]["digest"] = "sha256:" + "0" * 64

        result, _ = run_bootstrap(json.dumps([item]))

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not match its published digest", result.stderr)
        self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
