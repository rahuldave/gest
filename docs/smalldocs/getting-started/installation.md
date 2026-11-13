---
title: Installation
description: Install gest, the agentic workflow CLI for parallel AI agents, with the install script, Homebrew, Cargo, or a precompiled binary on macOS, Linux, or Windows.
---

## Quick install (macOS and Linux)

The install script is the fastest way to get gest. It downloads a pre-built binary for your
platform:

```sh
curl -fsSL https://gest.aaronmallen.dev/install | sh
```

The script installs `gest` in `~/.local/bin`. Make sure that this directory is on your `PATH`:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

To install a specific version, or to install into another directory, set these variables:

```sh
GEST_VERSION=x.y.z GEST_INSTALL_PATH=/usr/local/bin \
  curl -fsSL https://gest.aaronmallen.dev/install | sh
```

## Install via Cargo

If you already have a Rust toolchain, install gest from [crates.io](https://crates.io/crates/gest):

```sh
cargo install gest
```

Or use [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) to get a pre-built binary:

```sh
cargo binstall gest
```

## Download from GitHub Releases

Each release has pre-built binaries for macOS (Apple Silicon and Intel) and Linux (x86_64 and
aarch64). Download the archive for your platform from the
[Releases page](https://github.com/aaronmallen/gest/releases). Extract it, and move the binary to a
directory on your `PATH`:

```sh
# Example for macOS Apple Silicon
tar xzf gest-vX.Y.Z-aarch64-apple-darwin.tar.gz
mv gest ~/.local/bin/
```

## Verify the installation {#verify-installation}

After the installation, check that gest runs:

```sh
gest version
```

The output shows the installed version, the platform and the build revision.

The `version` command also checks for updates. To update an existing installation, run:

```sh
gest self-update
```
