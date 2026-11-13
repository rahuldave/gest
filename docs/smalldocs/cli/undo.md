---
title: gest undo
---

Alias: `gest u`

Undo the most recent commands that changed data. Gest reads the log of transactions, and reverses the
changes of each command.

Gest wraps most commands that change data in a database transaction, for example `create`, `update`,
`archive` or `link`. It records the changes to each row in the tables `transactions` and
`transaction_events`. `gest undo` applies the inverse of each recorded change of the most recent
transaction. Gest does not record commands that only read data, such as `show`, `list`, `search` or
`version`.

The log of transactions is in the same SQLite database as your data, so the history of undo goes
with the database. With the global store, the history stays on one machine. If you set `[database]`
to a remote libsql database, the history is shared between machines.

## Usage

```text
gest undo [OPTIONS] [STEPS]
```

## Arguments

| Argument  | Description                               |
|-----------|-------------------------------------------|
| `[STEPS]` | Number of commands to undo (default: `1`) |

## Options

| Flag            | Description                     |
|-----------------|---------------------------------|
| `-v, --verbose` | Increase verbosity (repeatable) |
| `-h, --help`    | Print help                      |

## Examples

Undo the last command:

```sh
gest undo
```

Undo the last 3 commands:

```sh
gest undo 3
```

## Exit codes

| Code | When                                                                      |
|------|---------------------------------------------------------------------------|
| 0    | Success                                                                   |
| 2    | The parser rejected the command line, for example a non-numeric `[STEPS]` |
| 69   | Nothing to undo (no undoable transactions recorded)                       |
| 74   | Store I/O error while replaying the transaction log                       |
| 78   | Not a gest project (run `gest init`)                                      |

See [Exit Codes](./exit-codes.md) for the full contract.

## How it works

Each command that changes data runs in a database transaction. For each row that the command
inserts, updates or deletes, gest records a row in `transaction_events` with the state of the row
before the change. `gest undo` takes the events of the most recent transaction in reverse order, and
applies the inverse of each event:

- **An insert** becomes a delete.
- **An update** restores the recorded row.
- **A delete** inserts the recorded row again.

For each command that it undoes, gest prints `undone` and the name of the command.

If you ask for more steps than the log has, gest undoes all the transactions that it can, and exits
with 0.

Gest undoes each command in one database transaction. If one event of a command fails, gest rolls back
the undo of that command, and exits with code 74. Gest does not mark that command as undone. The commands
that gest undid before it stay undone. So the next `gest undo` starts again at the command that failed.

::: {.callout-tip}
Gest does not record the undo command in the log. It marks each transaction that it undoes. So each
new `gest undo` goes one more step back in the history, and adds no entry.
:::
