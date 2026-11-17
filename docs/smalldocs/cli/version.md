---
title: gest version
---

Print the current version and platform information, and check for a newer release. The command looks
for releases in the GitHub repository `aaronmallen/gest`. If a newer release exists, the command
prints a notice. If the check fails, for example with no network, the command prints no notice and
still exits with 0.

`gest --version` and `gest -V` print one line and do not check for a release. The line starts with
the name and version, so a script can read the version from it:

```text
gest 0.5.6 (macos-aarch64, 2026-11-12, revision 9e3c7e7)
```

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
