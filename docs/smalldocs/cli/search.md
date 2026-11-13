---
title: gest search
description: gest search — full-text search across tasks, artifacts, and iterations with structured filters by type, status, and tag.
---

Search the tasks, artifacts and iterations of the current project by keyword. Gest matches the query
against titles, descriptions and bodies. Filter prefixes in the query narrow the results by type, by
status or by tag. `gest grep` is a hidden alias of the command.

## Usage

```text
gest search [OPTIONS] <QUERY>
```

## Arguments

| Argument  | Description                                                                        |
|-----------|------------------------------------------------------------------------------------|
| `<QUERY>` | Text and filter expressions matched against titles, descriptions, and body content |

## Query syntax

A query holds **free text** and **filter expressions**, with spaces between them. A filter expression
has the form `prefix:value` and limits the results by one field. Gest joins all the words of free text
with single spaces, and matches the result as one phrase against titles, descriptions and bodies.

### Filter prefixes

| Prefix    | Description                                             | Example       |
|-----------|---------------------------------------------------------|---------------|
| `is:`     | Filter by entity type (`task`, `artifact`, `iteration`) | `is:task`     |
| `tag:`    | Filter by tag name                                      | `tag:urgent`  |
| `status:` | Filter by status                                        | `status:open` |

Tags give an artifact its category in v0.5.0. Use `tag:spec` in place of the removed filter
`type:spec`. A word with an unknown prefix, such as `type:spec`, is free text.

Artifacts have no status. So a `status:` filter matches no artifact, and gest ignores a negated
`status:` filter for artifacts.

Prefixes and values ignore case, so `IS:Task`, `is:task` and `Is:TASK` give the same result.

### Negation

Put `-` before a filter to **exclude** the items that match it:

```text
-tag:wip          # exclude items tagged "wip"
-status:done      # exclude items with status "done"
-is:artifact      # exclude artifacts from results
```

### Combination rules

Filters combine by these rules:

- **Filters with the same prefix** combine with OR. For example, `is:task is:artifact` matches tasks
  *or* artifacts.
- **Filters with different prefixes** combine with AND. For example, `is:task tag:urgent` matches
  tasks that *also* have the tag "urgent".
- **Free text** combines with the filters by AND. For example, `is:task fix login` matches tasks whose
  title or description contains the phrase "fix login".
- **Negated filters** combine with all the rest by AND. For example, `tag:urgent -status:done` matches
  urgent items that are *not* done.

## Options

| Flag            | Description                                         |
|-----------------|-----------------------------------------------------|
| `-a, --all`     | Include archived and resolved items                 |
| `-e, --expand`  | Show the full content of each result, not truncated |
| `-j, --json`    | Emit results as JSON                                |
| `--limit <N>`   | Cap the number of items returned                    |
| `-v, --verbose` | Increase verbosity (repeatable)                     |
| `-h, --help`    | Print help                                          |

On a terminal, gest sends a long list of results through a pager. Gest uses the `pager.command`
config value, then `$GEST_PAGER`, then `$PAGER`, then `less -R`. Gest prints directly to stdout in
these cases:

- stdout is not a terminal, for example a pipe or a file
- you pass `--no-pager`, or set `pager.enabled` to `false`
- the output fits on the screen

## Examples

```sh
# Basic keyword search
gest search "authentication"

# Filter to tasks only
gest search "is:task"

# Tasks tagged "urgent"
gest search "is:task tag:urgent"

# Tasks or artifacts matching "login"
gest search "is:task is:artifact login"

# Exclude work-in-progress items
gest search "tag:api -tag:wip"

# Open tasks about migrations
gest search "is:task status:open migration"

# Filter to iterations
gest search "is:iteration"

# Artifacts tagged "spec"
gest search "is:artifact tag:spec"

# Combine negation with free text
gest search "is:task -status:done fix auth"

# Include archived/resolved items
gest search "login" --all

# Expanded detail view
gest search "is:task tag:urgent" --expand

# JSON output for scripting
gest search "status:open" --json
```

## Exit codes

| Code | When                                                                 |
|------|----------------------------------------------------------------------|
| 0    | Success (including zero matches)                                     |
| 2    | The parser rejected the command line, for example a missing query    |
| 65   | Could not serialize results to JSON                                  |
| 74   | Store I/O error                                                      |
| 78   | Not a gest project (run `gest init`)                                 |

See [Exit Codes](./exit-codes.md) for the full contract.
