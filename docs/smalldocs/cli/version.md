---
title: gest version
---

Print the current version and platform information, and check for a newer release. The command looks
for releases in the GitHub repository `aaronmallen/gest`. If a newer release exists, the command
prints a notice. If the check fails, for example with no network, the command prints no notice and
still exits with 0. `gest --version` and `gest -V` do the same.

## Usage

```text
gest version [OPTIONS]
```

## Options

| Flag            | Description                     |
|-----------------|---------------------------------|
| `-v, --verbose` | Increase verbosity (repeatable) |
| `-h, --help`    | Print help                      |

## Examples

```sh
gest version
```

## Exit codes

| Code | When                                                                 |
|------|----------------------------------------------------------------------|
| 0    | Success, also when the update check fails                            |
| 2    | The parser rejected the command line, for example an unknown flag    |
| 74   | Could not open the store at startup                                  |
| 78   | Could not load user config                                           |

See [Exit Codes](./exit-codes.md) for the full contract.
