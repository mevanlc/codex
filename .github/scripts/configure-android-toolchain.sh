#!/usr/bin/env bash
set -euo pipefail

NDK_HOME="$ANDROID_HOME/ndk/27.2.12479018"
NDK_HOST_TAG="linux-x86_64"
if [ -d "$NDK_HOME/toolchains/llvm/prebuilt/linux-aarch64" ]; then
  NDK_HOST_TAG="linux-aarch64"
fi
NDK_TOOLCHAIN="$NDK_HOME/toolchains/llvm/prebuilt/$NDK_HOST_TAG"
SYSROOT="$NDK_TOOLCHAIN/sysroot"
NDK_RESOURCE_DIR="$NDK_TOOLCHAIN/lib/clang/18"
BUILTINS="$NDK_RESOURCE_DIR/lib/linux/libclang_rt.builtins-aarch64-android.a"
test -f "$BUILTINS"

if [ "$NDK_HOST_TAG" = "linux-aarch64" ]; then
  TOOLCHAIN_BIN="$NDK_TOOLCHAIN/bin"
  LINKER="$TOOLCHAIN_BIN/aarch64-linux-android35-clang"
  CXX="$TOOLCHAIN_BIN/aarch64-linux-android35-clang++"
  AR="$TOOLCHAIN_BIN/llvm-ar"
  RANLIB="$TOOLCHAIN_BIN/llvm-ranlib"
else
  TOOLCHAIN_BIN="$RUNNER_TEMP/android-a1a-toolchain"
  mkdir -p "$TOOLCHAIN_BIN"
  HOST_CC="$(command -v clang)"
  HOST_CXX="$(command -v clang++)"
  HOST_RESOURCE_DIR="$("$HOST_CC" -print-resource-dir)"
  if [ "$("$HOST_CXX" -print-resource-dir)" != "$HOST_RESOURCE_DIR" ]; then
    echo "clang and clang++ must use the same resource headers" >&2
    exit 1
  fi
  test -f "$HOST_RESOURCE_DIR/include/arm_neon.h"

  # Intrinsic headers must match the compiler, while Android runtime libraries
  # still come from the NDK. Do not put both resource include directories on the
  # search path: include_next in headers such as stdint.h must reach the sysroot.
  RESOURCE_DIR="$TOOLCHAIN_BIN/clang-resource"
  mkdir -p "$RESOURCE_DIR"
  ln -sfn "$HOST_RESOURCE_DIR/include" "$RESOURCE_DIR/include"
  ln -sfn "$NDK_RESOURCE_DIR/lib" "$RESOURCE_DIR/lib"

  LINKER="$TOOLCHAIN_BIN/aarch64-linux-android35-clang"
  CXX="$TOOLCHAIN_BIN/aarch64-linux-android35-clang++"
  cat > "$LINKER" <<EOF
#!/usr/bin/env bash
exec "$HOST_CC" --target=aarch64-linux-android35 --sysroot="$SYSROOT" -resource-dir "$RESOURCE_DIR" -fuse-ld=lld --rtlib=compiler-rt "\$@"
EOF
  cat > "$CXX" <<EOF
#!/usr/bin/env bash
exec "$HOST_CXX" --target=aarch64-linux-android35 --sysroot="$SYSROOT" -resource-dir "$RESOURCE_DIR" -fuse-ld=lld --rtlib=compiler-rt "\$@"
EOF
  chmod +x "$LINKER" "$CXX"
  AR="$(command -v llvm-ar)"
  RANLIB="$(command -v llvm-ranlib)"
fi

file "$LINKER" || true
"$LINKER" --version
{
  echo "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$LINKER"
  echo "CC_aarch64_linux_android=$LINKER"
  echo "CXX_aarch64_linux_android=$CXX"
  echo "AR_aarch64_linux_android=$AR"
  echo "RANLIB_aarch64_linux_android=$RANLIB"
  echo "RUSTFLAGS=-C link-arg=$BUILTINS"
} >> "$GITHUB_ENV"
# Create prefixed symlinks that vendored OpenSSL expects.
ln -sf "$RANLIB" "$TOOLCHAIN_BIN/aarch64-linux-android-ranlib"
ln -sf "$AR" "$TOOLCHAIN_BIN/aarch64-linux-android-ar"
echo "$TOOLCHAIN_BIN" >> "$GITHUB_PATH"
