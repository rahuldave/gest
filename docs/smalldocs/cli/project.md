---
title: gest project
---

Show or manage the project of the current working directory.

In gest v0.5.0, each tracked project is a row in the `projects` table, with its root path as the key.
`gest init` creates the row, and the `gest project` commands show and change it.

## Usage

```text
gest project [OPTIONS] [COMMAND]
```

### Options (bare `gest project`)

With no subcommand, `gest project` shows the details of the project of the current directory. Outside
a project, it fails with exit code 78.

| Flag     | Description                                                 |
|----------|-------------------------------------------------------------|
| `--json` | Emit output as JSON (only applies to the default show view) |

## Subcommands

| Command                           | Aliases | Description                                                        |
|-----------------------------------|---------|--------------------------------------------------------------------|
| [`archive`](#project-archive)     |         | Soft-archive a project, hiding it from default list views          |
| [`attach`](#project-attach)       |         | Attach the current directory to an existing project as a workspace |
| [`delete`](#project-delete)       | `rm`    | Delete a project and all of its owned entities                     |
| [`detach`](#project-detach)       |         | Detach the current directory from its project                      |
| [`list`](#project-list)           | `ls`    | List all known projects                                            |
| [`unarchive`](#project-unarchive) |         | Restore an archived project to active status                       |

The alias `gest p` runs `gest project`.

## Exit codes

| Code | When                                                                         |
|------|------------------------------------------------------------------------------|
| 0    | Success                                                                      |
| 2    | The parser rejected the command line, for example a missing argument         |
| 64   | The ID prefix holds characters that no ID can have                           |
| 65   | Could not serialize the `Envelope<Project>` to JSON                          |
| 66   | Project id did not resolve, or `detach` found no workspace                   |
| 74   | Store I/O error, or `attach` got an ID that is malformed or names no project |
| 78   | Could not load user config, or bare `gest project` ran outside a project     |

See [Exit Codes](./exit-codes.md) for the full contract.

---

## project archive

Soft-archive a project. The command detaches all the workspaces of the project, and hides its
entities from the default list views. The project and its data stay in the database, and
[`unarchive`](#project-unarchive) restores the project.

A confirmation prompt shows the number of workspaces, tasks, iterations and artifacts that the
command changes.

```text
gest project archive [OPTIONS] <ID>
```

### Arguments

| Argument | Description                 |
|----------|-----------------------------|
| `<ID>`   | Project ID or unique prefix |

### Options

| Flag          | Description                              |
|---------------|------------------------------------------|
| `--yes`       | Skip the interactive confirmation prompt |
| `-j, --json`  | Emit output as JSON                      |
| `-q, --quiet` | Print only the project ID                |
| `-r, --raw`   | Emit script-friendly plain output        |

### Examples

```sh
gest project archive abc123
gest project archive --yes abc123
```

---

## project attach

Attach the current directory to an existing project as one more workspace. A workspace is a
directory that shares the data of the project. Attach a directory when you have several checkouts of
the same repository, for example jj workspaces or git worktrees, and want them all to share the same
entity data.

The command needs the full 32-character project ID. It does not accept a prefix, although its
`--help` says that it does. To find the full ID, run `gest project list --json`.

```text
gest project attach <ID>
```

---

## project delete

Delete a project and all the entities that it owns. The delete is permanent, and it removes every
task, iteration and artifact of the project, with all their notes, tags and relationships. Gest
writes tombstone files, so that a later sync import does not bring the deleted data back.

You cannot reverse the delete with `gest undo`.

```text
gest project delete [OPTIONS] <ID>
```

### Arguments

| Argument | Description                 |
|----------|-----------------------------|
| `<ID>`   | Project ID or unique prefix |

### Options

| Flag          | Description                              |
|---------------|------------------------------------------|
| `--yes`       | Skip the interactive confirmation prompt |
| `-j, --json`  | Emit output as JSON                      |
| `-q, --quiet` | Print only the project ID                |
| `-r, --raw`   | Emit script-friendly plain output        |

### Examples

```sh
gest project delete abc123
gest project rm --yes abc123
```

---

## project detach

Remove the current directory from the list of workspaces of its project. The command does not delete
the project. It removes only the link between the directory and the project.

```text
gest project detach
```

---

## project list

List every project in the database. By default, the list leaves out archived projects.

```text
gest project list [OPTIONS]
```

### Options

| Flag          | Description                              |
|---------------|------------------------------------------|
| `-a, --all`   | Include archived projects in the listing |
| `--limit <N>` | Cap the number of items returned         |
| `-j, --json`  | Emit output as JSON                      |
| `-q, --quiet` | Print only project IDs                   |
| `-r, --raw`   | Emit script-friendly plain output        |

### Examples

```sh
gest project list
gest project list --all
gest project list --json
gest project list --limit 5
```

---

## project unarchive

Restore an archived project to active status. The command clears the `archived_at` timestamp of the
project. It does not restore the workspace paths, so attach them again with
[`gest project attach`](#project-attach).

```text
gest project unarchive [OPTIONS] <ID>
```

### Arguments

| Argument | Description                 |
|----------|-----------------------------|
| `<ID>`   | Project ID or unique prefix |

The command takes the output flags `-j, --json`, `-q, --quiet` and `-r, --raw`.

### Examples

```sh
gest project unarchive abc123
```

---

## Examples

Show the project for the current directory:

```sh
gest project
```

List every project:

```sh
gest project list
```

Attach a secondary checkout:

```sh
cd ../myapp-workspace-2
gest project attach <project-id>
```
