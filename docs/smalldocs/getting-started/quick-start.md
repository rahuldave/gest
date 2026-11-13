---
title: Quick Start
description: Quick start for gest. Initialize a store, create tasks and artifacts, link them into iterations, and run parallel AI agents across phased workspaces.
---

This guide shows the core workflow of gest. You set up a store, create tasks and artifacts, link them
together and search them.

## Initialize a store

Set up a data store before you use gest. By default, gest uses a global store in the data directory of
your system, `~/.local/share/gest/` on Linux or `~/Library/Application Support/gest/` on macOS:

```sh
gest init
```

To keep the data inside your repository instead, use the `--local` flag. It creates a `.gest/`
directory in the current project:

```sh
gest init --local
```

## Create a task

A task tracks one unit of work. Create one with a title:

```sh
gest task create "Implement auth middleware"
```

You can add details on the same line. The `--phase` flag needs an iteration, so create one first with
`gest iteration create`:

```sh
gest task create "Add rate limiting" \
  -d "Implement token-bucket rate limiting on API endpoints" \
  -p 1 \
  -s open \
  --tag "api,security" \
  --iteration <iteration-id> \
  --phase 2
```

- `-d` sets the description. Without it, gest opens your `$EDITOR`.
- `-p` sets the priority, from `0` (highest) to `4` (lowest).
- `-s` sets the first status, which is `open`, `in-progress`, `done` or `cancelled`.
- `--tag` adds tags. Repeat the flag, or separate the tags with commas.
- `--iteration` adds the task to an iteration, and `--phase` gives its phase in that iteration.

List your tasks:

```sh
gest task list
```

## Create an artifact

An artifact stores a document, such as a spec, an ADR, an RFC or a note. Tags give each artifact its
category. You can create an artifact from a file:

```sh
gest artifact create "Auth spec" --source auth-spec.md --tag spec --tag auth
```

Or write the body on the command line:

```sh
gest artifact create "Rate Limiting Design" \
  -b "Token-bucket algorithm with per-user quotas." \
  --tag adr \
  --tag api \
  --tag design
```

- The title is the first argument. When you pipe the body on standard input, you can leave out the
  title, and gest takes the first `#` heading of the body.
- `-b` gives the body.
- `--tag` or `-t` adds a tag. Tags such as `spec`, `adr`, `rfc` and `note` give the category.
- `--source` or `-s` reads the body from a file.

List the artifacts:

```sh
gest artifact list
```

## Link entities

You can link a task to another task or to an artifact. gest has these types of relationship:

- `blocks`
- `blocked-by`
- `child-of`
- `parent-of`
- `relates-to`

Give the type with `--rel`. Without it, the type is `relates-to`. Link two tasks:

```sh
# Use the task ID or a unique prefix
gest task link <task-id> <other-task-id> --rel blocked-by
```

gest stores the link once. Both tasks list it under `relationships` in their JSON.

Link a task to an artifact with the `--artifact` flag:

```sh
gest task link <task-id> <artifact-id> --rel relates-to --artifact
```

Show the details of a task as JSON to see its links:

```sh
gest task show <task-id> --json
```

## Search

Search tasks, artifacts and iterations by keyword:

```sh
gest search "auth"
```

Add `--expand` to see the full details of each result:

```sh
gest search "auth" --expand
```

Use `--json` to get output for scripts:

```sh
gest search "auth" --json
```

By default, the search leaves out resolved tasks and archived artifacts. Pass `--all` to include them:

```sh
gest search "auth" --all
```

## Next steps

- Read [Core Concepts](./concepts.md) for the data model in depth.
- See the [CLI reference](../cli/task.md) for the full set of commands.
