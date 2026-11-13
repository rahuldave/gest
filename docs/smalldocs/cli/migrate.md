---
title: gest migrate
---

Import the data of a v0.4.x `.gest/` directory into the SQLite database of the current project.

Gest v0.4.x kept each kind of entity in its own directory of TOML and Markdown files. Gest v0.5.0
moved all the data into one SQLite database. `gest migrate --from v0.4` reads the old directories and
inserts their tasks, artifacts and iterations into the database. It also brings over their tags,
notes, links, metadata and iteration assignments. Each imported entity gets a new ID.

If the current directory has no project, the command creates one.

## Usage

```text
gest migrate [OPTIONS] --from <FROM>
```

## Options

| Flag             | Description                                                          |
|------------------|----------------------------------------------------------------------|
| `--from <FROM>`  | The source format version to migrate from. Only `v0.4` is supported. |
| `--path <PATH>`  | Path to the legacy data directory (defaults to auto-discovery).      |
| `-v, --verbose`  | Increase verbosity (repeatable).                                     |
| `-h, --help`     | Print help.                                                          |

Without `--path`, gest looks for a legacy directory in this order:

1. `.gest/` in the current directory.
2. `.gest/` in each parent directory.
3. The directory for the current path in the global gest data directory.

A legacy directory is a `.gest/` directory that holds `tasks/`, `artifacts/` or `iterations/`.

## Examples

Find the v0.4.x `.gest/` directory of the current project, and migrate all its data into the new
database:

```sh
gest migrate --from v0.4
```

Migrate from a specific path:

```sh
gest migrate --from v0.4 --path ~/projects/myapp/.gest
```

## How it works

The migration handles the kinds of entity in this order:

1. Tasks, from `tasks/` and `tasks/resolved/`.
2. Artifacts, from `artifacts/` and `artifacts/archive/`. An archived artifact stays archived, and the
   legacy `type` of an artifact becomes one more tag.
3. Iterations, from `iterations/`.

For each file, gest parses the v0.4.x format and inserts a new row. If gest cannot parse a file, it
skips the file, logs a warning and counts it. At the end, the command prints the number of each kind
of entity that it migrated, and the number of skipped files.

The migration does not run in one database transaction. If it stops with an error, the entities that
it imported before the error stay in the database. `gest undo` does not undo a migration.

Gest leaves the old directories in place, so you can check the result before you delete them. If
`storage.sync` is on, the next command writes the new data as YAML and Markdown files into new
directories of `.gest/`, for example `task/` and `artifact/`. The legacy directories `tasks/`,
`artifacts/` and `iterations/` stay until you delete them.

## Exit codes

| Code | When                                                                       |
|------|----------------------------------------------------------------------------|
| 0    | Success, also with skipped files or warnings about dangling references     |
| 2    | The parser rejected the command line, for example an unknown `--from`      |
| 66   | The directory that `--path` names does not exist                           |
| 74   | Auto-discovery found no legacy directory, or a read or a write failed      |
| 78   | Could not load user config                                                 |

See [Exit Codes](./exit-codes.md) for the full contract.

## Troubleshooting

A warning such as `could not resolve link ref: tasks/<id>` means that the legacy data has a link to an
entity that gest could not find. The migration continues past the warning. The new database then
has fewer links than the source directory.

## See also

[Migrating from v0.4.x to v0.5.0](../migration/v0-4-to-v0-5.md) gives the migration step by step. It
has a checklist for before the migration, and a table of how v0.4.x data maps into the v0.5.0
tables. It also gives the checks after the migration, and the steps to roll back.
