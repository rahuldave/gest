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

- **Parallel AI agents.** Group tasks into phased iterations with dependency tracking. Tasks in the same
  phase run concurrently across isolated agent workspaces, with no sequential, single-context-window
  bottlenecks.
- **Web dashboard.** Browse tasks, artifacts and iterations in a built-in web UI. Inspect status at a
  glance, view kanban boards, search across everything, and read rendered Markdown, all from
  `gest serve`.
- **Specs, ADRs and artifacts.** Store specs, ADRs, RFCs and design documents as versioned Markdown with
  YAML frontmatter. Keep architectural decisions next to the code they describe.
- **Agent-native CLI.** Every command supports `--json` output. Claude Code, Cursor, Aider, Copilot
  Workspace and other agentic coding tools read the work queue, claim tasks, update status and store
  artifacts through the same CLI that you use by hand.
- **SQLite first, Git friendly.** Entity data lives in a single SQLite database, with atomic writes,
  relational queries and fast dependency graphs. Opt into a `.gest/` sync mirror (YAML and Markdown) to
  commit your data alongside your code.
- **Global or local.** By default, projects share one SQLite database at
  `~/.local/share/gest/gest.db`. Add `--local` on `init` to also materialize a `.gest/` mirror inside
  the repository for version control and collaboration.

## The parts of this site

- **Getting Started** installs gest and walks from an idea to parallel work.
- **CLI Reference** has a page for each command.
- **Configuration** covers the TOML files and the colours of the terminal.
- **Migration** covers the upgrades from v0.4 and from v0.5.
- **Reference** documents the Rust code of gest, made from its doc comments.
