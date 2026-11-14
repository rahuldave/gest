# Development Commands

This page lists the commands that build, test, format and lint Gest. Run them from the root of the repository. CI runs
the same commands in `.github/workflows/build.yml` and `.github/workflows/security.yml`.

## What the machine needs

The commands need these programs:

| Program | Install |
| --- | --- |
| `rustup` and `cargo` | <https://rustup.rs/> |
| The nightly toolchain, for rustfmt and clippy | `rustup toolchain install nightly-2026-11-12 --component rustfmt,clippy` |
| cargo-sort and taplo | `cargo install --locked cargo-sort@2.1.4 taplo-cli@0.10.0` |
| cargo-llvm-cov and cargo-nextest | `cargo install --locked cargo-llvm-cov@0.9.1 cargo-nextest@0.9.151` |
| cargo-audit | `cargo install --locked cargo-audit@0.23.0` |
| editorconfig-checker 4.0.2 | A release binary from <https://github.com/editorconfig-checker/editorconfig-checker/releases> |
| markdownlint-cli2 0.24.1 | `npx --yes markdownlint-cli2@0.24.1`, with Node |
| Node, for the test of `templates/forms.js` | <https://nodejs.org/> |
| yamlfmt v0.21.0 | `go install github.com/google/yamlfmt/cmd/yamlfmt@v0.21.0`, with Go |

The versions are the versions that CI installs. The workflows name them, so change both places together.

## Build and check

```bash
cargo build --workspace --all-features
cargo check --workspace --all-targets --all-features
```

## Test

```bash
cargo test --all-targets --all-features    # Every test
cargo test <test-name>                     # The tests whose names contain <test-name>
```

CI runs the tests with coverage, and nextest runs each test in its own process:

```bash
cargo llvm-cov nextest --bin gest --summary-only --all-features --no-tests pass      # Unit tests
cargo llvm-cov nextest --test '*' --summary-only --all-features --no-tests pass      # Integration tests
```

The Markdown preview in `templates/forms.js` has a test that runs with Node, without a browser:

```bash
node tests/js/forms_preview.js templates/forms.js
```

## Format

```bash
cargo sort --workspace --config .config/tomlfmt.toml
cargo +nightly-2026-11-12 fmt --all -- --config-path .config/rustfmt.toml
taplo format --config .config/taplo.toml
yamlfmt -conf .config/.yamlfmt .
```

Two lint tools can also fix what they find:

```bash
cargo +nightly-2026-11-12 clippy --fix --allow-dirty --allow-staged
npx --yes markdownlint-cli2@0.24.1 --fix --config .config/.markdownlint.yml "*.md" "**/*.md" "#.agents" "#target"
```

## Lint

```bash
cargo sort --workspace --check --config .config/tomlfmt.toml
cargo +nightly-2026-11-12 fmt --all --check -- --config-path .config/rustfmt.toml
cargo +nightly-2026-11-12 clippy --all-targets --all-features -- -D warnings
editorconfig-checker -config .config/editorconfig-checker.json
npx --yes markdownlint-cli2@0.24.1 --config .config/.markdownlint.yml "*.md" "**/*.md" "#.agents" "#.cargo" \
  "#coverage" "#.github/PULL_REQUEST_TEMPLATE.md" "#target" "#tmp" "#**/node_modules"
taplo lint --config .config/taplo.toml
yamlfmt -lint -conf .config/.yamlfmt .
```

## Audit

```bash
cargo audit
```

## Documentation site

The site is a smalldocs project in `docs/smalldocs`. Its Rust reference needs the nightly toolchain above.

```bash
uvx smalldocs==0.5.11 preview docs/smalldocs
```
