---
title: Agent Usage
description: How Claude Code, Cursor, Aider, Copilot Workspace and other AI coding agents use gest to track tasks and run in parallel across isolated workspaces.
---

gest works with AI coding agents. An agent that can run shell commands can use gest to track tasks,
store artifacts and share a plan with other agents that work at the same time. The
[`.agents/` directory](https://github.com/aaronmallen/gest/tree/main/.agents) of the original gest
repository shows how the gest project itself uses agents.

## Workspace setup for parallel agents

Parallel agents often work in several checkouts of the same repository, such as jj workspaces, git
worktrees or separate clones. Attach each extra checkout to the project of the first checkout.
Otherwise each checkout has its own project, and the agents do not see the work of each other.

First, get the ID of the project in the first checkout:

```sh
gest project
```

Then attach each other checkout to it:

```sh
cd ../myapp-feature-a
gest project attach <project-id>
```

After that, every checkout shares the same tasks, artifacts and iterations. See
[`gest project`](../cli/project.md) for the full reference.

## Read the work queue

List the open tasks to find the work that remains. Use `--json` to get output for a program:

```sh
gest task list --json
```

Filter by status, tag or assignment:

```sh
gest task list --status open --tag api
gest task list --assigned-to agent-1 --json
```

## Track task progress

Update the status of a task as the work goes on:

```sh
gest task update <id> --status in-progress
# ... do the work ...
gest task complete <id>
```

Short commands do the most common updates:

```sh
gest task complete <id>          # shortcut for task update <id> --status done
gest task cancel <id>            # shortcut for task update <id> --status cancelled
gest task block <id> <other-id>  # shortcut for task link <id> <other-id> --rel blocks
```

Assign a task to the current agent, so that other agents know that the task is taken:

```sh
gest task update <id> --assigned-to agent-1
```

## Store design documents

Save specs, ADRs, RFCs and other documents as artifacts. Give the body on the command line with `-b`,
so that gest does not open `$EDITOR`:

```sh
gest artifact create "Auth Middleware Design" \
  -b "Token-bucket rate limiting with per-user quotas." \
  --tag spec \
  --tag auth \
  --tag design
```

Or read the body from a file:

```sh
gest artifact create "Auth Middleware Design" --source design.md --tag adr --tag architecture
```

Link a task to the artifact that it comes from:

```sh
gest task link <task-id> <artifact-id> --rel child-of --artifact
```

## Build execution plans

Group related tasks into an iteration with phases. `gest iteration next` gives out the tasks of lower
phases first, and tasks in the same phase can run in parallel. A task that another task blocks waits
for it, whatever its phase. So put each task in a later phase than the tasks that block it.

```sh
# Create tasks (no phase yet)
gest task create "Add parser types"
gest task create "Add CLI flag"
gest task create "Integrate parser"

# Create an iteration and assign tasks to phases
gest iteration create "Implement feature X"
gest iteration add <iteration-id> <parser-id>    --phase 1
gest iteration add <iteration-id> <cli-flag-id>  --phase 1
gest iteration add <iteration-id> <integrate-id> --phase 2

# Visualize the plan
gest iteration graph <iteration-id>
```

## Orchestrate multiple agents

gest gives the basic operations that let several agents work on the same iteration at the same time.
An orchestrator hands out the work, agents claim tasks, and gest tracks the progress through the
phases. The basic loop has these steps:

1. **Find work.** List the iterations that have open tasks.
2. **Claim a task.** Give the next free task to an agent, in one atomic step.
3. **Check progress.** Ask for the counts of the tasks of the iteration.
4. **Stop.** End the loop when no task of the iteration is open or in progress.

### Find work

`--has-available` lists only the iterations that have at least one task with the status `open`:

```sh
gest iteration list --has-available
gest iteration list --has-available --json
```

```json
[
  {
    "completed_at": null,
    "created_at": "2026-11-13T23:16:07.276868Z",
    "description": "",
    "id": "a1b2c3d4e5f6...",
    "metadata": {},
    "project_id": "sytnwwznxnzl...",
    "status": "active",
    "title": "Implement export command",
    "updated_at": "2026-11-13T23:16:07.276868Z",
    "relationships": [],
    "tags": []
  }
]
```

To see the work that an agent already has:

```sh
gest task list --assigned-to my-agent --json
```

### Claim a task

`gest iteration next` finds the next free task of an iteration. Without `--claim`, it only shows the
task and changes nothing. With `--claim`, it sets the task to `in-progress` in one atomic step. With
`--agent`, it also assigns the task to that agent:

```sh
# Peek at the next available task
gest iteration next <iteration-id>

# Claim it (assigning to an agent is optional but recommended)
gest iteration next <iteration-id> --claim --agent my-agent

# Claim without naming an agent (assignment is left empty)
gest iteration next <iteration-id> --claim
```

`--agent` needs `--claim`, but `--claim` works alone.

Add `--json` to get structured output. The `assigned_to` field holds the ID of the author record that
gest keeps for the agent, not its name:

```sh
gest iteration next <iteration-id> --claim --agent my-agent --json
```

```json
{
  "assigned_to": "vuxvztqywyro...",
  "id": "f7e8d9c0...",
  "phase": 1,
  "priority": 1,
  "status": "in-progress",
  "title": "Add CSV formatter"
}
```

#### Exit codes

gest follows the BSD [`sysexits.h`](https://man.openbsd.org/sysexits.3) convention. Scripts and agents
can read the category of an error from `$?`:

| Code | Name             | Meaning                              |
|------|------------------|--------------------------------------|
| 0    | —                | Success                              |
| 64   | `EX_USAGE`       | Command-line usage error             |
| 65   | `EX_DATAERR`     | User data format error               |
| 66   | `EX_NOINPUT`     | Input entity not found               |
| 69   | `EX_UNAVAILABLE` | Resource not in the required state   |
| 70   | `EX_SOFTWARE`    | Internal software error              |
| 74   | `EX_IOERR`       | Filesystem or database I/O error     |
| 75   | `EX_TEMPFAIL`    | Try again later (valid empty result) |
| 78   | `EX_CONFIG`      | Configuration or setup error         |

When no unblocked task is free, `gest iteration next` exits with **75** (`EX_TEMPFAIL`). The code
differs from the codes of real errors, so a script can tell "nothing to do" from "something broke":

```sh
gest iteration next <id> --claim --agent my-agent --json
status=$?
if [ $status -eq 75 ]; then
  echo "No work available -- idle"
elif [ $status -ne 0 ]; then
  echo "Error"
fi
```

#### Task selection order

`gest iteration next` looks at every task of the iteration with the status `open`. It leaves out each
task that another task blocks, until that other task is `done` or `cancelled`. It also leaves out a
task whose blocker is not in the iteration. It then sorts the remaining tasks:

1. **Phase.** The lowest phase comes first.
2. **Priority.** The lowest value comes first, so P0 comes before P4. Tasks without a priority come
   last.

gest uses no further tie-break. It does not wait for a phase to finish. A task of phase 2 can come
out while phase 1 still has tasks in progress, and only a link of type `blocked-by` or `blocks` makes
a task wait.

### Check progress

`gest iteration status` counts the tasks of the whole iteration by status:

```sh
gest iteration status <iteration-id> --json
```

```json
{
  "cancelled": 0,
  "done": 0,
  "id": "pnztwqnyputt...",
  "in_progress": 1,
  "open": 2,
  "phases": 2,
  "progress": 0,
  "status": "active",
  "title": "Impl X",
  "total_tasks": 3
}
```

| Field         | Description                                          |
|---------------|------------------------------------------------------|
| `open`        | Tasks with the status `open`                         |
| `in_progress` | Tasks with the status `in-progress`                  |
| `done`        | Tasks with the status `done`                         |
| `cancelled`   | Tasks with the status `cancelled`                    |
| `total_tasks` | All tasks of the iteration                           |
| `phases`      | Number of distinct phases in the iteration           |
| `progress`    | Percentage of the tasks that are done, from 0 to 100 |

### Advance phase

gest works out the active phase each time that it needs it. The active phase is the lowest phase that
still has a task that is not `done` or `cancelled`. So no command has to move the iteration to the
next phase.

`gest iteration advance` checks the active phase:

```sh
gest iteration advance <iteration-id>
```

The command fails with exit code 69 while the active phase has open or in-progress tasks. It also fails
with 69 when every task of the iteration is `done` or `cancelled`. With `--force`, it skips the check
and prints the active phase and the next phase:

```sh
gest iteration advance <iteration-id> --force
```

`gest iteration advance` changes no stored data, even with `--force`.

### A minimal orchestrator loop

```sh
ITER_ID="a1b2c3d4"

while true; do
  # Try to claim work
  task=$(gest iteration next "$ITER_ID" --claim --agent worker-1 --json 2>/dev/null)
  status=$?

  if [ $status -eq 75 ]; then
    # No free task now -- stop when nothing is open or in progress, else wait
    left=$(gest iteration status "$ITER_ID" --json | jq '.open + .in_progress')
    [ "$left" -eq 0 ] && break
    sleep 10
    continue
  elif [ $status -ne 0 ]; then
    echo "Error claiming task" >&2
    break
  fi

  task_id=$(echo "$task" | jq -r '.id')

  # ... do the work ...

  gest task complete "$task_id" -q
done
```

## Search before creating

Before you create a task or an artifact, search for one that exists, so that you do not make a
duplicate:

```sh
gest search "rate limiting"
```

Add `--expand` to see the full details, or `--json` to get output for a program:

```sh
gest search "rate limiting" --expand
gest search "rate limiting" --json
```

The search leaves out resolved tasks and archived artifacts by default. Pass `--all` to include them:

```sh
gest search "rate limiting" --all
```

## Scripting tips

### JSON and quiet output

Most commands print JSON with `--json`. Commands that change data also take `-q` or `--quiet`, and
then print only the ID of the record. These commands include `create`, `update`, `complete`,
`cancel`, `block`, `link`, `tag`, `untag`, `note add`, `note update` and `meta set`.

```sh
gest task show <id> --json
gest task list --json

# Get just the ID for scripting
task_id=$(gest task create "My task" -q)
gest task complete "$task_id" -q
```

### Stdin piping

You can pipe the description of a task or the body of an artifact into gest. If you leave out
`--description` or `--body` and standard input is a pipe, gest reads the text from it:

```sh
echo "Detailed description" | gest task create "My task"
cat spec.md | gest artifact create --tag spec
```

The second command takes the title of the artifact from the first `#` heading of `spec.md`.

### Batch creation

Use `--batch` to create many records from NDJSON, which has one JSON object on each line:

```sh
cat tasks.ndjson | gest task create --batch
cat artifacts.ndjson | gest artifact create --batch
```

Add many tasks to an iteration, with a phase for each record:

```sh
cat <<'EOF' | gest iteration add <iteration-id> --batch
{"task":"<task-id-1>","phase":1}
{"task":"<task-id-2>","phase":1}
{"task":"<task-id-3>","phase":2}
{"task":"<task-id-4>"}
EOF
```

When a record has no `phase`, gest uses the highest phase of the iteration plus 1.

### Iteration and link flags

Create a task that has a link and belongs to an iteration, in one command:

```sh
gest task create "Add auth" \
  -i <iteration-id> \
  -l child-of:<spec-artifact-id>
```
