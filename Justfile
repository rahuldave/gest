set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# The nightly that formats and lints, the same as NIGHTLY in .github/workflows/build.yml
nightly := "nightly-2026-11-12"

default: verify

setup:
    cargo fetch

fmt:
    cargo +{{nightly}} fmt --all -- --config-path .config/rustfmt.toml

fmt-check:
    cargo +{{nightly}} fmt --all --check -- --config-path .config/rustfmt.toml

lint:
    cargo +{{nightly}} clippy --all-targets --all-features -- -D warnings

typecheck:
    cargo check --workspace --all-targets --all-features

build:
    cargo build

test:
    cargo test --all-targets --all-features

smoke:
    cargo run -- --version

diff-check:
    git diff --check

verify: fmt-check lint test smoke diff-check

# Every check of the repository: the release version, then verify
check: release-check verify

# The release tags. scripts/release_tag.py is a copy of .agents/skills/gpa/scripts/release_tag.py of
# agent_gest_git_skills at the commit 799a44b. The job release of build.yml runs just release on each push to main.
# The script runs just <profile>::version for each name of the key profiles of .agents/stack.json, here rust, so the
# tag is v plus the version of Cargo.toml.
release_tag := "uv run --no-project --quiet --no-python-downloads --python '>=3.9' python scripts/release_tag.py"

# rust::version prints the version of Cargo.toml. The rust profile of jnterfaces gives the same recipe. When gest
# installs that profile, take out this line and scripts/rust.just, because just stops on a module defined twice
mod rust 'scripts/rust.just'

# Tag HEAD with v plus the version of Cargo.toml, push the tag and check it. Takes --dry-run first
[positional-arguments]
release *args:
    @{{release_tag}} "$@"

# Fail when the version of Cargo.toml is below the last tag or the bump that the commits ask for. It needs no network
release-check:
    {{release_tag}} --check

# Build the release binary for TARGET, a Rust target triple, and pack it as dist/gest-TARGET.tar.gz with gest at its
# top level, plus dist/gest-TARGET.tar.gz.sha256. The job binaries of build.yml runs it, and scripts/install.sh
# installs it
dist target:
    rustup target add {{target}}
    cargo build --release --locked --target {{target}}
    mkdir -p dist
    COPYFILE_DISABLE=1 tar -czf dist/gest-{{target}}.tar.gz -C target/{{target}}/release gest
    cd dist && shasum -a 256 gest-{{target}}.tar.gz > gest-{{target}}.tar.gz.sha256
