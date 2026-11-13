---
title: gest artifact
description: gest artifact — create, update, and manage spec, ADR, and RFC artifacts alongside your project with gest, the agentic workflow CLI.
---

An artifact is a Markdown document that gest stores with your project, for example a spec, an ADR or an
RFC. The `gest artifact` commands create, list, update and delete artifacts.

## Usage

```text
gest artifact <COMMAND> [OPTIONS]
```

## Subcommands

| Command                        | Aliases | Description                               |
|--------------------------------|---------|-------------------------------------------|
| [`archive`](#artifact-archive) |         | Archive an artifact                       |
| [`create`](#artifact-create)   | `new`   | Create a new artifact                     |
| [`delete`](#artifact-delete)   | `rm`    | Delete an artifact and its dependent rows |
| [`list`](#artifact-list)       | `ls`    | List artifacts with optional filters      |
| [`show`](#artifact-show)       | `view`  | Display an artifact's full details        |
| [`update`](#artifact-update)   | `edit`  | Update an artifact's fields               |
| [`tag`](#artifact-tag)         |         | Add one tag to an artifact                |
| [`untag`](#artifact-untag)     |         | Remove one tag from an artifact           |
| [`meta`](#artifact-meta)       |         | Read, write or remove metadata fields     |
| [`note`](#artifact-note)       |         | Manage notes attached to an artifact      |

Each command that acts on an artifact or a note takes the output flags `-j, --json`, `-q, --quiet` and
`-r, --raw`. The flag `--raw` prints plain output with no themed styling, for scripts.

## Exit codes

| Code | When                                                                             |
|------|----------------------------------------------------------------------------------|
| 0    | Success                                                                          |
| 2    | The parser rejected the command line, for example an unknown flag                |
| 64   | Missing title, malformed NDJSON batch input, or a bad `--metadata` value         |
| 65   | `meta set --as-json` got a value that is not valid JSON                          |
| 66   | The artifact ID, note ID or metadata key did not resolve                         |
| 70   | The editor failed, an edited body was empty, or a `--source` file was unreadable |
| 74   | Store I/O error                                                                  |
| 78   | Not a gest project (run `gest init`)                                             |

See [Exit Codes](./exit-codes.md) for the full contract.

---

## artifact archive

Archive an artifact. The command sets the `archived_at` timestamp of the artifact. Then
`gest artifact list` hides the artifact, unless you pass `--all` or `--archived`.

```text
gest artifact archive [OPTIONS] <ID>
```

### Arguments

| Argument | Description                  |
|----------|------------------------------|
| `<ID>`   | Artifact ID or unique prefix |

### Options

| Flag          | Description                         |
|---------------|-------------------------------------|
| `-j, --json`  | Output as JSON                      |
| `-q, --quiet` | Print only the artifact ID          |
| `-r, --raw`   | Plain output with no themed styling |

### Examples

```sh
gest artifact archive abc123
```

---

## artifact create

Create a new artifact. The body comes from inline text, a source file, an editor or stdin.

If you give a title, gest takes the body from the first of these sources that is present:

1. The file that `--source` names.
2. The text of `--body`.
3. `$EDITOR`, when stdin is a terminal.
4. Stdin, when it is a pipe.

If you give no title and stdin is a pipe, gest reads stdin. The first `#` heading becomes the title, and
the whole input becomes the body. Gest does not take a title from a `--source` file, so give a title
with `--source`.

```text
gest artifact create [OPTIONS] [TITLE]
```

### Arguments

| Argument  | Description                                                                         |
|-----------|-------------------------------------------------------------------------------------|
| `[TITLE]` | Artifact title (taken from the first `#` heading when you pipe stdin with no title) |

### Options

| Flag                          | Description                                                                         |
|-------------------------------|-------------------------------------------------------------------------------------|
| `--batch`                     | Read NDJSON from stdin (one artifact per line)                                      |
| `-b, --body <BODY>`           | Body content as an inline string (skips editor and stdin)                           |
| `-i, --iteration <ITERATION>` | Add the artifact to an iteration (ID or prefix)                                     |
| `-j, --json`                  | Output the created artifact as JSON                                                 |
| `-m, --metadata <KEY=VALUE>`  | Set a metadata key=value pair (repeatable; supports dot-paths and scalar inference) |
| `--metadata-json <JSON>`      | Merge a JSON object into metadata (repeatable; applied after `--metadata` pairs)    |
| `-q, --quiet`                 | Print only the artifact ID                                                          |
| `-r, --raw`                   | Plain output with no themed styling                                                 |
| `-s, --source <SOURCE>`       | Read body content from a file path (not together with `--body`)                     |
| `-t, --tag <TAG>`             | Tag (repeatable; commas split one value into several tags)                          |

The flag `--batch` takes no title, and no other option that sets the content of the artifact.

Tags give an artifact its category. Version 0.5.0 removed the `--type`/`-k` flag and the `kind`
field. Tag your artifacts with a category such as `spec`, `adr` or `rfc`, and filter the list with
`--tag`.

### Examples

```sh
# Create from inline body
gest artifact create "Auth Spec" --tag spec --body "## Overview\nAuth flow details..."

# Create from a file (give a title: gest does not read one from the file)
gest artifact create "Storage ADR" --tag adr --source docs/decisions/001-storage.md

# Create interactively (opens $EDITOR)
gest artifact create "My RFC" --tag rfc --tag backend --tag v2

# Add to an iteration
gest artifact create "Sprint 3 Notes" --tag note -i iter123

# Pipe body from stdin
echo "# My Spec\nDetails..." | gest artifact create --tag spec

# Batch-create artifacts from NDJSON
cat artifacts.ndjson | gest artifact create --batch

# Machine-readable output
gest artifact create "Quick note" --tag note --json
gest artifact create "Quick note" --tag note -q
```

---

## artifact delete

Delete an artifact and its dependent rows, which are its tags, notes and links. The command asks for
confirmation first. To hide an artifact from the list and keep it, use [`archive`](#artifact-archive)
instead. If you delete an artifact by mistake, `gest undo` can restore it.

```text
gest artifact delete [OPTIONS] <ID>
```

### Arguments

| Argument | Description                  |
|----------|------------------------------|
| `<ID>`   | Artifact ID or unique prefix |

### Options

| Flag          | Description                                                                     |
|---------------|---------------------------------------------------------------------------------|
| `--yes`       | Skip the interactive confirmation prompt                                        |
| `--force`     | Reserved for future guards; currently a no-op (artifacts have no guards today)  |
| `-j, --json`  | Output as JSON                                                                  |
| `-q, --quiet` | Print only the artifact ID                                                      |
| `-r, --raw`   | Plain output with no themed styling                                             |

### Examples

```sh
# Interactive (prompts for confirmation)
gest artifact delete abc123

# Non-interactive (scripts, CI)
gest artifact delete abc123 --yes
```

---

## artifact list

List artifacts, newest first. You can filter the list by tag or by archive status.

```text
gest artifact list [OPTIONS]
```

### Options

| Flag              | Description                                             |
|-------------------|---------------------------------------------------------|
| `-a, --all`       | Include archived artifacts alongside active ones        |
| `--archived`      | Show only archived artifacts (alias: `--archived-only`) |
| `--limit <N>`     | Cap the number of items returned                        |
| `-j, --json`      | Output as JSON                                          |
| `-q, --quiet`     | Print only artifact IDs                                 |
| `-r, --raw`       | Plain output with no themed styling                     |
| `-t, --tag <TAG>` | Filter by tag                                           |

### Examples

```sh
# List active artifacts
gest artifact list

# Filter by tag (use category tags like `spec`, `adr`, `rfc`)
gest artifact list --tag spec

# Include archived
gest artifact list --all

# JSON output for scripting
gest artifact list --json
```

---

## artifact show

Display the full details of an artifact, with its rendered body.

```text
gest artifact show [OPTIONS] <ID>
```

### Arguments

| Argument | Description                  |
|----------|------------------------------|
| `<ID>`   | Artifact ID or unique prefix |

### Options

| Flag          | Description                                |
|---------------|--------------------------------------------|
| `-j, --json`  | Output as JSON instead of formatted detail |
| `-q, --quiet` | Print only the artifact ID                 |
| `-r, --raw`   | Plain output with no themed styling        |

### Examples

```sh
gest artifact show abc123
gest artifact show abc123 --json
```

---

## artifact update

Change the title, the body, the metadata or the tags of an artifact.

```text
gest artifact update [OPTIONS] <ID>
```

### Arguments

| Argument | Description                  |
|----------|------------------------------|
| `<ID>`   | Artifact ID or unique prefix |

### Options

| Flag                         | Description                                                                         |
|------------------------------|-------------------------------------------------------------------------------------|
| `-b, --body <BODY>`          | Replace the body content                                                            |
| `-e, --edit`                 | Open `$EDITOR` pre-filled with the current body                                     |
| `-j, --json`                 | Output as JSON                                                                      |
| `-m, --metadata <KEY=VALUE>` | Set a metadata key=value pair (repeatable; supports dot-paths and scalar inference) |
| `--metadata-json <JSON>`     | Merge a JSON object into metadata (repeatable; applied after `--metadata` pairs)    |
| `-q, --quiet`                | Print only the artifact ID                                                          |
| `-r, --raw`                  | Plain output with no themed styling                                                 |
| `-t, --tag <TAG>`            | Replace all tags (repeatable; commas split one value into several tags)             |
| `-T, --title <TITLE>`        | New title                                                                           |

If you save an empty body in the editor, gest stops with exit code 70 and keeps the old body.

### Examples

```sh
gest artifact update abc123 -T "Updated Title"
gest artifact update abc123 --tag approved --tag backend

# Machine-readable output
gest artifact update abc123 -T "New Title" --json
```

---

## artifact tag

Add one tag to an artifact. If the artifact already has the tag, nothing changes.

```text
gest artifact tag [OPTIONS] <ID> <LABEL>
```

### Arguments

| Argument  | Description                                      |
|-----------|--------------------------------------------------|
| `<ID>`    | Artifact ID or unique prefix                     |
| `<LABEL>` | The tag to add (a comma stays part of the label) |

### Options

| Flag          | Description                               |
|---------------|-------------------------------------------|
| `-j, --json`  | Output the artifact as JSON after tagging |
| `-q, --quiet` | Output only the artifact ID               |
| `-r, --raw`   | Plain output with no themed styling       |

To add several tags, run the command once for each tag. The other way is `gest artifact update --tag`,
which replaces all the tags of the artifact.

### Examples

```sh
gest artifact tag abc123 approved
gest artifact tag abc123 reviewed
```

---

## artifact untag

Remove one tag from an artifact.

```text
gest artifact untag [OPTIONS] <ID> <LABEL>
```

### Arguments

| Argument  | Description                  |
|-----------|------------------------------|
| `<ID>`    | Artifact ID or unique prefix |
| `<LABEL>` | The tag to remove            |

### Options

| Flag          | Description                                 |
|---------------|---------------------------------------------|
| `-j, --json`  | Output the artifact as JSON after untagging |
| `-q, --quiet` | Output only the artifact ID                 |
| `-r, --raw`   | Plain output with no themed styling         |

### Examples

```sh
gest artifact untag abc123 draft
```

---

## artifact meta

Read, write or remove the metadata fields of an artifact. A key path uses dots for nested values, as in
`outer.inner`. With an ID and no subcommand, `gest artifact meta <ID>` shows all the metadata of the
artifact.

```text
gest artifact meta [OPTIONS] [ID] [COMMAND]
```

Each `meta` subcommand takes the output flags `-j, --json`, `-q, --quiet` and `-r, --raw`.

### meta get

Print one metadata value.

```text
gest artifact meta get [OPTIONS] <ID> <PATH>
```

| Argument | Description                                 |
|----------|---------------------------------------------|
| `<ID>`   | Artifact ID or unique prefix                |
| `<PATH>` | Dot-delimited key path (e.g. `outer.inner`) |

| Flag         | Description                           |
|--------------|---------------------------------------|
| `-j, --json` | Output as a JSON object               |
| `-r, --raw`  | Output the bare value with no styling |

### meta set

Set a metadata value. Gest reads `true` and `false` as booleans, and an integer or a decimal as a
number. Every other value is a string, and so is `null`. To store `null`, an array or an object, pass
`--as-json` and give the value as JSON.

```text
gest artifact meta set [OPTIONS] <ID> <PATH> <VALUE>
```

| Argument  | Description                                    |
|-----------|------------------------------------------------|
| `<ID>`    | Artifact ID or unique prefix                   |
| `<PATH>`  | Dot-delimited key path (e.g. `config.timeout`) |
| `<VALUE>` | Value to set                                   |

| Flag          | Description                       |
|---------------|-----------------------------------|
| `--as-json`   | Parse the value as a JSON literal |
| `-j, --json`  | Output as JSON                    |
| `-q, --quiet` | Print only the entity ID          |

### meta unset

Remove a metadata value. If the key is not present, the command fails with exit code 66.

```text
gest artifact meta unset [OPTIONS] <ID> <PATH>
```

| Argument | Description                  |
|----------|------------------------------|
| `<ID>`   | Artifact ID or unique prefix |
| `<PATH>` | Dot-delimited key path       |

### Examples

```sh
# Set a metadata field
gest artifact meta set abc123 status "approved"

# Read it back
gest artifact meta get abc123 status

# JSON output
gest artifact meta get abc123 status --json

# Raw value (no styling)
gest artifact meta get abc123 status --raw
```

---

## artifact note

Manage the notes of an artifact. A note is a short Markdown text attached to an artifact. A note does
not change the body of the artifact. So notes suit running commentary, review threads and agent
annotations.

```text
gest artifact note <COMMAND>
```

### note add

Add a new note to an artifact. To write the note in `$EDITOR`, pass `--body -`.

```text
gest artifact note add [OPTIONS] --body <BODY> <ID>
```

| Argument | Description                  |
|----------|------------------------------|
| `<ID>`   | Artifact ID or unique prefix |

| Flag                | Description                                     |
|---------------------|-------------------------------------------------|
| `-b, --body <BODY>` | Note body (required; use `-` to open `$EDITOR`) |
| `--agent <AGENT>`   | Set the author (agent) identifier for this note |
| `-j, --json`        | Output as JSON                                  |
| `-q, --quiet`       | Print only the note ID                          |
| `-r, --raw`         | Plain output with no themed styling             |

### note list

List all the notes of an artifact, newest first.

```text
gest artifact note list [OPTIONS] <ID>
```

| Argument | Description                  |
|----------|------------------------------|
| `<ID>`   | Artifact ID or unique prefix |

| Flag          | Description                         |
|---------------|-------------------------------------|
| `--limit <N>` | Cap the number of items returned    |
| `-j, --json`  | Output as JSON                      |
| `-q, --quiet` | Print only note IDs                 |
| `-r, --raw`   | Plain output with no themed styling |

### note show

Display a single note.

```text
gest artifact note show [OPTIONS] <ID>
```

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Note ID or unique prefix |

| Flag          | Description                         |
|---------------|-------------------------------------|
| `-j, --json`  | Output as JSON                      |
| `-q, --quiet` | Print only the note ID              |
| `-r, --raw`   | Plain output with no themed styling |

### note update

Replace the body of a note. With no `--body`, or with `--body -`, gest opens the current body in
`$EDITOR`.

```text
gest artifact note update [OPTIONS] <ID>
```

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Note ID or unique prefix |

| Flag                | Description                               |
|---------------------|-------------------------------------------|
| `-b, --body <BODY>` | New body text (use `-` to open `$EDITOR`) |
| `-j, --json`        | Output as JSON                            |
| `-q, --quiet`       | Print only the note ID                    |
| `-r, --raw`         | Plain output with no themed styling       |

### note delete

Delete a note from an artifact.

```text
gest artifact note delete [OPTIONS] <ID>
```

| Argument | Description              |
|----------|--------------------------|
| `<ID>`   | Note ID or unique prefix |

| Flag          | Description                              |
|---------------|------------------------------------------|
| `--yes`       | Skip the interactive confirmation prompt |
| `-j, --json`  | Output as JSON                           |
| `-q, --quiet` | Suppress normal output                   |
| `-r, --raw`   | Plain output with no themed styling      |

### Examples

```sh
# Add a note inline
gest artifact note add abc123 --body "Reviewed by security team; approved."

# Add a note via $EDITOR
gest artifact note add abc123 --body -

# Attribute a note to an agent
gest artifact note add abc123 --body "Drafted outline" --agent implement-agent

# List notes for an artifact
gest artifact note list abc123
gest artifact note list abc123 --limit 5 --json

# Show and update a single note
gest artifact note show note456
gest artifact note update note456 --body "Updated commentary"

# Delete a note (non-interactive)
gest artifact note delete note456 --yes
```
