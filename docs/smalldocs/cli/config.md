---
title: gest config
---

View and change the configuration of gest. Gest reads TOML config files at two levels, the global
level and the project level. A project value overrides a global value.

## Usage

```text
gest config <COMMAND> [OPTIONS]
```

## Subcommands

| Command                | Description                           |
|------------------------|---------------------------------------|
| [`get`](#config-get)   | Retrieve a single configuration value |
| [`set`](#config-set)   | Persist a configuration value         |
| [`show`](#config-show) | Display the merged configuration      |

## Exit codes

| Code | When                                                                           |
|------|--------------------------------------------------------------------------------|
| 0    | Success                                                                        |
| 2    | The parser rejected the command line, for example a missing argument           |
| 74   | Could not read or write the config file                                        |
| 78   | Config TOML is syntactically invalid, or XDG directories could not be resolved |

See [Exit Codes](./exit-codes.md) for the full contract.

---

## config get

Print a single configuration value. The key is a path with dots, as in `log.level`. If the key has no
value, the command prints `(not set)` and exits with 0.

```text
gest config get [OPTIONS] <KEY>
```

### Arguments

| Argument | Description                                        |
|----------|----------------------------------------------------|
| `<KEY>`  | Dot-delimited config key (e.g. `storage.data_dir`) |

The command also takes the output flags `-j, --json`, `-q, --quiet` and `-r, --raw`.

### Examples

```sh
gest config get storage.data_dir
gest config get log.level
gest config get database.url
```

---

## config set

Write a configuration value to a TOML config file. By default, the command writes to the project
config. That file is the nearest `.gest.toml` or `.config/gest.toml` in the current directory or a
parent directory. If no such file exists, gest creates `.gest.toml` in the current directory.

With `--global`, the command writes to the user config instead. That file is the path in
`$GEST_CONFIG`, or else `gest/config.toml` in the user config directory.

Gest reads `true` and `false` as booleans, and an integer or a decimal as a number. Every other value
is a string.

```text
gest config set [OPTIONS] <KEY> <VALUE>
```

### Arguments

| Argument  | Description                                 |
|-----------|---------------------------------------------|
| `<KEY>`   | Dot-delimited config key (e.g. `log.level`) |
| `<VALUE>` | Value to assign                             |

### Options

| Flag           | Description                                                           |
|----------------|-----------------------------------------------------------------------|
| `-g, --global` | Write to the global (user-level) config instead of the project config |

### Examples

```sh
# Set project-level config
gest config set log.level debug

# Set global config
gest config set log.level info --global
```

---

## config show

Display the merged configuration, and the paths of the config files that gest read.

```text
gest config show [OPTIONS]
```

The command takes the output flags `-j, --json`, `-q, --quiet` and `-r, --raw`.

### Examples

```sh
gest config show
```
