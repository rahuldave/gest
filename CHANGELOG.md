# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog], and this project adheres to
[Break Versioning].

## [Unreleased]

## [v0.6.0] - 2026-11-18

The first release of the rahuldave/gest fork.

### Changed

- `gest --version` and `gest -V` print one line, `gest 0.6.0 (rahuldave)`, before reading the config. The
  `(rahuldave)` marker tells this fork from the `gest` crate on crates.io, which is a different program
- Exit codes follow `sysexits.h` (see the v0.5-to-v0.6 migration page)
- `gest serve` reads `.gest/` before each web save and writes it after, so the next CLI command keeps the edit

### Fixed

- Undo: all-or-nothing, and correct for tag, iteration and link removals, artifact bodies, phases, null
  columns and old entries recorded without a key
- Web UI: edits record undo steps, saving a form keeps incoming links, a live update keeps a note being typed,
  Preview works after a live update, and user text cannot receive the CSRF token or CSP nonce
- `GEST_` variables get the type their config key needs
- The v0.4 migrator points links at the new IDs of every entity
- Commands wait up to 5 s for another connection's lock, and batch create takes the write lock up front

## [v0.5.6] - 2026-04-19

### Added

- Pager support on `gest artifact show` and `gest iteration show` — long detail views honor
  `--no-pager`, `pager.enabled` config, and TTY/terminal-height checks, matching list and search
  commands

### Changed

- `project archive`, `project delete`, and `project unarchive` `--json` output now uses the
  Envelope format (with relationships, tags, and notes sidecars), matching other entity commands
- `gest purge` batches cascade deletes into one `IN (...)` query per child table for tasks,
  iterations, and artifacts, eliminating per-row query fanout on large cleanups
- Transaction replay validates `table_name` and `before_data` column keys against a closed
  allowlist — a corrupt or tampered audit row is rejected before its identifiers reach the
  rendered SQL
- Consolidated shared helpers for title inference, status transitions, unlink audit payloads,
  short-ID `--quiet` output, and summary rendering across CLI command handlers

### Fixed

- Web UI `data-nav-select` navigation now refuses non-same-origin option values, so a selector
  populated with an attacker-controlled `javascript:` URI or protocol-relative host can't be used
  to navigate away

## [v0.5.5] - 2026-04-17

### Added

- `gest task unlink` and `gest iteration unlink` commands for removing a specific relationship between two entities,
  with optional `--rel <type>` to disambiguate when multiple edge types connect the same pair and `--artifact` for
  artifact targets; iteration-to-iteration unlinks remove both the forward and reciprocal rows atomically so `undo`
  restores both halves (see [#63])
- `--rel <type>` flag on `iteration link` matching the `task link` shape
- Global kanban board view in the web UI showing every task in the project across iterations

### Deprecated

- Positional `rel` argument on `iteration link <id> <rel> <target>` — use `--rel <type>` instead; the old form still
  works but emits a stderr deprecation warning

## [v0.5.4] - 2026-04-14

### Added

- Mermaid diagram rendering in the web UI — fenced ` ```mermaid ` code blocks render as interactive
  diagrams on all detail pages, timeline notes, and the markdown preview pane, loaded from jsDelivr CDN
  with graceful fallback to raw code when offline
- `--json` and `--quiet` output on `project delete`, `project archive`, and `project unarchive` commands

### Changed

- Iteration descriptions now render as full markdown on the web detail page, matching the behavior of
  artifact and task descriptions
- Iteration list, artifact list, and task list commands batch status, phase, and tag queries instead of
  issuing per-row lookups
- `iteration next` returns proper error codes (exit 2 when no tasks are available) instead of calling
  `process::exit` directly
- Consolidated entity operations behind composable action traits (`Findable`, `Prefixable`, `HasMetadata`,
  `Taggable`, `HasNotes`), reducing duplication across task, artifact, and iteration command handlers

### Fixed

- `purge` no longer prompts for confirmation twice when run without `--yes`

## [v0.5.3] - 2026-04-12

### Fixed

- `gest self-update` downloads the `.sha256` checksum file instead of the `.tar.gz` release archive —
  the asset identifier filter was dropped during the v0.5.0 rewrite, broken since v0.5.0

## [v0.5.2] - 2026-04-12

### Added

- `gest project archive` and `gest project unarchive` commands for soft-archiving projects, with entity count display,
  workspace detach reporting, and a reattach hint on unarchive (see [#38])
- `--all` flag on `project list` to include archived projects, which are hidden by default and shown with
  an `[archived]` badge (see [#38])
- `gest project delete` command for non-undoable cascade deletion of a project and all owned entities
  (tasks, iterations, artifacts), with confirmation prompt and `--yes` bypass (see [#38])
- `gest purge` command for bulk cleanup of terminal tasks, terminal iterations, archived artifacts, archived projects,
  dangling relationships, and orphan tombstones — supports `--dry-run` preview and `--yes` bypass (see [#39])

### Changed

- ID prefixes in list and search output now use per-ID variable lengths, showing the shortest unique prefix for each
  entity instead of a single shared minimum

## [v0.5.1] - 2026-04-10

### Added

- Priority labels (`critical`, `high`, `medium`, `low`, `lowest`) accepted on `task create --priority` and
  `task update --priority` alongside the existing 0–4 integer form (see [#34])
- Priority dropdown on web task create and edit forms, replacing the free-text numeric input (see [#34])
- Markdown preview toggle on web task create and edit forms, with input preservation on validation failure
  (see [#36])
- Iteration graph view restored with phase headers, priority badges, parallel column indicators, and
  blocked/blocking dependency markers (see [#37])
- CSP nonce middleware replaces `'unsafe-inline'` script/style sources, plus `Referrer-Policy`,
  `Permissions-Policy`, `frame-ancestors`, `base-uri`, and `form-action` directives (see [#47])
- CSRF protection via double-submit signed cookies on all mutating web requests (see [#49])
- First-party avatar proxy at `/avatars/{hash}` so the web dashboard no longer leaks browsing activity to
  Gravatar and works in air-gapped deployments (see [#50])
- `#` level markers on rendered markdown headings in terminal output (see [#53])
- `--json` output on entity commands now uses an Envelope format with relationships, tags, and notes
  sidecars (see [#54])
- `--batch` flag on `iteration add` for bulk NDJSON task addition with per-record phase control and
  all-or-nothing semantics
- Interactive Yes/No confirmation selector with arrow-key, h/l, j/k, and y/n navigation, replacing the
  text-based `[y/N]` prompt

### Changed

- Web handler errors unified under a single `web::Error` type — store `NotFound` surfaces as HTTP 404 and
  bad requests as 400 instead of generic 500s (see [#45])
- Web list pages batch tag and relationship lookups in two queries, eliminating per-row N+1 fan-out
  (see [#46])
- Store errors consolidated into a single `store::Error` enum with `find_required_by_id` helpers, removing
  per-module error types (see [#43])
- SQL table references in the resolver use a closed `Table` enum instead of caller-supplied strings
  (see [#44])
- Typed error page in the web UI renders proper HTML with contextual status codes instead of plain-text
  error strings (see [#35])

### Fixed

- Iteration tags now render as clickable `#` links in the web UI (see [#52])
- `--tag` and `--tags` arguments accept comma-separated values (e.g. `--tag a,b` produces two tags)
  (see [#51])
- Empty priority on web task create no longer returns a plain-text 500 (see [#34])
- Banner author coloring restored to amber bold
- Self-update changelog link updated for the Docusaurus docs site URL scheme

## [v0.5.0] - 2026-04-08

### Added

- `gest migrate --from v0.4` imports an existing v0.4.x flat-file `.gest/` directory into the new
  SQLite-backed store in a single command, preserving IDs, tags, links, notes, and iteration membership
- `gest project` command group for managing project identity across checkouts, with `attach`, `detach`,
  and `list` subcommands so multiple worktrees can share a single project row
- `gest task delete` and `gest artifact delete` for hard deletion (with `--yes` to skip the confirmation
  prompt and `--force` on task delete to remove iteration membership first)
- `gest task claim` shortcut that assigns the task to the current author and marks it in-progress in a
  single step
- `gest artifact note add|list|show|update|delete` subcommands, bringing full note management to
  artifacts alongside tasks
- Remote database support via a new `[database]` config section with `url`, `host`, `port`, `scheme`,
  `username`, `password`, and `auth_token` fields for connecting to hosted libsql/Turso instances
- `storage.sync` config field (and `GEST_STORAGE__SYNC` env var) controlling whether the database is
  mirrored to a `.gest/` directory of YAML/Markdown files for git-committable inspection
- `--raw` (`-r`) output flag on entity commands for script-friendly plain output without themed styling
- `--limit <N>` flag on `task list`, `iteration list`, `artifact list`, and `search` for capping
  result counts
- `--metadata-json` flag on create/update commands for merging JSON metadata objects on top of
  `--metadata key=value` pairs
- `-i`/`--iteration` flag on `artifact create` for linking a new artifact to an iteration inline
- `-s`/`--source` flag on `artifact create` for reading the body from a file

### Changed

- **SQLite-first storage.** Entity data now lives in a single libsql database at `<data_dir>/gest.db`.
  For local-mode projects, `.gest/` becomes a sync mirror that is regenerated from the database on
  every mutation, with entities grouped into singular per-entity subdirectories (`task/`, `artifact/`,
  `iteration/`) instead of the old plural layout
- **Project identity is explicit.** Projects are rows in the database keyed by an assigned ID rather
  than by hashing the current working directory, so multiple worktrees of the same repo can attach to
  the same project and share a live view of tasks, artifacts, and iterations
- **Destructive commands prompt by default.** `task delete`, `artifact delete`, `task note delete`,
  and other removal commands now confirm interactively; pass `--yes` to skip the prompt in scripts
- Complete theme overhaul with a new token hierarchy and richer palette semantics — see the
  [theming reference](https://gest.aaronmallen.dev/configuration/theming) for the full token list

### Fixed

- `gest search` now supports `--limit <N>` so you can cap large result sets without relying on pager
  navigation
- `config get storage.data_dir` returns the resolved path instead of `null` when no explicit override
  is set, matching the fix landed for `storage.project_dir` in v0.4.4

### Breaking

- **Storage format changed to SQLite.** Existing v0.4.x projects must be imported with
  `gest migrate --from v0.4` — see the
  [v0.4 → v0.5 migration guide](https://gest.aaronmallen.dev/migration/v0-4-to-v0-5) for step-by-step
  instructions, rollback procedure, and data mapping
- **Artifact `kind` field removed.** Artifact categorization is now tag-driven. The migrator converts
  existing `kind: "spec"` values into a `spec` tag automatically
- **`artifact create` CLI reshaped.** The command now takes a positional `[TITLE]` argument; the
  `--title` and `--type` flags have been removed, and `-t` is now a shortcut for `--tag` instead of
  `--type`
- **`type:` search filter removed.** Use `tag:<name>` instead (e.g. `gest search 'tag:spec'`)
- **Environment variables renamed or removed.** `GEST_LOG_LEVEL` is now `GEST_LOG__LEVEL` (double
  underscore); `GEST_DATA_DIR` is now `GEST_STORAGE__DATA_DIR`; and `GEST_PROJECT_DIR`,
  `GEST_STATE_DIR`, `GEST_ARTIFACT_DIR`, `GEST_TASK_DIR`, and `GEST_ITERATION_DIR` have all been
  removed — there are no per-entity directory overrides in the new storage model
- **Config fields removed.** `storage.project_dir`, `storage.state_dir`, `storage.artifact_dir`,
  `storage.task_dir`, and `storage.iteration_dir` are no longer honored and should be deleted from
  existing `gest.toml` files
- **Undo state lives in the database.** There is no longer a separate state directory; undo history
  follows whichever database the command ran against

## [v0.4.4] - 2026-04-04

### Added

- Security headers (`Content-Security-Policy`, `X-Frame-Options`, `X-Content-Type-Options`) on all web server responses

### Changed

- Faster iteration orchestration via optimized collection operations and reduced cloning

### Fixed

- `-q` flag now outputs the 8-character short ID instead of the full 32-character ID
- Store writes use proper temporary files to avoid collisions under concurrent access
- `config get storage.project_dir` now returns the resolved path instead of `null` (see [#31])

## [v0.4.3] - 2026-04-03

### Added

- `iteration cancel` and `iteration reopen` shortcut commands for iteration lifecycle management,
  with automatic cascade — cancelling an iteration cancels all non-terminal tasks, reopening restores
  them
- `Cancelled` iteration status with `failed` retained as a deprecated alias for backward
  compatibility
- Live reload in the web UI via server-sent events — pages update automatically when project files
  change on disk, with configurable debounce (`serve.debounce_ms`, default 2000ms)
- Per-request HTTP logging with method, path, status, and elapsed time (`serve.log_level`, default
  info)

### Changed

- Dashboard now shows open + in-progress task counts instead of all tasks, with a new iteration
  status breakdown row
- Web server handler I/O is now offloaded to blocking threads, preventing file reads from starving
  async connection handling

### Fixed

- Editor content is no longer silently discarded when using `read_from_editor`

## [v0.4.2] - 2026-04-02

### Added

- Command aliases for faster navigation: top-level shortcuts (`a`, `t`, `i`, `grep`), subcommand
  aliases (`new`, `ls`, `view`, `edit`, `rm`), and single-letter aliases (`u` for undo, `s` for
  serve)
- `task complete`, `task cancel`, and `task block` shortcut commands for common lifecycle transitions
- `--json` and `-q` output flags on all mutation commands (create, update, status, tag, untag, link,
  meta set, note) for machine-readable and script-friendly output
- `--json` and `--raw` flags on `meta get` for structured and raw value output
- `--batch` flag on create commands for NDJSON bulk creation of tasks and artifacts via stdin
- `-i`/`--iteration` flag on create commands to add new entities to an iteration inline
- `-l`/`--link` flag on `task create` for inline link creation (e.g. `--link blocks:abc`)
- Implicit stdin support for body and description fields — pipe content directly without flags
- Unified `--tag` flag on create and update commands and comma-separated positional tags on tag/untag
  commands (e.g. `gest task tag <id> rust,cli`)
- Search output is now paged through `$PAGER` (falling back to `less -R`) when stdout is a terminal
- `-j` short flag for `--json` on `task note list` and `task note show`

### Changed

- Consolidated entity operations behind generic trait-based action functions, reducing duplication
  across task, artifact, and iteration command handlers
- Replaced catch-all error variants with domain-specific error types for clearer diagnostics

### Fixed

- Tag list table now renders with themed formatting

## [v0.4.1] - 2026-04-01

### Added

- Search queries now support structured filters: `is:`, `tag:`, `status:`, and `type:` prefixes with negation via `-`
  prefix, OR-combination within the same filter type, and AND-combination across different types

### Changed

- Unified metadata handling across tasks and artifacts via a shared trait, reducing code duplication in the store layer

### Fixed

- Entity file moves are now atomic (write-to-temp then rename), eliminating partial-read and TOCTOU race conditions
- Unrecognized event types in the database now produce a user-visible error instead of panicking
- Web view tables no longer overflow the container on narrow viewports
- HTML error responses in the web server are sanitized to prevent XSS
- Dashboard handler now logs errors instead of silently swallowing them
- Task link updates are now atomic via a dedicated `links` patch field
- Metadata values set via the CLI are parsed as typed TOML values (integers, booleans, floats) instead of always strings
- Native TOML datetime values are handled correctly during deserialization
- Project directory resolution no longer contains an unreachable code branch
- Integer casts in iteration orchestration replaced with correct `usize` types
- UI text truncation no longer panics on certain inputs

## [v0.4.0] - 2026-04-01

### Added

- `gest undo` command reverses the most recent mutating transaction by restoring file snapshots, backed by a new SQLite
  event store
- `GEST_STATE_DIR` environment variable and `storage.state_dir` config field for controlling event store location
- `gest tag add|remove|list` subcommands for managing tags across tasks, artifacts, and iterations
- `gest tags` command for listing all tags with optional entity-type filtering
- `--no-color` global flag for disabling colored output
- Cross-entity ID resolution with ambiguity detection and collision-safe ID generation
- Automatic event generation on task and iteration status, phase, and priority changes with author attribution
- Merged event/note activity timeline in `task show`, `iteration show`, and the web task detail page
- Two-tier color configuration: `[colors.palette]` for 11 semantic color slots and `[colors.overrides]` for per-token
  customization, with palette values cascading through to all referencing tokens
- `config show` now displays palette and override counts separately
- Web UI accessibility improvements: semantic landmarks and skip navigation, rem-based font sizes, WCAG 2.1 AA color
  contrast, focus-visible keyboard styles, form ARIA labels, semantic heading hierarchy, and keyboard-accessible
  relationship modal

### Changed

- **Breaking:** `GEST_DATA_DIR` now points to the global root directory instead of the project-specific path; use the
  new `GEST_PROJECT_DIR` env var (or `storage.project_dir` config field) for the old behavior
- **Breaking:** The flat `[colors]` config section is replaced by `[colors.palette]` and `[colors.overrides]`
- **Breaking:** Project discovery no longer matches undotted `gest/` directories; use `.gest/` or set
  `$GEST_PROJECT_DIR`
- **Breaking:** Task and iteration TOML files no longer write `resolved_at = ""` or `completed_at = ""` for unset
  datetime fields; existing files with empty strings are still read correctly

### Fixed

- `config set` now writes typed TOML values (integers, booleans, floats) instead of wrapping all values as strings
- Event recording no longer silently skipped when git `user.name` is unset — falls back to "unknown" author

## [v0.3.5] - 2026-03-31

### Added

- Task notes with author attribution (human via git config, agent via `--agent` flag), timestamps, and rendered markdown
  body — managed via `gest task note add|list|show|update|delete` subcommands
- Notes appear in `task show` output and the web UI task detail page with Gravatar avatars and agent badges
- Iteration orchestration commands: `iteration status` for progress tracking, `iteration next` for peeking at or
  claiming the next available task, and `iteration advance` for signaling phase completion
- `--assigned-to` filter on `task list` for narrowing tasks by assignee
- `--has-available` filter on `iteration list` for finding iterations with claimable work
- `gest search` now includes iterations in search results

## [v0.3.4] - 2026-03-31

### Fixed

- ID prefix validation now rejects invalid characters, preventing potential path injection via crafted entity IDs

## [v0.3.3] - 2026-03-31

### Added

- Task and artifact create/edit forms in the web UI with markdown preview, relationship management, and inline search
- Dashboard status cards now link to filtered task views
- Navigation buttons (new/edit) on list and detail pages

### Changed

- Short ID prefixes are now displayed with clickable links, tags are clickable, and artifact type label is used
  consistently across the web UI

### Fixed

- Store write functions now respect entity location (active vs. archive/resolved), preventing duplicates on mutation
- CLI flags (`--version`, `--verbose`) and shell completion variants now have help text (see [#24])

## [v0.3.2] - 2026-03-31

### Added

- Built-in web server (`gest serve`) with dashboard, task/artifact/iteration views, kanban board, and full-text search
  (see [#13], [#14], [#15], [#16], [#17], [#18], [#19], [#20], [#21], [#22], [#23])
- `--port`, `--bind`, and `--no-open` flags on `gest serve` for controlling the local server (see [#15])
- `[serve]` config section with `port`, `bind_address`, and `open` settings (see [#14])

### Changed

- Cached iteration phase counts to avoid recomputing on each read
- `resolve_dot_path` now walks by reference instead of cloning, reducing allocations
- Consolidated shared helpers for store operations, CLI metadata handling, and tag/untag commands

### Fixed

- Multi-word `EDITOR` values (e.g. `code --wait`) are now parsed correctly via shell tokenization
- Nested metadata `set_nested` is now bounds-checked with a depth limit to prevent stack overflows
- Version line no longer appears in the `--help` banner
- `gest search --expand` now shows full content instead of truncated snippets (see [#11])

## [v0.3.1] - 2026-03-30

### Added

- Per-entity directory overrides via `GEST_ARTIFACT_DIR`, `GEST_TASK_DIR`, `GEST_ITERATION_DIR` environment variables
  and corresponding `storage.*_dir` config fields

### Changed

- Batched disk reads when resolving blocked-by status for task lists, replacing per-task I/O with a single pass over
  unique references
- Faster search filtering by avoiding redundant string allocations during case-insensitive matching
- Early-exit prefix matching stops scanning after two matches instead of collecting all candidates
- Skip metadata serialization during search when metadata maps are empty
- Store functions now receive the full application config, allowing future access to any setting without plumbing
  changes
- Consolidated shared helpers for editor invocation, TOML metadata operations, iteration task-loading, and status
  display across CLI commands

## [v0.3.0] - 2026-03-30

### Changed

- **Breaking:** Configuration files are now TOML only — JSON and YAML configs are no longer supported
- **Breaking:** `gest init` now creates the global data store by default; use `gest init --local`
  for in-repo `.gest/` mode
- Complete UI overhaul with an atomic architecture (atoms, composites, views) for consistent,
  aligned rendering across all commands

## [v0.2.3] - 2026-03-29

### Added

- Iteration entity type with storage, UI components, CLI commands, and graph visualization for planning multi-phase work
- `priority`, `assigned_to`, and `phase` fields on tasks for richer project tracking
- `--expand` flag on `gest search` now works without `--json`, showing full detail blocks directly in the terminal

## [v0.2.2] - 2026-03-27

### Added

- `gest version` now checks for newer releases and suggests `gest self-update` when one is available
- `--expand` (`-e`) flag on `gest search` enriches `--json` output with full item details
- `indicator.blocked` and `indicator.blocking` theme tokens for task list status indicators

### Changed

- Reduced unnecessary memory allocations in UI rendering and search
- Extracted shared helpers and removed dead code for cleaner internals

## [v0.2.1] - 2026-03-26

### Fixed

- Install script now downloads the correct checksum file during installation
- Self-update no longer fails with a 404 when fetching the target release

## [v0.2.0] - 2026-03-26

### Added

- Shell completions and man-page generation commands (see [#1])
- Markdown rendering in the terminal with styled headings, code blocks, lists, blockquotes, and more (see [#2], [#3])
- Artifact and task detail views now render descriptions as styled markdown (see [#4], [#5])
- Verbose logging across commands, store operations, and config discovery

### Changed

- `artifact list --include-archived` renamed to `--all` (`-a`), matching task and search commands (**breaking**)
- Faster unique ID prefix computation and shorter ID encoding

### Fixed

- Log level semantics now correctly follow debug=why, trace=result convention

## v0.1.0 - 2026-03-26

Initial release

[Break Versioning]: https://www.taoensso.com/break-versioning
[Keep a Changelog]: https://keepachangelog.com/en/1.1.0/

[#1]: https://github.com/aaronmallen/gest/issues/1
[#2]: https://github.com/aaronmallen/gest/issues/2
[#3]: https://github.com/aaronmallen/gest/issues/3
[#4]: https://github.com/aaronmallen/gest/issues/4
[#5]: https://github.com/aaronmallen/gest/issues/5
[#11]: https://github.com/aaronmallen/gest/issues/11
[#13]: https://github.com/aaronmallen/gest/issues/13
[#14]: https://github.com/aaronmallen/gest/issues/14
[#15]: https://github.com/aaronmallen/gest/issues/15
[#16]: https://github.com/aaronmallen/gest/issues/16
[#17]: https://github.com/aaronmallen/gest/issues/17
[#18]: https://github.com/aaronmallen/gest/issues/18
[#19]: https://github.com/aaronmallen/gest/issues/19
[#20]: https://github.com/aaronmallen/gest/issues/20
[#21]: https://github.com/aaronmallen/gest/issues/21
[#22]: https://github.com/aaronmallen/gest/issues/22
[#23]: https://github.com/aaronmallen/gest/issues/23
[#24]: https://github.com/aaronmallen/gest/issues/24
[#31]: https://github.com/aaronmallen/gest/issues/31
[#34]: https://github.com/aaronmallen/gest/issues/34
[#35]: https://github.com/aaronmallen/gest/issues/35
[#36]: https://github.com/aaronmallen/gest/issues/36
[#37]: https://github.com/aaronmallen/gest/issues/37
[#38]: https://github.com/aaronmallen/gest/issues/38
[#39]: https://github.com/aaronmallen/gest/issues/39
[#43]: https://github.com/aaronmallen/gest/issues/43
[#44]: https://github.com/aaronmallen/gest/issues/44
[#45]: https://github.com/aaronmallen/gest/issues/45
[#46]: https://github.com/aaronmallen/gest/issues/46
[#47]: https://github.com/aaronmallen/gest/issues/47
[#49]: https://github.com/aaronmallen/gest/issues/49
[#50]: https://github.com/aaronmallen/gest/issues/50
[#51]: https://github.com/aaronmallen/gest/issues/51
[#52]: https://github.com/aaronmallen/gest/issues/52
[#53]: https://github.com/aaronmallen/gest/issues/53
[#54]: https://github.com/aaronmallen/gest/issues/54
[#63]: https://github.com/aaronmallen/gest/issues/63

[Unreleased]: https://github.com/rahuldave/gest/compare/v0.6.0...main
[v0.6.0]: https://github.com/rahuldave/gest/compare/0.5.6...v0.6.0
[v0.5.6]: https://github.com/aaronmallen/gest/compare/0.5.5...0.5.6
[v0.5.5]: https://github.com/aaronmallen/gest/compare/0.5.4...0.5.5
[v0.5.4]: https://github.com/aaronmallen/gest/compare/0.5.3...0.5.4
[v0.5.3]: https://github.com/aaronmallen/gest/compare/0.5.2...0.5.3
[v0.5.2]: https://github.com/aaronmallen/gest/compare/0.5.1...0.5.2
[v0.5.1]: https://github.com/aaronmallen/gest/compare/0.5.0...0.5.1
[v0.5.0]: https://github.com/aaronmallen/gest/compare/0.4.4...0.5.0
[v0.4.4]: https://github.com/aaronmallen/gest/compare/0.4.3...0.4.4
[v0.4.3]: https://github.com/aaronmallen/gest/compare/0.4.2...0.4.3
[v0.4.2]: https://github.com/aaronmallen/gest/compare/0.4.1...0.4.2
[v0.4.1]: https://github.com/aaronmallen/gest/compare/0.4.0...0.4.1
[v0.4.0]: https://github.com/aaronmallen/gest/compare/0.3.5...0.4.0
[v0.3.5]: https://github.com/aaronmallen/gest/compare/0.3.4...0.3.5
[v0.3.4]: https://github.com/aaronmallen/gest/compare/0.3.3...0.3.4
[v0.3.3]: https://github.com/aaronmallen/gest/compare/0.3.2...0.3.3
[v0.3.2]: https://github.com/aaronmallen/gest/compare/0.3.1...0.3.2
[v0.3.1]: https://github.com/aaronmallen/gest/compare/0.3.0...0.3.1
[v0.3.0]: https://github.com/aaronmallen/gest/compare/0.2.3...0.3.0
[v0.2.3]: https://github.com/aaronmallen/gest/compare/0.2.2...0.2.3
[v0.2.2]: https://github.com/aaronmallen/gest/compare/0.2.1...0.2.2
[v0.2.1]: https://github.com/aaronmallen/gest/compare/0.2.0...0.2.1
[v0.2.0]: https://github.com/aaronmallen/gest/compare/0.1.0...0.2.0
