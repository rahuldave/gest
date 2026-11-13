//! Task list/detail/create/edit/notes handlers.

use askama::Template;
use axum::{
  body::Bytes,
  extract::{Form, Path, Query, State},
  response::{Html, IntoResponse, Redirect, Response},
};
use libsql::Connection;
use serde::Deserialize;

use crate::{
  store::{
    model::{
      note,
      primitives::{EntityType, Id, Priority, RelationshipType, TaskStatus},
      relationship, task,
    },
    repo,
  },
  web::{
    self, AppState,
    forms::{self, ExistingLink, NoteFormData},
    markdown,
    timeline::{self, TimelineItem},
  },
};

/// Query parameters for the global task board view (per-column row caps).
#[derive(Deserialize)]
pub struct TaskBoardParams {
  /// The most rows the Cancelled column shows: `10`, `20` or `50`. A missing or other value means 10.
  cancelled: Option<String>,
  /// The most rows the Done column shows: `10`, `20` or `50`. A missing or other value means 10.
  done: Option<String>,
}

/// Query parameters for the task list view (status tab selection).
#[derive(Deserialize)]
pub struct TaskListParams {
  /// The status tab to show: `all`, or a task status such as `open`.
  ///
  /// A missing value means `all`. A value that is not a status also shows every task.
  status: Option<String>,
}

/// Shared data backing both the full task board page and the SSE fragment.
struct TaskBoardData {
  /// The most rows the Cancelled column shows: 10, 20 or 50.
  cancelled_limit: u32,
  /// The project's cancelled tasks, cut to `cancelled_limit` rows.
  cancelled_tasks: Vec<task::Model>,
  /// The most rows the Done column shows: 10, 20 or 50.
  done_limit: u32,
  /// The project's done tasks, cut to `done_limit` rows.
  done_tasks: Vec<task::Model>,
  /// The project's tasks that are in progress.
  in_progress_tasks: Vec<task::Model>,
  /// The project's open tasks.
  open_tasks: Vec<task::Model>,
}

/// Shared data backing both the full task detail page and the SSE fragment.
struct TaskDetailData {
  /// Whether the task is the source of a `blocks` link to another entity.
  blocking: bool,
  /// The task's Markdown description rendered to HTML, or empty when it has none.
  description_html: String,
  /// The task's relationships, each labelled from this task's side.
  display_links: Vec<DisplayLink>,
  /// Whether the task is the source of a `blocked-by` link to another entity.
  is_blocked: bool,
  /// The labels of the task's tags.
  tags: Vec<String>,
  /// The task that the page shows.
  task: task::Model,
  /// The task's timeline, from `timeline::build_timeline`.
  timeline_items: Vec<TimelineItem>,
}

/// Shared data backing both the full task list page and the SSE fragment.
struct TaskListData {
  /// How many of the project's tasks are cancelled, whatever the status tab.
  cancelled_count: usize,
  /// The selected status tab: the `status` query value, or `all` when there is none.
  current_status: String,
  /// How many of the project's tasks are done, whatever the status tab.
  done_count: usize,
  /// How many of the project's tasks are in progress, whatever the status tab.
  in_progress_count: usize,
  /// How many of the project's tasks are open, whatever the status tab.
  open_count: usize,
  /// The tasks that the selected status tab shows, with their tags and blocking state.
  rows: Vec<TaskRow>,
}

/// A display-friendly representation of a relationship link (task detail view).
struct DisplayLink {
  /// The short id of the entity at the other end of the link.
  display_text: String,
  /// The link to the other entity's page, for a task or an artifact; `None` for any other kind.
  href: Option<String>,
  /// The relationship type from this task's side: the inverse type when the task is the link's target.
  rel: String,
}

/// The board's columns alone, which the SSE live reload swaps into the board page.
#[derive(Template)]
#[template(path = "tasks/board_content.html")]
struct TaskBoardFragmentTemplate {
  /// The most rows the Cancelled column shows: 10, 20 or 50.
  cancelled_limit: u32,
  /// The project's cancelled tasks, cut to `cancelled_limit` rows.
  cancelled_tasks: Vec<task::Model>,
  /// The most rows the Done column shows: 10, 20 or 50.
  done_limit: u32,
  /// The project's done tasks, cut to `done_limit` rows.
  done_tasks: Vec<task::Model>,
  /// The project's tasks that are in progress.
  in_progress_tasks: Vec<task::Model>,
  /// The project's open tasks.
  open_tasks: Vec<task::Model>,
}

/// The full board page of the project's tasks, in one column for each status.
#[derive(Template)]
#[template(path = "tasks/board.html")]
struct TaskBoardTemplate {
  /// The most rows the Cancelled column shows: 10, 20 or 50.
  cancelled_limit: u32,
  /// The project's cancelled tasks, cut to `cancelled_limit` rows.
  cancelled_tasks: Vec<task::Model>,
  /// The most rows the Done column shows: 10, 20 or 50.
  done_limit: u32,
  /// The project's done tasks, cut to `done_limit` rows.
  done_tasks: Vec<task::Model>,
  /// The project's tasks that are in progress.
  in_progress_tasks: Vec<task::Model>,
  /// The project's open tasks.
  open_tasks: Vec<task::Model>,
}

/// The page with the form that creates a task.
#[derive(Template)]
#[template(path = "tasks/create.html")]
struct TaskCreateTemplate {
  /// The description to fill the form with: empty at first, or what the user sent.
  description: String,
  /// The message shown above the form, or `None` when there is no error.
  error: Option<String>,
  /// The choices of the priority dropdown, with the current one selected.
  priority_options: Vec<PriorityOption>,
  /// The title to fill the form with: empty at first, or what the user sent.
  title: String,
}

/// The detail content alone, which the SSE live reload swaps into the detail page.
#[derive(Template)]
#[template(path = "tasks/detail_content.html")]
struct TaskDetailFragmentTemplate {
  /// Whether the task is the source of a `blocks` link to another entity.
  blocking: bool,
  /// The task's Markdown description rendered to HTML, or empty when it has none.
  description_html: String,
  /// The task's relationships, each labelled from this task's side.
  display_links: Vec<DisplayLink>,
  /// Whether the task is the source of a `blocked-by` link to another entity.
  is_blocked: bool,
  /// The labels of the task's tags.
  tags: Vec<String>,
  /// The task that the page shows.
  task: task::Model,
  /// The task's timeline, from `timeline::build_timeline`.
  timeline_items: Vec<TimelineItem>,
}

/// The full detail page of one task: description, tags, links and timeline.
#[derive(Template)]
#[template(path = "tasks/detail.html")]
struct TaskDetailTemplate {
  /// Whether the task is the source of a `blocks` link to another entity.
  blocking: bool,
  /// The task's Markdown description rendered to HTML, or empty when it has none.
  description_html: String,
  /// The task's relationships, each labelled from this task's side.
  display_links: Vec<DisplayLink>,
  /// Whether the task is the source of a `blocked-by` link to another entity.
  is_blocked: bool,
  /// The labels of the task's tags.
  tags: Vec<String>,
  /// The task that the page shows.
  task: task::Model,
  /// The task's timeline, from `timeline::build_timeline`.
  timeline_items: Vec<TimelineItem>,
}

/// The page with the form that edits a task.
#[derive(Template)]
#[template(path = "tasks/edit.html")]
struct TaskEditTemplate {
  /// The task's description, to fill the form with.
  description: String,
  /// The message shown above the form; the edit handler always sets `None`.
  error: Option<String>,
  /// The task's relationships, as rows that the form can keep or remove.
  existing_links: Vec<ExistingLink>,
  /// The choices of the priority dropdown, with the task's priority selected.
  priority_options: Vec<PriorityOption>,
  /// The labels of the task's tags, joined with `, `.
  tags: String,
  /// The task being edited.
  task: task::Model,
  /// The task's title, to fill the form with.
  title: String,
}

/// The list content alone, which the SSE live reload swaps into the list page.
#[derive(Template)]
#[template(path = "tasks/list_content.html")]
struct TaskListFragmentTemplate {
  /// How many of the project's tasks are cancelled, whatever the status tab.
  cancelled_count: usize,
  /// The selected status tab: the `status` query value, or `all` when there is none.
  current_status: String,
  /// How many of the project's tasks are done, whatever the status tab.
  done_count: usize,
  /// How many of the project's tasks are in progress, whatever the status tab.
  in_progress_count: usize,
  /// How many of the project's tasks are open, whatever the status tab.
  open_count: usize,
  /// The tasks that the selected status tab shows, with their tags and blocking state.
  rows: Vec<TaskRow>,
}

/// The full task list page, with a tab for each status.
#[derive(Template)]
#[template(path = "tasks/list.html")]
struct TaskListTemplate {
  /// How many of the project's tasks are cancelled, whatever the status tab.
  cancelled_count: usize,
  /// The selected status tab: the `status` query value, or `all` when there is none.
  current_status: String,
  /// How many of the project's tasks are done, whatever the status tab.
  done_count: usize,
  /// How many of the project's tasks are in progress, whatever the status tab.
  in_progress_count: usize,
  /// How many of the project's tasks are open, whatever the status tab.
  open_count: usize,
  /// The tasks that the selected status tab shows, with their tags and blocking state.
  rows: Vec<TaskRow>,
}

/// Dropdown option for the task priority `<select>`.
struct PriorityOption {
  /// The text the option shows, such as a priority's name or `— none —`.
  label: String,
  /// Whether the option is the one selected when the form opens.
  selected: bool,
  /// The value the form sends: the priority's number, or empty for no priority.
  value: String,
}

/// Enriched row for the task list view.
struct TaskRow {
  /// `blocked by` and the short ids of the entities blocking the task, or empty when none do.
  blocked_by_display: String,
  /// Whether the task is the source of a `blocks` link to another entity.
  blocking: bool,
  /// Whether the task is the source of a `blocked-by` link to another entity.
  is_blocked: bool,
  /// The labels of the task's tags.
  tags: Vec<String>,
  /// The task that the row shows.
  task: task::Model,
}

/// Parsed scalar fields from the task update form body (link pairs are
/// extracted separately alongside the same single parsing pass).
#[derive(Default)]
struct TaskUpdateFields {
  /// The `description` value, which replaces the task's description.
  description: String,
  /// The `link_rel[]` values in order; each pairs with the `link_refs` entry at its index.
  link_rels: Vec<String>,
  /// The `link_ref[]` values in order; each pairs with the `link_rels` entry at its index.
  link_refs: Vec<String>,
  /// The `priority` value: a number, a priority name, or empty for no priority.
  priority: String,
  /// The `status` value, or empty to keep the task's status.
  status: String,
  /// The `tags` value: the tag labels as the user typed them, for `forms::parse_tags`.
  tags: String,
  /// The `title` value, which replaces the task's title.
  title: String,
}

/// Add a note to a task.
///
/// Serves `POST /tasks/{id}/notes`, then signals the live reload. The note's author is the server's author.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection and the reload signal
/// * `id` - the task's id or a prefix that names one task, from the path
/// * `form` - the form, with the note's body
///
/// # Returns
///
/// A redirect to the task's detail page. Fails with `web::Error::NotFound` when no task has the id, and with another
/// `web::Error` when the store fails.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
pub async fn note_add(
  State(state): State<AppState>,
  Path(id): Path<String>,
  Form(form): Form<NoteFormData>,
) -> Result<Redirect, web::Error> {
  log::debug!("note_add: task={id}");
  let conn = state.store().connect().await?;
  let task_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Tasks, &id).await?;

  let new = note::New {
    body: form.body,
    author_id: state.author_id().clone(),
  };
  repo::note::create(&conn, EntityType::Task, &task_id, &new).await?;

  let _ = state.reload_tx().send(());
  Ok(Redirect::to(&format!("/tasks/{task_id}")))
}

/// Global task board page.
///
/// Serves `GET /tasks/board`, with all the project's tasks.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection and the project id
/// * `params` - the query, with the row caps of the Done and Cancelled columns
///
/// # Returns
///
/// The page as HTML. Fails with a `web::Error` when the store or the template fails.
pub async fn task_board(
  State(state): State<AppState>,
  Query(params): Query<TaskBoardParams>,
) -> Result<Html<String>, web::Error> {
  let data = load_task_board(&state, params.done.as_deref(), params.cancelled.as_deref()).await?;
  let tmpl = TaskBoardTemplate {
    cancelled_limit: data.cancelled_limit,
    cancelled_tasks: data.cancelled_tasks,
    done_limit: data.done_limit,
    done_tasks: data.done_tasks,
    in_progress_tasks: data.in_progress_tasks,
    open_tasks: data.open_tasks,
  };
  Ok(Html(tmpl.render()?))
}

/// Global task board fragment (for SSE live reload).
///
/// Serves `GET /tasks/_board`.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection and the project id
/// * `params` - the query, with the row caps of the Done and Cancelled columns
///
/// # Returns
///
/// The fragment as HTML. Fails with a `web::Error` when the store or the template fails.
pub async fn task_board_fragment(
  State(state): State<AppState>,
  Query(params): Query<TaskBoardParams>,
) -> Result<Html<String>, web::Error> {
  let data = load_task_board(&state, params.done.as_deref(), params.cancelled.as_deref()).await?;
  let tmpl = TaskBoardFragmentTemplate {
    cancelled_limit: data.cancelled_limit,
    cancelled_tasks: data.cancelled_tasks,
    done_limit: data.done_limit,
    done_tasks: data.done_tasks,
    in_progress_tasks: data.in_progress_tasks,
    open_tasks: data.open_tasks,
  };
  Ok(Html(tmpl.render()?))
}

/// Task create form.
///
/// Serves `GET /tasks/new`, with the form empty.
///
/// # Returns
///
/// The page as HTML. Fails with a `web::Error` when the template fails.
pub async fn task_create_form() -> Result<Html<String>, web::Error> {
  let tmpl = TaskCreateTemplate {
    description: String::new(),
    error: None,
    priority_options: build_priority_options(None),
    title: String::new(),
  };
  Ok(Html(tmpl.render()?))
}

/// Handle task creation from form.
///
/// Serves `POST /tasks`, then signals the live reload. The body is read as a URL-encoded form with `title`,
/// `description` and `priority`. An empty priority means none.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection, the project id and the reload signal
/// * `body` - the URL-encoded form body
///
/// # Returns
///
/// A redirect to the new task's detail page. When the priority is not a known number or name, the form again with the
/// user's input and an error, and no task is made. Fails with a `web::Error` when the store or the template fails.
pub async fn task_create_submit(State(state): State<AppState>, body: Bytes) -> Result<Response, web::Error> {
  let mut title = String::new();
  let mut description = String::new();
  let mut priority_str = String::new();
  for (key, value) in form_urlencoded::parse(&body) {
    match key.as_ref() {
      "title" => title = value.into_owned(),
      "description" => description = value.into_owned(),
      "priority" => priority_str = value.into_owned(),
      _ => {}
    }
  }
  log::debug!("task_create_submit: title={title}");

  let priority: Option<u8> = if priority_str.is_empty() {
    None
  } else {
    match parse_priority_byte(&priority_str) {
      Some(byte) => Some(byte),
      None => {
        log::error!("task_create_submit: invalid priority: {priority_str}");
        return render_create_form_error(title, description, priority_str, "invalid priority".to_owned());
      }
    }
  };

  let conn = state.store().connect().await?;
  let new = task::New {
    description,
    priority,
    title,
    ..Default::default()
  };
  let task = repo::task::create(&conn, state.project_id(), &new).await?;
  let _ = state.reload_tx().send(());
  Ok(Redirect::to(&format!("/tasks/{}", task.id())).into_response())
}

/// Task detail page.
///
/// Serves `GET /tasks/{id}`.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection
/// * `id` - the task's id or a prefix that names one task, from the path
///
/// # Returns
///
/// The page as HTML. Fails with `web::Error::NotFound` when no task has the id, and with another `web::Error` when the
/// store or the template fails.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
pub async fn task_detail(State(state): State<AppState>, Path(id): Path<String>) -> Result<Html<String>, web::Error> {
  let data = load_task_detail(&state, &id).await?;
  let tmpl = TaskDetailTemplate {
    blocking: data.blocking,
    description_html: data.description_html,
    display_links: data.display_links,
    is_blocked: data.is_blocked,
    tags: data.tags,
    task: data.task,
    timeline_items: data.timeline_items,
  };
  Ok(Html(tmpl.render()?))
}

/// Task detail fragment (for SSE live reload).
///
/// Serves `GET /tasks/{id}/_detail`.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection
/// * `id` - the task's id or a prefix that names one task, from the path
///
/// # Returns
///
/// The fragment as HTML. Fails with `web::Error::NotFound` when no task has the id, and with another `web::Error` when
/// the store or the template fails.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
pub async fn task_detail_fragment(
  State(state): State<AppState>,
  Path(id): Path<String>,
) -> Result<Html<String>, web::Error> {
  let data = load_task_detail(&state, &id).await?;
  let tmpl = TaskDetailFragmentTemplate {
    blocking: data.blocking,
    description_html: data.description_html,
    display_links: data.display_links,
    is_blocked: data.is_blocked,
    tags: data.tags,
    task: data.task,
    timeline_items: data.timeline_items,
  };
  Ok(Html(tmpl.render()?))
}

/// Task edit form.
///
/// Serves `GET /tasks/{id}/edit`, filled with the task's fields, tags and links.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection
/// * `id` - the task's id or a prefix that names one task, from the path
///
/// # Returns
///
/// The page as HTML. Fails with `web::Error::NotFound` when no task has the id, and with another `web::Error` when the
/// store or the template fails.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
pub async fn task_edit_form(State(state): State<AppState>, Path(id): Path<String>) -> Result<Html<String>, web::Error> {
  let conn = state.store().connect().await?;
  let task_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Tasks, &id).await?;
  let task = repo::task::find_by_id(&conn, task_id.clone())
    .await?
    .ok_or(web::Error::NotFound)?;

  let tags = repo::tag::for_entity(&conn, EntityType::Task, &task_id).await?;

  let rels = repo::relationship::for_entity(&conn, EntityType::Task, &task_id).await?;
  let existing_links = forms::build_existing_links_for_entity(&task_id, EntityType::Task, &rels);

  let priority_options = build_priority_options(task.priority());
  let tmpl = TaskEditTemplate {
    title: task.title().to_owned(),
    description: task.description().to_owned(),
    priority_options,
    tags: tags.join(", "),
    task,
    error: None,
    existing_links,
  };
  Ok(Html(tmpl.render()?))
}

/// Task list page.
///
/// Serves `GET /tasks`.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection and the project id
/// * `params` - the query, with the status tab to show
///
/// # Returns
///
/// The page as HTML. Fails with a `web::Error` when the store or the template fails.
pub async fn task_list(
  State(state): State<AppState>,
  Query(params): Query<TaskListParams>,
) -> Result<Html<String>, web::Error> {
  let data = load_task_list(&state, params.status).await?;
  let tmpl = TaskListTemplate {
    cancelled_count: data.cancelled_count,
    current_status: data.current_status,
    done_count: data.done_count,
    in_progress_count: data.in_progress_count,
    open_count: data.open_count,
    rows: data.rows,
  };
  Ok(Html(tmpl.render()?))
}

/// Task list fragment (for SSE live reload).
///
/// Serves `GET /tasks/_list`.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection and the project id
/// * `params` - the query, with the status tab to show
///
/// # Returns
///
/// The fragment as HTML. Fails with a `web::Error` when the store or the template fails.
pub async fn task_list_fragment(
  State(state): State<AppState>,
  Query(params): Query<TaskListParams>,
) -> Result<Html<String>, web::Error> {
  let data = load_task_list(&state, params.status).await?;
  let tmpl = TaskListFragmentTemplate {
    cancelled_count: data.cancelled_count,
    current_status: data.current_status,
    done_count: data.done_count,
    in_progress_count: data.in_progress_count,
    open_count: data.open_count,
    rows: data.rows,
  };
  Ok(Html(tmpl.render()?))
}

/// Handle task update from edit form.
///
/// Serves `POST /tasks/{id}`, then signals the live reload. It sets the title and description, the status when one
/// is given, and the priority (none when empty). It replaces all the task's tags with the form's tags, and syncs its
/// links with the form's link rows.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection and the reload signal
/// * `id` - the task's id or a prefix that names one task, from the path
/// * `body` - the URL-encoded form body
///
/// # Returns
///
/// A redirect to the task's detail page. Fails with `web::Error::BadRequest` for an unknown status or priority, with
/// `web::Error::NotFound` when no task has the id, and with another `web::Error` when the store fails. A bad status or
/// priority is found after the id resolves and before anything is saved.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
pub async fn task_update(
  State(state): State<AppState>,
  Path(id): Path<String>,
  body: Bytes,
) -> Result<Redirect, web::Error> {
  log::debug!("task_update: task={id}");
  let conn = state.store().connect().await?;
  let task_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Tasks, &id).await?;

  let fields = parse_task_update_fields(&body);

  let status: Option<TaskStatus> = if fields.status.is_empty() {
    None
  } else {
    Some(
      fields
        .status
        .parse()
        .map_err(|e: String| web::Error::BadRequest(format!("invalid status: {e}")))?,
    )
  };

  let priority: Option<Option<u8>> = if fields.priority.is_empty() {
    Some(None)
  } else {
    let byte =
      parse_priority_byte(&fields.priority).ok_or_else(|| web::Error::BadRequest("invalid priority".to_owned()))?;
    Some(Some(byte))
  };

  let patch = task::Patch {
    title: Some(fields.title),
    description: Some(fields.description),
    status,
    priority,
    ..Default::default()
  };

  repo::task::update(&conn, &task_id, &patch).await?;

  repo::tag::detach_all(&conn, EntityType::Task, &task_id).await?;
  for label in forms::parse_tags(&fields.tags) {
    repo::tag::attach(&conn, EntityType::Task, &task_id, &label).await?;
  }

  forms::sync_form_links(&conn, EntityType::Task, &task_id, &fields.link_rels, &fields.link_refs)
    .await
    .map_err(web::Error::Internal)?;

  let _ = state.reload_tx().send(());
  Ok(Redirect::to(&format!("/tasks/{task_id}")))
}

/// Build display links from relationships for detail view.
///
/// A task or artifact at the other end gets a link to its page; any other kind gets none.
///
/// # Arguments
///
/// * `task_id` - the task whose relationships these are
/// * `rels` - the relationships that involve the task
///
/// # Returns
///
/// One display link for each relationship, in the same order.
fn build_display_links(task_id: &Id, rels: &[relationship::Model]) -> Vec<DisplayLink> {
  let mut links = Vec::new();
  for rel in rels {
    let (rel_label, other_id, other_type) = if rel.source_id() == task_id {
      (rel.rel_type().to_string(), rel.target_id().clone(), rel.target_type())
    } else {
      (
        rel.rel_type().inverse().to_string(),
        rel.source_id().clone(),
        rel.source_type(),
      )
    };

    let href = match other_type {
      EntityType::Task => Some(format!("/tasks/{other_id}")),
      EntityType::Artifact => Some(format!("/artifacts/{other_id}")),
      _ => None,
    };

    links.push(DisplayLink {
      rel: rel_label,
      display_text: other_id.short(),
      href,
    });
  }
  links
}

/// Build the list of priority options rendered in the task create/edit forms.
///
/// The resulting list always contains a leading "no priority" option followed
/// by one option per [`Priority::ALL`] variant.
///
/// # Arguments
///
/// * `current` - the task's current priority (or `None` for the create form)
///
/// # Returns
///
/// The options, with the one that matches `current` selected.
fn build_priority_options(current: Option<u8>) -> Vec<PriorityOption> {
  let mut options = Vec::with_capacity(Priority::ALL.len() + 1);
  options.push(PriorityOption {
    label: "— none —".to_owned(),
    selected: current.is_none(),
    value: String::new(),
  });
  for priority in Priority::ALL {
    let byte: u8 = (*priority).into();
    options.push(PriorityOption {
      label: priority.to_string(),
      selected: current == Some(byte),
      value: byte.to_string(),
    });
  }
  options
}

/// Build enriched task rows from a list of tasks, batching tag and
/// relationship lookups into single queries to avoid N+1 fan-out.
///
/// # Arguments
///
/// * `conn` - the store connection
/// * `tasks` - the tasks to show, in order
///
/// # Returns
///
/// One row for each task, in the same order. Fails with a `web::Error` when a store query fails.
async fn build_task_rows(conn: &Connection, tasks: Vec<task::Model>) -> Result<Vec<TaskRow>, web::Error> {
  if tasks.is_empty() {
    return Ok(Vec::new());
  }

  let ids: Vec<_> = tasks.iter().map(|t| t.id().clone()).collect();
  let tags_by_id = repo::tag::for_entities(conn, EntityType::Task, &ids).await?;
  let rels_by_id = repo::relationship::for_entities(conn, EntityType::Task, &ids).await?;

  let empty_rels: Vec<relationship::Model> = Vec::new();
  let mut rows = Vec::with_capacity(tasks.len());
  for task in tasks {
    let task_id = task.id().clone();
    let tags = tags_by_id
      .get(&task_id)
      .map(|ts| ts.iter().map(|t| t.label().to_owned()).collect::<Vec<_>>())
      .unwrap_or_default();
    let rels = rels_by_id.get(&task_id).unwrap_or(&empty_rels);
    let (is_blocked, blocking, blocked_by_display) = compute_blocking(&task_id, rels);

    rows.push(TaskRow {
      task,
      tags,
      is_blocked,
      blocking,
      blocked_by_display,
    });
  }
  Ok(rows)
}

/// Determine blocked/blocking status and build a display string for "blocked by" tasks.
///
/// Only links whose source is the task count.
///
/// # Arguments
///
/// * `task_id` - the task to look at
/// * `rels` - the relationships that involve the task
///
/// # Returns
///
/// Whether the task is blocked, whether it blocks another entity, and `blocked by` with the short ids of its blockers
/// (empty when it has none).
fn compute_blocking(task_id: &Id, rels: &[relationship::Model]) -> (bool, bool, String) {
  let mut is_blocked = false;
  let mut blocking = false;
  let mut blocked_by_ids = Vec::new();

  for rel in rels {
    match rel.rel_type() {
      RelationshipType::BlockedBy if rel.source_id() == task_id => {
        is_blocked = true;
        blocked_by_ids.push(rel.target_id().short());
      }
      RelationshipType::Blocks if rel.source_id() == task_id => {
        blocking = true;
      }
      _ => {}
    }
  }

  let blocked_by_display = if blocked_by_ids.is_empty() {
    String::new()
  } else {
    format!("blocked by {}", blocked_by_ids.join(", "))
  };

  (is_blocked, blocking, blocked_by_display)
}

/// Load the shared task board payload used by both the full page and the
/// fragment handler.
///
/// Tasks are bucketed by status into four columns. The Done and Cancelled
/// buckets are truncated to a per-column cap parsed from `done_raw` and
/// `cancelled_raw`; any unrecognized value falls back to the default cap.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection and the project id
/// * `done_raw` - the `done` query value, the cap of the Done column
/// * `cancelled_raw` - the `cancelled` query value, the cap of the Cancelled column
///
/// # Returns
///
/// The four columns and the two caps. Fails with a `web::Error` when the store fails.
async fn load_task_board(
  state: &AppState,
  done_raw: Option<&str>,
  cancelled_raw: Option<&str>,
) -> Result<TaskBoardData, web::Error> {
  let conn = state.store().connect().await?;
  let tasks = repo::task::all(&conn, state.project_id(), &task::Filter::all()).await?;

  let mut open_tasks = Vec::new();
  let mut in_progress_tasks = Vec::new();
  let mut done_tasks = Vec::new();
  let mut cancelled_tasks = Vec::new();
  for t in tasks {
    match t.status() {
      TaskStatus::InProgress => in_progress_tasks.push(t),
      TaskStatus::Done => done_tasks.push(t),
      TaskStatus::Cancelled => cancelled_tasks.push(t),
      TaskStatus::Open => open_tasks.push(t),
    }
  }

  let done_limit = parse_board_limit(done_raw);
  let cancelled_limit = parse_board_limit(cancelled_raw);
  done_tasks.truncate(done_limit as usize);
  cancelled_tasks.truncate(cancelled_limit as usize);

  Ok(TaskBoardData {
    cancelled_limit,
    cancelled_tasks,
    done_limit,
    done_tasks,
    in_progress_tasks,
    open_tasks,
  })
}

/// Load the shared task detail payload used by both the full page and the
/// fragment handler.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection
/// * `id` - the task's id or a prefix that names one task
///
/// # Returns
///
/// The task with its rendered description, tags, blocking state, links and timeline. Fails with `web::Error::NotFound`
/// when no task has the id, and with another `web::Error` when the store or the timeline fails.
async fn load_task_detail(state: &AppState, id: &str) -> Result<TaskDetailData, web::Error> {
  let conn = state.store().connect().await?;
  let task_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Tasks, id).await?;
  let task = repo::task::find_by_id(&conn, task_id.clone())
    .await?
    .ok_or(web::Error::NotFound)?;

  let tags = repo::tag::for_entity(&conn, EntityType::Task, &task_id).await?;
  let rels = repo::relationship::for_entity(&conn, EntityType::Task, &task_id).await?;

  let description_html = if task.description().is_empty() {
    String::new()
  } else {
    markdown::render_markdown_to_html(task.description())
  };

  let (is_blocked, blocking, _) = compute_blocking(&task_id, &rels);
  let display_links = build_display_links(&task_id, &rels);

  let timeline_items = timeline::build_timeline(&conn, EntityType::Task, &task_id)
    .await
    .map_err(web::Error::Internal)?;

  Ok(TaskDetailData {
    blocking,
    description_html,
    display_links,
    is_blocked,
    tags,
    task,
    timeline_items,
  })
}

/// Load the shared task list payload used by both the full page and the
/// fragment handler.
///
/// When `status_param` is `None`, defaults to `all`. The special value `all`
/// bypasses status filtering. Status counts always reflect every task in the
/// project so the tab badges stay stable regardless of the current filter.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection and the project id
/// * `status_param` - the status tab from the query
///
/// # Returns
///
/// The rows for the tab and the count of tasks in each status. Fails with a `web::Error` when a store query fails.
async fn load_task_list(state: &AppState, status_param: Option<String>) -> Result<TaskListData, web::Error> {
  let conn = state.store().connect().await?;

  let counts = repo::task::status_counts(&conn, state.project_id()).await?;

  let current_status = status_param.unwrap_or_else(|| "all".to_owned());

  let filter = match current_status.as_str() {
    "all" => task::Filter::all(),
    s => task::Filter {
      status: s.parse::<TaskStatus>().ok(),
      ..task::Filter::all()
    },
  };

  let tasks = repo::task::all(&conn, state.project_id(), &filter).await?;
  let rows = build_task_rows(&conn, tasks).await?;

  Ok(TaskListData {
    cancelled_count: counts.cancelled as usize,
    current_status,
    done_count: counts.done as usize,
    in_progress_count: counts.in_progress as usize,
    open_count: counts.open as usize,
    rows,
  })
}

/// Parse a task-board column cap query value.
///
/// Accepts `10`, `20`, or `50`; any missing or unrecognized value falls back to `10`.
///
/// # Arguments
///
/// * `raw` - the query value, or `None` when the query has none
///
/// # Returns
///
/// The cap: 10, 20 or 50.
fn parse_board_limit(raw: Option<&str>) -> u32 {
  match raw {
    Some("20") => 20,
    Some("50") => 50,
    _ => 10,
  }
}

/// Parse a form-supplied priority string (numeric byte or label) into the byte
/// value stored in the task row.
///
/// # Arguments
///
/// * `raw` - the form's priority value
///
/// # Returns
///
/// The priority byte. Returns `None` for any unrecognized input.
fn parse_priority_byte(raw: &str) -> Option<u8> {
  if let Ok(byte) = raw.parse::<u8>() {
    return Priority::try_from(byte).ok().map(|_| byte);
  }
  raw.parse::<Priority>().ok().map(|p| p.into())
}

/// Parse scalar task-update fields and repeated link pairs from a url-encoded
/// form body in a single pass.
///
/// Unknown keys are skipped, and a missing key leaves its field empty.
///
/// # Arguments
///
/// * `body` - the URL-encoded form body
///
/// # Returns
///
/// The fields of the form.
fn parse_task_update_fields(body: &[u8]) -> TaskUpdateFields {
  let mut fields = TaskUpdateFields::default();
  for (key, value) in form_urlencoded::parse(body) {
    match key.as_ref() {
      "title" => fields.title = value.into_owned(),
      "description" => fields.description = value.into_owned(),
      "status" => fields.status = value.into_owned(),
      "priority" => fields.priority = value.into_owned(),
      "tags" => fields.tags = value.into_owned(),
      "link_rel[]" => fields.link_rels.push(value.into_owned()),
      "link_ref[]" => fields.link_refs.push(value.into_owned()),
      _ => {}
    }
  }
  fields
}

/// Re-render the task create form, preserving user input and surfacing an error message.
///
/// The response has status 200. No priority option is selected, since the rejected value matches none.
///
/// # Arguments
///
/// * `title` - the title the user sent
/// * `description` - the description the user sent
/// * `priority_str` - the priority the user sent
/// * `error` - the message to show above the form
///
/// # Returns
///
/// The form page as a response. Fails with a `web::Error` when the template fails.
fn render_create_form_error(
  title: String,
  description: String,
  priority_str: String,
  error: String,
) -> Result<Response, web::Error> {
  let current_priority = priority_str.parse::<u8>().ok();
  let tmpl = TaskCreateTemplate {
    description,
    error: Some(error),
    priority_options: build_priority_options(current_priority),
    title,
  };
  Ok(Html(tmpl.render()?).into_response())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{self, model::Project};

  async fn setup() -> AppState {
    let (store, tmp) = store::open_temp().await.unwrap();
    let conn = store.connect().await.unwrap();
    let project = Project::new("/tmp/web-task-test".into());
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

    // Seed: one task per status
    for (title, status) in [
      ("Open A", TaskStatus::Open),
      ("Open B", TaskStatus::Open),
      ("In progress", TaskStatus::InProgress),
      ("Done", TaskStatus::Done),
      ("Cancelled", TaskStatus::Cancelled),
    ] {
      let new = task::New {
        title: title.into(),
        status: Some(status),
        ..Default::default()
      };
      repo::task::create(&conn, &project_id, &new).await.unwrap();
    }
    // Leak the tempdir for the duration of the test process
    std::mem::forget(tmp);
    AppState::new(store, project_id)
  }

  mod build_priority_options {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_emits_a_leading_none_option_followed_by_every_priority_variant() {
      let options = build_priority_options(None);

      assert_eq!(options.len(), Priority::ALL.len() + 1);
      assert_eq!(options[0].value, "");
      assert_eq!(options[0].label, "— none —");
      assert!(options[0].selected);
      for (i, priority) in Priority::ALL.iter().enumerate() {
        let option = &options[i + 1];
        let byte: u8 = (*priority).into();
        assert_eq!(option.value, byte.to_string());
        assert_eq!(option.label, priority.to_string());
        assert!(!option.selected);
      }
    }

    #[test]
    fn it_selects_the_matching_option_when_a_current_priority_is_given() {
      let options = build_priority_options(Some(2));

      let selected: Vec<_> = options.iter().filter(|o| o.selected).collect();
      assert_eq!(selected.len(), 1);
      assert_eq!(selected[0].value, "2");
      assert_eq!(selected[0].label, "medium");
    }
  }

  mod parse_priority_byte {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_numeric_bytes_in_range() {
      assert_eq!(parse_priority_byte("0"), Some(0));
      assert_eq!(parse_priority_byte("2"), Some(2));
      assert_eq!(parse_priority_byte("4"), Some(4));
    }

    #[test]
    fn it_accepts_labels_case_insensitively() {
      assert_eq!(parse_priority_byte("critical"), Some(0));
      assert_eq!(parse_priority_byte("Medium"), Some(2));
      assert_eq!(parse_priority_byte("LOWEST"), Some(4));
    }

    #[test]
    fn it_rejects_out_of_range_bytes() {
      assert_eq!(parse_priority_byte("9"), None);
      assert_eq!(parse_priority_byte("255"), None);
    }

    #[test]
    fn it_rejects_unknown_labels() {
      assert_eq!(parse_priority_byte("urgent"), None);
      assert_eq!(parse_priority_byte(""), None);
    }
  }

  mod parse_task_update_fields {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_collects_scalars_and_link_pairs_in_a_single_pass() {
      let body =
        b"title=Refactor&description=body&status=in-progress&priority=high&tags=a,b&link_rel[]=blocks&link_ref[]=tasks/abc&link_rel[]=blocked-by&link_ref[]=tasks/def";

      let fields = parse_task_update_fields(body);

      assert_eq!(fields.title, "Refactor");
      assert_eq!(fields.description, "body");
      assert_eq!(fields.status, "in-progress");
      assert_eq!(fields.priority, "high");
      assert_eq!(fields.tags, "a,b");
      assert_eq!(fields.link_rels, vec!["blocks", "blocked-by"]);
      assert_eq!(fields.link_refs, vec!["tasks/abc", "tasks/def"]);
    }

    #[test]
    fn it_returns_empty_defaults_when_no_known_keys_are_present() {
      let fields = parse_task_update_fields(b"unrelated=value");

      assert!(fields.title.is_empty());
      assert!(fields.link_rels.is_empty());
      assert!(fields.link_refs.is_empty());
    }
  }

  mod task_create_submit {
    use axum::{
      body::{Bytes, to_bytes},
      extract::State,
      http::StatusCode,
    };
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_re_renders_the_create_form_with_preserved_input_when_priority_is_out_of_range() {
      let state = setup().await;

      let body = Bytes::from("title=bad-priority&description=keep+this&priority=9");
      let response = super::super::task_create_submit(State(state), body)
        .await
        .expect("validation failure should re-render, not error");

      let status = response.status();
      let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
      let html = String::from_utf8(body_bytes.to_vec()).unwrap();

      assert_eq!(status, StatusCode::OK);
      assert!(html.contains("invalid priority"));
      assert!(html.contains("value=\"bad-priority\""));
      assert!(html.contains("keep this"));
      assert!(html.contains("md-preview-toggle"));
    }

    #[tokio::test]
    async fn it_treats_an_empty_priority_as_no_priority_and_redirects() {
      let state = setup().await;

      let body = Bytes::from("title=no-priority&description=&priority=");
      let response = super::super::task_create_submit(State(state), body)
        .await
        .expect("empty priority should succeed");

      assert_eq!(response.status(), StatusCode::SEE_OTHER);
    }
  }

  mod task_detail_error_response {
    use axum::{
      body::to_bytes,
      extract::{Path, State},
      http::StatusCode,
      response::IntoResponse,
    };
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_renders_html_404_when_a_well_formed_prefix_matches_no_task() {
      let state = setup().await;

      // Valid base32 characters but no matching task: should propagate as NotFound → 404.
      let err = task_detail(State(state), Path("zzzzzzzz".into()))
        .await
        .expect_err("unknown id prefix should propagate as not-found");
      let response = err.into_response();
      let status = response.status();
      let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
      let body = String::from_utf8(body_bytes.to_vec()).unwrap();

      assert_eq!(status, StatusCode::NOT_FOUND);
      assert!(body.contains("<html"));
      assert!(body.contains("404"));
    }

    #[tokio::test]
    async fn it_renders_html_500_when_the_id_prefix_is_malformed() {
      let state = setup().await;

      let err = task_detail(State(state), Path("!!!-not-a-valid-prefix".into()))
        .await
        .expect_err("invalid id prefix should propagate as an internal error");
      let response = err.into_response();
      let status = response.status();
      let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
      let body = String::from_utf8(body_bytes.to_vec()).unwrap();

      assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
      assert!(body.contains("<html"));
      assert!(body.contains("500"));
    }
  }

  mod load_task_board {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_status_tasks(state: &AppState, title: &str, status: TaskStatus, count: usize) {
      let conn = state.store().connect().await.unwrap();
      for i in 0..count {
        repo::task::create(
          &conn,
          state.project_id(),
          &task::New {
            title: format!("{title} {i}"),
            status: Some(status),
            ..Default::default()
          },
        )
        .await
        .unwrap();
      }
    }

    #[tokio::test]
    async fn it_buckets_tasks_by_status_across_all_projects_tasks() {
      let state = setup().await;

      let data = load_task_board(&state, None, None).await.unwrap();

      assert_eq!(data.open_tasks.len(), 2);
      assert_eq!(data.in_progress_tasks.len(), 1);
      assert_eq!(data.done_tasks.len(), 1);
      assert_eq!(data.cancelled_tasks.len(), 1);
    }

    #[tokio::test]
    async fn it_caps_done_and_cancelled_at_ten_by_default() {
      let (store_arc, tmp) = store::open_temp().await.unwrap();
      let conn = store_arc.connect().await.unwrap();
      let project = Project::new("/tmp/web-task-board-default-cap".into());
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
      std::mem::forget(tmp);
      let state = AppState::new(store_arc, project_id);

      seed_status_tasks(&state, "Done", TaskStatus::Done, 15).await;
      seed_status_tasks(&state, "Cancelled", TaskStatus::Cancelled, 15).await;

      let data = load_task_board(&state, None, None).await.unwrap();

      assert_eq!(data.done_limit, 10);
      assert_eq!(data.cancelled_limit, 10);
      assert_eq!(data.done_tasks.len(), 10);
      assert_eq!(data.cancelled_tasks.len(), 10);
    }

    #[tokio::test]
    async fn it_honors_overrides_to_twenty_and_fifty() {
      let (store_arc, tmp) = store::open_temp().await.unwrap();
      let conn = store_arc.connect().await.unwrap();
      let project = Project::new("/tmp/web-task-board-override".into());
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
      std::mem::forget(tmp);
      let state = AppState::new(store_arc, project_id);

      seed_status_tasks(&state, "Done", TaskStatus::Done, 60).await;
      seed_status_tasks(&state, "Cancelled", TaskStatus::Cancelled, 60).await;

      let data = load_task_board(&state, Some("20"), Some("50")).await.unwrap();

      assert_eq!(data.done_limit, 20);
      assert_eq!(data.cancelled_limit, 50);
      assert_eq!(data.done_tasks.len(), 20);
      assert_eq!(data.cancelled_tasks.len(), 50);
    }

    #[tokio::test]
    async fn it_falls_back_to_default_when_a_query_param_is_invalid() {
      let state = setup().await;

      let data = load_task_board(&state, Some("99"), Some("abc")).await.unwrap();

      assert_eq!(data.done_limit, 10);
      assert_eq!(data.cancelled_limit, 10);
    }
  }

  mod parse_board_limit {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_ten_twenty_and_fifty() {
      assert_eq!(parse_board_limit(Some("10")), 10);
      assert_eq!(parse_board_limit(Some("20")), 20);
      assert_eq!(parse_board_limit(Some("50")), 50);
    }

    #[test]
    fn it_defaults_to_ten_when_missing() {
      assert_eq!(parse_board_limit(None), 10);
    }

    #[test]
    fn it_defaults_to_ten_when_value_is_not_allowed() {
      assert_eq!(parse_board_limit(Some("")), 10);
      assert_eq!(parse_board_limit(Some("99")), 10);
      assert_eq!(parse_board_limit(Some("abc")), 10);
    }
  }

  mod load_task_list {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_defaults_to_all_when_no_status_given() {
      let state = setup().await;

      let data = load_task_list(&state, None).await.unwrap();

      assert_eq!(data.current_status, "all");
      assert_eq!(data.rows.len(), 5);
      assert_eq!(
        (
          data.open_count,
          data.in_progress_count,
          data.done_count,
          data.cancelled_count
        ),
        (2, 1, 1, 1),
      );
    }

    #[tokio::test]
    async fn it_filters_to_a_specific_status_at_the_db_layer() {
      let state = setup().await;

      let data = load_task_list(&state, Some("done".into())).await.unwrap();

      assert_eq!(data.current_status, "done");
      assert_eq!(data.rows.len(), 1);
      assert_eq!(data.rows[0].task.status(), TaskStatus::Done);
    }

    #[tokio::test]
    async fn it_reports_counts_across_every_status_regardless_of_filter() {
      let state = setup().await;

      let all_data = load_task_list(&state, None).await.unwrap();
      let done_data = load_task_list(&state, Some("done".into())).await.unwrap();

      assert_eq!(
        (
          all_data.open_count,
          all_data.in_progress_count,
          all_data.done_count,
          all_data.cancelled_count,
        ),
        (2, 1, 1, 1),
      );
      assert_eq!(
        (
          done_data.open_count,
          done_data.in_progress_count,
          done_data.done_count,
          done_data.cancelled_count,
        ),
        (2, 1, 1, 1),
      );
    }

    #[tokio::test]
    async fn it_returns_every_task_when_status_is_all() {
      let state = setup().await;

      let data = load_task_list(&state, Some("all".into())).await.unwrap();

      assert_eq!(data.current_status, "all");
      assert_eq!(data.rows.len(), 5);
      assert_eq!(
        (
          data.open_count,
          data.in_progress_count,
          data.done_count,
          data.cancelled_count
        ),
        (2, 1, 1, 1),
      );
    }
  }

  mod task_detail_timeline {
    use pretty_assertions::assert_eq;

    use crate::{
      store::{
        self,
        model::{
          Project, note,
          primitives::{EntityType, TaskStatus},
          task,
        },
        repo,
      },
      web::{AppState, timeline},
    };

    #[tokio::test]
    async fn it_filters_out_events_with_null_semantic_type() {
      let (store_arc, tmp) = store::open_temp().await.unwrap();
      let conn = store_arc.connect().await.unwrap();
      let project = Project::new("/tmp/web-task-timeline-nullfilter".into());
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
      std::mem::forget(tmp);

      let task = repo::task::create(
        &conn,
        &project_id,
        &task::New {
          title: "Task".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      // A plain modified event (no semantic_type) should NOT appear.
      let tx = repo::transaction::begin(&conn, &project_id, "task update")
        .await
        .unwrap();
      repo::transaction::record_event(&conn, tx.id(), "tasks", &task.id().to_string(), "modified", None)
        .await
        .unwrap();

      let items = timeline::build_timeline(&conn, EntityType::Task, task.id())
        .await
        .unwrap();

      assert!(items.is_empty());
    }

    #[tokio::test]
    async fn it_merges_notes_and_semantic_events_for_a_task_in_chronological_order() {
      let (store_arc, tmp) = store::open_temp().await.unwrap();
      let conn = store_arc.connect().await.unwrap();
      let project = Project::new("/tmp/web-task-timeline".into());
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
      std::mem::forget(tmp);

      let task = repo::task::create(
        &conn,
        &project_id,
        &task::New {
          title: "Task".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      // Record a semantic create event.
      let tx = repo::transaction::begin(&conn, &project_id, "task create")
        .await
        .unwrap();
      repo::transaction::record_semantic_event(
        &conn,
        tx.id(),
        "tasks",
        &task.id().to_string(),
        "created",
        None,
        Some("created"),
        None,
        None,
      )
      .await
      .unwrap();

      // Add a note via the repo (simulating a web note_add).
      repo::note::create(
        &conn,
        EntityType::Task,
        task.id(),
        &note::New {
          body: "first note".into(),
          author_id: None,
        },
      )
      .await
      .unwrap();

      // A second semantic event: status change.
      let tx2 = repo::transaction::begin(&conn, &project_id, "task claim")
        .await
        .unwrap();
      let _ = repo::task::update(
        &conn,
        task.id(),
        &task::Patch {
          status: Some(TaskStatus::InProgress),
          ..Default::default()
        },
      )
      .await
      .unwrap();
      repo::transaction::record_semantic_event(
        &conn,
        tx2.id(),
        "tasks",
        &task.id().to_string(),
        "modified",
        None,
        Some("status-change"),
        Some("open"),
        Some("in-progress"),
      )
      .await
      .unwrap();

      // Prevent the temp store from dropping.
      let _state = AppState::new(store_arc.clone(), project_id.clone());
      let items = timeline::build_timeline(&conn, EntityType::Task, task.id())
        .await
        .unwrap();

      assert_eq!(items.len(), 3);
      // First item should be the created event.
      assert!(items[0].as_event().is_some());
      // Second should be the note.
      assert!(items[1].as_note().is_some());
      // Third should be the status-change event.
      assert_eq!(
        items[2].as_event().unwrap().display_text,
        "someone changed status from open to in-progress"
      );
    }
  }
}
