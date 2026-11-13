---
title: gest tag
---

Add, remove and list tags on tasks, artifacts and iterations, without the type of the entity. Gest
finds the entity from the ID prefix.

> [!NOTE]
> This command replaces the previous `gest tags` command, which only supported listing.

## Usage

```text
gest tag [OPTIONS] [COMMAND]
```

Running `gest tag` without a subcommand is equivalent to `gest tag list`.

## Subcommands

| Command                 | Description                                              |
|-------------------------|----------------------------------------------------------|
| [`add`](#tag-add)       | Add tags to any entity by ID prefix                      |
| [`remove`](#tag-remove) | Remove tags from any entity by ID prefix                 |
| [`list`](#tag-list)     | List all unique tags, optionally filtered by entity type |

## Exit codes

| Code | When                                                                 |
|------|----------------------------------------------------------------------|
| 0    | Success                                                              |
| 2    | The parser rejected the command line, for example a missing argument |
| 64   | The ID prefix holds characters that no ID can have                   |
| 66   | Entity id did not resolve, or was ambiguous across types             |
| 74   | Store I/O error                                                      |
| 78   | `add` or `remove` ran outside a gest project (run `gest init`)       |

See [Exit Codes](./exit-codes.md) for the full contract.

---

## tag add

Add one or more tags to an entity. Gest looks for the ID prefix in tasks, artifacts and iterations.
If the prefix matches more than one entity, the command fails with exit code 66.

```text
gest tag add [OPTIONS] <ID> <TAGS>...
```

### Arguments

| Argument    | Description                                          |
|-------------|------------------------------------------------------|
| `<ID>`      | Entity ID or unique prefix                           |
| `<TAGS>...` | Tags to add (at least one; space or comma-separated) |

The command takes the output flags `-j, --json`, `-q, --quiet` and `-r, --raw`.

### Examples

```sh
gest tag add zyxw rust cli    # tag a task (or artifact/iteration) with "rust" and "cli"
gest tag add zyxw rust,cli    # comma-separated form
```

---

## tag remove

Remove one or more tags from an entity. Gest finds the entity in the same way as `tag add`.

```text
gest tag remove [OPTIONS] <ID> <TAGS>...
```

### Arguments

| Argument    | Description                                             |
|-------------|---------------------------------------------------------|
| `<ID>`      | Entity ID or unique prefix                              |
| `<TAGS>...` | Tags to remove (at least one; space or comma-separated) |

The command takes the output flags `-j, --json`, `-q, --quiet` and `-r, --raw`.

### Examples

```sh
gest tag remove zyxw cli      # remove the "cli" tag from the matched entity
```

---

## tag list

List the tags in the store, in order of label. Tags are global, so the list holds the tags of every
project, and it also holds tags that no entity has now. The command works outside a project too.

To list only the tags on one type of entity, pass `--task`, `--artifact` or `--iteration`. Each of
these flags excludes the other two.

```text
gest tag list [OPTIONS]
```

### Options

| Option        | Description                       |
|---------------|-----------------------------------|
| `--task`      | Show only tags from tasks         |
| `--artifact`  | Show only tags from artifacts     |
| `--iteration` | Show only tags from iterations    |
| `--limit <N>` | Cap the number of items returned  |
| `-j, --json`  | Emit output as JSON               |
| `-q, --quiet` | Print only the tag labels         |
| `-r, --raw`   | Emit script-friendly plain output |

### Examples

```sh
gest tag list                    # all tags across all entity types
gest tag list --task             # only tags used on tasks
gest tag list --artifact         # only tags used on artifacts
gest tag list --limit 20 --json  # machine-readable output, capped
```
