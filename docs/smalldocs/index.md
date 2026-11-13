---
title: gest
subtitle: Agentic workflow CLI for parallel AI agents
description: Agentic workflow CLI for parallel AI agents. Track specs, tasks, and iterations across Claude Code, Cursor, Aider, and Copilot Workspace.
---

gest tracks specs, tasks and iterations for AI coding agents, such as Claude Code, Cursor, Aider and
Copilot Workspace. An iteration is a plan of tasks in phases, and the tasks of one phase can run at the
same time.

This site documents [Rahul Dave's fork of gest](https://github.com/rahuldave/gest). Aaron Allen wrote
gest, and [the original repository](https://github.com/aaronmallen/gest) is on GitHub.

[Get started](getting-started/how-to-use-gest.md) with a walk through a real feature, or
[install gest](getting-started/installation.md) first. Questions go to the
[Discord server](https://discord.gg/PqQdhf9VMF) of gest.

## What gest does

- **Parallel AI agents.** Group tasks into iterations with phases, and link the tasks that depend on
  each other. Agents in separate workspaces then work on the tasks of one phase at the same time.
  No single agent has to work through the whole list in one context window.
- **Web dashboard.** `gest serve` starts a web interface for tasks, artifacts and iterations. It shows
  the status of the work, a kanban board, a search over everything, and the Markdown of each document.
- **Specs, ADRs and artifacts.** An artifact is a document, such as a spec, an architecture decision
  record (ADR), an RFC or a design. gest stores each artifact as Markdown, so the decisions stay next
  to the code that they describe.
- **A command line for agents.** Most commands print JSON with `--json`. Claude Code, Cursor, Aider,
  Copilot Workspace and other coding agents use the same commands as you do. They read the work queue,
  claim tasks, update the status and store artifacts.
- **SQLite first, Git friendly.** Each project keeps its data in a SQLite database, with atomic
  writes, relational queries and fast dependency graphs. A `.gest/` directory can also hold a copy of
  the data as YAML and Markdown files, which you commit with your code.
- **Global or local.** By default, projects share one SQLite database, `gest.db`, in the data
  directory of your user, for example `~/.local/share/gest/` on Linux. `gest init --local` creates a
  `.gest/` directory in the repository instead. The project then keeps its database in
  `.gest/gest.db`, and gest writes the YAML and Markdown copy next to it.

## The parts of this site

- **Getting Started** installs gest and walks from an idea to parallel work.
- **CLI Reference** has a page for each command.
- **Configuration** covers the TOML files and the colours of the terminal.
- **Migration** covers the upgrades from v0.4 and from v0.5.
- **Reference** documents the Rust code of gest, made from its doc comments.
