---
title: FAQ
description: Frequently asked questions about gest, the agentic workflow CLI for parallel AI agents. How it differs from Jira and GitHub Issues, agent compatibility, and more.
---

## What is gest?

gest is a CLI that tracks the tasks and artifacts of software development with AI agents. An artifact
is a document, such as a spec. gest stores the data in a SQLite database on your machine, through the
libsql library. An optional sync layer copies everything to a `.gest/` directory as YAML and Markdown
files. You can then read the data, move it and keep it under version control. See the
[quick start](./getting-started/quick-start.md) to begin.

## How does gest differ from GitHub Issues or Jira?

gest is a fast, local tracker for one developer and their agents. It does not manage the projects of a
whole team. It differs from those tools in these ways:

- **No server or network.** gest keeps everything in a SQLite database on your machine. A remote database
  is possible through `[database]`, but only when you set it up.
- **Made for AI workflows.** Tasks and artifacts match what AI agents produce during development.
- **Parallel work.** Iterations have phases and dependencies, so agents can work at the same time in
  separate workspaces.
- **Built-in web dashboard.** `gest serve` gives you an overview, a kanban board and a full-text
  search, with no other tool.
- **Scriptable.** Most commands print JSON with `--json`. Most commands that change data also print
  only an ID with `-q`, so other tools can use the output.

gest works next to an issue tracker and does not replace it. Use `gest` for the work in progress during
development. Move an item to GitHub Issues when more people need to see it.

## Where is my data stored?

gest stores the data in SQLite:

- A global project uses `<data_dir>/gest.db`. By default, that is `~/.local/share/gest/gest.db` on
  Linux or `~/Library/Application Support/gest/gest.db` on macOS.
- A project that `gest init --local` set up uses `.gest/gest.db`.

A data directory or a remote database in your configuration replaces both defaults. Run
`gest config show` to see the configuration that gest uses.

With `--local`, gest also creates a `.gest/` directory inside your project. While `storage.sync` is on,
which is the default, every command syncs the database with YAML and Markdown files in `.gest/`, in
both directions. You can commit these files with your code, and their history is easy to read and to
diff.

## What is the difference between global and local stores? {#whats-the-difference-between-global-and-local-stores}

**Global only** (`gest init`):

- One SQLite database in the data directory of your user, which every project on the machine shares
- Not shared through version control
- Best for personal task tracking across projects

**Local sync** (`gest init --local`):

- A SQLite database of the project in `.gest/gest.db`, and YAML and Markdown files in `.gest/`
- The sync layer copies the database to the files, so that version control can keep the data
- Best for the tasks of one project that you want to share or track in git

The database is always the source of truth. gest rewrites the files from the database when it exports.
It imports the files again when they change on disk.

## Can I use gest with CI/CD?

Yes. Most list commands and `gest search` print JSON with `--json`, so you can script gest in a CI
pipeline. For example:

```sh
# Fail the build if there are open tasks tagged "blocker"
if gest task list --json | jq -e '.[] | select(.tags[] == "blocker")' > /dev/null 2>&1; then
  echo "Blocker tasks remain"; exit 1
fi
```

Use `gest init --local`, so that the `.gest/` files are in the checkout of your CI job. On the first
run, gest imports them into a new SQLite database.

## How do I move data between machines?

You have three options:

1. **Local sync (`gest init --local`).** Commit the `.gest/` directory. On the other machine, check out
   the repository and run any `gest` command. The sync layer imports the files into the database of
   that machine.
2. **Remote libsql database.** Set `database.url` and `database.auth_token` in your config, so that
   gest uses a shared libsql server. Every machine with the same URL sees the same data.
3. **Copy the SQLite file by hand.** `gest.db` is a normal SQLite file. You can copy it between
   machines with `rsync` or `cp` while gest is not running.

## How do I migrate from v0.4.x flat files?

Run `gest migrate --from v0.4`. The command reads the TOML and Markdown files of your old `.gest/`
directory and imports them into the SQLite database. The
[v0.4 to v0.5 migration guide](migration/v0-4-to-v0-5.md) gives each step. It covers the checks
before the migration, how the data maps, the checks after it and how to roll back. The short reference
of the command is [gest migrate](./cli/migrate.md).

## What does the `.gest/` sync mirror look like?

When `storage.sync` is on and the project root has a `.gest/` directory, gest writes one file for each
record. Each kind of record has its own directory, with a singular name:

```text
.gest/
├── project.yaml
├── artifact/       # <id>.md (body + YAML frontmatter)
│   └── notes/      # <note_id>.yaml
├── author/         # <id>.yaml
├── event/          # <yyyy-mm>/<id>.yaml (sharded by month)
├── iteration/      # <id>.yaml
├── relationship/   # <id>.yaml
├── tag/            # <id>.yaml
└── task/           # <id>.yaml
    └── notes/      # <note_id>.yaml
```

The database itself, `gest.db`, is also in `.gest/`. Do not commit it. Most files are YAML. Only the
artifacts are Markdown files, with YAML front matter. At the start of each command, gest imports the
files that changed since the last sync. At the end of the command, it writes the changed records back
to the files.

Configuration files are plain TOML, namely `.config/gest.toml`, `.gest.toml` and the global config.
gest loads them from the search paths in [Configuration](./configuration/index.md). The database never
stores them, and gest does **not** look for a config file inside `.gest/`.

## How does search work?

`gest search <QUERY>` finds the tasks, artifacts and iterations whose title, description or body
contains the text, without regard to case. See [gest search](./cli/search.md) for the full reference.

These flags are useful:

- `--expand` or `-e` shows the full detail of each result.
- `--json` or `-j` prints the results as JSON for scripts.
- `--all` or `-a` includes archived and resolved records, which the search leaves out by default.

## Are there shortcuts for common task operations?

Yes. Short commands do the most common changes of status:

- `gest task complete <id>` marks a task as done, like `task update <id> --status done`.
- `gest task cancel <id>` cancels a task, like `task update <id> --status cancelled`.
- `gest task block <id> <other-id>` marks one task as blocking another, like
  `task link <id> <other-id> --rel blocks`.

All the short commands take `--json` and `-q` for output that a program reads.

## How do I get machine-readable output?

Most commands print structured JSON with `--json`. Commands that change data also take `-q` or
`--quiet`, and then print only the ID of the record. Scripts can use the ID:

```sh
task_id=$(gest task create "My task" -q)
gest task complete "$task_id" -q
```

Commands that read data, such as `meta get`, take `--json` and `--raw`. `--raw` prints the bare value,
without styling.

## Can multiple people use gest on the same project?

Yes. Two patterns work:

1. **Local sync through git.** Run `gest init --local` and commit the `.gest/` directory. The database
   of each person imports and exports through the files. Each file is named by the unique ID of its
   record, so a merge conflict happens only when two people change the same record.
2. **Shared remote database.** Point `database.url` at a shared libsql server. Everyone writes to the
   same store at the same time, with no files to merge.

## How do I back up my gest data?

For a **local sync mirror**, your version control is the backup. Commit the `.gest/` directory.

For the **database itself**, back up `.gest/gest.db` for a local project, or `<data_dir>/gest.db` for a
global project or a project with a data directory that you set. It is a standard SQLite file, so any
backup tool works, for example rsync, Time Machine or `sqlite3 .backup`. For a remote libsql database,
use the backup of your hosting provider.

## Does gest have a web UI?

Yes. Run `gest serve` to start a web dashboard on your machine at `http://localhost:2300`. It shows an
overview of the status, the tasks and artifacts with their Markdown rendered, and the tasks of each
iteration grouped by phase. It also has a kanban board and a full-text search. The dashboard reads and
writes the same SQLite database as the CLI, so a change in one shows up at once in the other.

See [gest serve](./cli/serve.md) for its options.

## What are iterations and when should I use them?

An iteration groups related tasks into a plan with ordered phases. Use one for a set of tasks that
belong together and that must happen in some order. An example is a feature that spans several files or
steps.

Create an iteration in these cases:

- You have several related tasks with dependencies between them.
- You want to track the progress of a plan with several steps.
- You coordinate the work of AI agents across phases.

Plain tasks are enough for tasks that stand alone and need no order.

## How do I update gest?

Run the built-in update command:

```sh
gest self-update
```

It downloads the latest release from GitHub. You can also install again with Cargo:

```sh
cargo install gest
```

Or use the install script with a fixed version:

```sh
GEST_VERSION=0.x.x curl -fsSL https://gest.aaronmallen.dev/install | sh
```
