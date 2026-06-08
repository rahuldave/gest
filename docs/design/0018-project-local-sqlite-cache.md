---
id: "0018"
title: "Project-local SQLite cache under .gest"
status: proposed
tags: [storage, architecture, sqlite, sync]
created: 2026-06-08
supersedes: ["0013"]
updates: ["0016"]
---

# ADR-0018: Project-local SQLite cache under .gest

## Problem Statement

Gest currently opens its default local SQLite database at
`<storage.data_dir>/gest.db`. In Codex and other sandboxed agent environments,
that default often resolves outside the writable workspace, so even read-looking
commands can fail when Gest imports sync files, records transactions, updates
digests, or opens SQLite sidecar files.

We want the normal project workflow to keep all mutable local Gest state inside
the project tree, while preserving the merge-friendly `.gest/` files introduced
by ADR-0016.

## Proposed Solution

For initialized projects that have a `.gest/` directory and no explicit
`database.url` or `storage.data_dir` override, Gest should open a file-backed
SQLite cache at:

```text
<project-root>/.gest/gest.db
```

The per-entity `.gest/` YAML and Markdown files remain the version-controlled
source of truth for shared project state. SQLite remains a derived local query
cache plus local operational store. Gest should continue to import from `.gest/`
on process start and export back to `.gest/` on process exit.

SQLite should run in WAL mode for file-backed local databases. This should apply
to both project-local databases and explicit local data-dir databases. WAL is not
applied to remote `database.url` connections.

## Scope

### In Scope

- Resolve project `.gest/` early enough to choose the default local database
  path before opening the store.
- Use `.gest/gest.db` as the default database for local initialized projects.
- Keep `database.url` as the highest-precedence database choice.
- Keep `storage.data_dir` as an explicit opt-out for users who still want a
  global or custom local database path.
- Enable `PRAGMA journal_mode = WAL` and `PRAGMA foreign_keys = ON` for
  file-backed local SQLite connections.
- Ignore SQLite cache sidecar files in Git:
  - `.gest/gest.db`
  - `.gest/gest.db-wal`
  - `.gest/gest.db-shm`
- Update tests and docs that currently assume `.gest-data/gest.db`.

### Out of Scope

- Removing the ADR-0016 per-entity YAML/Markdown mirror.
- Changing the public entity file schemas.
- Synchronizing undo history across collaborators.
- Migrating arbitrary global databases automatically into `.gest/gest.db`.
- Changing remote libsql/Turso behavior.

## JSON/YAML File Policy

Existing `.gest` entity files should not be deleted or replaced by SQLite. They
are the durable project representation and should continue to be committed.

The old shared-array JSON files described in older docs are already superseded
by ADR-0016. If encountered during migration or sync, they should be treated as
legacy input only. New writes should use the current per-entity YAML/Markdown
layout and should not recreate shared JSON aggregates.

SQLite cache files are local operational artifacts and should not be committed.

## Startup Semantics

The current startup sequence opens the store before resolving the project. The
new behavior needs a lightweight project-discovery step before `store::open`.

Recommended sequence:

1. Load config.
2. Discover the current project root and `.gest` directory from the filesystem.
3. Resolve the store path:
   - `database.url` configured: open the remote database.
   - `storage.data_dir` configured or `GEST_STORAGE__DATA_DIR` set: open
     `<data_dir>/gest.db`.
   - discovered `.gest` directory: open `<gest_dir>/gest.db`.
   - no project discovered: fall back to `<storage.data_dir>/gest.db` so
     bootstrap commands such as `gest init` still work.
4. Run migrations.
5. Resolve or create the project row.
6. Configure import/export sync when `.gest` exists and `storage.sync` is true.

`gest init --local` should create `.gest/` first, then ensure subsequent project
commands use `.gest/gest.db` by default.

## Acceptance Criteria

- A local initialized project with `.gest/` and no storage override creates and
  uses `.gest/gest.db`.
- Creating a task in such a project writes both `.gest/gest.db` and the
  per-entity `.gest/task/<id>.yaml` mirror.
- An explicit `GEST_STORAGE__DATA_DIR` or `[storage] data_dir = ...` keeps using
  `<data_dir>/gest.db`.
- Remote `database.url` keeps using the configured remote connection.
- File-backed SQLite databases report `journal_mode = wal`.
- Simultaneous reader connections can query while another connection holds a
  write transaction.
- Git does not report `.gest/gest.db`, `.gest/gest.db-wal`, or
  `.gest/gest.db-shm` as untracked project state.

## References

- ADR-0013: Global-Only Storage with Project Identity
- ADR-0016: Per-entity `.gest/` layout as live source of truth
- Gest task `mkqnnspqwoxyskotmxuwpvupysqtszoo`: Gest fork local workspace
  storage and agent workflow
