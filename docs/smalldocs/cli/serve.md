---
title: gest serve
description: gest serve — start the built-in web dashboard to browse tasks, artifacts, iterations, and kanban boards while parallel AI agents run.
---

Alias: `gest s`

Start the built-in web dashboard, where you can browse and manage the tasks, artifacts and iterations
of your project. Agents work with gest through the CLI and its `--json` output. The dashboard gives a
person a view of all the work. The view helps most when several agents work in parallel on one
iteration.

The dashboard has these views:

- an overview of status
- lists of tasks and artifacts that you can filter, with rendered Markdown
- the detail of each iteration, with its tasks grouped by phase
- a kanban board that shows the progress of tasks
- a full-text search across all entities

## Usage

```text
gest serve [OPTIONS]
```

## Options

| Flag                          | Description                                                                             |
|-------------------------------|-----------------------------------------------------------------------------------------|
| `-b, --bind <BIND_ADDRESS>`   | Address to bind to (overrides `[serve].bind_address`, default `127.0.0.1`)              |
| `--debounce-ms <DEBOUNCE_MS>` | File watcher debounce in milliseconds (overrides `[serve].debounce_ms`, default `2000`) |
| `--no-open`                   | Do not automatically open the browser                                                   |
| `-p, --port <PORT>`           | Port to listen on (overrides `[serve].port`, default `2300`)                            |
| `-v, --verbose`               | Increase verbosity (repeatable)                                                         |
| `-h, --help`                  | Print help                                                                              |

The flag `--bind` also has the hidden alias `--host`. To stop the browser from opening by default, set
`[serve].open` to `false`.

## Views

| Path                    | Description                                              |
|-------------------------|----------------------------------------------------------|
| `/`                     | Dashboard with entity counts and status breakdown        |
| `/tasks`                | Task list with status, priority, tags, and blocking info |
| `/tasks/:id`            | Task detail with description, links, and metadata        |
| `/artifacts`            | Artifact list with tags and archive status               |
| `/artifacts/:id`        | Artifact detail with rendered Markdown body              |
| `/iterations`           | Iteration list with status and phase count               |
| `/iterations/:id`       | Iteration detail with tasks grouped by phase             |
| `/iterations/:id/board` | Kanban board with columns mapped to task status          |
| `/search?q=...`         | Full-text search across tasks, artifacts, and iterations |

## Examples

```sh
# Start with defaults (localhost:2300, auto-open browser)
gest serve

# Custom port, no browser
gest serve --port 8080 --no-open

# Bind to all interfaces
gest serve --bind 0.0.0.0
```

## Exit codes

| Code | When                                                                         |
|------|------------------------------------------------------------------------------|
| 0    | Success (graceful shutdown)                                                  |
| 2    | The parser rejected the command line, for example an invalid address or port |
| 74   | Could not bind to the requested port or open the store                       |
| 78   | Not a gest project (run `gest init`)                                         |

See [Exit Codes](./exit-codes.md) for the full contract.
