---
title: gest init
description: gest init — register a project with gest, or pass --local to keep the SQLite cache and synced project files under .gest/.
---

Initialize gest for the current directory. The command registers a project with the current directory
as its root. Then each `gest` command that runs in the directory tree finds the same project. If the
directory already has a project, the command reuses it.

By default, the command creates no files in the project. Your data lives in the shared global
database, together with every other project on the machine. Pass `--local` to create a `.gest/`
directory in the current project. A local project uses `.gest/gest.db` as its SQLite cache by default.
When `storage.sync` is on, each command also syncs the database with YAML and Markdown files in
`.gest/`.

## Usage

```text
gest init [OPTIONS]
```

## Options

| Flag            | Description                                                                                                |
|-----------------|------------------------------------------------------------------------------------------------------------|
| `--local`       | Also create a `.gest/` directory in the current project to materialize the sync mirror alongside your code |
| `-v, --verbose` | Increase verbosity (repeatable)                                                                            |
| `-h, --help`    | Print help                                                                                                 |

## Examples

Initialize with global storage only (default):

```sh
gest init
```

Initialize with a local `.gest/` sync mirror:

```sh
gest init --local
```

## Exit codes

| Code | When                                                                 |
|------|----------------------------------------------------------------------|
| 0    | Success                                                              |
| 2    | The parser rejected the command line, for example an unknown flag    |
| 74   | Could not create the database or `.gest/` mirror                     |
| 78   | Could not load user config or resolve `data_dir`                     |

See [Exit Codes](./exit-codes.md) for the full contract.

## See also

- [Configuration](../configuration/index.md) explains how `data_dir`, `storage.sync` and the
  `[database]` section work together.
- [FAQ: global vs local stores](../faq.md#whats-the-difference-between-global-and-local-stores)
