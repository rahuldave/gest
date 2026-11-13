---
title: gest purge
---

Delete terminal, archived and orphaned data from the store. A terminal task is done or cancelled, and
a terminal iteration is completed or cancelled.

By default, the command works on the current project. Pass `--all-projects` to purge the whole store.
With no selector flag, the command applies all the selectors, and the confirmation prompt protects you.

## Usage

```text
gest purge [OPTIONS]
```

## Selectors

Each selector flag adds one kind of data to the purge. With no selector flag, the command purges all
kinds.

| Flag              | Description                                      |
|-------------------|--------------------------------------------------|
| `--tasks`         | Purge terminal tasks (done/cancelled)            |
| `--iterations`    | Purge terminal iterations (completed/cancelled)  |
| `--artifacts`     | Purge archived artifacts                         |
| `--projects`      | Purge archived projects (non-undoable)           |
| `--relationships` | Purge dangling relationship rows                 |
| `--tombstones`    | Purge orphan tombstone files                     |

## Options

| Flag              | Description                                                  |
|-------------------|--------------------------------------------------------------|
| `--all-projects`  | Operate across every project instead of just the current one |
| `--dry-run`       | Show what would be purged without making any changes         |
| `--yes`           | Skip the interactive confirmation prompt                     |

## Examples

Preview what would be purged in the current project:

```sh
gest purge --dry-run
```

Purge all categories with confirmation:

```sh
gest purge
```

Purge only terminal tasks and iterations, skipping the prompt:

```sh
gest purge --tasks --iterations --yes
```

Purge across all projects:

```sh
gest purge --all-projects
```

Purge only orphan tombstone files:

```sh
gest purge --tombstones --yes
```

## Exit codes

| Code | When                                                                    |
|------|-------------------------------------------------------------------------|
| 0    | Success (including dry-run with no-op result)                           |
| 2    | The parser rejected the command line, for example an unknown flag       |
| 66   | `--all-projects` ran outside a project, and the store has no project    |
| 74   | Store I/O error, or a tombstone file could not be removed               |
| 78   | Not a gest project (run `gest init`), without `--all-projects`          |

See [Exit Codes](./exit-codes.md) for the full contract.

## Notes

- `gest undo` restores purged tasks, iterations and artifacts.
- `gest undo` does not restore archived projects, dangling relationships or tombstone files. Gest
  deletes them outside the log of transactions.
- With no selector flag, the command applies all the selectors.
- With `--dry-run`, the command prints the summary and stops before the prompt, so it changes nothing.
