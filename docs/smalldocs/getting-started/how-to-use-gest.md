---
title: How to use gest
description: A guided walkthrough of gest, from a rough idea to parallel execution, using a real feature from the gest repo.
---

## What is gest?

**gest** tracks the work of software development with AI agents. You describe a feature, and an agent
splits it into pieces. gest stores the pieces, and the relationships between them, in a SQLite
database on your machine. You need no server, no account and no cloud service.

Most commands print JSON, so agents and scripts use the same CLI as you do. They read the work queue,
claim work and write back results. The data stays on your machine. It can also live in a
[sync mirror][storage], which is a copy of the data as files that you commit with your code. Then you
can diff the data, and it travels with your repository.

### The three things gest tracks

- An [**artifact**][artifacts] is a design document, for example a spec, an
  [ADR][categorizing-artifacts], an RFC or the notes of a brainstorm. gest stores it as Markdown.
- A [**task**][tasks] is a unit of work, for example a thing to build, fix or check. A task has a
  [priority][priority], and in an iteration it has a [phase][phase].
- An [**iteration**][iterations] is a plan to run. It groups tasks into a
  [graph of phases and dependencies][dependency-graphs], and agents work through it from start to
  finish.

Every artifact, task and iteration has a short ID of 8 characters, for example `xtnsuxns`. The CLI
prints these IDs, commands take them, and agents pass them around.

## How the maintainers build gest with gest {#how-we-build-gest-with-gest}

gest itself is a CLI and a database, and it has no agent integration. The maintainers of the original
gest drive the workflow with three [Claude Code][claude-code] skills. The skills live in the gest
repository next to the source, and this page walks through them.

Each skill is a text file with a prompt. The prompt tells an agent what to do, which `gest` commands to
run, and when to hand off to the next skill. You can [take the skills from the repository][gest-skills]
and use them as they are. You can also change the prompts for another agent, or skip the skills and
run the commands by hand. gest works the same way in each case.

- **[`/brainstorm`][brainstorm-skill]** turns a rough idea into a spec [artifact][artifacts]. You
  describe the problem in your own words. The skill asks questions, proposes approaches with their
  trade-offs, and saves the direction that you agree on.
- **[`/plan`][plan-skill]** turns a spec into a plan. It splits the spec into [tasks][tasks], groups
  them into [phases][phase] that can run in parallel, and puts the result in an
  [iteration][iterations].
- **[`/orchestrate`][orchestrate-skill]** runs the plan. For each phase, it creates a
  [separate worktree][working-across-workspaces] for each task and starts an agent in each worktree.
  The tasks of one phase run at the same time.

## A worked example

This example follows a real feature that the original gest repository tracked, which is **proper exit
codes for the gest CLI**. When the maintainers recorded this example, every error exited with `1`. A script that
called `gest` could not tell "no tasks available" from "bad flag" without reading the error message.
The goal was a distinct, standard code for each category of error.

### Step 1: Start with an idea

Open [Claude Code][claude-code] and run `/brainstorm` with a rough description of the problem:

```text
/brainstorm every gest CLI error exits with 1, scripts can't branch on category
```

The skill reads the relevant parts of the code and asks questions. One example is "should we use BSD
[`sysexits.h`][sysexits], or invent our own scheme?". When the direction is clear, the skill saves a
spec [artifact][artifacts] with this command:

```bash
cat spec.md | gest artifact create "Exit Code Taxonomy (sysexits.h)" \
  --tag spec --tag cli -q
```

```text
xtnsuxns
```

`xtnsuxns` is the ID of the spec. You can [view the spec][gest-artifact-show] with
`gest artifact show xtnsuxns`, or in the [built-in web dashboard][gest-serve] of `gest serve`.

The brainstorm also raised an architectural decision, which needed an [ADR][categorizing-artifacts] of
its own. The decision was to narrow one variant of an enum and to add new variants. The skill writes the
ADR in the same way:

```bash
cat adr.md | gest artifact create \
  "ADR-DRAFT: Exit Code Contract for the gest CLI" \
  --tag adr --tag cli -q
```

```text
prsooyor
```

### Step 2: Plan the work

Pass the ID of the spec to `/plan`:

```text
/plan xtnsuxns
```

The skill reads the spec and decides whether the work is one issue or many. If the work is big enough
to split, the skill creates the [tasks][tasks] in a batch, as JSON with one object on each line. Each
task gets a [phase][phase] number. Tasks in the same phase can run in parallel, and tasks in a later
phase come after the earlier ones.

```bash
cat <<'EOF' | gest task create --batch -q
{"title":"Audit cli::Error construction sites","phase":1,"priority":1,"tags":["cli"],"links":["child-of:xtnsuxns"]}
{"title":"Update agent profile docs for new exit codes","phase":1,"priority":2,"tags":["docs","cli"],"links":["child-of:xtnsuxns"]}
{"title":"Refactor cli::Error: add variants, exit_code, ExitCode main","phase":2,"priority":0,"tags":["cli"],"links":["child-of:xtnsuxns"]}
EOF
```

```text
qzkwulpt
sxyqyqlr
ywnqunto
```

gest prints one ID for each task, in the order of creation. gest uses the `phase` of a batch record
only when the record also names an `iteration`. Otherwise, give the phase when you add the task to the
iteration, with `gest iteration add <iteration-id> <task-id> --phase <n>`.

`/plan` then creates an [iteration][iterations] for the tasks and names it. When the tasks are in the
iteration, the skill prints the plan as text with [`iteration graph`][gest-iteration]:

```bash
gest iteration create "Exit Code Taxonomy (sysexits.h)" -q
gest iteration graph yzyomkzz
```

```text
Exit Code Taxonomy (sysexits.h)
  4 phases · 8 tasks

  ◆  Phase 1  ──
  ├─╮
  ○ │  qzkwulpt  [P1]  Audit cli::Error construction site…  ○ open  ! blocking
  │ ○  sxyqyqlr  [P2]  Update agent profile documentation…  ○ open
  ├─╯
  │
  ◆  Phase 2  ──
  ⊗  ywnqunto  [P0]  Refactor cli::Error: add variants,…  ⊗ blocked  ! blocking  blocked-by qzkwulpt
  │
  ◆  Phase 3  ──
  ├─╮─╮─╮
  ⊗ │ │ │  pkpopvny  [P1]  Fix additional cli::Error miscateg…  ⊗ blocked
  │ ⊗ │ │  tnlzoutr  [P0]  Fix detach: replace Io misuse with…  ⊗ blocked
  │ │ ⊗ │  nlumrswo  [P0]  Fix iteration advance: replace Edi…  ⊗ blocked
  │ │ │ ⊗  osnqmnyn  [P0]  Fix iteration next: replace proces…  ⊗ blocked
  ├─╯─╯─╯
  │
  ◆  Phase 4  ──
  ⊗  sotpkmsx  [P0]  Add integration test coverage for …  ⊗ blocked
```

Each `◆  Phase N  ──` line starts a phase. The tasks under `Phase 1` come first, and they can run in
parallel. The tasks of later phases come after them. A `blocked-by` label on a task points back to the
task that it waits for.

### Step 3: Run the plan

Pass the ID of the iteration to `/orchestrate`:

```text
/orchestrate yzyomkzz
```

For each phase, `/orchestrate` looks at the unblocked tasks and starts one agent for each task. Phase 1
above has two tasks, `qzkwulpt` and `sxyqyqlr`. When a phase has more than one task, the skill creates
a [separate worktree][working-across-workspaces] for each agent, so that the agents do not edit the
same files. Every agent calls [`iteration next --claim`][gest-iteration]. gest gives each agent a
different task in one atomic step, so two agents never take the same task:

```bash
# agent A                                             # agent B
gest iteration next yzyomkzz --claim \                gest iteration next yzyomkzz --claim \
  --agent agent-a --json                                --agent agent-b --json
```

```json
// agent A                                            // agent B
{"id":"qzkwulpt...","title":"Audit cli::Error...",    {"id":"sxyqyqlr...","title":"Update agent profile...",
 "status":"inprogress","phase":1,"priority":1,         "status":"inprogress","phase":1,"priority":2,
 "assigned_to":"agent-a"}                               "assigned_to":"agent-b"}
```

The build that this page describes writes the status as `in-progress`, and `assigned_to` holds the
author ID of the agent, not its name.

When every task of the phase is `done`, `/orchestrate` moves on to phase 2, then to phase 3, and so on
until the iteration is complete. You can check the progress at any time. The web dashboard uses the
same JSON as this command:

```bash
gest iteration status yzyomkzz --json
```

```json
{
  "id": "yzyomkzzxrlprqmppxllvouxlooyxxnw",
  "title": "Exit Code Taxonomy (sysexits.h)",
  "status": "active",
  "phases": 4,
  "total_tasks": 8,
  "open": 8,
  "in_progress": 0,
  "done": 0,
  "progress": 0
}
```

### Step 4: Ship it

Each agent commits its work to your repository when it finishes its task. When the whole iteration is
done, a separate [`/changelog`][changelog-skill] skill reads the new commits and turns them into release
notes in `CHANGELOG.md`. The first piece of this work on exit codes already shipped in v0.5.4. In that
version, `iteration next` returns `2` when no tasks are available. The real [CHANGELOG][changelog] has
the entry:

```text
## [v0.5.4] - 2026-04-14

### Changed

- `iteration next` returns proper error codes (exit 2 when no tasks are
  available) instead of calling `process::exit` directly
```

## Where to go next

- **[Install gest][installation].** One command installs it on macOS or Linux.
- **[Quick start][quick-start].** The shortest path from nothing to a working gest project, by hand
  and without an agent.
- **[Core concepts][concepts].** The full reference for artifacts, tasks, iterations, phases, links
  and storage.
- **[CLI reference][cli-task].** Every flag of every `gest` command.

---

*The output comes from commit [`eb5a2ba`][capture-commit] of the original gest repository. The IDs
show their short form of 8 characters. Inside gest, an ID has 32 lowercase letters, from `k` to `z`.*

[artifacts]: ./concepts.md#artifacts

[categorizing-artifacts]: ./concepts.md#categorizing-artifacts

[changelog]: https://github.com/aaronmallen/gest/blob/main/CHANGELOG.md

[changelog-skill]: https://github.com/aaronmallen/gest/blob/main/.agents/profiles/git/skills/changelog/SKILL.md

[capture-commit]: https://github.com/aaronmallen/gest/commit/eb5a2ba0a016ca6c9d332cc7754b81a02a9e5baf

[brainstorm-skill]: https://github.com/aaronmallen/gest/blob/main/.agents/profiles/default/skills/brainstorm/SKILL.md

[claude-code]: https://claude.com/claude-code

[cli-task]: ../cli/task.md

[concepts]: ./concepts.md

[dependency-graphs]: ./concepts.md#dependency-graphs

[gest-artifact-show]: ../cli/artifact.md

[gest-skills]: https://github.com/aaronmallen/gest/tree/main/.agents/profiles

[gest-iteration]: ../cli/iteration.md

[gest-serve]: ../cli/serve.md

[installation]: ./installation.md

[iterations]: ./concepts.md#iterations

[orchestrate-skill]: https://github.com/aaronmallen/gest/blob/main/.agents/profiles/git/skills/orchestrate/SKILL.md

[phase]: ./concepts.md#phase

[plan-skill]: https://github.com/aaronmallen/gest/blob/main/.agents/profiles/default/skills/plan/SKILL.md

[priority]: ./concepts.md#priority

[quick-start]: ./quick-start.md

[storage]: ./concepts.md#storage-modes

[sysexits]: https://man.openbsd.org/sysexits.3

[tasks]: ./concepts.md#tasks

[working-across-workspaces]: ./concepts.md#working-across-workspaces
