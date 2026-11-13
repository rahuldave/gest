---
title: gest self-update
---

Update gest to the latest release, or to the release that you name. The command reads the releases of
the GitHub repository `aaronmallen/gest`. It downloads the binary for your platform and replaces the
current installation.

## Usage

```text
gest self-update [OPTIONS]
```

## Options

| Flag                | Description                                           |
|---------------------|-------------------------------------------------------|
| `--target <TARGET>` | Pin to a specific version (bare semver, e.g. `1.2.3`) |
| `-v, --verbose`     | Increase verbosity (repeatable)                       |
| `-h, --help`        | Print help                                            |

## Examples

```sh
# Update to the latest release
gest self-update

# Pin to a specific version
gest self-update --target 0.3.0
```

## Exit codes

| Code | When                                                                       |
|------|----------------------------------------------------------------------------|
| 0    | Success (or already on the requested version)                              |
| 2    | The parser rejected the command line, for example an unknown flag          |
| 74   | Could not find the `--target` release, or download or install the binary   |
| 78   | Could not load user config                                                 |

See [Exit Codes](./exit-codes.md) for the full contract.
