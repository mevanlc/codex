# Checking out and building Chromium on Linux

To check out the source code locally, do not use `git clone`.

## Contents

- [System requirements](#system-requirements)
- [Install depot_tools](#install-depot_tools)
- [Get the code](#get-the-code)
- [Install additional build dependencies](#install-additional-build-dependencies)
- [Run the hooks](#run-the-hooks)
- [Speed up Git operations](#speed-up-git-operations)
- [Setting up the build](#setting-up-the-build)
- [Faster builds](#faster-builds)
- [Smaller builds](#smaller-builds)
- [Build Chromium](#build-chromium)
- [Run Chromium](#run-chromium)
- [Running test targets](#running-test-targets)
- [Update your checkout](#update-your-checkout)
- [Tips, tricks, and troubleshooting](#tips-tricks-and-troubleshooting)
- [More links](#more-links)
- [Next steps](#next-steps)
- [Notes for other distros](#notes-for-other-distros)
- [Docker](#docker)

## System requirements

- An x86-64 machine with at least 8GB of RAM. More than 16GB is highly recommended.
- If your machine has an SSD, it is recommended to have `>=32GB` / `>=16GB` of swap for machines with 8GB / 16GB of RAM respectively.
- At least 100GB of free disk space (does not have to be on the same drive). Allocate about 50-80GB on HDD for builds.
- You must have Git and Python v3.9+ installed already (and `python3` must point to a Python v3.9+ binary).
- `depot_tools` bundles an appropriate Python in `$depot_tools/python-bin` if needed.
- Chromium's build infrastructure and `depot_tools` currently use Python 3.11.
- `libc++` is currently the only supported STL.
- `clang` is the only officially supported compiler (community members generally keep gcc building too).
- Most development is done on Ubuntu (build infrastructure currently runs 22.04 / Jammy Jellyfish).

## Install depot_tools

Clone the `depot_tools` repository:

```bash
git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
```

Add `depot_tools` to the beginning of your `PATH`:

```bash
export PATH="/path/to/depot_tools:$PATH"
```

If cloning to your home directory, do not use `~` in `PATH` (it can break `gclient runhooks`):

```bash
export PATH="${HOME}/depot_tools:$PATH"
```

## Get the code

Create a Chromium directory for checkout and change to it:

```bash
mkdir ~/chromium && cd ~/chromium
```

Run `fetch` from `depot_tools`:

```bash
fetch --nohooks chromium
```

NixOS users: tools like `fetch` will not work without a Nix shell. Clone the tools repo with git, then run `nix-shell tools/nix/shell.nix`.

If you do not want full history, add `--no-history` to `fetch`.

Expect this to take 30 minutes on fast connections and much longer on slow ones.

If build dependencies are already installed on the machine, you can omit `--nohooks` and `fetch` will run `gclient runhooks` automatically.

When `fetch` completes, switch into `src`:

```bash
cd src
```

## Install additional build dependencies

On Ubuntu:

```bash
./build/install-build-deps.sh
```

You may need distro-specific dependency adjustments.

## Run the hooks

Once `install-build-deps` has run at least once:

```bash
gclient runhooks
```

Optional: install API keys if you want your build to talk to some Google services.

## Speed up Git operations

Enable `fsmonitor` and `untrackedCache` in the Chromium repo:

```bash
# If this fails, install watchman:
# https://facebook.github.io/watchman/docs/install
which watchman

cd ~/chromium/src

# Copy executable
cp .git/hooks/fsmonitor-watchman.sample ~/bin/query-watchman

# Enable optimization
git config core.untrackedCache true
git config core.fsmonitor "$HOME/bin/query-watchman"

# Let watchman ignore out. You should gitignore .watchmanconfig globally.
echo '{ "ignore_dirs": ["out"] }' > .watchmanconfig

# Increase inotify parameters if needed:
sudo vim /etc/sysctl.d/99-inotify.conf
```

Add the following in `99-inotify.conf`:

```conf
fs.inotify.max_user_instances = 8192
fs.inotify.max_user_watches = 10485760
```

Apply the change:

```bash
sudo sysctl --system
```

Optional faster add alias for your shell config:

```bash
alias gaa="git status --porcelain | awk '{print \$2}' | xargs -r git add"
```

## Setting up the build

Chromium uses Siso and GN. Create a build directory:

```bash
gn gen out/Default
```

You only need to run this once per build directory. Use any name under `out/`.

For more info:

```bash
gn help
```

## Faster builds

### Use remote execution

Chromium builds can be significantly faster with an REAPI-compatible backend.

### Google RBE

If using Chromium's Google RBE service, first authenticate:

```bash
siso login
```

If OAuth2 is blocked:

```bash
gcloud auth login
export SISO_CREDENTIAL_HELPER=gcloud
```

Set `rbe_instance` in `.gclient`:

```python
solutions = [
  {
    ...,
    "custom_vars": {
      # Correct instance for Chromium contributor Google RBE.
      # If using your own backend, set reapi_address, reapi_instance,
      # and reapi_backend_config_path instead.
      "rbe_instance": "projects/rbe-chromium-untrusted/instances/default_instance",
    },
  },
]
```

Then sync:

```bash
gclient sync
```

### Non Google RBE

For other compatible backends, configure auth according to backend requirements:

- No auth / closed network: set `RBE_service_no_security=true`
- mTLS: set `RBE_tls_client_auth_key` and `RBE_tls_client_auth_cert`
- Google OAuth2: use `gcloud auth login` and `SISO_CREDENTIAL_HELPER=gcloud`
- Custom helper: set `SISO_CREDENTIAL_HELPER=/path/to/your/credhelper`

Example `backend.star`:

```python
load("@builtin//struct.star", "module")

def __platform_properties(ctx):
    # This image is created by:
    # https://chromium.googlesource.com/infra/infra/+/refs/heads/main/rbe/images/siso-chromium/linux/Dockerfile
    container_image = "docker://gcr.io/chops-public-images-prod/rbe/siso-chromium/linux@sha256:d7cb1ab14a0f20aa669c23f22c15a9dead761dcac19f43985bf9dd5f41fbef3a"
    return {
        "default": {
            "OSFamily": "Linux",
            "container-image": container_image,
            # "Pool": "linux_x64",
        },
        # No large workers. Empty platform properties run locally.
        "large": {},
    }

backend = module(
    "backend",
    platform_properties = __platform_properties,
)
```

Set backend vars in `.gclient`:

```python
solutions = [
  {
    "custom_vars": {
      "reapi_instance": "default",  # your instance
      "reapi_address": "remotebuild.example.com:443",  # your backend
      "reapi_backend_config_path": "/path/to/your/backend.star",
    },
  }
]
```

Then sync:

```bash
gclient sync
```

Optional Siso flags (`build/config/siso/.sisorc`):

```bash
ninja --reapi_grpc_conn_pool=1 --reapi_keep_exec_stream
```

### GN setup for remote execution

Add to `args.gn`:

```gn
use_remoteexec = true
use_siso = true
```

If `args.gn` contains `use_reclient=true`, drop it or set `use_reclient=false`.

Always use `autoninja` for Chromium builds.

### Include fewer debug symbols

By default GN uses `is_debug=true` and `symbol_level=2`.

- `symbol_level=1`: enough for stack traces.
- `symbol_level=0`: no debug symbols.

### Disable debug symbols for Blink and v8

```gn
blink_symbol_level=0
v8_symbol_level=0
```

### Use Icecc

Icecc can help distributed builds (not useful with Siso). Set:

```gn
use_debug_fission=false
is_clang=false
```

### ccache

You can use `ccache` for local build speedups (again, not useful with Siso).

Suggested shell config:

```bash
alias cd="cd -P"
```

This helps keep `$PWD` physical, not logical.

### Using tmpfs

Mount `tmpfs` for build output:

```bash
mount -t tmpfs -o size=20G,nr_inodes=40k,mode=1777 tmpfs /path/to/out
```

Caveat: you need enough RAM + swap (about 20GB for full debug build).

Quick benchmark on HP Z600 (Intel i7, 16 hyperthreaded cores, 12GB RAM):

- With tmpfs: `12m:20s`
- Without tmpfs: `15m:40s`

## Smaller builds

You can reduce Chrome binary size by stripping embedded symbols and using:

```gn
is_official_build = true
```

## Build Chromium

Build `chrome` target:

```bash
autoninja -C out/Default chrome
```

`autoninja` automatically provides optimal arguments to Siso/Ninja.

List available targets:

```bash
gn ls out/Default
```

Example target build:

```bash
autoninja -C out/Default chrome/test:unit_tests
```

## Run Chromium

```bash
out/Default/chrome
```

For Chrome Remote Desktop environments, add to shell profile:

```bash
if [[ -z "${DISPLAY}" ]]; then
  # Chrome Remote Desktop starts from display :20 and increments.
  export DISPLAY=:20
fi
```

## Running test targets

Find test target from test file:

```bash
gn refs out/Default --testonly=true --type=executable --all chrome/browser/ui/browser_unittest.cc
```

Example output:

```text
//chrome/test:unit_tests
```

Build target:

```bash
autoninja -C out/Default unit_tests
```

Run with optional filter:

```bash
out/Default/unit_tests --gtest_filter="BrowserListUnitTest.*"
```

## Update your checkout

```bash
git rebase-update
gclient sync
```

- `git rebase-update` updates Chromium source and rebases local branches on `origin/main`.
- `gclient sync` updates dependencies and reruns hooks as needed.

## Tips, tricks, and troubleshooting

### Linker crashes

If during link stage:

```text
LINK out/Debug/chrome
```

You may see:

```text
collect2: ld terminated with signal 6 Aborted terminate called after throwing an instance of 'std::bad_alloc'
collect2: ld terminated with signal 11 [Segmentation fault], core dumped
```

Or:

```text
LLVM ERROR: out of memory
```

This usually means out-of-memory during linking. Try:

- `is_debug = false`
- `symbol_level = 0`
- `is_component_build = true` (dev only; slower and may be less stable)

For official ThinLTO Linux builds, increase kernel `vm.max_map_count`:

```bash
sudo sysctl -w vm.max_map_count=262144
```

Persist in `/etc/sysctl.conf`:

```conf
vm.max_map_count=262144
```

## More links

- Information about building with Clang.
- Use a chroot to isolate versioning/packaging conflicts.
- Cross-compiling for ARM: `LinuxChromiumArm`.
- Eclipse IDE support: `LinuxEclipseDev`.
- Use built version as default browser: `LinuxDevBuildAsDefaultBrowser`.

## Next steps

If you want to contribute to Chromium on Linux, see the Linux Development page.

## Notes for other distros

### Arch Linux

```bash
sudo pacman -S --needed python perl gcc gcc-libs bison flex gperf pkgconfig \
  nss alsa-lib glib2 gtk3 nspr freetype2 cairo dbus xorg-server-xvfb \
  xorg-xdpyinfo
```

Optional package notes:

- `php-cgi` is provided with `pacman`.
- `wdiff` is not in the main repo; `dwdiff` is. `wdiff` is available in AUR/yaourt.

### Crostini (Debian based)

```bash
sudo apt-get install file lsb-release
sudo install-build-deps.sh --no-arm
```

### Fedora

```bash
su -c 'yum install git python bzip2 tar pkgconfig atk-devel alsa-lib-devel \
  bison binutils brlapi-devel bluez-libs-devel bzip2-devel cairo-devel \
  cups-devel dbus-devel dbus-glib-devel expat-devel fontconfig-devel \
  freetype-devel gcc-c++ glib2-devel glibc.i686 gperf glib2-devel \
  gtk3-devel java-1.*.0-openjdk-devel libatomic libcap-devel libffi-devel \
  libgcc.i686 libjpeg-devel libstdc++.i686 libX11-devel libXScrnSaver-devel \
  libXtst-devel libxkbcommon-x11-devel ncurses-compat-libs nspr-devel nss-devel \
  pam-devel pango-devel pciutils-devel pulseaudio-libs-devel zlib.i686 httpd \
  mod_ssl php php-cli python-psutil wdiff xorg-x11-server-Xvfb'
```

Optional package notes:

- `php-cgi` is provided by `php-cli`.
- `sun-java6-fonts` is covered by the linked font instructions.

### Gentoo

```bash
emerge www-client/chromium
```

### NixOS

Get a shell with the dev environment:

```bash
nix-shell tools/nix/shell.nix
```

Run a command in the dev environment:

```bash
NIX_SHELL_RUN='autoninja -C out/Default chrome' nix-shell tools/nix/shell.nix
```

Find clangd path for editor config:

```bash
NIX_SHELL_RUN='readlink /usr/bin/clangd' nix-shell tools/nix/shell.nix
```

### OpenSUSE

OpenSUSE 11.1+:

```bash
sudo zypper in subversion pkg-config python perl bison flex gperf \
  mozilla-nss-devel glib2-devel gtk-devel wdiff lighttpd gcc gcc-c++ \
  mozilla-nspr mozilla-nspr-devel php5-fastcgi alsa-devel libexpat-devel \
  libjpeg-devel libbz2-devel
```

OpenSUSE 11.0:

```bash
sudo zypper in subversion pkg-config python perl \
  bison flex gperf mozilla-nss-devel glib2-devel gtk-devel \
  libnspr4-0d libnspr4-dev wdiff lighttpd gcc gcc-c++ libexpat-devel \
  php5-cgi alsa-devel gtk3-devel jpeg-devel
```

Install Java/font dependencies:

```bash
sudo zypper in java-1_6_0-sun
sudo zypper in fetchmsttfonts pullin-msttf-fonts
```

Create Ubuntu-compatible symlinks:

```bash
sudo mkdir -p /usr/share/fonts/truetype/msttcorefonts
sudo ln -s /usr/share/fonts/truetype/arial.ttf /usr/share/fonts/truetype/msttcorefonts/Arial.ttf
sudo ln -s /usr/share/fonts/truetype/arialbd.ttf /usr/share/fonts/truetype/msttcorefonts/Arial_Bold.ttf
sudo ln -s /usr/share/fonts/truetype/arialbi.ttf /usr/share/fonts/truetype/msttcorefonts/Arial_Bold_Italic.ttf
sudo ln -s /usr/share/fonts/truetype/ariali.ttf /usr/share/fonts/truetype/msttcorefonts/Arial_Italic.ttf
sudo ln -s /usr/share/fonts/truetype/comic.ttf /usr/share/fonts/truetype/msttcorefonts/Comic_Sans_MS.ttf
sudo ln -s /usr/share/fonts/truetype/comicbd.ttf /usr/share/fonts/truetype/msttcorefonts/Comic_Sans_MS_Bold.ttf
sudo ln -s /usr/share/fonts/truetype/cour.ttf /usr/share/fonts/truetype/msttcorefonts/Courier_New.ttf
sudo ln -s /usr/share/fonts/truetype/courbd.ttf /usr/share/fonts/truetype/msttcorefonts/Courier_New_Bold.ttf
sudo ln -s /usr/share/fonts/truetype/courbi.ttf /usr/share/fonts/truetype/msttcorefonts/Courier_New_Bold_Italic.ttf
sudo ln -s /usr/share/fonts/truetype/couri.ttf /usr/share/fonts/truetype/msttcorefonts/Courier_New_Italic.ttf
sudo ln -s /usr/share/fonts/truetype/impact.ttf /usr/share/fonts/truetype/msttcorefonts/Impact.ttf
sudo ln -s /usr/share/fonts/truetype/times.ttf /usr/share/fonts/truetype/msttcorefonts/Times_New_Roman.ttf
sudo ln -s /usr/share/fonts/truetype/timesbd.ttf /usr/share/fonts/truetype/msttcorefonts/Times_New_Roman_Bold.ttf
sudo ln -s /usr/share/fonts/truetype/timesbi.ttf /usr/share/fonts/truetype/msttcorefonts/Times_New_Roman_Bold_Italic.ttf
sudo ln -s /usr/share/fonts/truetype/timesi.ttf /usr/share/fonts/truetype/msttcorefonts/Times_New_Roman_Italic.ttf
sudo ln -s /usr/share/fonts/truetype/verdana.ttf /usr/share/fonts/truetype/msttcorefonts/Verdana.ttf
sudo ln -s /usr/share/fonts/truetype/verdanab.ttf /usr/share/fonts/truetype/msttcorefonts/Verdana_Bold.ttf
sudo ln -s /usr/share/fonts/truetype/verdanai.ttf /usr/share/fonts/truetype/msttcorefonts/Verdana_Italic.ttf
sudo ln -s /usr/share/fonts/truetype/verdanaz.ttf /usr/share/fonts/truetype/msttcorefonts/Verdana_Bold_Italic.ttf
```

Java fonts:

```bash
sudo mkdir -p /usr/share/fonts/truetype/ttf-lucida
sudo find /usr/lib*/jvm/java-1.6.*-sun-*/jre/lib -iname '*.ttf' -print \
  -exec ln -s {} /usr/share/fonts/truetype/ttf-lucida \;
```

## Docker

### Prerequisites

While uncommon, Chromium compilation can work in Docker. Ensure these tools exist in the container:

- `curl`
- `git`
- `lsb_release`
- `python3`
- `sudo`
- `file`

There may be Docker-specific issues; see the referenced Chromium bug. Clone `depot_tools` first.

### Build steps

Put the following `Dockerfile` in `/path/to/chromium/`:

```Dockerfile
# Use an official Ubuntu base image with Docker already installed
FROM ubuntu:22.04

# Set environment variables
ENV DEBIAN_FRONTEND=noninteractive

# Install mandatory tools (curl git python3) and optional tools (vim sudo)
RUN apt-get update && \
    apt-get install -y curl git lsb-release python3 git file vim sudo && \
    rm -rf /var/lib/apt/lists/*

# Export depot_tools path
ENV PATH="/depot_tools:${PATH}"

# Configure git for safe.directory
RUN git config --global --add safe.directory /depot_tools && \
    git config --global --add safe.directory /chromium/src

# Set the working directory to the existing Chromium source directory.
# This can be either "/chromium/src" or "/chromium".
WORKDIR /chromium/src

# Expose any necessary ports (if needed)
# EXPOSE 8080

# Create a dummy user and group to avoid permission issues
RUN groupadd -g 1001 chrom-d && \
    useradd -u 1000 -g 1001 -m chrom-d

# Create normal user with name "chrom-d". Optional (you can use root, not advised).
USER chrom-d

# Start Chromium Builder "chrom-d" (modify this command as needed)
# CMD ["autoninja -C out/Default chrome"]
CMD ["bash"]
```

Build container:

```bash
# chrom-b is just a name; if changed, update all commands accordingly.
docker build -t chrom-b .
```

Run container as root to install dependencies:

```bash
docker run \
  -it \
  --name chrom-b \
  -u root \
  -v /path/on/machine/to/chromium:/chromium \
  -v /path/on/machine/to/depot_tools:/depot_tools \
  chrom-b
```

Install dependencies:

```bash
./build/install-build-deps.sh
```

Before running hooks in container, add `third_party` directories as safe Git dirs:

```bash
for dir in /chromium/src/third_party/*; do
  if [ -d "$dir" ]; then
    git config --global --add safe.directory "$dir"
  fi
done
```

Save container image as `dpv1.0` (run on host machine, not in container):

```bash
# Get container id for chrom-b
docker container ls -a

# Commit/tag container
docker commit <ID from above step> chrom-b:dpv1.0

# Optional cleanup
docker image rmi chrom-b:latest && docker image prune \
  && docker container prune && docker builder prune
```

Run tagged container as non-root user matching host UID/GID:

```bash
docker run --rm \
  -it \
  --name chrom-b \
  -u "$(id -u):$(id -g)" \
  -v /path/on/machine/to/chromium:/chromium \
  -v /path/on/machine/to/depot_tools:/depot_tools \
  chrom-b:dpv1.0
```
