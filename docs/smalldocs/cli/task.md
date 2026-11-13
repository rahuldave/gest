---
title: gest task
description: gest task — create, update, list, and manage tasks in gest, the agentic workflow CLI for parallel AI agents. Includes --json output for agents.
---

The `gest task` commands create, update, list and manage tasks. A task is a unit of work. It
has a title, a description, a status, a priority, tags, metadata and links to other entities.

## Usage

```text
gest task <COMMAND> [OPTIONS]
```

## Subcommands

| Command                      | Aliases | Description                                     |
|------------------------------|---------|-------------------------------------------------|
| [`block`](#task-block)       |         | Mark a task as blocking another task            |
| [`cancel`](#task-cancel)     |         | Cancel a task (sets the status to `cancelled`)  |
| [`claim`](#task-claim)       |         | Claim a task (assign and mark in-progress)      |
| [`complete`](#task-complete) |         | Mark a task as done (sets the status to `done`) |
| [`create`](#task-create)     | `new`   | Create a new task                               |
| [`delete`](#task-delete)     | `rm`    | Delete a task and its dependent rows            |
| [`list`](#task-list)         | `ls`    | List tasks with optional filters                |
| [`show`](#task-show)         | `view`  | Display a task's full details                   |
| [`update`](#task-update)     | `edit`  | Update a task's fields                          |
| [`tag`](#task-tag)           |         | Add a tag to a task                             |
| [`untag`](#task-untag)       |         | Remove a tag from a task                        |
| [`link`](#task-link)         |         | Link a task to another entity                   |
| [`unlink`](#task-unlink)     |         | Remove a relationship from a task               |
| [`meta`](#task-meta)         |         | Read or write metadata fields                   |
| [`note`](#task-note)         |         | Manage notes on a task                          |

## Exit codes

| Code | When                                                                                          |
|------|-----------------------------------------------------------------------------------------------|
| 0    | Success                                                                                       |
| 2    | The parser rejected the command line, for example an unknown flag or a missing argument       |
| 64   | No title, malformed NDJSON batch input, an unknown `--link` type, or `unlink` without `--rel` |
| 65   | Could not serialize to or deserialize from JSON or TOML                                       |
| 66   | A task, note, iteration, relationship or metadata key did not resolve                         |
| 69   | `delete` of a task that is still in an iteration, without `--force`                           |
| 70   | The editor failed to start, exited non-zero, or left a required body empty                    |
| 74   | Store I/O error, for example a link that already exists                                       |
| 78   | Not a gest project (run `gest init`)                                                          |

See [Exit Codes](./exit-codes.md) for the full contract.

---

## task block

Mark a task as blocking another task. The command is a shortcut for
`task link <id> <blocked> --rel blocks`, and both IDs must name tasks.

```text
gest task block [OPTIONS] <ID> <BLOCKED>
```

### Arguments

| Argument    | Description                                     |
|-------------|-------------------------------------------------|
| `<ID>`      | ID or unique prefix of the task that blocks     |
| `<BLOCKED>` | ID or unique prefix of the task that is blocked |

### Options

| Flag          | Description                                    |
|---------------|------------------------------------------------|
| `-j, --json`  | Output the task as JSON after linking          |
| `-q, --quiet` | Output only the task ID                        |
| `-r, --raw`   | Output plain text for scripts, with no styling |

To mark a task as blocking an artifact, run
`gest task link <id> <artifact-id> --rel blocks --artifact`.

### Examples

```sh
# Task abc123 blocks task def456
gest task block abc123 def456
```

---

## task cancel

Cancel a task. The command sets the status of the task to `cancelled`, as
`task update <id> --status cancelled` does.

```text
gest task cancel [OPTIONS] <ID>
```

### Arguments

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Task ID or unique prefix |

### Options

| Flag          | Description                                    |
|---------------|------------------------------------------------|
| `-j, --json`  | Output as JSON                                 |
| `-q, --quiet` | Print only the task ID                         |
| `-r, --raw`   | Output plain text for scripts, with no styling |

### Examples

```sh
gest task cancel abc123
```

---

## task claim

Claim a task. The command assigns the task to an author and sets its status to
`in-progress`. Without `--as`, the author is the `user.name` of `git config`. If git has no
user name, the author is `$USER`. If `$USER` is also unset, the author is `unknown`.

```text
gest task claim [OPTIONS] <ID>
```

### Arguments

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Task ID or unique prefix |

### Options

| Flag               | Description                                            |
|--------------------|--------------------------------------------------------|
| `--as <AS_AUTHOR>` | Claim as a specific author name (defaults to git user) |
| `-j, --json`       | Output as JSON                                         |
| `-q, --quiet`      | Print only the task ID                                 |
| `-r, --raw`        | Output plain text for scripts, with no styling         |

### Examples

```sh
# Claim as the current git user
gest task claim abc123

# Claim on behalf of an agent
gest task claim abc123 --as implement-agent
```

---

## task complete

Mark a task as done. The command sets the status of the task to `done`, as
`task update <id> --status done` does.

```text
gest task complete [OPTIONS] <ID>
```

### Arguments

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Task ID or unique prefix |

### Options

| Flag          | Description                                    |
|---------------|------------------------------------------------|
| `-j, --json`  | Output as JSON                                 |
| `-q, --quiet` | Print only the task ID                         |
| `-r, --raw`   | Output plain text for scripts, with no styling |

### Examples

```sh
gest task complete abc123
```

---

## task create

Create a new task, with optional metadata, tags, links and status.

The description comes from one of these places:

- The `--description` flag, when you give it.
- `$EDITOR`, when you omit `--description` and stdin is a terminal.
- The piped content, when stdin is a pipe.

If you pipe the content and omit `[TITLE]`, gest takes the title from the first `# heading`
of the piped content. If the content has no such heading, the command exits with code 64.

```text
gest task create [OPTIONS] [TITLE]
```

### Arguments

| Argument  | Description                                                              |
|-----------|--------------------------------------------------------------------------|
| `[TITLE]` | Task title (auto-extracted from the first `# heading` when piping stdin) |

### Options

| Flag                              | Description                                                                          |
|-----------------------------------|--------------------------------------------------------------------------------------|
| `--assign <ASSIGN>`               | Assign the task to an author by name                                                 |
| `--batch`                         | Read NDJSON from stdin (one task per line)                                           |
| `-d, --description <DESCRIPTION>` | Description text (opens `$EDITOR` if omitted and stdin is a terminal)                |
| `-i, --iteration <ITERATION>`     | Add the task to an iteration (ID or prefix)                                          |
| `-j, --json`                      | Output the created task as JSON                                                      |
| `-l, --link <LINK>`               | Create a link on the new task (repeatable, format: `<rel>:<target_id>`)              |
| `-m, --metadata <KEY=VALUE>`      | Set a metadata key=value pair (repeatable; supports dot-paths and scalar inference)  |
| `--metadata-json <JSON>`          | Merge a JSON object into metadata (repeatable; applied after `--metadata` pairs)     |
| `--phase <PHASE>`                 | Phase within `--iteration` (requires `--iteration`; defaults to max + 1)             |
| `-p, --priority <PRIORITY>`       | Priority `0` to `4` (0 is highest), or `critical`, `high`, `medium`, `low`, `lowest` |
| `-q, --quiet`                     | Print only the task ID                                                               |
| `-r, --raw`                       | Output plain text for scripts, with no styling                                       |
| `-s, --status <STATUS>`           | Initial status: `open`, `in-progress`, `done`, or `cancelled` (default: `open`)      |
| `--tag <TAG>`                     | Tag (repeatable; a comma-separated value makes several tags)                         |

A `--link` value without a `<rel>:` part makes a `relates-to` link. The target can be a task,
an artifact or an iteration, and gest looks for the ID in that order.

### Examples

```sh
# Create a simple task
gest task create "Implement login page"

# Create a task with description and tags
gest task create "Fix memory leak" -d "OOM after 24h uptime" --tag "bug,critical"

# Create a high-priority task assigned to an agent
gest task create "Write migration" -p 0 --assign agent

# Create a task and add it to an iteration with a link
gest task create "Add auth" -i iter123 -l child-of:spec456

# Create a task pinned to a specific phase of an iteration
gest task create "Add auth" -i iter123 --phase 2

# Pipe description from stdin
echo "Detailed description here" | gest task create "My task"

# Batch-create tasks from NDJSON
cat tasks.ndjson | gest task create --batch

# Machine-readable output
gest task create "Quick task" --json
gest task create "Quick task" -q
```

---

## task delete

Delete a task and its dependent rows. The dependent rows are its notes, metadata, tags and links.
`gest undo` restores the task and these rows. By default, the command does not delete a task that still belongs to an
iteration, and it exits with code 69. Pass `--force` to remove the task from its iterations
first.

```text
gest task delete [OPTIONS] <ID>
```

### Arguments

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Task ID or unique prefix |

### Options

| Flag          | Description                                                        |
|---------------|--------------------------------------------------------------------|
| `--yes`       | Skip the interactive confirmation prompt                           |
| `--force`     | Remove the task from every iteration it belongs to before deleting |
| `-j, --json`  | Output as JSON                                                     |
| `-q, --quiet` | Suppress normal output                                             |
| `-r, --raw`   | Output plain text for scripts, with no styling                     |

### Examples

```sh
# Interactive delete
gest task delete abc123

# Non-interactive
gest task delete abc123 --yes

# Delete even if task is still in iterations
gest task delete abc123 --yes --force
```

---

## task list

List tasks. The filters for status, tag and assignee are optional. Without `--all`, the list
leaves out tasks that are done or cancelled.

```text
gest task list [OPTIONS]
```

### Options

| Flag                          | Description                                                     |
|-------------------------------|-----------------------------------------------------------------|
| `-a, --all`                   | Include resolved (done/cancelled) tasks                         |
| `--assigned-to <ASSIGNED_TO>` | Filter by assigned-to name                                      |
| `--limit <N>`                 | Cap the number of items returned                                |
| `-s, --status <STATUS>`       | Filter by status: `open`, `in-progress`, `done`, or `cancelled` |
| `-t, --tag <TAG>`             | Filter by tag                                                   |
| `-j, --json`                  | Output task list as JSON                                        |
| `-q, --quiet`                 | Print only task IDs                                             |
| `-r, --raw`                   | Output plain text for scripts, with no styling                  |

### Examples

```sh
# List active tasks
gest task list

# List all tasks including resolved
gest task list --all

# Filter by status
gest task list -s in-progress

# Filter by assignee
gest task list --assigned-to agent

# JSON output for scripting
gest task list --json
```

---

## task show

Display the full details of a task, with its description and links.

```text
gest task show [OPTIONS] <ID>
```

### Arguments

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Task ID or unique prefix |

### Options

| Flag          | Description                                    |
|---------------|------------------------------------------------|
| `-j, --json`  | Output task details as JSON                    |
| `-q, --quiet` | Print only the task ID                         |
| `-r, --raw`   | Output plain text for scripts, with no styling |

### Examples

```sh
# Show task by full ID
gest task show abc123

# Show task by prefix
gest task show ab

# JSON output
gest task show abc123 --json
```

---

## task update

Update the title, description, status, priority, assignee, phase, tags or metadata of a task.

```text
gest task update [OPTIONS] <ID>
```

### Arguments

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Task ID or unique prefix |

### Options

| Flag                              | Description                                                                          |
|-----------------------------------|--------------------------------------------------------------------------------------|
| `--assigned-to <ASSIGNED_TO>`     | Set the assigned author by name                                                      |
| `-d, --description <DESCRIPTION>` | New description text                                                                 |
| `-j, --json`                      | Output as JSON                                                                       |
| `-m, --metadata <KEY=VALUE>`      | Set a metadata key=value pair (repeatable; supports dot-paths and scalar inference)  |
| `--metadata-json <JSON>`          | Merge a JSON object into metadata (repeatable; applied after `--metadata` pairs)     |
| `--phase <PHASE>`                 | Move the task to a phase within its iteration                                        |
| `-p, --priority <PRIORITY>`       | Priority `0` to `4` (0 is highest), or `critical`, `high`, `medium`, `low`, `lowest` |
| `-q, --quiet`                     | Print only the task ID                                                               |
| `-r, --raw`                       | Output plain text for scripts, with no styling                                       |
| `-s, --status <STATUS>`           | New status (done/cancelled auto-resolves; open/in-progress un-resolves)              |
| `--tag <TAG>`                     | Replace all tags (repeatable; a comma-separated value makes several tags)            |
| `-t, --title <TITLE>`             | New title                                                                            |

### Examples

```sh
# Mark a task as done
gest task update abc123 -s done

# Update title and description
gest task update abc123 -t "New title" -d "Updated description"

# Add metadata
gest task update abc123 -m estimate=3h -m complexity=high

# Machine-readable output
gest task update abc123 -s done --json
```

---

## task tag

Add one tag to a task. If the task already has the tag, nothing changes. The command takes
one label and does not split it on commas, so run it once for each tag.

```text
gest task tag [OPTIONS] <ID> <LABEL>
```

### Arguments

| Argument  | Description              |
|-----------|--------------------------|
| `<ID>`    | Task ID or unique prefix |
| `<LABEL>` | The tag label to add     |

### Options

| Flag          | Description                                    |
|---------------|------------------------------------------------|
| `-j, --json`  | Output the task as JSON after tagging          |
| `-q, --quiet` | Output only the task ID                        |
| `-r, --raw`   | Output plain text for scripts, with no styling |

### Examples

```sh
gest task tag abc123 bug
gest task tag abc123 critical
```

---

## task untag

Remove one tag from a task.

```text
gest task untag [OPTIONS] <ID> <LABEL>
```

### Arguments

| Argument  | Description              |
|-----------|--------------------------|
| `<ID>`    | Task ID or unique prefix |
| `<LABEL>` | The tag label to remove  |

### Options

| Flag          | Description                                    |
|---------------|------------------------------------------------|
| `-j, --json`  | Output the task as JSON after untagging        |
| `-q, --quiet` | Output only the task ID                        |
| `-r, --raw`   | Output plain text for scripts, with no styling |

### Examples

```sh
gest task untag abc123 critical
```

---

## task link

Create a relationship from a task to a task, an artifact or an iteration. The `--rel` flag
sets the type of the relationship, and the default type is `relates-to`.

```text
gest task link [OPTIONS] <ID> <TARGET>
```

### Arguments

| Argument   | Description                       |
|------------|-----------------------------------|
| `<ID>`     | Source task ID or unique prefix   |
| `<TARGET>` | Target entity ID or unique prefix |

### Options

| Flag                          | Description                                                                             |
|-------------------------------|-----------------------------------------------------------------------------------------|
| `--rel <REL>`                 | `blocked-by`, `blocks`, `child-of`, `parent-of` or `relates-to` (default: `relates-to`) |
| `--artifact`                  | Target is an artifact instead of a task                                                 |
| `--target-type <TARGET_TYPE>` | Target entity type: `task`, `artifact`, or `iteration` (default: `task`)                |
| `-j, --json`                  | Output the task as JSON after linking                                                   |
| `-q, --quiet`                 | Output only the task ID                                                                 |
| `-r, --raw`                   | Output plain text for scripts, with no styling                                          |

The `--artifact` flag is the same as `--target-type artifact`, and you cannot give both. A link
that already exists makes the command exit with code 74.

### Examples

```sh
# Task blocks another task
gest task link abc123 def456 --rel blocks

# Task relates to an artifact
gest task link abc123 art789 --artifact
```

---

## task unlink

Remove a relationship from a task to a task, an artifact or an iteration. The transaction log
records the deletion, so `gest undo` restores the relationship.

```text
gest task unlink [OPTIONS] <ID> <TARGET>
```

### Arguments

| Argument   | Description                       |
|------------|-----------------------------------|
| `<ID>`     | Source task ID or unique prefix   |
| `<TARGET>` | Target entity ID or unique prefix |

### Options

| Flag                          | Description                                                              |
|-------------------------------|--------------------------------------------------------------------------|
| `--artifact`                  | Target is an artifact instead of a task                                  |
| `--rel <REL>`                 | Filter to relationships of this type. Required when multiple edges exist |
| `--target-type <TARGET_TYPE>` | Target entity type: `task`, `artifact`, or `iteration` (default: `task`) |
| `-j, --json`                  | Output the task as JSON after unlinking                                  |
| `-q, --quiet`                 | Output only the task ID                                                  |
| `-r, --raw`                   | Output plain text for scripts, with no styling                           |

The `--rel` flag is optional when exactly one relationship joins the two entities:

- If several relationships join them and you omit `--rel`, the command lists the candidate
  types and exits with code 64.
- If no matching relationship exists, the command exits with code 66.

### Examples

```sh
# Remove a single relates-to edge (no --rel needed when unambiguous)
gest task unlink abc123 def456

# Remove the `blocks` edge when multiple edges exist between the pair
gest task unlink abc123 def456 --rel blocks

# Remove a link to an artifact
gest task unlink abc123 art789 --artifact
```

---

## task meta

Read or write the metadata fields of a task. A metadata path puts a dot between the keys of
nested values. With an ID and no subcommand, `gest task meta <ID>` prints all the metadata of
the task.

```text
gest task meta [OPTIONS] [ID] [COMMAND]
```

If the metadata key does not exist, `get` and `unset` exit with code 66.

### meta get

Retrieve a single metadata value.

```text
gest task meta get [OPTIONS] <ID> <PATH>
```

| Argument | Description                                 |
|----------|---------------------------------------------|
| `<ID>`   | Task ID or unique prefix                    |
| `<PATH>` | Dot-delimited key path (e.g. `outer.inner`) |

| Flag          | Description                           |
|---------------|---------------------------------------|
| `-j, --json`  | Output as a JSON object               |
| `-q, --quiet` | Suppress normal output                |
| `-r, --raw`   | Output the bare value with no styling |

### meta set

Set a metadata value. Gest reads `true` and `false` as booleans, and integers and decimal
numbers as numbers. Every other value is a string, and `null` is a string too. Pass
`--as-json` to give the value as a JSON literal, for example `null`, an array or an object.

```text
gest task meta set [OPTIONS] <ID> <PATH> <VALUE>
```

| Argument  | Description                                 |
|-----------|---------------------------------------------|
| `<ID>`    | Task ID or unique prefix                    |
| `<PATH>`  | Dot-delimited key path (e.g. `outer.inner`) |
| `<VALUE>` | Value to set                                |

| Flag          | Description                                                 |
|---------------|-------------------------------------------------------------|
| `--as-json`   | Parse the value as a JSON literal instead of auto-detecting |
| `-j, --json`  | Output as JSON                                              |
| `-q, --quiet` | Print only the entity ID                                    |
| `-r, --raw`   | Output plain text for scripts, with no styling              |

### meta unset

Remove a metadata value. The hidden alias `delete` runs the same command.

```text
gest task meta unset [OPTIONS] <ID> <PATH>
```

| Argument | Description                                 |
|----------|---------------------------------------------|
| `<ID>`   | Task ID or unique prefix                    |
| `<PATH>` | Dot-delimited key path (e.g. `outer.inner`) |

| Flag          | Description                                    |
|---------------|------------------------------------------------|
| `-j, --json`  | Output as JSON                                 |
| `-q, --quiet` | Print only the entity ID                       |
| `-r, --raw`   | Output plain text for scripts, with no styling |

### Examples

```sh
# Set a metadata field
gest task meta set abc123 estimate "3 hours"

# Read it back
gest task meta get abc123 estimate

# JSON output
gest task meta get abc123 estimate --json

# Raw value (no styling)
gest task meta get abc123 estimate --raw

# Remove the field
gest task meta unset abc123 estimate
```

---

## task note

Manage the notes on a task. A note is an entry with a time and an author. Use notes to
record decisions, progress and observations, as you use comments on a GitHub issue.

```text
gest task note <COMMAND>
```

| Command                  | Description              |
|--------------------------|--------------------------|
| [`add`](#note-add)       | Add a note to a task     |
| [`list`](#note-list)     | List all notes on a task |
| [`show`](#note-show)     | Show a single note       |
| [`update`](#note-update) | Update a note's body     |
| [`delete`](#note-delete) | Delete a note            |

### note add

Add a note to a task. The `--body` flag is required, and the value `-` opens `$EDITOR` for
the body. An empty body makes the command exit with code 70. Without `--agent`, the author is
the `user.name` and `user.email` of `git config`, or `$USER` when git has no user name.

```text
gest task note add [OPTIONS] --body <BODY> <ID>
```

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Task ID or unique prefix |

| Flag                | Description                                     |
|---------------------|-------------------------------------------------|
| `--agent <AGENT>`   | Set the author (agent) identifier for this note |
| `-b, --body <BODY>` | Note body (required; use `-` to open `$EDITOR`) |
| `-j, --json`        | Output as JSON                                  |
| `-q, --quiet`       | Print only the note ID                          |
| `-r, --raw`         | Output plain text for scripts, with no styling  |

### note list

List all notes on a task, newest first.

```text
gest task note list [OPTIONS] <ID>
```

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Task ID or unique prefix |

| Flag          | Description                                    |
|---------------|------------------------------------------------|
| `--limit <N>` | Cap the number of items returned               |
| `-j, --json`  | Output as JSON                                 |
| `-q, --quiet` | Print only note IDs                            |
| `-r, --raw`   | Output plain text for scripts, with no styling |

### note show

Show a single note with its author and its rendered Markdown body.

```text
gest task note show [OPTIONS] <ID>
```

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Note ID or unique prefix |

| Flag          | Description                                    |
|---------------|------------------------------------------------|
| `-j, --json`  | Output as JSON                                 |
| `-q, --quiet` | Print only the note ID                         |
| `-r, --raw`   | Output plain text for scripts, with no styling |

### note update

Replace the body of a note. Without `--body`, or with `--body -`, the command opens
`$EDITOR` with the current body.

```text
gest task note update [OPTIONS] <ID>
```

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Note ID or unique prefix |

| Flag                | Description                                    |
|---------------------|------------------------------------------------|
| `-b, --body <BODY>` | New body text (use `-` to open `$EDITOR`)      |
| `-j, --json`        | Output as JSON                                 |
| `-q, --quiet`       | Print only the note ID                         |
| `-r, --raw`         | Output plain text for scripts, with no styling |

### note delete

Delete a note from a task.

```text
gest task note delete [OPTIONS] <ID>
```

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Note ID or unique prefix |

| Flag          | Description                                    |
|---------------|------------------------------------------------|
| `--yes`       | Skip the interactive confirmation prompt       |
| `-j, --json`  | Output as JSON                                 |
| `-q, --quiet` | Suppress normal output                         |
| `-r, --raw`   | Output plain text for scripts, with no styling |

### Examples

```sh
# Add a human note (author from git config)
gest task note add abc123 --body "Found the root cause in the parser"

# Add an agent note
gest task note add abc123 --agent claude --body "Completed code review, no issues found"

# Open $EDITOR for the note body
gest task note add abc123 --body -

# List notes
gest task note list abc123

# Show a specific note (by note ID)
gest task note show nfkbqmrx

# Update a note
gest task note update nfkbqmrx --body "Updated analysis"

# Delete a note (non-interactive)
gest task note delete nfkbqmrx --yes
```
