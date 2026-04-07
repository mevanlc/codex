# Session Summary: v8-code-mode-android
**Session ID:** `65d69e07-1612-4fc0-8511-02b452ce8edf`
**Session slug:** `compiled-wibbling-pine`
**Time span:** 2026-04-06 ~20:00 MDT to 2026-04-07 ~07:40 MDT (~12 hours)
**Working directory:** `/Users/mclark/p/my/codex`

---

## Overview

This was a long, intensive session focused entirely on getting V8 (the `codex-code-mode` / JS REPL feature) working in Android aarch64 CI builds of the `mevanlc/codex` fork. The session began when the user noticed a `cargo install` failure caused by a `temporal_rs`/`icu_calendar` API mismatch (a side effect of commenting out a v8 git override in `Cargo.toml`). The session quickly pivoted to the real goal: building a V8 Android prebuilt static library so the Android release workflow could include code-mode rather than disabling it with `--no-default-features`.

The arc: diagnose a build break → re-read prior session history (via 11 parallel summarizing agents) → formalize a plan (`compiled-wibbling-pine.md`) → build the `v8-android-prebuilt.yml` workflow → fight `chromium.googlesource.com` 403/429 rate-limiting on submodule fetches → build V8 from source locally in an OrbStack VM as a fallback → upload the prebuilt to `mevanlc/codex` releases tag `v8-v146.4.0` → re-enable `android-release.yml` with code-mode → fix linker errors (`__clear_cache` undefined symbol) → push what appears to be the penultimate fix attempt before the session ended. The build was still in-flight or unresolved at session close.

---

## Key Decisions & Changes (Early/Mid Session)

### Initial Build Break (lines 1–50)
- `cargo install --path codex-rs/cli` failed because `temporal_rs v0.1.2` was compiled against an older `icu_calendar` API, but the lock file had been re-resolved after a `Cargo.toml` change that commented out the `v8` git override.
- **Fix proposed:** `git checkout codex-rs/Cargo.lock` + `cargo install --locked`.
- The `v8` git source line had been commented out because Android builds were being handled separately.

### Session History Review via Parallel Agents (lines 40–100)
- User requested 11 parallel sub-agents to summarize prior CC sessions to re-establish context. The agents ran concurrently and reported back with summaries covering:
  - Prior Cargo.lock fix work
  - Successful past Android aarch64 release (binary was 32MB, used swap + thin LTO to avoid OOM)
  - The `mevanlc/rusty_v8` fork is an unmodified mirror; the plan was to never maintain it and always clone from `denoland/rusty_v8`
  - Key finding: `rusty_v8`'s `build.rs` supports `RUSTY_V8_MIRROR` and `RUSTY_V8_ARCHIVE` env vars to redirect prebuilt downloads to an alternate GitHub release URL

### Plan Formalized (line ~440 area)
**Plan file:** `/Users/mclark/.claude/plans/compiled-wibbling-pine.md`
- **Phase 1:** Build V8 Android prebuilt via `v8-android-prebuilt.yml` (workflow_dispatch only)
- **Phase 2:** Re-create `android-release.yml` with code-mode enabled (using `RUSTY_V8_ARCHIVE` + `RUSTY_V8_SRC_BINDING_PATH`)
- **Phase 3:** Update `build-fork.sh` for Termux on-device builds
- **Phase 4:** Preserve investigation workflows

Key mechanism: `RUSTY_V8_ARCHIVE` redirects `librusty_v8_release_aarch64-linux-android.a.gz` download; `RUSTY_V8_SRC_BINDING_PATH` provides the FFI binding `.rs` file.

### `android-release.yml` Recreated (line ~302)
- Wrote a new `android-release.yml` with:
  - `ubuntu-latest` runner, 120-minute timeout
  - NDK 27.2.12479018, `aarch64-linux-android35-clang` linker
  - 8GB swap file at `/mnt/swapfile`
  - `CARGO_BUILD_JOBS: 2`, `CARGO_PROFILE_RELEASE_LTO: thin`
  - Version computed from nearest `rust-v*` tag + commit distance

### `build-fork.sh` Updated (line ~400 area)
- Updated to auto-configure `RUSTY_V8_ARCHIVE` and `RUSTY_V8_SRC_BINDING_PATH` when run on Termux (detects `uname -m == aarch64` + `/system/build.prop` present)
- Points to `https://github.com/mevanlc/codex/releases/download/v8-v146.4.0/...`
- No `--no-default-features` needed; code-mode enabled by default

### V8 Prebuilt Workflow Created: `v8-android-prebuilt.yml` (lines ~450–500)
- Triggered `Build V8 Android Prebuilt` workflow run: https://github.com/mevanlc/codex/actions/runs/24054947148
- First attempt failed at step 9 ("Fetch V8 source") — 403 from `chromium.googlesource.com`

---

## Recent Work (Final Quarter)

This covers approximately lines 500–1663, roughly 20:46 MDT through 07:40 MDT.

### Phase: Fighting googlesource.com Rate Limiting (lines 504–800)

The core challenge: `chromium.googlesource.com` returns 403/429 when parallel submodule fetches are attempted from GitHub Actions runners (unauthenticated). Multiple iterations of the `v8-android-prebuilt.yml` "Fetch submodules" step:

1. **Run 24054947148** — failed at "Fetch V8 source" (403, step 9)
2. **Sequential fetch with retry + exponential backoff** added. `clone_submodule()` function: 6 retries, 45s initial delay doubling each attempt. GitHub-hosted submodules fetched first, then googlesource.com submodules one at a time with 20s sleep between.
3. **429 rate-limit discovery**: User mentioned Gemini suggested depot_tools auth. Claude realized 429 = authenticated (good) vs 403 = forbidden — the gitcookies secret (`GOOGLESOURCE_GITCOOKIES`) was working, just hitting rate limits.
4. **Bug found in submodule classification grep** (line ~800): The `grep 'github\.com'` in the workflow script matched googlesource URLs that embed `github.com` in their path (e.g., `chromium.googlesource.com/external/github.com/llvm/...`). This caused submodules like `third_party/libc++/src` to be attempted twice, masking failures. Fixed to match on the URL column only.

**Submodule breakdown** (confirmed via local `git config --file .gitmodules` query):
- GitHub-hosted: `v8`, `build`, `third_party/fp16/src` (and several that appear in both lists due to the bug)
- googlesource: `tools/clang`, `third_party/jinja2`, `third_party/markupsafe`, `buildtools`, `third_party/icu`, `third_party/abseil-cpp`, `third_party/simdutf`, `third_party/partition_alloc`, `third_party/rust`, `tools/win` + more

### Phase: Parallel Local Build in OrbStack VM (lines ~900–1450)

While the GitHub Actions workflow was grinding through submodule fetches, a local build was started in a VM over SSH (`ssh v8-builder-x64@orb`) as a faster parallel path.

**Local build errors encountered:**
1. `tools/clang/scripts/update.py` missing → the `tools/clang` submodule had failed to clone (429) and `continue-on-error` masked it. Fix: clone `tools/clang` from the GitHub Chromium mirror instead.
2. `gn gen` failing: `Can't load input file: build/config/gclient_args.gni` → file needs to be generated (normally by `gclient`). Fix: create a minimal `gclient_args.gni` stub.
3. After installing a host sysroot (`install_debian_sysroot.py`), gn succeeded.
4. ninja build ran but panicked at `build.rs:1121` — `ninja` itself failed. Needed `libclang-dev` installed for the bindgen step (line ~1400).
5. After `sudo apt-get install -y libclang-dev`, the build completed successfully at exit code 0.

**V8 Build artifacts uploaded (line ~1410–1450):**
- `librusty_v8_release_aarch64-linux-android.a.gz` (~30 MB)
- `src_binding_release_aarch64-linux-android.rs`
- Published to GitHub release tag `v8-v146.4.0` on `mevanlc/codex` repo
- Tarball noise note: 10,000+ lines of `tar: Ignoring unknown extended header keyword 'LIBARCHIVE.xattr.com.apple.provenance'` — cosmetic, from macOS extended attributes. Claude noted the fix for next time: `COPYFILE_DISABLE=1` or `--no-xattrs` when creating tarballs.

**Plan status at that point (line ~1457–1459):**
| Phase | Status |
|---|---|
| Phase 1: Build V8 prebuilt | Done — uploaded to `mevanlc/codex` releases `v8-v146.4.0` |
| Phase 2: android-release.yml with code-mode | Ready — workflow existed `.disabled`, needed re-enable |
| Phase 3: build-fork.sh for Termux | Done |
| Phase 4: Restore investigation workflows | Done (`.disabled`) |

### Phase: Re-enable Android Release + First Code-Mode Build (lines 1460–1604)

- User: "re-enable and push"
- Renamed `.github/workflows/android-release.yml.disabled` → `android-release.yml`
- Committed and pushed; workflow triggered: https://github.com/mevanlc/codex/actions/runs/24064559959
- Session title was set to `v8-code-mode-android`

**First linker error — `__clear_cache` undefined:**
- The build compiled but failed at final link: `undefined reference to __clear_cache`
- Root cause: V8's ARM64 codegen uses `__clear_cache` (instruction cache flush). This was historically in `libgcc`, but NDK 23+ replaced libgcc with clang's compiler-rt. The symbol lives in `libclang_rt.builtins-aarch64-android.a`.
- Claude initially tried `-C link-arg=-lcompiler_rt-extras` (wrong library). Verified locally that `__clear_cache` is not in `libcompiler_rt-extras.a` on the local NDK.
- Committed: "Link compiler_rt-extras to resolve __clear_cache" — known to be wrong but pushed to test.

### Phase: Diagnosing `__clear_cache` Correctly (lines 1640–1663)

- Searched all NDK `.a` files for `__clear_cache`. Found it in:
  - `lib/clang/18/lib/linux/libclang_rt.builtins-aarch64-android.a`
  - (also baremetal and musl variants, not relevant)
- **Root cause refined:** Rust passes `-nodefaultlibs` to the linker which strips automatic compiler-rt linkage. Must link explicitly.
- Tried `-C link-arg=-lgcc` (NDK 23+ removed libgcc, would fail).
- Tried `-C link-arg=--rtlib=compiler-rt` (clang flag, not linker flag, won't work via `-C link-arg`).
- **Final approach:** Link by full absolute path:
  ```
  RUSTFLAGS: -C link-arg=/usr/local/lib/android/sdk/ndk/27.2.12479018/toolchains/llvm/prebuilt/linux-x86_64/lib/clang/18/lib/linux/libclang_rt.builtins-aarch64-android.a
  ```
- Also reverted `build-fork.sh` back to just `-lc++_static -lc++abi` (Termux builds natively, no cross-compilation workaround needed).
- Committed and pushed: `80a74c143` — "Link clang_rt builtins explicitly for __clear_cache on Android"

**Last push state (line 1663):**
- Session ended shortly after this push. The Android release workflow would have been triggered again.
- `/compact` was attempted at 15:34 but failed: "Conversation too long."
- `/status` was checked, then `/exit` was used to close the session.

---

## Unresolved Issues / Next Steps

1. **The `__clear_cache` fix may or may not work.** The full-path `-C link-arg` approach is correct in principle. However, if the CI runner's NDK installs to a different path (e.g., different clang version within NDK 27), the path `/usr/local/lib/android/sdk/ndk/27.2.12479018/toolchains/llvm/prebuilt/linux-x86_64/lib/clang/18/lib/linux/libclang_rt.builtins-aarch64-android.a` may not exist. The CI runner installs NDK via `sdkmanager` and the ANDROID_HOME path should be `/usr/local/lib/android/sdk`. This needs verification.

2. **The `android-v8-investigate.yml` workflow** was left as a debugging scaffold. Should be cleaned up or kept only as a test harness.

3. **`v8-android-prebuilt.yml`** reached iteration 21 by session end — this workflow is now known-working (it successfully built V8 and uploaded artifacts). Can be kept as-is for re-running when V8 version bumps.

4. **V8 version `146.4.0` is pinned** in both `android-release.yml` (via `RUSTY_V8_ARCHIVE` URL) and `build-fork.sh`. When upstream bumps v8 (in `Cargo.toml`), these will need to be updated and a new prebuilt built.

5. **Termux on-device build** hasn't been tested end-to-end with the new prebuilt. `build-fork.sh` is set up, but the actual on-device test hasn't been confirmed.

6. **The xattr tar noise** — next rebuild should use `COPYFILE_DISABLE=1 tar czf ...` to avoid 10K+ noisy log lines.

7. **`/compact` failed** at session end ("Conversation too long") — if this session is resumed it will need to start fresh with context primed manually or via memory files at `/Users/mclark/.claude/projects/-Users-mclark-p-my-codex/memory/`.

---

## Key Files Modified

- `.github/workflows/android-release.yml` — re-created with RUSTY_V8_ARCHIVE, RUSTY_V8_SRC_BINDING_PATH, linker flags for `__clear_cache`. Version 27 at session end.
- `.github/workflows/v8-android-prebuilt.yml` — new workflow, built V8 from source, published prebuilt. Version 21 at session end.
- `.github/workflows/v8-android-prebuilt-mac.yml` — created for Mac-side testing (ultimately Android can only build on Linux per rusty_v8 assertion).
- `.github/workflows/android-v8-investigate.yml` — diagnostic/investigation workflow (kept as disabled or debug tool).
- `.github/workflows/android-release.yml.disabled` — backup of old release workflow.
- `codex-rs/scripts/build-fork.sh` — updated to set `RUSTY_V8_ARCHIVE` + `RUSTY_V8_SRC_BINDING_PATH` on Termux. Final RUSTFLAGS: `-lc++_static -lc++abi` (no compiler_rt-extras).
- `/Users/mclark/.claude/plans/compiled-wibbling-pine.md` — the 4-phase plan document.
- `/Users/mclark/.claude/projects/-Users-mclark-p-my-codex/memory/MEMORY.md` — project memory file, updated.
- `/Users/mclark/.claude/projects/-Users-mclark-p-my-codex/memory/project_v8_android_prebuilt.md` — dedicated memory for v8 prebuilt work.

## Key GitHub Actions Runs

| Run | Outcome | Notes |
|---|---|---|
| 24054947148 | Failed | First prebuilt attempt, 403 on Fetch V8 source |
| 24056155275 | Failed | Sequential fetch with 15s delay, still 429 |
| ~24061162973 | Failed at build | gn gen failed (missing gclient_args.gni) |
| ~24062045272 | In progress at lines 1200+ | "Build V8 from source" was running |
| 24064559959 | Triggered | First code-mode Android release attempt |
| Latest | In-flight at session close | With `__clear_cache` fix via libclang_rt.builtins full path |
