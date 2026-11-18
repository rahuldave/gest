---
title: Installation
description: Install gest, the agentic workflow CLI for parallel AI agents, with the install script or from source, on macOS or Linux.
---

## Quick install (macOS and Linux)

The install script is the main way to get gest. It needs no Rust. It downloads the pre-built binary
for your platform from the GitHub release, checks its SHA-256 hash, and installs it:

```sh
curl -fsSL https://raw.githubusercontent.com/rahuldave/gest/main/scripts/install.sh | sh
```

The script installs `gest` in `~/.local/bin`. Make sure that this directory is on your `PATH`:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

To install a given release, or to install into another directory, pass `--tag` and `--dir`:

```sh
curl -fsSL https://raw.githubusercontent.com/rahuldave/gest/main/scripts/install.sh \
  | sh -s -- --tag v0.6.0 --dir /usr/local/bin
```

The script stops with an error on a platform that has no release, and when the hash does not match.
To upgrade, run it again.

## Build from source

With a Rust toolchain, build a tagged release from the repository:

```sh
cargo install --locked --git https://github.com/rahuldave/gest --tag v0.6.0
```

The `gest` crate on crates.io is a different program, so `cargo install gest` does not install this
one.

## Download from GitHub Releases

Each release has pre-built binaries for macOS (Apple Silicon and Intel) and Linux (x86_64 and
aarch64). The Linux binaries are static, so they run on any distribution. Each archive has a
`.sha256` file beside it. Download the archive for your platform from the
[Releases page](https://github.com/rahuldave/gest/releases). Then extract it, and move the binary to
a directory on your `PATH`:

```sh
# Example for macOS Apple Silicon
tar xzf gest-aarch64-apple-darwin.tar.gz
mv gest ~/.local/bin/
```

## Verify the installation {#verify-installation}

After the installation, check that gest runs:

```sh
gest --version
```

It prints one line, such as `gest 0.6.0 (rahuldave)`. `gest version` also shows the platform and the
build revision.
