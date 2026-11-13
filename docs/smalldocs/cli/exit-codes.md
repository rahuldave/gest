---
title: Exit Codes
description: The BSD sysexits.h contract that gest CLI commands use so scripts and agents can branch on error category via $? alone.
---

Every `gest` command returns a process exit code from the BSD
[`sysexits.h`](https://man.openbsd.org/sysexits.3) list. A script or an agent can read the kind of
failure from `$?` alone, and it does not have to parse stderr.

The page of each command lists the codes that the command can return. This page defines each code and
its meaning.

## Contract

| Code | Name             | Meaning                                          |
|------|------------------|--------------------------------------------------|
| 0    | none             | Success                                          |
| 64   | `EX_USAGE`       | Command-line usage error                         |
| 65   | `EX_DATAERR`     | User data format error                           |
| 66   | `EX_NOINPUT`     | Input entity not found                           |
| 69   | `EX_UNAVAILABLE` | Resource not in the required state               |
| 70   | `EX_SOFTWARE`    | Internal software error                          |
| 74   | `EX_IOERR`       | Filesystem or database I/O error                 |
| 75   | `EX_TEMPFAIL`    | Try again later (valid empty result)             |
| 78   | `EX_CONFIG`      | Configuration or setup error                     |

Source of truth: [ADR-0017](https://github.com/aaronmallen/gest/blob/main/docs/design/0017-exit-code-contract.md).

## Code reference

### 0: Success {#success}

The command did what you asked. Gest applied the changes and printed the output, and no error
occurred.

### 64: `EX_USAGE` {#ex_usage}

Gest parsed the command line, but an argument is missing, malformed or in conflict with another flag.
Typical causes:

- `meta` ran with no subcommand and no ID.
- An ID prefix holds characters that no ID can have.
- A record of NDJSON batch input is not valid JSON.
- A relationship spec such as `<rel>:<target>` names an unknown relationship.
- `iteration next` got `--agent` without `--claim`.

The parser of the command line rejects some errors before gest runs, for example an unknown flag, a
missing required argument or an unknown subcommand. These errors exit with code `2`, the default of the
parser, and not with `EX_USAGE`. Code `2` is outside the sysexits contract.

### 65: `EX_DATAERR` {#ex_dataerr}

Gest could not convert structured data to or from JSON, TOML or YAML. Typical causes:

- `meta set --as-json` got a value that is not valid JSON.
- A value fails to convert to JSON for `--json` output, or to TOML when gest writes the config file.
- The store cannot convert a JSON or YAML record.

An invalid configuration file gives `EX_CONFIG` (78), and not `EX_DATAERR`. A failure to decode a
database column, for example a malformed ID, gives `EX_IOERR` (74).

### 66: `EX_NOINPUT` {#ex_noinput}

Gest could not find a named entity. Typical causes:

- An ID or a prefix matches no entity in the store.
- An ID prefix matches more than one entity.
- `gest migrate --from v0.4 --path <dir>` names a directory that does not exist.
- `iteration next <id>` names an iteration that does not exist.
- `project detach` runs in a directory that has no attached workspace.
- `meta get <id> <key>` or `meta unset <id> <key>` names a key that the entity does not have.

`config get` with an unknown key is not an error. It prints `(not set)` and exits with 0.

### 69: `EX_UNAVAILABLE` {#ex_unavailable}

The entity exists, but its state does not permit the change. Typical causes:

- `iteration advance` runs on an iteration that is completed, cancelled or has no tasks. It also fails
  when the current phase still has a task that is not done or cancelled, unless you pass `--force`.
- `iteration next` runs on an iteration that is not active.
- `task delete <id>` names a task that still belongs to one or more iterations. Pass `--force` to
  remove it from the iterations and delete it.
- `undo` finds no transaction to undo in the current project.

### 70: `EX_SOFTWARE` {#ex_software}

The code is for failures of the editor on commands that open `$EDITOR`. Gest runs `$EDITOR`, then
`$VISUAL`, then `vi`. Typical causes:

- The editor could not start, or it exited with a code other than 0.
- The editor saved an empty body where gest needs content.
- `artifact create --source` could not read its file.

### 74: `EX_IOERR` {#ex_ioerr}

A filesystem or database operation failed. Typical causes:

- Gest cannot open, read, write or lock the store (`gest.db`).
- Gest cannot read `stdin`, or write to `stdout` or `stderr`.
- `self-update` cannot download or install the new binary.
- `migrate` cannot read the legacy `.gest/` tree.
- The database rejects a write, for example for a UNIQUE constraint.
- `project attach` gets an ID that is malformed or names no project.

### 75: `EX_TEMPFAIL` {#ex_tempfail}

The code means a valid empty result, and not an error. Only one command returns it:

- `iteration next`, when the iteration has no open task that is free of blockers.

An orchestrator loop tests `$? -eq 75` to choose between advancing the iteration and waiting.

### 78: `EX_CONFIG` {#ex_config}

Gest found a problem in its configuration or setup. Typical causes:

- Gest could not load the user config file, or the file is invalid.
- A command that needs a project ran in a directory with no `gest.db` and no attached parent
  workspace. Run `gest init` or `gest project attach <id>`.
- Gest could not find the XDG base directories, for example because `$HOME` is not set.

## Scripting patterns

### Find out if there is work to do {#is-there-work-to-do}

Use `iteration next` as an attempt to claim a task, and test for 75:

```sh
gest iteration next "$ITER_ID" --claim --agent worker-1 --json
case $? in
  0)  echo "claimed a task" ;;
  75) echo "no work available -- try advancing or idling" ;;
  *)  echo "error" >&2; exit 1 ;;
esac
```

### Tell missing input from a worse failure {#did-that-command-fail-because-of-missing-input-or-something-worse}

```sh
gest task show "$id" --json
case $? in
  0)  : ;;
  66) echo "no such task: $id" ;;
  *)  echo "unexpected error" >&2 ;;
esac
```

### Find out if the directory has a project {#is-this-directory-initialized}

Save the exit code before you test it. Each command, `!` included, replaces `$?`.

```sh
gest task list >/dev/null 2>&1
if [ $? -eq 78 ]; then
  gest init
fi
```
