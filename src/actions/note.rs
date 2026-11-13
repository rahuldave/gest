//! Generic note actions: add, show, list, update and delete the notes on a task or an artifact.

use std::fmt::Write;

use crate::{
  AppContext,
  actions::HasNotes,
  cli::{Error, limit::LimitArgs, prompt},
  io::git,
  store::{
    model::{
      note::{New, Patch},
      primitives::AuthorType,
    },
    repo,
  },
  ui::{
    components::{FieldList, SuccessMessage},
    json,
  },
};

/// Display a single note by its ID.
///
/// # Arguments
///
/// * `context` - the application context: settings, store and current project
/// * `raw_note_id` - the note's ID or a unique prefix of it
/// * `output` - the output mode: normal, `--json`, `--quiet` or `--raw`
///
/// # Returns
///
/// `Ok(())` once the note is printed, or the error from resolving or loading it.
pub async fn show(context: &AppContext, raw_note_id: &str, output: &json::Flags) -> Result<(), Error> {
  let conn = context.store().connect().await?;
  let note_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Notes, raw_note_id).await?;
  let note = repo::note::find_required_by_id(&conn, note_id).await?;

  let short_id = note.id().short();
  output.print_entity(&note, &short_id, || {
    let mut fields = FieldList::new()
      .field("id", note.id().short())
      .field("body", note.body().to_string())
      .field("created", note.created_at().to_rfc3339());

    if let Some(author) = note.author_id() {
      fields = fields.field("author", author.short());
    }

    fields = fields.field("updated", note.updated_at().to_rfc3339());

    fields.to_string()
  })?;
  Ok(())
}

/// Create a new note on the resolved entity, resolving the author from flags or git identity.
///
/// # Arguments
///
/// * `context` - the application context: settings, store and current project
/// * `raw_id` - the parent entity's ID or a unique prefix of it
/// * `body` - the note text, or `-` to write it in `$EDITOR`
/// * `agent` - the agent's name, which makes the author an agent; with `None`, the git or `$USER` identity is used
/// * `output` - the output mode: normal, `--json`, `--quiet` or `--raw`
///
/// # Returns
///
/// `Ok(())` once the note is saved and printed, [`Error::Editor`] when the body is empty or the
/// editor fails, or the error from resolving the entity or saving the note.
pub async fn add<E: HasNotes>(
  context: &AppContext,
  raw_id: &str,
  body: &str,
  agent: Option<&str>,
  output: &json::Flags,
) -> Result<(), Error> {
  let entity_type = E::entity_type();
  let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
  let conn = context.store().connect().await?;
  let parent_id = repo::resolve::resolve_id(&conn, E::table(), raw_id).await?;

  let body = if body == "-" {
    crate::io::editor::edit_text_with_suffix("", ".md").map_err(|e| Error::Editor(e.to_string()))?
  } else {
    body.to_owned()
  };

  if body.trim().is_empty() {
    return Err(Error::Editor("Aborting: empty note body".into()));
  }

  let author_id = match agent {
    Some(name) => {
      let author = repo::author::find_or_create(&conn, name, None, AuthorType::Agent).await?;
      Some(author.id().clone())
    }
    None => {
      if let Some(ga) = git::resolve_author_or_env() {
        let author = repo::author::find_or_create(&conn, &ga.name, ga.email.as_deref(), AuthorType::Human).await?;
        Some(author.id().clone())
      } else {
        None
      }
    }
  };

  let new = New {
    author_id,
    body,
  };
  let label = format!("{entity_type} note add");
  let tx = repo::transaction::begin(&conn, project_id, &label).await?;
  let note = repo::note::create(&conn, entity_type, &parent_id, &new).await?;
  repo::transaction::record_event(&conn, tx.id(), "notes", &note.id().to_string(), "created", None).await?;

  let entity_label = entity_type.to_string();
  let short_id = note.id().short();
  output.print_entity(&note, &short_id, || {
    log::info!("added note");
    SuccessMessage::new("added note")
      .id(note.id().short())
      .field(&entity_label, parent_id.short())
      .to_string()
  })?;
  Ok(())
}

/// Confirm and delete the resolved note within a recorded transaction.
///
/// # Arguments
///
/// * `context` - the application context: settings, store and current project
/// * `raw_note_id` - the note's ID or a unique prefix of it
/// * `yes` - skip the confirmation prompt
/// * `entity_label` - the parent's kind, such as `task`, used in the prompt and the transaction label
/// * `output` - the output mode: normal, `--json`, `--quiet` or `--raw`
///
/// # Returns
///
/// `Ok(())` when the note is deleted or the user declines the prompt, or the error that
/// stopped it.
pub async fn delete(
  context: &AppContext,
  raw_note_id: &str,
  yes: bool,
  entity_label: &str,
  output: &json::Flags,
) -> Result<(), Error> {
  let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
  let conn = context.store().connect().await?;
  let note_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Notes, raw_note_id).await?;

  let before_note = repo::note::find_by_id(&conn, note_id.clone()).await?;
  if !prompt::confirm_destructive("delete", &format!("{entity_label} note {}", note_id.short()), yes)? {
    log::info!("{entity_label} note delete: aborted by user");
    return Ok(());
  }
  let tx_label = format!("{entity_label} note delete");
  let tx = repo::transaction::begin(&conn, project_id, &tx_label).await?;
  repo::note::delete(&conn, &note_id).await?;
  if let Some(note) = &before_note {
    let before = serde_json::to_value(note)?;
    repo::transaction::record_event(&conn, tx.id(), "notes", &note_id.to_string(), "deleted", Some(&before)).await?;
  }

  let short_id = note_id.short();
  output.print_delete(|| SuccessMessage::new("deleted note").id(short_id.clone()).to_string())?;
  Ok(())
}

/// Render notes attached to the resolved entity.
///
/// # Arguments
///
/// * `context` - the application context: settings, store and current project
/// * `raw_id` - the parent entity's ID or a unique prefix of it
/// * `limit` - the `--limit` option, which caps how many notes print
/// * `output` - the output mode: normal, `--json`, `--quiet` or `--raw`
///
/// # Returns
///
/// `Ok(())` once the notes are printed, through the pager unless `--json` or `--quiet` is
/// set, or the error that stopped it.
pub async fn list<E: HasNotes>(
  context: &AppContext,
  raw_id: &str,
  limit: &LimitArgs,
  output: &json::Flags,
) -> Result<(), Error> {
  let conn = context.store().connect().await?;
  let parent_id = repo::resolve::resolve_id(&conn, E::table(), raw_id).await?;

  let mut notes = repo::note::for_entity(&conn, E::entity_type(), &parent_id).await?;
  limit.apply(&mut notes);

  if output.json || output.quiet {
    return output.print_entities(
      &notes,
      || notes.iter().map(|n| n.id().short()).collect(),
      || unreachable!("normal branch is handled by the pager below"),
    );
  }

  if notes.is_empty() {
    crate::io::pager::page("  no notes\n", context)?;
    return Ok(());
  }

  let mut buf = String::new();
  for (i, note) in notes.iter().enumerate() {
    if i > 0 {
      buf.push('\n');
    }
    let fields = FieldList::new()
      .field("id", note.id().short())
      .field("body", note.body().to_string());
    let _ = writeln!(buf, "{fields}");
  }
  crate::io::pager::page(&buf, context)?;

  Ok(())
}

/// Replace the resolved note's body within a recorded transaction.
///
/// # Arguments
///
/// * `context` - the application context: settings, store and current project
/// * `raw_note_id` - the note's ID or a unique prefix of it
/// * `body` - the new text; `-` or `None` opens the current body in `$EDITOR`
/// * `entity_label` - the parent's kind, such as `task`, used in the transaction label
/// * `output` - the output mode: normal, `--json`, `--quiet` or `--raw`
///
/// # Returns
///
/// `Ok(())` once the note is saved and printed, [`Error::Editor`] when the new body is empty or
/// the editor fails, or the error from resolving or saving the note.
pub async fn update(
  context: &AppContext,
  raw_note_id: &str,
  body: Option<&str>,
  entity_label: &str,
  output: &json::Flags,
) -> Result<(), Error> {
  let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
  let conn = context.store().connect().await?;
  let note_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Notes, raw_note_id).await?;

  let existing = repo::note::find_required_by_id(&conn, note_id.clone()).await?;

  let body = match body {
    Some("-") => {
      crate::io::editor::edit_text_with_suffix(existing.body(), ".md").map_err(|e| Error::Editor(e.to_string()))?
    }
    Some(b) => b.to_owned(),
    None => {
      crate::io::editor::edit_text_with_suffix(existing.body(), ".md").map_err(|e| Error::Editor(e.to_string()))?
    }
  };

  if body.trim().is_empty() {
    return Err(Error::Editor("Aborting: empty note body".into()));
  }

  let before = serde_json::to_value(&existing)?;
  let tx_label = format!("{entity_label} note update");
  let tx = repo::transaction::begin(&conn, project_id, &tx_label).await?;
  let patch = Patch {
    body: Some(body),
  };
  let note = repo::note::update(&conn, &note_id, &patch).await?;
  repo::transaction::record_event(&conn, tx.id(), "notes", &note_id.to_string(), "modified", Some(&before)).await?;

  let short_id = note.id().short();
  output.print_entity(&note, &short_id, || {
    SuccessMessage::new("updated note").id(note.id().short()).to_string()
  })?;
  Ok(())
}
