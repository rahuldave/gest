---
title: Theming
---

gest lets you change the colours of its terminal interface. You can override two things:

- The **palette**. Each palette slot is a named colour, such as `primary` or `warning`, and many
  elements of the interface use the same slot.
- Single **theme tokens**. A token is one element of the interface, such as `log.error`.

The theme settings go in the `[colors]` section of your config file.

## How theme resolution works

gest builds each style in three layers, and each layer overrides the one before it:

1. The built-in defaults.
2. The overrides in `[colors.palette]`.
3. The token overrides in `[colors.overrides]`.

A palette override replaces the foreground colour of every token that uses that slot. The token keeps
its built-in modifiers, such as `bold`, `italic` or `underline`.

A token override is the most specific layer. It can replace the foreground and background colours of
one token, and add text modifiers to it.

## Supported color formats

A colour can have one of two formats:

- A named ANSI colour, such as `red`, `yellow` or `bright_cyan`. The names ignore case.
- A hex colour with 6 digits, in the format `#RRGGBB`.

gest knows these named colours:

`black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`,
`bright_black`, `bright_red`, `bright_green`, `bright_yellow`, `bright_blue`,
`bright_magenta`, `bright_cyan`, `bright_white`

Put a palette key or a token name that has a dot in double quotes, such as `"text.muted"`. Without the
quotes, TOML reads the dot as a nested table, and gest stops with a configuration error.

## Quick example

```toml
[colors.palette]
primary = "#7CC4F0"
warning = "#E3A22B"
"text.muted" = "#8B93A7"

[colors.overrides]
"log.error" = "#FF6B6B"
"tag" = { fg = "#4EA8E0", italic = true }
"config.heading" = { fg = "#4EA8E0", bold = true, underline = true }
```

## Palette slots

The palette has 11 slots:

| Key             | Default   | Used for          |
|-----------------|-----------|-------------------|
| `accent`        | `#D05830` | warm highlights   |
| `border`        | `#30323A` | rules and borders |
| `error`         | `#D03838` | error states      |
| `primary`       | `#4EA8E0` | brand emphasis    |
| `primary.dark`  | `#3278B0` | dark accents      |
| `primary.light` | `#7CC4F0` | light accents     |
| `success`       | `#36BE78` | success states    |
| `text`          | `#C4C8D4` | body text         |
| `text.dim`      | `#585E6E` | dim text          |
| `text.muted`    | `#7C8294` | secondary text    |
| `warning`       | `#CC9820` | warning states    |

Example:

```toml
[colors.palette]
primary = "#5AB0FF"
success = "#47C97B"
text = "#E3E7F1"
"text.muted" = "#98A1B3"
```

## Token overrides

Token overrides go in `[colors.overrides]`. Each key is the name of a token, with dots between its
parts.

### Simple form

The string form sets only the foreground color:

```toml
[colors.overrides]
"log.warn" = "yellow"
"message.success.icon" = "#36BE78"
```

### Table form

The table form can set colours and text modifiers:

```toml
[colors.overrides]
"search.query" = { fg = "#F4F7FF", bold = true }
"markdown.blockquote" = { fg = "#8B93A7", italic = true }
"banner.update.version" = { fg = "#FFCC4D", bold = true }
```

The table form has these fields:

| Field       | Type    | Description        |
|-------------|---------|--------------------|
| `fg`        | string  | Foreground color   |
| `bg`        | string  | Background color   |
| `bold`      | boolean | Enable bold text   |
| `dim`       | boolean | Enable dim text    |
| `italic`    | boolean | Enable italic text |
| `underline` | boolean | Enable underline   |

## Token reference

gest knows the token keys below.

### Core

- `emphasis`
- `error`
- `muted`
- `success`
- `tag`
- `border`

### Artifact views

- `artifact.detail.label`
- `artifact.detail.separator`
- `artifact.detail.value`
- `artifact.list.archived.badge`
- `artifact.list.tag.archived`
- `artifact.list.title`
- `artifact.list.title.archived`

### Banner

- `banner.author`
- `banner.author.name`
- `banner.gradient.end`
- `banner.gradient.start`
- `banner.shadow`
- `banner.update.command`
- `banner.update.hint`
- `banner.update.message`
- `banner.update.version`
- `banner.version`
- `banner.version.date`
- `banner.version.revision`

### Configuration views

- `config.heading`
- `config.label`
- `config.no_overrides`
- `config.value`

### Confirm prompt

- `confirm.description`
- `confirm.indicator`
- `confirm.selected`
- `confirm.unselected`

### IDs and status indicators

- `id.prefix`
- `id.rest`
- `indicator.blocked`
- `indicator.blocked_by.id`
- `indicator.blocked_by.label`
- `indicator.blocking`

### Init output

- `init.command.prefix`
- `init.label`
- `init.section`
- `init.value`

### Iterations

- `iteration.detail.count.blocked`
- `iteration.detail.count.done`
- `iteration.detail.count.in_progress`
- `iteration.detail.count.open`
- `iteration.detail.label`
- `iteration.detail.value`
- `iteration.graph.branch`
- `iteration.graph.phase.icon`
- `iteration.graph.phase.label`
- `iteration.graph.phase.name`
- `iteration.graph.separator`
- `iteration.graph.title`
- `iteration.list.summary`
- `iteration.list.title`
- `iteration.status.label`
- `iteration.status.progress`
- `iteration.status.value`

### Lists

- `list.heading`
- `list.summary`

### Logs

- `log.debug`
- `log.error`
- `log.info`
- `log.timestamp`
- `log.trace`
- `log.warn`

### Markdown rendering

- `markdown.alert.caution.border`
- `markdown.alert.important.border`
- `markdown.alert.note.border`
- `markdown.alert.tip.border`
- `markdown.alert.warning.border`
- `markdown.blockquote`
- `markdown.blockquote.border`
- `markdown.code.block`
- `markdown.code.border`
- `markdown.code.inline`
- `markdown.emphasis`
- `markdown.heading`
- `markdown.link`
- `markdown.rule`
- `markdown.strong`

### Messages

- `message.created.label`
- `message.success.icon`
- `message.updated.label`

### Meta

- `meta.not_set`
- `meta.value`

### Migrate

- `migrate.count`

### Notes

- `note.detail.label`
- `note.detail.separator`
- `note.detail.value`
- `note.list.body`
- `note.list.id`

### Projects

- `project.list.archived.badge`
- `project.list.root`
- `project.list.root.archived`
- `project.show.value`

### Search

- `search.expand.separator`
- `search.no_results.hint`
- `search.query`
- `search.summary`
- `search.type.label`

### Serve

- `serve.url`

### Tags

- `tag.list.count`
- `tag.list.heading`

### Task and iteration status

- `status.cancelled`
- `status.done`
- `status.in_progress`
- `status.open`

### Tasks

- `task.detail.label`
- `task.detail.separator`
- `task.detail.title`
- `task.detail.value`
- `task.list.icon.cancelled`
- `task.list.icon.done`
- `task.list.icon.in_progress`
- `task.list.icon.open`
- `task.list.priority`
- `task.list.title`
- `task.list.title.cancelled`

## Markdown shorthand aliases

Each `markdown.*` token has a short alias that starts with `md.`. You can use the aliases in
`[colors.overrides]`:

| Shorthand                   | Resolves to                       |
|-----------------------------|-----------------------------------|
| `md.alert.caution.border`   | `markdown.alert.caution.border`   |
| `md.alert.important.border` | `markdown.alert.important.border` |
| `md.alert.note.border`      | `markdown.alert.note.border`      |
| `md.alert.tip.border`       | `markdown.alert.tip.border`       |
| `md.alert.warning.border`   | `markdown.alert.warning.border`   |
| `md.blockquote`             | `markdown.blockquote`             |
| `md.blockquote.border`      | `markdown.blockquote.border`      |
| `md.code.block`             | `markdown.code.block`             |
| `md.code.border`            | `markdown.code.border`            |
| `md.code`                   | `markdown.code.inline`            |
| `md.emphasis`               | `markdown.emphasis`               |
| `md.heading`                | `markdown.heading`                |
| `md.link`                   | `markdown.link`                   |
| `md.rule`                   | `markdown.rule`                   |
| `md.strong`                 | `markdown.strong`                 |

::: {.callout-tip}
`md.code` maps to `markdown.code.inline`, not to `markdown.code`.
:::

## Notes

- `banner.gradient.start`, `banner.gradient.end`, `banner.shadow` and
  `markdown.alert.important.border` have built-in RGB colours. No palette slot changes them.
- `markdown.emphasis` and `markdown.strong` have only modifiers by default, so a palette override does
  not change them. Override the token itself to colour them.
- gest ignores an unknown palette key or an unknown token name, and prints a warning.
