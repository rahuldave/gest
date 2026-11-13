---
title: Core Concepts
description: Core concepts in gest. Tasks, artifacts, iterations, links and tags, and how they connect into phased plans for parallel AI agents.
---

gest manages three kinds of records, which are tasks, artifacts and iterations. Links, tags and
metadata connect them. The records live in a SQLite database on your machine, which gest opens through
the libsql library. A project can also have a `.gest/` directory. Then a sync layer copies every change
to YAML and Markdown files in that directory, so that you can commit the data with your code.

## Tasks

A **task** is one unit of work. Each task is a row in the `tasks` table, with the fields below. When
the project has a `.gest/` directory and sync is on, the task also has a YAML copy in
`.gest/task/<id>.yaml`.

| Field         | Description                                   |
|---------------|-----------------------------------------------|
| `title`       | Short summary of the work                     |
| `description` | Longer explanation (Markdown)                 |
| `status`      | Current state of the task                     |
| `priority`    | Urgency level, 0 (highest) through 4 (lowest) |
| `phase`       | Numeric execution phase for parallel grouping |
| `assigned_to` | Actor responsible for the task                |
| `tags`        | Freeform labels for filtering and grouping    |
| `metadata`    | Arbitrary key-value pairs (JSON object)       |
| `links`       | Relationships to other tasks or artifacts     |

The `phase` of a task belongs to its place in an iteration, so gest stores it with the iteration. The
tags and the links are rows in their own tables.

### Task statuses

| Status        | Meaning                       |
|---------------|-------------------------------|
| `open`        | Not yet started (default)     |
| `in-progress` | Actively being worked on      |
| `done`        | Completed successfully        |
| `cancelled`   | Abandoned or no longer needed |

`done` and `cancelled` are the final statuses, and a task with either status is resolved. Lists and
searches leave out resolved tasks by default. Pass `--all` to include them.

### Priority

Priority is a number from `0` to `4`, and `0` is the most urgent. A task does not need a priority. A
task without one has no priority level at all, and `gest iteration next` puts it after the tasks that
have one.

### Phase

A phase is a number that groups the tasks of an iteration for parallel work. Tasks in the same phase do
not wait for each other, so they can run at the same time.

`gest iteration next` gives out the tasks of lower phases first. It does not wait for a phase to
finish. Only a blocking link makes a task wait. Put a task in a later phase than the tasks that block
it. gest does not check this rule.

## Artifacts

An **artifact** is a document, for example a spec, an ADR, an RFC, a design note or any other text from
the development work. Each artifact is a row in the `artifacts` table. When sync is on, it also has a
copy in `.gest/artifact/<id>.md`, as Markdown with YAML front matter.

| Field         | Description                                                           |
|---------------|-----------------------------------------------------------------------|
| `title`       | Document title (from the first `# heading` of piped input if not set) |
| `body`        | Markdown content                                                      |
| `tags`        | Freeform labels for filtering                                         |
| `metadata`    | Arbitrary key-value pairs (JSON object)                               |
| `archived_at` | Timestamp set when the artifact is archived                           |

### Categorizing artifacts

Tags give an artifact its category. Tag an artifact with `spec`, `adr`, `rfc`, `note` or any other label
that fits your workflow. Then filter with `--tag`:

```sh
gest artifact create "Auth spec" --tag spec --body "..."
gest artifact list --tag spec
```

The artifacts of the gest project use these tags:

| Tag    | Description                      |
|--------|----------------------------------|
| `spec` | Product or feature specification |
| `adr`  | Architecture Decision Record     |
| `rfc`  | Request for Comments             |
| `note` | General-purpose document         |

### Archiving

`gest artifact archive <id>` archives an artifact. Lists and searches leave out archived artifacts by
default, but the artifacts stay in the database. Use `--all` to include them.

## Iterations

An **iteration** groups related tasks into a plan to run. Each iteration is a row in the `iterations`
table. The `iteration_tasks` table records which tasks belong to which iteration, and the phase of each
task.

| Field         | Description                                    |
|---------------|------------------------------------------------|
| `title`       | Name of the iteration                          |
| `description` | Goal or scope of the iteration (Markdown)      |
| `status`      | Current state of the iteration                 |
| `tasks`       | List of task IDs that belong to this iteration |
| `tags`        | Freeform labels                                |
| `metadata`    | Arbitrary key-value pairs (JSON object)        |
| `links`       | Relationships to other entities                |

### Iteration statuses

| Status      | Meaning                              |
|-------------|--------------------------------------|
| `active`    | Currently in progress (default)      |
| `cancelled` | Iteration was deliberately abandoned |
| `completed` | All tasks finished successfully      |

### Dependency graphs

`gest iteration graph <id>` draws the plan of an iteration. The graph groups the tasks by phase, and
it marks each task that a `blocked-by` or `blocks` link holds back. You can then see which tasks can
run in parallel and which tasks must wait.

### Managing tasks in an iteration

```sh
gest iteration add <iteration-id> <task-id>      # add a task
gest iteration remove <iteration-id> <task-id>   # remove a task
```

## Working across workspaces

gest finds the project of a checkout from its path. So by default, each checkout has its own project.
A second jj workspace, a git worktree or a fresh clone does not see the tasks, artifacts or iterations
of the first checkout. Attach the second checkout to the same project to share them.

First, print the ID of the project in the first checkout:

```sh
gest project
```

Second, attach each other checkout to that ID:

```sh
gest project attach <project-id>
```

Before you remove a workspace, detach it, so that the project does not keep a path that no longer
exists:

```sh
gest project detach
```

When several checkouts share a project, the agent in each workspace can run
`gest iteration next <iteration-id> --claim --agent <name>` to take a different unblocked task. The
agents then work on the tasks of a phase in parallel.

- [Dependency graphs](#dependency-graphs) shows how phases and blocking links work together.
- [Orchestrate multiple agents](agents.md#orchestrate-multiple-agents) gives the whole loop.
- [`gest project`](../cli/project.md) is the full reference of the command.

## Linking

A link is a typed relationship between two records. gest stores a task link once, as a row from the
source to the target. A link between two iterations stores a row each way. `gest task show <id> --json`
lists the link under `relationships` for both records.

### Relationship types

| Type         | Inverse      | Meaning                                      |
|--------------|--------------|----------------------------------------------|
| `blocks`     | `blocked-by` | Source must complete before target can start |
| `blocked-by` | `blocks`     | Source waits on target                       |
| `parent-of`  | `child-of`   | Source is the parent of target               |
| `child-of`   | `parent-of`  | Source is a child of target                  |
| `relates-to` | `relates-to` | General association (symmetric)              |

Give the type with `--rel`. Without it, the type is `relates-to`. Link a task to another task:

```sh
gest task link <source-id> <target-id> --rel blocks
```

Link a task to an artifact:

```sh
gest task link <source-id> <artifact-id> --rel relates-to --artifact
```

## Tagging and metadata

### Tags

A tag is a free text label. Add tags with `--tag` when you create a record, or add them later, one
label at a time:

```sh
gest task tag <id> api
gest task tag <id> security
gest task untag <id> security
```

Use tags to filter lists:

```sh
gest task list --tag api
gest artifact list --tag design
```

### Metadata

Metadata holds any key-value pairs on a record. Set values with the `-m` flag when you create the
record, or with the `meta` subcommand:

```sh
gest task meta set <id> complexity high
gest task meta get <id> complexity
```

The database stores all metadata as JSON. When sync is on, the YAML file of a task or an iteration in
`.gest/` holds its metadata. The metadata of an artifact goes into the YAML front matter of its
Markdown file.

## Storage modes

gest has two modes of storage:

- **Global store** (default). The data lives in one SQLite database, `gest.db`, in the data directory
  of your user. The file is `~/.local/share/gest/gest.db` on Linux, or
  `~/Library/Application Support/gest/gest.db` on macOS. Every project on the machine shares this
  database, and each project is a row in it.
- **Local sync.** The project has its own SQLite database in `.gest/gest.db`. gest also keeps YAML and
  Markdown files in `.gest/`, in one directory for each kind of record, for example `task/`,
  `artifact/` and `iteration/`. At the start of each command, gest imports the files that changed
  since the last sync. At the end, it exports the changed rows to the files. You get files that you
  can read and commit, and you keep the atomic writes, the relational integrity and the fast queries
  of the database.

Use `gest init` for the global store, or `gest init --local` to create the `.gest/` directory. A remote
libsql database is also possible, through the `[database]` section of the configuration. See
[Configuration](../configuration/index.md) for details.

### Migrating from v0.4.x

In gest v0.4.x, the data lived in a directory for each kind of record under `.gest/`. Run
`gest migrate --from v0.4` to import that `.gest/` tree into the SQLite database. The
[v0.4 to v0.5 migration guide](../migration/v0-4-to-v0-5.md) gives each step, and
[gest migrate](../cli/migrate.md) is the short reference of the command.
