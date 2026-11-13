---
title: Why gest?
description: Why gest exists. It turns plans from AI agents into parallel workstreams and keeps track of what depends on what.
---

An AI agent can split a feature request into dozens of tasks, write the specs and propose a plan.
Someone must then do the harder work, which is to turn the plan into parallel workstreams and
keep track of which task depends on which.

gest is a small store for tasks and artifacts. An artifact is a document, such as a spec or a design.
The store is a SQLite database on your machine, which gest opens through the libsql library. The
database is the source of truth, and it gives atomic writes, relational queries and fast dependency
graphs.

A `.gest/` directory in the repository can also hold a copy of the data as YAML and Markdown
files. You can read, diff and commit these files with your code. gest needs no server, no accounts and
no shared infrastructure.

## The problem

A typical workflow with an AI assistant has these steps:

1. You describe a feature.
2. An agent produces a plan with 15 to 20 tasks.
3. You run the tasks one at a time, in one context window, and hope that nothing gets lost.

The workflow breaks down quickly. One task at a time is slow. A single context window cannot hold the
full plan and the code that the plan changes. If the agent loses its context in the middle, you must
work out by hand what is done and what remains.

You need a way to split the work into phases, track the dependencies, and give independent tasks to
agents in parallel. You also need to do it without new infrastructure.

## The workflow

gest has three kinds of records:

- An **artifact** holds the design.
- A **task** holds one piece of the work.
- An **iteration** puts tasks into phases, as a plan to run.

For example, say that you add a new CLI command with several parts. First, write the design as an
artifact, create an iteration, and create the tasks in their phases:

```sh
# Capture the design as an artifact
gest artifact create "Add export command" \
  --tag spec \
  --body "Export tasks and artifacts to JSON or CSV for external tooling."

# Group the work into an iteration
gest iteration create "Implement export command"

# Create tasks with phase assignments
gest task create "Add export data types" --iteration <iteration-id> --phase 1 --priority 0
gest task create "Add CSV formatter" --iteration <iteration-id> --phase 1 --priority 1
gest task create "Add JSON formatter" --iteration <iteration-id> --phase 1 --priority 2
gest task create "Wire up CLI command" --iteration <iteration-id> --phase 2 --priority 0
gest task create "Add integration tests" --iteration <iteration-id> --phase 3 --priority 0
```

The tasks of phase 1 do not depend on each other. Agents can build the data types, the CSV formatter
and the JSON formatter at the same time. Phase 2 joins the parts, and phase 3 tests the result. Next,
link each task to the tasks that it waits for, and draw the plan:

```sh
# Set dependencies
gest task link <wire-up-id> <data-types-id> --rel blocked-by
gest task link <wire-up-id> <csv-id> --rel blocked-by
gest task link <wire-up-id> <json-id> --rel blocked-by

# Visualize the plan
gest iteration graph <iteration-id>
```

The graph of the iteration shows which tasks can run in parallel and which tasks must wait. Agents or
developers can take the independent tasks at the same time, each in its own workspace.

## Phases and dependency links {#what-makes-it-work}

Two parts of the data model of gest make the parallel work possible:

- **Phased tasks.** Each task in an iteration has a phase, which is a number. Tasks in the same phase
  do not wait for each other, so separate agents can work on them at the same time.
  `gest iteration next` gives out the tasks of the lowest phase first.
- **Dependency links.** A `blocked-by` or `blocks` link between two tasks makes one task wait for the
  other. `gest iteration next` does not give out a task while a task that blocks it is still open or
  in progress. `gest iteration graph` shows which tasks can run now and which tasks wait. Agents take
  the unblocked tasks and mark them done, and the tasks that waited on them become free.

Phases and links together turn a flat list of 15 tasks into a few waves of parallel work, with the
dependencies between the waves written down.

## A dashboard for people

Agents use gest through the CLI and its JSON output, but people need a visual view. `gest serve` starts
a web dashboard on your machine. You can browse and manage everything that gest tracks:

- **Status overview.** The count of each kind of record, and the count of tasks in each status.
- **Task and artifact views.** Filter, search and read each record, with its Markdown rendered.
- **Iteration detail.** The tasks of an iteration, grouped by phase, with their dependencies.
- **Kanban board.** One column for each task status, to follow the progress of an iteration.
- **Full-text search.** Find any task, artifact or iteration without its ID.

You can also change data in the dashboard, for example update tasks, change a status or manage an
iteration. The dashboard runs on your machine and needs no setup beyond `gest serve`. It reads and
writes the same SQLite database as the CLI, so a change in one shows up at once in the other.

Parallel agents make many changes in a short time. When three agents work on phase 1 of an iteration,
the dashboard shows in one place what is in progress, what is blocked and what is done. You do not
have to switch between terminal windows.

## gest and the tracker of your team {#fits-your-existing-stack}

gest does not replace GitHub Issues, Jira or the project tracker of your team. It is for the work in
progress during development, which changes too fast for a tool that the whole team reads.

Use gest to split the work and to run it in parallel during development. Some tasks need a wider
audience, for example a bug report, a feature request or a dependency on another team. Move those
tasks to the tracker of your team. Most list commands and `gest search` print JSON with `--json`, so a
script can do the handoff:

```sh
# Example: find all open tasks tagged "blocker" and export for triage
gest task list --tag blocker --json
```

gest handles the fast, local loop of each developer. The tracker of your team handles the
coordination and the visibility across the team.
