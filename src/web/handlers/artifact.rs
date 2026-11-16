//! Artifact list/detail/create/edit handlers.

use askama::Template;
use axum::{
  body::Bytes,
  extract::{Form, Path, Query, State},
  response::{Html, Redirect},
};
use libsql::Connection;
use serde::Deserialize;

use crate::{
  store::{
    model::{artifact, note, primitives::EntityType},
    repo,
  },
  web::{
    self, AppState,
    forms::{self, ExistingLink, NoteFormData},
    markdown,
    timeline::{self, TimelineItem},
  },
};

/// Form body for artifact create submissions.
#[derive(Deserialize)]
pub struct ArtifactForm {
  /// The Markdown body; missing means an empty body.
  body: Option<String>,
  /// Comma-separated tag labels to attach.
  tags: Option<String>,
  /// The artifact's title, required by the form and saved as typed.
  title: String,
}

/// Query parameters for the artifact list view (status tab selection).
#[derive(Deserialize)]
pub struct ArtifactListParams {
  /// The tab to show: `all`, `archived`, or anything else (the default, `open`) for the unarchived artifacts.
  status: Option<String>,
}

/// The new-artifact form, with empty fields.
#[derive(Template)]
#[template(path = "artifacts/create.html")]
struct ArtifactCreateTemplate {
  /// The Markdown body shown in the editor.
  body: String,
  /// A message shown above the form, if any; the create form always sets `None` today.
  error: Option<String>,
  /// The comma-separated tags shown in the tags field.
  tags: String,
  /// The title shown in the title field.
  title: String,
}

/// Shared data backing both the full artifact detail page and the SSE fragment.
struct ArtifactDetailData {
  /// The artifact shown.
  artifact: artifact::Model,
  /// The artifact's body rendered from Markdown to HTML.
  body_html: String,
  /// The labels of the artifact's tags.
  tags: Vec<String>,
  /// The artifact's notes and events, in the order the timeline shows them.
  timeline_items: Vec<TimelineItem>,
}

/// The artifact detail content alone, which the live-reload script swaps in.
#[derive(Template)]
#[template(path = "artifacts/detail_content.html")]
struct ArtifactDetailContentTemplate {
  /// The artifact shown.
  artifact: artifact::Model,
  /// The artifact's body rendered from Markdown to HTML.
  body_html: String,
  /// The labels of the artifact's tags.
  tags: Vec<String>,
  /// The artifact's notes and events, in the order the timeline shows them.
  timeline_items: Vec<TimelineItem>,
}

/// The full artifact detail page.
#[derive(Template)]
#[template(path = "artifacts/detail.html")]
struct ArtifactDetailTemplate {
  /// The artifact shown.
  artifact: artifact::Model,
  /// The artifact's body rendered from Markdown to HTML.
  body_html: String,
  /// The labels of the artifact's tags.
  tags: Vec<String>,
  /// The artifact's notes and events, in the order the timeline shows them.
  timeline_items: Vec<TimelineItem>,
}

/// The edit form for an existing artifact, with its fields pre-filled.
#[derive(Template)]
#[template(path = "artifacts/edit.html")]
struct ArtifactEditTemplate {
  /// The artifact being edited, for its id and the form's action.
  artifact: artifact::Model,
  /// The Markdown body shown in the editor.
  body: String,
  /// A message shown above the form, if any.
  error: Option<String>,
  /// The artifact's relationships, shown as editable link rows.
  existing_links: Vec<ExistingLink>,
  /// The comma-separated tags shown in the tags field.
  tags: String,
  /// The title shown in the title field.
  title: String,
}

/// Shared data backing both the full artifact list page and the SSE fragment.
struct ArtifactListData {
  /// How many artifacts are archived, shown in the summary line.
  archived_count: usize,
  /// The rows of the selected tab.
  artifacts: Vec<ArtifactRow>,
  /// The selected tab: `open`, `archived`, `all`, or the raw `status` value given.
  current_status: String,
  /// How many artifacts are not archived, shown in the summary line.
  open_count: usize,
}

/// The artifact list content alone, which the live-reload script swaps in.
#[derive(Template)]
#[template(path = "artifacts/list_content.html")]
struct ArtifactListContentTemplate {
  /// How many artifacts are archived, shown in the summary line.
  archived_count: usize,
  /// The rows of the selected tab.
  artifacts: Vec<ArtifactRow>,
  /// The selected tab: `open`, `archived`, `all`, or the raw `status` value given.
  current_status: String,
  /// How many artifacts are not archived, shown in the summary line.
  open_count: usize,
}

/// The full artifact list page.
#[derive(Template)]
#[template(path = "artifacts/list.html")]
struct ArtifactListTemplate {
  /// How many artifacts are archived, shown in the summary line.
  archived_count: usize,
  /// The rows of the selected tab.
  artifacts: Vec<ArtifactRow>,
  /// The selected tab: `open`, `archived`, `all`, or the raw `status` value given.
  current_status: String,
  /// How many artifacts are not archived, shown in the summary line.
  open_count: usize,
}

/// One row of the artifact list: the artifact with its tags and its number of relationships.
struct ArtifactRow {
  /// The artifact shown in the row.
  artifact: artifact::Model,
  /// How many relationships the artifact has.
  link_count: usize,
  /// The labels of the artifact's tags.
  tags: Vec<String>,
}

/// Archive an artifact.
///
/// Handles `POST /artifacts/{id}/archive` and tells live-reload listeners.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store and the live-reload channel
/// * `id` - the artifact id or a unique prefix of it, from the path
///
/// # Returns
///
/// A redirect to `/artifacts`, or a [`web::Error`] when the id does not resolve or the store fails.
pub async fn artifact_archive(State(state): State<AppState>, Path(id): Path<String>) -> Result<Redirect, web::Error> {
  log::debug!("artifact_archive: artifact={id}");
  let conn = state.store().connect().await?;
  let artifact_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Artifacts, &id).await?;
  let before = repo::artifact::find_required_by_id(&conn, artifact_id.clone()).await?;
  let before = serde_json::to_value(&before).map_err(crate::store::Error::from)?;
  let tx = forms::begin(&state, &conn, "artifact archive").await?;
  repo::artifact::archive(&conn, &artifact_id).await?;
  repo::transaction::record_semantic_event(
    &conn,
    tx.id(),
    "artifacts",
    &artifact_id.to_string(),
    "modified",
    Some(&before),
    Some("archived"),
    None,
    None,
  )
  .await?;

  let _ = state.reload_tx().send(());
  Ok(Redirect::to("/artifacts"))
}

/// Artifact create form.
///
/// Handles `GET /artifacts/new` with empty fields.
///
/// # Returns
///
/// The rendered form, or a [`web::Error`] when the template fails.
pub async fn artifact_create_form() -> Result<Html<String>, web::Error> {
  let tmpl = ArtifactCreateTemplate {
    title: String::new(),
    body: String::new(),
    tags: String::new(),
    error: None,
  };
  Ok(Html(tmpl.render()?))
}

/// Handle artifact creation from form.
///
/// Handles `POST /artifacts`: creates the artifact, attaches its tags, and tells live-reload listeners.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store and the live-reload channel
/// * `form` - the submitted title, body and tags
///
/// # Returns
///
/// A redirect to the new artifact's page, or a [`web::Error`] when the store fails.
pub async fn artifact_create_submit(
  State(state): State<AppState>,
  Form(form): Form<ArtifactForm>,
) -> Result<Redirect, web::Error> {
  log::debug!("artifact_create_submit: title={}", form.title);
  let conn = state.store().connect().await?;

  let new = artifact::New {
    title: form.title,
    body: form.body.unwrap_or_default(),
    ..Default::default()
  };
  let tx = forms::begin(&state, &conn, "artifact create").await?;
  let artifact = repo::artifact::create(&conn, state.project_id(), &new).await?;
  repo::transaction::record_semantic_event(
    &conn,
    tx.id(),
    "artifacts",
    &artifact.id().to_string(),
    "created",
    None,
    Some("created"),
    None,
    None,
  )
  .await?;

  // Attach tags
  if let Some(tags_str) = &form.tags {
    for label in forms::parse_tags(tags_str) {
      repo::tag::attach_recorded(&conn, tx.id(), EntityType::Artifact, artifact.id(), &label).await?;
    }
  }

  let _ = state.reload_tx().send(());
  Ok(Redirect::to(&format!("/artifacts/{}", artifact.id())))
}

/// Artifact detail page.
///
/// Handles `GET /artifacts/{id}`.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store
/// * `id` - the artifact id or a unique prefix of it, from the path
///
/// # Returns
///
/// The rendered page, or a [`web::Error`]: not found for an unknown id, or a store or template failure.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
pub async fn artifact_detail(
  State(state): State<AppState>,
  Path(id): Path<String>,
) -> Result<Html<String>, web::Error> {
  let data = load_artifact_detail(&state, &id).await?;
  let tmpl = ArtifactDetailTemplate {
    artifact: data.artifact,
    body_html: data.body_html,
    tags: data.tags,
    timeline_items: data.timeline_items,
  };
  Ok(Html(tmpl.render()?))
}

/// Artifact detail fragment (SSE live reload).
///
/// Handles `GET /artifacts/{id}/_detail`.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store
/// * `id` - the artifact id or a unique prefix of it, from the path
///
/// # Returns
///
/// The rendered content without the page layout, or a [`web::Error`]: not found for an unknown id, or a store or
/// template failure.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
pub async fn artifact_detail_fragment(
  State(state): State<AppState>,
  Path(id): Path<String>,
) -> Result<Html<String>, web::Error> {
  let data = load_artifact_detail(&state, &id).await?;
  let tmpl = ArtifactDetailContentTemplate {
    artifact: data.artifact,
    body_html: data.body_html,
    tags: data.tags,
    timeline_items: data.timeline_items,
  };
  Ok(Html(tmpl.render()?))
}

/// Artifact edit form.
///
/// Handles `GET /artifacts/{id}/edit`, with the title, body, tags and links pre-filled.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store
/// * `id` - the artifact id or a unique prefix of it, from the path
///
/// # Returns
///
/// The rendered form, or a [`web::Error`]: not found for an unknown id, or a store or template failure.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
pub async fn artifact_edit_form(
  State(state): State<AppState>,
  Path(id): Path<String>,
) -> Result<Html<String>, web::Error> {
  let conn = state.store().connect().await?;
  let artifact_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Artifacts, &id).await?;
  let artifact = repo::artifact::find_by_id(&conn, artifact_id.clone())
    .await?
    .ok_or(web::Error::NotFound)?;

  let tags = repo::tag::for_entity(&conn, EntityType::Artifact, &artifact_id).await?;

  let rels = repo::relationship::for_entity(&conn, EntityType::Artifact, &artifact_id).await?;
  let existing_links = forms::build_existing_links_for_entity(&artifact_id, EntityType::Artifact, &rels);

  let tmpl = ArtifactEditTemplate {
    title: artifact.title().to_owned(),
    body: artifact.body().to_owned(),
    tags: tags.join(", "),
    artifact,
    error: None,
    existing_links,
  };
  Ok(Html(tmpl.render()?))
}

/// Artifact list page.
///
/// Handles `GET /artifacts`.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store
/// * `params` - the query string, holding the selected status tab
///
/// # Returns
///
/// The rendered page, or a [`web::Error`] when the store or the template fails.
pub async fn artifact_list(
  State(state): State<AppState>,
  Query(params): Query<ArtifactListParams>,
) -> Result<Html<String>, web::Error> {
  let data = load_artifact_list(&state, params.status).await?;
  let tmpl = ArtifactListTemplate {
    artifacts: data.artifacts,
    open_count: data.open_count,
    archived_count: data.archived_count,
    current_status: data.current_status,
  };
  Ok(Html(tmpl.render()?))
}

/// Artifact list fragment (SSE live reload).
///
/// Handles `GET /artifacts/_list`.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store
/// * `params` - the query string, holding the selected status tab
///
/// # Returns
///
/// The rendered content without the page layout, or a [`web::Error`] when the store or the template fails.
pub async fn artifact_list_fragment(
  State(state): State<AppState>,
  Query(params): Query<ArtifactListParams>,
) -> Result<Html<String>, web::Error> {
  let data = load_artifact_list(&state, params.status).await?;
  let tmpl = ArtifactListContentTemplate {
    artifacts: data.artifacts,
    open_count: data.open_count,
    archived_count: data.archived_count,
    current_status: data.current_status,
  };
  Ok(Html(tmpl.render()?))
}

/// Add a note to an artifact.
///
/// Handles `POST /artifacts/{id}/notes`; the note's author is the server's author, and live-reload listeners are
/// told.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store, the author id and the live-reload channel
/// * `id` - the artifact id or a unique prefix of it, from the path
/// * `form` - the submitted note body
///
/// # Returns
///
/// A redirect to the artifact's page, or a [`web::Error`] when the id does not resolve or the store fails.
pub async fn artifact_note_add(
  State(state): State<AppState>,
  Path(id): Path<String>,
  Form(form): Form<NoteFormData>,
) -> Result<Redirect, web::Error> {
  log::debug!("artifact_note_add: artifact={id}");
  let conn = state.store().connect().await?;
  let artifact_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Artifacts, &id).await?;

  let new = note::New {
    body: form.body,
    author_id: state.author_id().clone(),
  };
  let tx = forms::begin(&state, &conn, "artifact note add").await?;
  let note = repo::note::create(&conn, EntityType::Artifact, &artifact_id, &new).await?;
  repo::transaction::record_event(&conn, tx.id(), "notes", &note.id().to_string(), "created", None).await?;

  let _ = state.reload_tx().send(());
  Ok(Redirect::to(&format!("/artifacts/{artifact_id}")))
}

/// Handle artifact update from form.
///
/// Handles `POST /artifacts/{id}`. It reads the raw form body because the link rows repeat their field names. It
/// replaces the title, body and tags, syncs the relationships, and tells live-reload listeners.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store and the live-reload channel
/// * `id` - the artifact id or a unique prefix of it, from the path
/// * `body` - the URL-encoded form body, with `title`, `body`, `tags` and the link fields
///
/// # Returns
///
/// A redirect to the artifact's page, or a [`web::Error`] when the id does not resolve, the store fails, or a
/// link cannot be synced.
pub async fn artifact_update(
  State(state): State<AppState>,
  Path(id): Path<String>,
  body: Bytes,
) -> Result<Redirect, web::Error> {
  log::debug!("artifact_update: artifact={id}");
  let conn = state.store().connect().await?;
  let artifact_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Artifacts, &id).await?;

  // Parse form fields from raw body
  let mut title = String::new();
  let mut body_field = String::new();
  let mut tags_str = String::new();
  let (link_rels, link_refs) = forms::extract_link_fields(&body);
  for (key, value) in form_urlencoded::parse(&body) {
    match key.as_ref() {
      "title" => title = value.into_owned(),
      "body" => body_field = value.into_owned(),
      "tags" => tags_str = value.into_owned(),
      _ => {}
    }
  }

  let patch = artifact::Patch {
    title: Some(title),
    body: Some(body_field),
    ..Default::default()
  };
  // The model skips `body` when it serializes, so add it here: undo restores the body from before_data.
  let before_artifact = repo::artifact::find_required_by_id(&conn, artifact_id.clone()).await?;
  let mut before = serde_json::to_value(&before_artifact).map_err(crate::store::Error::from)?;
  before["body"] = serde_json::Value::from(before_artifact.body().as_str());
  let tx = forms::begin(&state, &conn, "artifact update").await?;
  // Save the fields only when the form changed one, so a save that changes nothing is not an undo step.
  if patch.title.as_deref() != Some(before_artifact.title())
    || patch.body.as_deref() != Some(before_artifact.body().as_str())
  {
    repo::artifact::update(&conn, &artifact_id, &patch).await?;
    repo::transaction::record_event(
      &conn,
      tx.id(),
      "artifacts",
      &artifact_id.to_string(),
      "modified",
      Some(&before),
    )
    .await?;
  }

  forms::sync_form_tags(
    &conn,
    tx.id(),
    EntityType::Artifact,
    &artifact_id,
    &forms::parse_tags(&tags_str),
  )
  .await
  .map_err(web::Error::Internal)?;

  // Sync relationships
  forms::sync_form_links(
    &conn,
    tx.id(),
    EntityType::Artifact,
    &artifact_id,
    &link_rels,
    &link_refs,
  )
  .await
  .map_err(web::Error::Internal)?;

  let _ = state.reload_tx().send(());
  Ok(Redirect::to(&format!("/artifacts/{artifact_id}")))
}

/// Build the enriched artifact rows for a page of artifacts, batching tag and
/// relationship lookups into single queries to avoid N+1 fan-out across the
/// row set.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `artifacts` - the artifacts to show, in display order
///
/// # Returns
///
/// One row per artifact, in the same order, or a [`web::Error`] when a lookup fails.
async fn build_artifact_rows(
  conn: &Connection,
  artifacts: Vec<artifact::Model>,
) -> Result<Vec<ArtifactRow>, web::Error> {
  if artifacts.is_empty() {
    return Ok(Vec::new());
  }

  let ids: Vec<_> = artifacts.iter().map(|a| a.id().clone()).collect();
  let tags_by_id = repo::tag::for_entities(conn, EntityType::Artifact, &ids).await?;
  let rels_by_id = repo::relationship::for_entities(conn, EntityType::Artifact, &ids).await?;

  let mut rows = Vec::with_capacity(artifacts.len());
  for a in artifacts {
    let tags = tags_by_id
      .get(a.id())
      .map(|ts| ts.iter().map(|t| t.label().to_owned()).collect::<Vec<_>>())
      .unwrap_or_default();
    let link_count = rels_by_id.get(a.id()).map_or(0, Vec::len);
    rows.push(ArtifactRow {
      artifact: a,
      link_count,
      tags,
    });
  }

  Ok(rows)
}

/// Load the shared artifact detail payload used by both the full page and the
/// fragment handler.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store
/// * `id` - the artifact id or a unique prefix of it
///
/// # Returns
///
/// The artifact with its rendered body, tags and timeline, or a [`web::Error`]: not found for an unknown id, or
/// a store or timeline failure.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
async fn load_artifact_detail(state: &AppState, id: &str) -> Result<ArtifactDetailData, web::Error> {
  let conn = state.store().connect().await?;
  let artifact_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Artifacts, id).await?;
  let artifact = repo::artifact::find_by_id(&conn, artifact_id.clone())
    .await?
    .ok_or(web::Error::NotFound)?;

  let tags = repo::tag::for_entity(&conn, EntityType::Artifact, &artifact_id).await?;

  let body_html = markdown::render_markdown_to_html(artifact.body());
  let timeline_items = timeline::build_timeline(&conn, EntityType::Artifact, &artifact_id)
    .await
    .map_err(web::Error::Internal)?;

  Ok(ArtifactDetailData {
    artifact,
    body_html,
    tags,
    timeline_items,
  })
}

/// Load the shared artifact list payload (rows, counts, current status filter)
/// used by both the full page and the fragment handler.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store and the project id
/// * `status` - the selected tab: `all`, `archived`, or anything else for the unarchived artifacts; `open` when
///   missing
///
/// # Returns
///
/// The rows of the selected tab with the open and archived counts, or a [`web::Error`] when the store fails.
async fn load_artifact_list(state: &AppState, status: Option<String>) -> Result<ArtifactListData, web::Error> {
  let conn = state.store().connect().await?;

  // Fetch all artifacts to compute counts
  let all_artifacts = repo::artifact::all(
    &conn,
    state.project_id(),
    &artifact::Filter {
      all: true,
      ..Default::default()
    },
  )
  .await?;

  let open_count = all_artifacts.iter().filter(|a| !a.is_archived()).count();
  let archived_count = all_artifacts.iter().filter(|a| a.is_archived()).count();

  let current_status = status.unwrap_or_else(|| "open".to_owned());

  // Build filter based on status param
  let filter = match current_status.as_str() {
    "all" => artifact::Filter {
      all: true,
      ..Default::default()
    },
    "archived" => artifact::Filter {
      only_archived: true,
      ..Default::default()
    },
    _ => artifact::Filter::default(), // default shows open only
  };

  let artifacts = repo::artifact::all(&conn, state.project_id(), &filter).await?;
  let rows = build_artifact_rows(&conn, artifacts).await?;

  Ok(ArtifactListData {
    archived_count,
    artifacts: rows,
    current_status,
    open_count,
  })
}

#[cfg(test)]
mod tests {
  use crate::{
    store::{
      self,
      model::{
        Project, artifact, note,
        primitives::{EntityType, Id},
      },
      repo,
    },
    web::timeline,
  };

  async fn setup_artifact_with_note_and_event() -> (std::sync::Arc<store::Db>, Id) {
    let (store_arc, tmp) = store::open_temp().await.unwrap();
    let conn = store_arc.connect().await.unwrap();
    let project = Project::new("/tmp/web-artifact-timeline".into());
    conn
      .execute(
        "INSERT INTO projects (id, root, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        [
          project.id().to_string(),
          project.root().to_string_lossy().into_owned(),
          project.created_at().to_rfc3339(),
          project.updated_at().to_rfc3339(),
        ],
      )
      .await
      .unwrap();
    let project_id = project.id().clone();

    let art = repo::artifact::create(
      &conn,
      &project_id,
      &artifact::New {
        title: "Spec".into(),
        ..Default::default()
      },
    )
    .await
    .unwrap();

    let tx = repo::transaction::begin(&conn, &project_id, "artifact create")
      .await
      .unwrap();
    repo::transaction::record_semantic_event(
      &conn,
      tx.id(),
      "artifacts",
      &art.id().to_string(),
      "created",
      None,
      Some("created"),
      None,
      None,
    )
    .await
    .unwrap();

    repo::note::create(
      &conn,
      EntityType::Artifact,
      art.id(),
      &note::New {
        body: "note body".into(),
        author_id: None,
      },
    )
    .await
    .unwrap();

    let art_id = art.id().clone();
    std::mem::forget(tmp);
    (store_arc, art_id)
  }

  mod artifact_detail_timeline {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_merges_notes_and_semantic_events_in_chronological_order() {
      let (store_arc, art_id) = setup_artifact_with_note_and_event().await;
      let conn = store_arc.connect().await.unwrap();

      let items = timeline::build_timeline(&conn, EntityType::Artifact, &art_id)
        .await
        .unwrap();

      assert_eq!(items.len(), 2);
      assert!(items[0].as_event().is_some());
      assert!(items[1].as_note().is_some());
    }
  }

  mod artifact_not_found_response {
    use axum::{
      extract::{Path, State},
      http::StatusCode,
      response::IntoResponse,
    };
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::web::{self, AppState};

    async fn empty_state() -> AppState {
      let (store_arc, tmp) = store::open_temp().await.unwrap();
      let conn = store_arc.connect().await.unwrap();
      let project = Project::new("/tmp/web-artifact-not-found".into());
      conn
        .execute(
          "INSERT INTO projects (id, root, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
          [
            project.id().to_string(),
            project.root().to_string_lossy().into_owned(),
            project.created_at().to_rfc3339(),
            project.updated_at().to_rfc3339(),
          ],
        )
        .await
        .unwrap();
      std::mem::forget(tmp);
      AppState::new(store_arc, project.id().clone())
    }

    #[tokio::test]
    async fn it_renders_404_when_the_artifact_prefix_does_not_resolve() {
      let state = empty_state().await;

      let err = super::super::artifact_detail(State(state), Path("llllllll".into()))
        .await
        .expect_err("missing artifact should surface as NotFound");

      assert!(matches!(err, web::Error::NotFound), "got {err:?}");
      assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);
    }
  }
}

#[cfg(test)]
mod web_undo_tests {
  use axum::{
    Form,
    body::Bytes,
    extract::{Path, State},
  };
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::store::{self, model::Project};

  /// A server state over a fresh store with one project, and a connection to it.
  async fn state() -> (AppState, libsql::Connection) {
    let (store_arc, tmp) = store::open_temp().await.unwrap();
    let conn = store_arc.connect().await.unwrap();
    let project = Project::new("/tmp/web-artifact-undo".into());
    conn
      .execute(
        "INSERT INTO projects (id, root, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        [
          project.id().to_string(),
          project.root().to_string_lossy().into_owned(),
          project.created_at().to_rfc3339(),
          project.updated_at().to_rfc3339(),
        ],
      )
      .await
      .unwrap();
    std::mem::forget(tmp);
    (AppState::new(store_arc, project.id().clone()), conn)
  }

  /// Undo the newest undoable transaction, as `gest undo` does.
  async fn undo_last(state: &AppState, conn: &libsql::Connection) {
    let tx = repo::transaction::latest_undoable(conn, state.project_id())
      .await
      .unwrap()
      .expect("the web write recorded an undo step");
    repo::transaction::undo(conn, tx.id()).await.unwrap();
  }

  async fn seed(conn: &libsql::Connection, state: &AppState) -> artifact::Model {
    let art = repo::artifact::create(
      conn,
      state.project_id(),
      &artifact::New {
        title: "Old".into(),
        body: "old body".into(),
        ..Default::default()
      },
    )
    .await
    .unwrap();
    repo::tag::attach(conn, EntityType::Artifact, art.id(), "old-tag")
      .await
      .unwrap();
    art
  }

  #[tokio::test]
  async fn it_undoes_a_web_artifact_create() {
    let (state, conn) = state().await;
    let form = ArtifactForm {
      body: Some("b".into()),
      tags: Some("t".into()),
      title: "Made on the web".into(),
    };
    let _ = artifact_create_submit(State(state.clone()), Form(form)).await.unwrap();

    undo_last(&state, &conn).await;

    let all = repo::artifact::all(
      &conn,
      state.project_id(),
      &artifact::Filter {
        all: true,
        ..Default::default()
      },
    )
    .await
    .unwrap();
    assert!(all.is_empty(), "{all:?}");
  }

  #[tokio::test]
  async fn it_undoes_a_web_artifact_update_with_its_body_tags_and_links() {
    let (state, conn) = state().await;
    let art = seed(&conn, &state).await;
    let other = seed(&conn, &state).await;
    let body = format!(
      "title=New&body=new+body&tags=new-tag&link_rel%5B%5D=relates-to&link_ref%5B%5D=artifacts%2F{}",
      other.id()
    );
    let _ = artifact_update(State(state.clone()), Path(art.id().to_string()), Bytes::from(body))
      .await
      .unwrap();
    assert_eq!(
      repo::artifact::find_required_by_id(&conn, art.id().clone())
        .await
        .unwrap()
        .title(),
      "New"
    );

    undo_last(&state, &conn).await;

    let restored = repo::artifact::find_required_by_id(&conn, art.id().clone())
      .await
      .unwrap();
    assert_eq!(restored.title(), "Old");
    assert_eq!(restored.body(), "old body");
    assert_eq!(
      repo::tag::for_entity(&conn, EntityType::Artifact, art.id())
        .await
        .unwrap(),
      vec!["old-tag".to_string()]
    );
    assert!(
      repo::relationship::for_entity(&conn, EntityType::Artifact, art.id())
        .await
        .unwrap()
        .is_empty()
    );
  }

  #[tokio::test]
  async fn it_records_no_undo_step_for_an_artifact_save_that_changes_nothing() {
    let (state, conn) = state().await;
    let art = seed(&conn, &state).await;
    let _ = artifact_update(
      State(state.clone()),
      Path(art.id().to_string()),
      Bytes::from("title=Old&body=old+body&tags=old-tag"),
    )
    .await
    .unwrap();

    assert!(
      repo::transaction::latest_undoable(&conn, state.project_id())
        .await
        .unwrap()
        .is_none()
    );
  }

  #[tokio::test]
  async fn it_undoes_a_web_artifact_archive() {
    let (state, conn) = state().await;
    let art = seed(&conn, &state).await;
    let _ = artifact_archive(State(state.clone()), Path(art.id().to_string()))
      .await
      .unwrap();

    undo_last(&state, &conn).await;

    let restored = repo::artifact::find_required_by_id(&conn, art.id().clone())
      .await
      .unwrap();
    assert!(!restored.is_archived());
  }

  #[tokio::test]
  async fn it_undoes_a_web_artifact_note() {
    let (state, conn) = state().await;
    let art = seed(&conn, &state).await;
    let _ = artifact_note_add(
      State(state.clone()),
      Path(art.id().to_string()),
      Form(forms::NoteFormData {
        body: "hi".into(),
      }),
    )
    .await
    .unwrap();

    undo_last(&state, &conn).await;

    assert!(
      repo::note::for_entity(&conn, EntityType::Artifact, art.id())
        .await
        .unwrap()
        .is_empty()
    );
  }

  #[tokio::test]
  async fn it_records_no_link_change_when_the_form_keeps_the_links() {
    let (state, conn) = state().await;
    let art = seed(&conn, &state).await;
    let other = seed(&conn, &state).await;
    let body = format!(
      "title=Old&body=old+body&tags=old-tag&link_rel%5B%5D=relates-to&link_ref%5B%5D=artifacts%2F{}",
      other.id()
    );
    for _ in 0..2 {
      let _ = artifact_update(
        State(state.clone()),
        Path(art.id().to_string()),
        Bytes::from(body.clone()),
      )
      .await
      .unwrap();
    }

    let links = repo::relationship::for_entity(&conn, EntityType::Artifact, art.id())
      .await
      .unwrap();
    assert_eq!(links.len(), 1, "a second save with the same link kept one row");
    let mut rows = conn
      .query(
        "SELECT count(*) FROM transaction_events WHERE table_name = 'relationships'",
        (),
      )
      .await
      .unwrap();
    let count: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(count, 1, "only the first save created a link");
  }
}
