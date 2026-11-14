---
title: gest iteration
description: gest iteration — group tasks into phased execution plans for parallel or sequential runs across multiple AI agent workspaces.
---

An iteration is an execution plan that groups tasks into numbered phases. Agents work
through the phases in order, and the tasks of one phase can run in parallel. The
`gest iteration` commands create iterations, fill them with tasks and hand out the
next task.

## Usage

```text
gest iteration <COMMAND> [OPTIONS]
```

## Subcommands

| Command                           | Aliases | Description                                          |
|-----------------------------------|---------|------------------------------------------------------|
| [`add`](#iteration-add)           |         | Add a task to an iteration                           |
| [`advance`](#iteration-advance)   |         | Check the active phase and report the next phase     |
| [`cancel`](#iteration-cancel)     |         | Set an iteration to `cancelled`                      |
| [`complete`](#iteration-complete) |         | Set an iteration to `completed`                      |
| [`create`](#iteration-create)     | `new`   | Create a new iteration                               |
| [`delete`](#iteration-delete)     |         | Delete an iteration and drop its task memberships    |
| [`graph`](#iteration-graph)       |         | Display the phased execution graph                   |
| [`link`](#iteration-link)         |         | Create a relationship between entities               |
| [`unlink`](#iteration-unlink)     |         | Remove a relationship between entities               |
| [`list`](#iteration-list)         | `ls`    | List iterations with optional filters                |
| [`meta`](#iteration-meta)         |         | Read or write metadata fields                        |
| [`next`](#iteration-next)         |         | Find or claim the next available task                |
| [`remove`](#iteration-remove)     | `rm`    | Remove a task from an iteration                      |
| [`reopen`](#iteration-reopen)     |         | Set a completed or cancelled iteration to `active`   |
| [`show`](#iteration-show)         | `view`  | Display an iteration's details                       |
| [`status`](#iteration-status)     |         | Display aggregated iteration progress                |
| [`tag`](#iteration-tag)           |         | Add a tag to an iteration                            |
| [`untag`](#iteration-untag)       |         | Remove a tag from an iteration                       |
| [`update`](#iteration-update)     | `edit`  | Update an iteration's fields                         |

Every subcommand also accepts `-r, --raw`, which prints plain output with no themed
styling. The tables below list it only where it changes the output in a useful way.

## Exit codes {#exit-codes}

| Code | When                                                                                        |
|------|---------------------------------------------------------------------------------------------|
| 0    | Success                                                                                     |
| 2    | The command line did not parse, for example an unknown flag or a missing argument           |
| 64   | Malformed NDJSON batch input, a bad positional relationship, or `--agent` without `--claim` |
| 65   | Could not serialize to or deserialize from JSON, for example a bad `--as-json` value        |
| 66   | Iteration, task, artifact or metadata key did not resolve                                   |
| 69   | State conflict, for example `advance` on a terminal iteration or `next` on an inactive one  |
| 74   | Store I/O error                                                                             |
| 75   | `iteration next` found no open, unblocked task                                              |
| 78   | Not a gest project (run `gest init`)                                                        |

[Exit Codes](./exit-codes.md) gives the full contract. The section on
`iteration next` below has a narrower table for that subcommand.

---

## iteration add

Add an existing task to an iteration. Without `--batch`, the command adds one task. With
`--batch`, it reads NDJSON records from stdin, one task for each line.

```text
gest iteration add [OPTIONS] <ITERATION> [TASK]
```

### Arguments

| Argument      | Description                                                |
|---------------|------------------------------------------------------------|
| `<ITERATION>` | Iteration ID or unique prefix                              |
| `[TASK]`      | Task ID or unique prefix to add (conflicts with `--batch`) |

### Options

| Flag                  | Description                                                                                  |
|-----------------------|----------------------------------------------------------------------------------------------|
| `--batch`             | Read NDJSON task records from stdin (conflicts with `TASK` and `--phase`)                    |
| `-j, --json`          | Output as JSON                                                                               |
| `-p, --phase <PHASE>` | Phase to add the task to (default: the highest existing phase + 1; conflicts with `--batch`) |
| `-q, --quiet`         | Print only the task ID, or the iteration ID in batch mode                                    |

### Batch NDJSON schema

Each line must be a JSON object with these fields:

| Field   | Type   | Required | Description                                              |
|---------|--------|----------|----------------------------------------------------------|
| `task`  | string | yes      | Task ID or unique prefix                                 |
| `phase` | number | no       | Phase to assign (see the rule for a missing phase below) |

Gest reads the highest phase once, before the first line. A line without `phase` takes
that highest phase + 1, and each later line without `phase` takes the number after it. A
line with an explicit `phase` does not change the count. In the example below, an empty
iteration gives `taskABC` phase 1.

### Examples

```sh
# Append to the next phase (max + 1)
gest iteration add iter123 task456

# Pin to an explicit phase
gest iteration add iter123 task456 --phase 2

# Batch-add from NDJSON stdin
cat <<'EOF' | gest iteration add iter123 --batch
{"task":"task456","phase":1}
{"task":"task789","phase":2}
{"task":"taskABC"}
EOF
```

---

## iteration advance

Check the active phase of an iteration and report the phase that comes after it. The
active phase is the lowest phase that has a task that is not `done` or `cancelled`.

Gest stores no phase pointer. It works out the active phase from the task statuses each
time. So `advance` changes nothing in the store, and it behaves as follows:

- Without `--force`, the command exits with 69 whenever the active phase has a task
  left. When a phase is finished, the next phase becomes the active phase at once.
- With `--force`, the command reports the move from the active phase to the next phase.
- On the last phase, or on an iteration with no tasks, the command exits with 69.
- When every task is `done` or `cancelled`, the command exits with 69.
- On a `completed` or `cancelled` iteration, the command exits with 69.

```text
gest iteration advance [OPTIONS] <ID>
```

### Arguments

| Argument | Description                   |
|----------|-------------------------------|
| `<ID>`   | Iteration ID or unique prefix |

### Options

| Flag          | Description                                              |
|---------------|----------------------------------------------------------|
| `--force`     | Advance even if the current phase has non-terminal tasks |
| `-j, --json`  | Output as JSON                                           |
| `-q, --quiet` | Print only the iteration ID                              |

### Examples

```sh
# Check the active phase (exits 69 while it has tasks left)
gest iteration advance abc123

# Force-advance past incomplete tasks
gest iteration advance abc123 --force
```

---

## iteration cancel

Set the status of an iteration to `cancelled`. The command changes only the iteration.
Its tasks keep their status, so cancel the open tasks with `gest task cancel` if you
want them closed too. `gest iteration update` has no `--status` flag, so use `cancel`,
`complete` and `reopen` to change the status.

```text
gest iteration cancel [OPTIONS] <ID>
```

### Arguments

| Argument | Description                   |
|----------|-------------------------------|
| `<ID>`   | Iteration ID or unique prefix |

### Options

| Flag          | Description       |
|---------------|-------------------|
| `-j, --json`  | Output as JSON    |
| `-q, --quiet` | Print only the ID |

### Examples

```sh
# Cancel an iteration
gest iteration cancel abc123

# Cancel with JSON output
gest iteration cancel abc123 --json
```

---

## iteration complete

Set the status of an iteration to `completed`. Like `cancel`, the command changes only
the iteration and leaves its tasks as they are.

```text
gest iteration complete [OPTIONS] <ID>
```

### Arguments

| Argument | Description                   |
|----------|-------------------------------|
| `<ID>`   | Iteration ID or unique prefix |

### Options

| Flag          | Description       |
|---------------|-------------------|
| `-j, --json`  | Output as JSON    |
| `-q, --quiet` | Print only the ID |

### Examples

```sh
gest iteration complete abc123
gest iteration complete abc123 --json
```

---

## iteration create

Create a new iteration.

```text
gest iteration create [OPTIONS] <TITLE>
```

### Arguments

| Argument  | Description     |
|-----------|-----------------|
| `<TITLE>` | Iteration title |

### Options

| Flag                              | Description                                                                         |
|-----------------------------------|-------------------------------------------------------------------------------------|
| `-d, --description <DESCRIPTION>` | Description text                                                                    |
| `-j, --json`                      | Output the created iteration as JSON                                                |
| `-m, --metadata <KEY=VALUE>`      | Set a metadata key=value pair (repeatable; supports dot-paths and scalar inference) |
| `--metadata-json <JSON>`          | Merge a JSON object into metadata (repeatable; applied after `--metadata` pairs)    |
| `-q, --quiet`                     | Print only the iteration ID                                                         |
| `-s, --status <STATUS>`           | Initial status: `active`, `cancelled`, or `completed` (default: `active`)           |
| `-t, --tag <TAG>`                 | Tag (repeatable; a comma-separated value gives several tags)                        |

An unknown `--status` value exits with 64.

### Examples

```sh
# Create a simple iteration
gest iteration create "Sprint 1"

# Create with description and tags
gest iteration create "Auth Refactor" -d "Rewrite authentication layer" --tag "backend,q2"

# Machine-readable output
gest iteration create "Sprint 2" --json
gest iteration create "Sprint 2" -q
```

---

## iteration delete

Delete an iteration with its notes, tags and relationships, and drop its task
memberships. The tasks stay in the store and only lose their link to the iteration. The
command asks for confirmation first. `gest undo` restores the iteration and its task
memberships.

```text
gest iteration delete [OPTIONS] <ID>
```

### Arguments

| Argument | Description                   |
|----------|-------------------------------|
| `<ID>`   | Iteration ID or unique prefix |

### Options

| Flag          | Description                                                                     |
|---------------|---------------------------------------------------------------------------------|
| `--yes`       | Skip the interactive confirmation prompt                                        |
| `--force`     | Reserved for future guards; currently a no-op (iterations have no guards today) |
| `-j, --json`  | Output as JSON                                                                  |
| `-q, --quiet` | Suppress normal output                                                          |

### Examples

```sh
# Interactive (prompts for confirmation)
gest iteration delete abc123

# Non-interactive
gest iteration delete abc123 --yes
```

---

## iteration graph

Display the phased execution graph of an iteration. The graph groups the tasks by phase
and shows the status and the blockers of each task. The command takes `-j, --json` for
JSON output and `-q, --quiet` to print only the iteration ID.

```text
gest iteration graph [OPTIONS] <ID>
```

### Arguments

| Argument | Description                   |
|----------|-------------------------------|
| `<ID>`   | Iteration ID or unique prefix |

### Examples

```sh
gest iteration graph abc123
```

---

## iteration link

Create a relationship between an iteration and another iteration or an artifact. A link
between two iterations also writes the reverse row. So the link `A blocks B` also
records `B blocked-by A`. A link to an artifact writes one row only.

```text
gest iteration link [OPTIONS] <ID> <TARGET> [--rel <REL>]
```

### Arguments

| Argument   | Description                                      |
|------------|--------------------------------------------------|
| `<ID>`     | Iteration ID or unique prefix                    |
| `<TARGET>` | Target iteration or artifact ID or unique prefix |

### Options

| Flag          | Description                                                                                              |
|---------------|----------------------------------------------------------------------------------------------------------|
| `--artifact`  | Target is an artifact instead of an iteration                                                            |
| `--rel <REL>` | Relationship type: `blocked-by`, `blocks`, `child-of`, `parent-of`, `relates-to` (default: `relates-to`) |
| `-j, --json`  | Output the iteration as JSON after linking                                                               |
| `-q, --quiet` | Output only the iteration ID                                                                             |

### Examples

```sh
gest iteration link abc123 def456 --rel blocks
gest iteration link abc123 art789 --artifact --rel relates-to
```

::: {.callout-caution}

## Deprecated: positional `<REL>`

Earlier releases accepted a positional `<REL>` argument between `<ID>` and `<TARGET>`:

```sh
# Deprecated; still works but emits a warning
gest iteration link abc123 blocks def456
```

Gest still accepts this form, but it prints a deprecation warning to stderr. A future
release removes the form, so use `--rel <type>`. If you give both a positional `<REL>`
and `--rel`, the command exits with 64.
:::

---

## iteration unlink

Remove a relationship between an iteration and another iteration or an artifact. For a
link between two iterations, the command deletes the named row and its reverse row in
one transaction. `iteration link` creates both rows in the same way. `gest undo` restores
the deleted rows.

```text
gest iteration unlink [OPTIONS] <ID> <TARGET>
```

### Arguments

| Argument   | Description                                      |
|------------|--------------------------------------------------|
| `<ID>`     | Iteration ID or unique prefix                    |
| `<TARGET>` | Target iteration or artifact ID or unique prefix |

### Options

| Flag          | Description                                                                    |
|---------------|--------------------------------------------------------------------------------|
| `--artifact`  | Target is an artifact instead of an iteration                                  |
| `--rel <REL>` | Filter to relationships of this type. Required when multiple edges exist       |
| `-j, --json`  | Output the iteration as JSON after unlinking                                   |
| `-q, --quiet` | Output only the iteration ID                                                   |

If exactly one relationship joins the source and the target, `--rel` is optional. If
several relationships join them and `--rel` is missing, the command exits with 64. Its
error lists the relationship types it found. If no relationship matches, the command
exits with 66.

### Examples

```sh
# Remove an iteration-to-iteration blocks edge (also removes the reciprocal blocked-by row)
gest iteration unlink abc123 def456 --rel blocks

# Remove a link to an artifact
gest iteration unlink abc123 art789 --artifact
```

---

## iteration list

List iterations, newest first. By default the list shows only `active` iterations. Use
`--all` to include `completed` and `cancelled` ones.

```text
gest iteration list [OPTIONS]
```

### Options

| Flag                    | Description                                                  |
|-------------------------|--------------------------------------------------------------|
| `-a, --all`             | Include resolved (completed/cancelled) iterations            |
| `--has-available`       | Only show iterations with at least one task in `open` status |
| `--limit <N>`           | Cap the number of items returned                             |
| `-s, --status <STATUS>` | Filter by status: `active`, `cancelled`, or `completed`      |
| `-t, --tag <TAG>`       | Filter by tag                                                |
| `-j, --json`            | Output iteration list as JSON                                |
| `-q, --quiet`           | Print only the iteration IDs                                 |

The filter `--status` works inside the default filter. So if you do not give `--all`,
`--status completed` and `--status cancelled` list nothing.

### Examples

```sh
gest iteration list
gest iteration list --all
gest iteration list -s active
```

---

## iteration meta

Read or write the metadata fields of an iteration. A metadata path joins the keys of
nested values with dots, for example `outer.inner`. The command `gest iteration meta <ID>`
with no subcommand prints all the metadata of the iteration.

```text
gest iteration meta [OPTIONS] [ID] [COMMAND]
```

### meta get

Print one metadata value. If the path holds no value, the command exits with 66.

```text
gest iteration meta get [OPTIONS] <ID> <PATH>
```

| Argument | Description                                 |
|----------|---------------------------------------------|
| `<ID>`   | Iteration ID or unique prefix               |
| `<PATH>` | Dot-delimited key path (e.g. `outer.inner`) |

| Flag         | Description                           |
|--------------|---------------------------------------|
| `-j, --json` | Output as a JSON object               |
| `-r, --raw`  | Output the bare value with no styling |

### meta set

Set a metadata value. Gest reads `true` and `false` as booleans and a number as a
number. Every other value is a string, and that includes `null`. Use `--as-json` to give
`null`, an array or an object.

```text
gest iteration meta set [OPTIONS] <ID> <PATH> <VALUE>
```

| Argument  | Description                                 |
|-----------|---------------------------------------------|
| `<ID>`    | Iteration ID or unique prefix               |
| `<PATH>`  | Dot-delimited key path (e.g. `outer.inner`) |
| `<VALUE>` | Value to set                                |

| Flag          | Description                                                |
|---------------|------------------------------------------------------------|
| `--as-json`   | Parse the value as a JSON literal (a bad literal exits 65) |
| `-j, --json`  | Output as JSON                                             |
| `-q, --quiet` | Print only the entity ID                                   |

### meta unset

Remove a metadata value. If the path holds no value, the command exits with 66.

```text
gest iteration meta unset [OPTIONS] <ID> <PATH>
```

| Argument | Description                                 |
|----------|---------------------------------------------|
| `<ID>`   | Iteration ID or unique prefix               |
| `<PATH>` | Dot-delimited key path (e.g. `outer.inner`) |

| Flag          | Description              |
|---------------|--------------------------|
| `-j, --json`  | Output as JSON           |
| `-q, --quiet` | Print only the entity ID |

### Examples

```sh
# Set a metadata field
gest iteration meta set abc123 goal "Ship auth module"

# Read it back
gest iteration meta get abc123 goal

# JSON output
gest iteration meta get abc123 goal --json

# Raw value (no styling)
gest iteration meta get abc123 goal --raw
```

---

## iteration next

Find the next available task in an iteration, and claim it with `--claim`. A task is a
candidate when its status is `open` and no unfinished task of the iteration blocks it. A blocking task
outside the iteration always blocks it, even when that task is done.

The candidates come from every phase, and a later phase can give a task while the active phase still
has tasks in progress. Gest sorts the candidates by phase, lowest first. Inside a phase,
a lower priority number comes first, and a task without a priority comes last. Gest applies no further tie-break.

```text
gest iteration next [OPTIONS] <ID>
```

### Arguments

| Argument | Description                   |
|----------|-------------------------------|
| `<ID>`   | Iteration ID or unique prefix |

### Options

| Flag              | Description                                                   |
|-------------------|---------------------------------------------------------------|
| `--claim`         | Set the task to in-progress and assign it (may be used alone) |
| `--agent <AGENT>` | Agent name for assignment (requires `--claim`)                |
| `-j, --json`      | Output as JSON                                                |
| `-q, --quiet`     | Print only the task ID                                        |

### Exit codes {#exit-codes-1}

Gest follows the BSD [`sysexits.h`](https://man.openbsd.org/sysexits.3)
convention. `iteration next` returns one of these codes:

| Code | Name             | Meaning                                        |
|------|------------------|------------------------------------------------|
| 0    |                  | Task found (and claimed if `--claim` was used) |
| 64   | `EX_USAGE`       | `--agent` without `--claim`                    |
| 66   | `EX_NOINPUT`     | Iteration ID did not resolve                   |
| 69   | `EX_UNAVAILABLE` | Iteration is not active                        |
| 75   | `EX_TEMPFAIL`    | No open, unblocked task in the iteration       |

A command line that does not parse, for example an unknown flag, exits with 2.

### Examples

```sh
# Peek at the next task without claiming
gest iteration next abc123

# Claim the next task for an agent
gest iteration next abc123 --claim --agent worker-1

# Machine-readable output
gest iteration next abc123 --claim --agent worker-1 --json
```

---

## iteration remove

Remove a task from an iteration. When the task is not in the iteration, the command prints
`task was not in iteration`, changes nothing and exits 0.

```text
gest iteration remove [OPTIONS] <ITERATION> <TASK>
```

### Arguments

| Argument      | Description                        |
|---------------|------------------------------------|
| `<ITERATION>` | Iteration ID or unique prefix      |
| `<TASK>`      | Task ID or unique prefix to remove |

### Options

| Flag          | Description                                          |
|---------------|------------------------------------------------------|
| `-j, --json`  | Output the iteration as JSON after removing the task |
| `-q, --quiet` | Output only the iteration ID                         |

### Examples

```sh
gest iteration remove iter123 task456
```

---

## iteration reopen

Set the status of a completed or cancelled iteration back to `active`. The command
changes only the iteration. Its tasks keep their status, and a cancelled task stays
cancelled.

```text
gest iteration reopen [OPTIONS] <ID>
```

### Arguments

| Argument | Description                   |
|----------|-------------------------------|
| `<ID>`   | Iteration ID or unique prefix |

### Options

| Flag          | Description       |
|---------------|-------------------|
| `-j, --json`  | Output as JSON    |
| `-q, --quiet` | Print only the ID |

### Examples

```sh
# Reopen a cancelled iteration
gest iteration reopen abc123

# Reopen with JSON output
gest iteration reopen abc123 --json
```

---

## iteration show

Display the details, the task counts and the phase summary of an iteration.

```text
gest iteration show [OPTIONS] <ID>
```

### Arguments

| Argument | Description                   |
|----------|-------------------------------|
| `<ID>`   | Iteration ID or unique prefix |

### Options

| Flag          | Description                      |
|---------------|----------------------------------|
| `-j, --json`  | Output iteration details as JSON |
| `-q, --quiet` | Print only the iteration ID      |

### Examples

```sh
gest iteration show abc123
gest iteration show abc123 --json
```

---

## iteration status

Display the progress of an iteration. The summary gives the status, the number of
phases, the task count for each task status and the percentage of tasks done.

```text
gest iteration status [OPTIONS] <ID>
```

### Arguments

| Argument | Description                   |
|----------|-------------------------------|
| `<ID>`   | Iteration ID or unique prefix |

### Options

| Flag         | Description                     |
|--------------|---------------------------------|
| `-j, --json` | Output iteration status as JSON |

### Examples

```sh
gest iteration status abc123
gest iteration status abc123 --json
```

---

## iteration tag

Add one tag to an iteration. If the iteration already has the tag, nothing changes. The
command takes one label and does not split it on commas. So `sprint-1,backend` is one
tag, and to add two tags you run the command twice.

```text
gest iteration tag [OPTIONS] <ID> <LABEL>
```

### Arguments

| Argument  | Description                   |
|-----------|-------------------------------|
| `<ID>`    | Iteration ID or unique prefix |
| `<LABEL>` | Tag to add                    |

### Options

| Flag          | Description                                |
|---------------|--------------------------------------------|
| `-j, --json`  | Output the iteration as JSON after tagging |
| `-q, --quiet` | Output only the iteration ID               |

### Examples

```sh
gest iteration tag abc123 sprint-1
gest iteration tag abc123 backend
```

---

## iteration untag

Remove one tag from an iteration.

```text
gest iteration untag [OPTIONS] <ID> <LABEL>
```

### Arguments

| Argument  | Description                   |
|-----------|-------------------------------|
| `<ID>`    | Iteration ID or unique prefix |
| `<LABEL>` | Tag to remove                 |

### Options

| Flag          | Description                                  |
|---------------|----------------------------------------------|
| `-j, --json`  | Output the iteration as JSON after untagging |
| `-q, --quiet` | Output only the iteration ID                 |

### Examples

```sh
gest iteration untag abc123 draft
```

---

## iteration update

Update the title, the description or the metadata of an iteration. To change the status,
use [`complete`](#iteration-complete), [`cancel`](#iteration-cancel) or
[`reopen`](#iteration-reopen). To change the tags, use [`tag`](#iteration-tag) and
[`untag`](#iteration-untag).

```text
gest iteration update [OPTIONS] <ID>
```

### Arguments

| Argument | Description                   |
|----------|-------------------------------|
| `<ID>`   | Iteration ID or unique prefix |

### Options

| Flag                              | Description                                                                         |
|-----------------------------------|-------------------------------------------------------------------------------------|
| `-d, --description <DESCRIPTION>` | New description                                                                     |
| `-j, --json`                      | Output as JSON                                                                      |
| `-m, --metadata <KEY=VALUE>`      | Set a metadata key=value pair (repeatable; supports dot-paths and scalar inference) |
| `--metadata-json <JSON>`          | Merge a JSON object into metadata (repeatable; applied after `--metadata` pairs)    |
| `-q, --quiet`                     | Print only the iteration ID                                                         |
| `-t, --title <TITLE>`             | New title                                                                           |

### Examples

```sh
gest iteration update abc123 -t "Sprint 1 - Revised"
gest iteration update abc123 -m goal="deliver auth"

# Machine-readable output
gest iteration update abc123 -t "Sprint 1 - Revised" --json
```
