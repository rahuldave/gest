---
title: Configuration
---

gest reads its settings from several TOML files and merges them when it runs. A file closer to your
working directory wins over the global file, so each project can set its own values.

## Config file locations

gest looks for configuration in two layers, a **global** file and **project** files.

### Global config

The global file is in the configuration directory of your platform:

| Platform | Path                                             |
|----------|--------------------------------------------------|
| Linux    | `~/.config/gest/config.toml`                     |
| macOS    | `~/Library/Application Support/gest/config.toml` |

The `GEST_CONFIG` environment variable sets another path for this file.

### Project config

gest checks every directory from the root of the file system down to your working directory. In each
directory, it looks for these project files, in this order:

1. `.config/gest.toml`
2. `.gest.toml`

gest merges all the files that it finds, table by table. A file closer to the working directory wins,
and in one directory `.gest.toml` wins over `.config/gest.toml`.

## Data storage: global vs local

gest stores the data of each project in SQLite, which includes the tasks, artifacts, iterations, notes,
events and relationships. A global project uses `<data_dir>/gest.db`. A local project, which
`gest init --local` sets up, uses `.gest/gest.db`. A configured data directory or remote database
replaces both defaults.

### Global data root

The global data root is the directory that holds `gest.db` for global projects. It also holds the
database when you set a data directory yourself. gest takes the first of these that is set:

1. The `$GEST_STORAGE__DATA_DIR` environment variable, which must be an absolute path.
2. `storage.data_dir` in a config file, which must be an absolute path.
3. The global data directory of the platform, `~/.local/share/gest/` on Linux or
   `~/Library/Application Support/gest/` on macOS. `$XDG_DATA_HOME/gest/` replaces it when
   `XDG_DATA_HOME` is set.

### Project resolution

The `projects` table of the database lists the projects by the path of their root. `gest init` adds a
row for the current working directory. After that, each `gest` command inside the tree walks up from
the working directory and matches the paths in the `projects` table to find the active project.

No setting names a separate project directory. The data of a project lives inside the SQLite database,
not in a subdirectory for each project.

### Global store (default)

By default, `gest init` records the project and creates no `.gest/` directory. All the data lives in
the global SQLite database at `<data_dir>/gest.db`, which every project on the machine shares. This
mode keeps gest files out of your project directory, and it suits personal use.

```sh
# Initialize a project that uses the global store only (default)
gest init
```

### Local store (sync mirror)

Use `gest init --local` to create a `.gest/` directory inside your project as well. When this directory
exists, gest keeps the SQLite database of the project at `.gest/gest.db`. A configured data directory
or remote database overrides this location.

With `storage.sync` on, which is the default, gest also syncs the database with YAML and Markdown files
in `.gest/` on every command, in both directions. Commit the YAML and Markdown files. Do not commit
`.gest/gest.db` or its WAL files, which SQLite writes next to it.

```sh
# Initialize with a local .gest/ sync mirror
gest init --local
```

## State storage

v0.5.0 has no separate directory for state. The active SQLite database holds the undo log, the
history of transactions, the digests of the sync and all other state of operation. The active database
is one of these:

- `.gest/gest.db` for a local project.
- `<data_dir>/gest.db` for a global project, or for a project with a data directory that you set.
- The remote database from `[database]`.

Each database has its own undo history. To share one undo history, point `gest` at a remote libsql URL.

## Configuration settings

[Theming](./theming.md) is a separate guide to the colours of the terminal interface.

### `[storage]`

| Key         | Type                   | Default           | Description                                                                                                                                                                                                                    |
|-------------|------------------------|-------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `cache_dir` | string (absolute path) | _(auto-resolved)_ | Override the global cache directory used for regeneratable artifacts such as the avatar cache. Must be an absolute path. Defaults to `$XDG_CACHE_HOME/gest`, or `~/.cache/gest` on Linux and `~/Library/Caches/gest` on macOS. |
| `data_dir`  | string (absolute path) | _(auto-resolved)_ | Override the global data root directory. Must be an absolute path.                                                                                                                                                             |
| `sync`      | boolean                | `true`            | Enable bidirectional sync between the SQLite database and a `.gest/` directory when one exists in the project root.                                                                                                            |

### `[database]`

gest v0.5.0 stores the data in a SQLite database, through the libsql library. A local project uses
`.gest/gest.db` by default. A global project, or a project with a data directory that you set, uses
`<data_dir>/gest.db`. To share data between devices, point gest at a remote libsql database instead.

Give the connection as a complete `url`, or as separate parts, which are `scheme`, `host`, `port`,
`username` and `password`. When you give both, the `url` wins. From the parts alone, gest builds a URL
of the form `scheme://[user[:pass]@]host[:port]`, and `scheme` defaults to `sqlite`.

| Key          | Type    | Default  | Description                                                        |
|--------------|---------|----------|--------------------------------------------------------------------|
| `auth_token` | string  | _none_   | Bearer or API token for the remote database.                       |
| `host`       | string  | _none_   | Database server hostname or IP address (used to synthesize a URL). |
| `password`   | string  | _none_   | Password for username/password authentication.                     |
| `port`       | integer | _none_   | Port number the database server listens on.                        |
| `scheme`     | string  | `sqlite` | Connection scheme used when synthesizing a URL from components.    |
| `url`        | string  | _none_   | Explicit connection URL. When set, component fields are ignored.   |
| `username`   | string  | _none_   | Username for database authentication.                              |

### `[serve]`

These settings control the built-in web dashboard of `gest serve`. The flags of `gest serve` override
them when the server runs.

| Key                | Type    | Default       | Description                                                                                                                  |
|--------------------|---------|---------------|------------------------------------------------------------------------------------------------------------------------------|
| `bind_address`     | string  | `127.0.0.1`   | IP address the server should bind to.                                                                                        |
| `csrf_signing_key` | string  | _(generated)_ | Hex-encoded 32-byte HMAC key used to sign CSRF cookies. Generated and persisted to the global config on first run if absent. |
| `debounce_ms`      | integer | `2000`        | File watcher debounce window in milliseconds.                                                                                |
| `log_level`        | string  | _none_        | Optional log level override applied only while `gest serve` is running.                                                      |
| `open`             | boolean | `true`        | Whether to automatically open the browser when the server starts.                                                            |
| `port`             | integer | `2300`        | Port the server should listen on.                                                                                            |

### `[log]`

| Key     | Type   | Default  | Description                                                                                                               |
|---------|--------|----------|---------------------------------------------------------------------------------------------------------------------------|
| `level` | string | `"warn"` | Log level filter. Valid values: `"off"`, `"error"`, `"warn"`, `"info"`, `"debug"`, `"trace"`, in any case, or `0` to `5`. |

### `[pager]`

| Key       | Type    | Default | Description                                                                                                     |
|-----------|---------|---------|-----------------------------------------------------------------------------------------------------------------|
| `enabled` | boolean | `true`  | Whether long output is piped through a pager. Set to `false` to always print directly to stdout.                |
| `command` | string  | _none_  | Optional pager command override (e.g. `"less -FR"`). An empty string is treated as unset and disables override. |

### `[colors]`

The `[colors]` section sets the colours of the terminal interface, through palette slots and overrides
for single tokens. The [Theming](./theming.md) guide is the full reference. It covers the palette
slots, the token overrides, the formats of a colour and the complete list of tokens.

## Example config file

```toml
[storage]
data_dir = "/home/user/.local/share/gest"
sync = true

[database]
# Optional: point at a remote libsql database for multi-device sync.
# url = "libsql://my-project.turso.io"
# auth_token = "eyJhbGci..."

[log]
level = "info"

[colors.palette]
primary = "#5AB0FF"

[colors.overrides]
"log.error" = "#D23434"
```

## Environment variables

gest reads the environment variables below. A `GEST_` variable wins over the value from a config file.

A `GEST_` variable can also set a config key whose name has no dots. Write the table and the key in capitals,
joined by two underscores. For example, `GEST_SERVE__PORT=8123` sets `serve.port`. gest gives the value the type that
the key needs, so `GEST_STORAGE__SYNC=false` sets a boolean. A value of the wrong type, such as
`GEST_SERVE__PORT=high`, stops gest with a configuration error (exit 78). A variable that names a whole table, such
as `GEST_PAGER`, sets no config key.

An empty boolean variable, such as `GEST_STORAGE__SYNC=`, means `false`. To fall back to the config file, unset the
variable instead.

| Variable                  | Description                                                                                                    |
|---------------------------|----------------------------------------------------------------------------------------------------------------|
| `GEST_CONFIG`             | Override the path to the global config file.                                                                   |
| `GEST_LOG__LEVEL`         | Override the log level filter: a name such as `debug`, in any case, or a number from `0` (off) to `5` (trace). |
| `GEST_PAGER`              | Pager program, used when `pager.command` is not set. It wins over `PAGER`.                                     |
| `GEST_STORAGE__CACHE_DIR` | Override the global cache directory for regeneratable artifacts (must be an absolute path).                    |
| `GEST_STORAGE__DATA_DIR`  | Override the global data root directory (must be an absolute path).                                            |
| `GEST_STORAGE__SYNC`      | Turn the sync to `.gest/` on or off. It takes `true` or `false`, and also `1`/`0`, `yes`/`no` and `on`/`off`.  |
| `EDITOR`                  | Preferred editor for interactive editing (checked before `VISUAL`).                                            |
| `VISUAL`                  | Fallback editor for interactive editing. Without either variable, gest runs `vi`.                              |
| `PAGER`                   | Pager program, used when neither `pager.command` nor `GEST_PAGER` is set (falls back to `less -R`).            |

## Managing config from the CLI

Subcommands of `gest config` show and change the configuration, so you do not have to edit files by
hand.

### `gest config show`

`gest config show` prints the merged configuration and the config files that gest found:

```sh
gest config show
```

#### Options

| Flag          | Description                       |
|---------------|-----------------------------------|
| `-j, --json`  | Emit output as JSON               |
| `-q, --quiet` | Suppress normal output            |
| `-r, --raw`   | Emit script-friendly plain output |

### `gest config get <KEY>`

`gest config get` prints one value. Name the value by its key, with dots between the parts:

```sh
gest config get storage.data_dir
gest config get log.level
```

### `gest config set <KEY> <VALUE>`

`gest config set` saves a value to the project config file. gest uses the nearest `.gest.toml` or
`.config/gest.toml` in the working directory or above it. If there is none, it creates `.gest.toml` in
the working directory.
Use `--global` to write to the global config instead:

```sh
# Set in the project config
gest config set log.level debug

# Set in the global config
gest config set --global log.level warn
```
