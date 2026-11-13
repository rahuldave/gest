---
title: gest generate
description: gest generate — produce shell completions and man pages for the gest CLI, the agentic workflow CLI for parallel AI agents.
---

Generate shell completions and man pages for gest.

## Usage

```text
gest generate <COMMAND> [OPTIONS]
```

## Subcommands

| Command                                | Description                              |
|----------------------------------------|------------------------------------------|
| [`completions`](#generate-completions) | Print shell completion scripts to stdout |
| [`man-pages`](#generate-man-pages)     | Write man page files to a directory      |

## Exit codes

| Code | When                                                                  |
|------|-----------------------------------------------------------------------|
| 0    | Success                                                               |
| 2    | The parser rejected the command line, for example an unknown shell    |
| 74   | Could not write completions or man pages to the target                |

See [Exit Codes](./exit-codes.md) for the full contract.

---

## generate completions

Print the shell completion script to stdout. Redirect the output to the completion folder of your
shell.

```text
gest generate completions [OPTIONS] <SHELL>
```

### Arguments

| Argument  | Description                                                 |
|-----------|-------------------------------------------------------------|
| `<SHELL>` | Target shell: `bash`, `elvish`, `fish`, `powershell`, `zsh` |

### Examples

```sh
# Bash
gest generate completions bash > ~/.local/share/bash-completion/completions/gest

# Zsh
gest generate completions zsh > ~/.zfunc/_gest

# Fish
gest generate completions fish > ~/.config/fish/completions/gest.fish
```

---

## generate man-pages

Write a roff man page for each command to a directory.

```text
gest generate man-pages [OPTIONS] [OUTPUT_DIR]
```

### Arguments

| Argument       | Description                                                     |
|----------------|-----------------------------------------------------------------|
| `[OUTPUT_DIR]` | Directory to write man page files into (default: `man`)         |

### Examples

```sh
gest generate man-pages ./man
```
