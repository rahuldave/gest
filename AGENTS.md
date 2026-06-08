# Agent Instructions

This repository is the local fork of Gest. Use the project-local Codex skill
family under `.agents/skills/`, especially `gtw`, for coding, debugging,
implementation, refactoring, documentation, verification, and project planning.

The user may invoke the router as `$gtw`, `gtw:`, or `/gtw`. Treat `/gtw` as a
natural-language prefix if it reaches the model.

If a request is substantial enough for Gest tracking but no `g*` command was
explicitly invoked, still use the appropriate Gest workflow. If an agent chooses
not to use Gest for a coding/debugging/refactoring/documentation/verification
request, it must say why in the final response.

## Project Context

- Project name: `gest`
- Main source directory: `src/`
- Integration tests: `tests/`
- Templates: `templates/`
- Documentation: `README.md`, `docs/`
- Reusable agent workflow source: `/Users/rahul/Projects/agent_gest_git_skills`

This is a Rust CLI project. Prefer native Cargo commands unless a future
Justfile defines a narrower command contract.

## Command Contract

Run commands from this repository root unless noted otherwise:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo test <test-name>
cargo run -- <gest-args>
git diff --check
```

For TDD work, add or update a focused failing test before changing production
code when the desired behavior can be expressed locally.

## Gest Workflow

Before creating new tasks, search and inspect existing work:

```bash
gest search "<project keyword>" --all --json
gest task list --all --json
gest iteration list --all --json
```

Use native Gest `child-of` / `parent-of` links for hierarchy. Tags are filters,
not hierarchy. Claim one leaf task at a time, verify before completion, and
keep long-lived outline parents open until the whole subtree is done.

For any Gest-tracked work that writes files, choose a VCS branch model and
execution model before editing. This fork uses normal Git unless GitButler is
explicitly detected. Branch names should be keyed to the highest meaningful
Gest task for the workstream, for example
`gest/<task-id-short>-two-word-summary`.

For non-trivial completed leaf tasks, add a Gest task note before completion:

```bash
gest task note add <task-id-or-prefix> --agent codex --body "Done: ...\nVerification: ...\nFollow-up: ..."
gest task complete <task-id-or-prefix> --quiet
```

Use task metadata for machine-queryable facts, not prose work logs.

## Current Fork Invariants

- Prefer project-local Gest state under `.gest/` when that avoids sandbox and
  multi-agent write issues.
- Preserve compatibility with existing global Gest state unless a task
  explicitly changes migration behavior.
- SQLite storage changes must document what happens to existing JSON sidecar
  files before implementation.
- SQLite connections used by the CLI should enable WAL mode when the database is
  file-backed so simultaneous readers are not blocked by a writer.
- Tests that change storage behavior should cover both default project-local
  behavior and any environment/config override paths.

## Commit And Review

After every code change, run an explicit review pass with `grv` or code-review
stance before completing the task. Treat missing focused tests for changed
callable code or CLI behavior as review findings.

Commit durable checkpoints when storage layout, migrations, public CLI behavior,
or reusable workflow material changes. After each Codex-created commit, run
`git status --short --branch` and push or report the exact blocker.
