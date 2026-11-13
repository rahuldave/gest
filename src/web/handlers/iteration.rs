//! Iteration list/detail/board handlers.

use std::collections::BTreeMap;

use askama::Template;
use axum::{
  extract::{Path, Query, State},
  response::Html,
};
use libsql::Connection;
use serde::Deserialize;

use crate::{
  store::{
    model::{
      iteration,
      primitives::{EntityType, IterationStatus},
    },
    repo::{
      self,
      iteration::{IterationTaskRow, StatusCounts},
    },
  },
  web::{
    self, AppState, markdown,
    timeline::{self, TimelineItem},
  },
};

/// Query parameters for the iteration list view (status tab selection).
#[derive(Deserialize)]
pub struct IterationListParams {
  /// The status tab to show: `all`, or an iteration status such as `active`.
  ///
  /// A missing value means `all`. A value that is not a status also shows every iteration.
  status: Option<String>,
}

/// Shared data backing both the full iteration board page and the SSE fragment.
struct IterationBoardData {
  /// Tasks of the iteration whose status is `cancelled`.
  cancelled_tasks: Vec<IterationTaskRow>,
  /// Tasks of the iteration whose status is `done`.
  done_tasks: Vec<IterationTaskRow>,
  /// Tasks of the iteration whose status is `in-progress`.
  in_progress_tasks: Vec<IterationTaskRow>,
  /// The iteration that the board shows.
  iteration: iteration::Model,
  /// Tasks of the iteration with any other status, shown in the open column.
  open_tasks: Vec<IterationTaskRow>,
}

/// The board's columns alone, which the SSE live reload swaps into the board page.
#[derive(Template)]
#[template(path = "iterations/board_content.html")]
struct IterationBoardFragmentTemplate {
  /// Tasks of the iteration whose status is `cancelled`.
  cancelled_tasks: Vec<IterationTaskRow>,
  /// Tasks of the iteration whose status is `done`.
  done_tasks: Vec<IterationTaskRow>,
  /// Tasks of the iteration whose status is `in-progress`.
  in_progress_tasks: Vec<IterationTaskRow>,
  /// The iteration that the board shows.
  iteration: iteration::Model,
  /// Tasks of the iteration with any other status, shown in the open column.
  open_tasks: Vec<IterationTaskRow>,
}

/// The full board page of one iteration, with its tasks in status columns.
#[derive(Template)]
#[template(path = "iterations/board.html")]
struct IterationBoardTemplate {
  /// Tasks of the iteration whose status is `cancelled`.
  cancelled_tasks: Vec<IterationTaskRow>,
  /// Tasks of the iteration whose status is `done`.
  done_tasks: Vec<IterationTaskRow>,
  /// Tasks of the iteration whose status is `in-progress`.
  in_progress_tasks: Vec<IterationTaskRow>,
  /// The iteration that the board shows.
  iteration: iteration::Model,
  /// Tasks of the iteration with any other status, shown in the open column.
  open_tasks: Vec<IterationTaskRow>,
}

/// Shared data backing both the full iteration detail page and the SSE fragment.
struct IterationDetailData {
  /// The iteration's Markdown description, rendered to HTML.
  description_html: String,
  /// The iteration that the page shows.
  iteration: iteration::Model,
  /// The iteration's tasks grouped by phase, in ascending phase order.
  phases: Vec<PhaseGroup>,
  /// How many of the iteration's tasks are in each status.
  status_counts: StatusCounts,
  /// The labels of the iteration's tags.
  tags: Vec<String>,
  /// How many tasks the iteration holds in all.
  task_count: i64,
  /// The iteration's timeline, from `timeline::build_timeline`.
  timeline_items: Vec<TimelineItem>,
}

/// The detail content alone, which the SSE live reload swaps into the detail page.
#[derive(Template)]
#[template(path = "iterations/detail_content.html")]
struct IterationDetailFragmentTemplate {
  /// The iteration's Markdown description, rendered to HTML.
  description_html: String,
  /// The iteration that the page shows.
  iteration: iteration::Model,
  /// The iteration's tasks grouped by phase, in ascending phase order.
  phases: Vec<PhaseGroup>,
  /// How many of the iteration's tasks are in each status.
  status_counts: StatusCounts,
  /// The labels of the iteration's tags.
  tags: Vec<String>,
  /// How many tasks the iteration holds in all.
  task_count: i64,
  /// The iteration's timeline, from `timeline::build_timeline`.
  timeline_items: Vec<TimelineItem>,
}

/// The full detail page of one iteration: description, tags, phases and timeline.
#[derive(Template)]
#[template(path = "iterations/detail.html")]
struct IterationDetailTemplate {
  /// The iteration's Markdown description, rendered to HTML.
  description_html: String,
  /// The iteration that the page shows.
  iteration: iteration::Model,
  /// The iteration's tasks grouped by phase, in ascending phase order.
  phases: Vec<PhaseGroup>,
  /// How many of the iteration's tasks are in each status.
  status_counts: StatusCounts,
  /// The labels of the iteration's tags.
  tags: Vec<String>,
  /// How many tasks the iteration holds in all.
  task_count: i64,
  /// The iteration's timeline, from `timeline::build_timeline`.
  timeline_items: Vec<TimelineItem>,
}

/// Shared data backing both the full iteration list page and the SSE fragment.
struct IterationListData {
  /// How many of the project's iterations are active, whatever the status tab.
  active_count: usize,
  /// How many of the project's iterations are cancelled, whatever the status tab.
  cancelled_count: usize,
  /// How many of the project's iterations are completed, whatever the status tab.
  completed_count: usize,
  /// The selected status tab: the `status` query value, or `all` when there is none.
  current_status: String,
  /// The iterations that the selected status tab shows, with their tags and counts.
  rows: Vec<IterationRow>,
}

/// The list content alone, which the SSE live reload swaps into the list page.
#[derive(Template)]
#[template(path = "iterations/list_content.html")]
struct IterationListFragmentTemplate {
  /// How many of the project's iterations are active, whatever the status tab.
  active_count: usize,
  /// How many of the project's iterations are cancelled, whatever the status tab.
  cancelled_count: usize,
  /// How many of the project's iterations are completed, whatever the status tab.
  completed_count: usize,
  /// The selected status tab: the `status` query value, or `all` when there is none.
  current_status: String,
  /// The iterations that the selected status tab shows, with their tags and counts.
  rows: Vec<IterationRow>,
}

/// The full iteration list page, with a tab for each status.
#[derive(Template)]
#[template(path = "iterations/list.html")]
struct IterationListTemplate {
  /// How many of the project's iterations are active, whatever the status tab.
  active_count: usize,
  /// How many of the project's iterations are cancelled, whatever the status tab.
  cancelled_count: usize,
  /// How many of the project's iterations are completed, whatever the status tab.
  completed_count: usize,
  /// The selected status tab: the `status` query value, or `all` when there is none.
  current_status: String,
  /// The iterations that the selected status tab shows, with their tags and counts.
  rows: Vec<IterationRow>,
}

/// One row of the iteration list: an iteration with its tags and counts.
struct IterationRow {
  /// The iteration that the row shows.
  iteration: iteration::Model,
  /// The highest phase number among the iteration's tasks, or 0 when it has none.
  phase_count: u32,
  /// The labels of the iteration's tags.
  tags: Vec<String>,
  /// How many tasks the iteration holds in all.
  task_count: i64,
}

/// The tasks of one phase of an iteration, for the detail page.
struct PhaseGroup {
  /// The phase number; the groups are in ascending phase order.
  number: u32,
  /// The tasks in this phase.
  tasks: Vec<IterationTaskRow>,
}

/// Iteration board page.
///
/// Serves `GET /iterations/{id}/board`.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection
/// * `id` - the iteration's id or a prefix that names one iteration, from the path
///
/// # Returns
///
/// The page as HTML. Fails with `web::Error::NotFound` when no iteration has the id, and with another `web::Error` when
/// the store or the template fails.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
pub async fn iteration_board(
  State(state): State<AppState>,
  Path(id): Path<String>,
) -> Result<Html<String>, web::Error> {
  let data = load_iteration_board(&state, &id).await?;
  let tmpl = IterationBoardTemplate {
    cancelled_tasks: data.cancelled_tasks,
    done_tasks: data.done_tasks,
    in_progress_tasks: data.in_progress_tasks,
    iteration: data.iteration,
    open_tasks: data.open_tasks,
  };
  Ok(Html(tmpl.render()?))
}

/// Iteration board fragment (for SSE live reload).
///
/// Serves `GET /iterations/{id}/_board`.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection
/// * `id` - the iteration's id or a prefix that names one iteration, from the path
///
/// # Returns
///
/// The fragment as HTML. Fails with `web::Error::NotFound` when no iteration has the id, and with another `web::Error`
/// when the store or the template fails.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
pub async fn iteration_board_fragment(
  State(state): State<AppState>,
  Path(id): Path<String>,
) -> Result<Html<String>, web::Error> {
  let data = load_iteration_board(&state, &id).await?;
  let tmpl = IterationBoardFragmentTemplate {
    cancelled_tasks: data.cancelled_tasks,
    done_tasks: data.done_tasks,
    in_progress_tasks: data.in_progress_tasks,
    iteration: data.iteration,
    open_tasks: data.open_tasks,
  };
  Ok(Html(tmpl.render()?))
}

/// Iteration detail page.
///
/// Serves `GET /iterations/{id}`.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection
/// * `id` - the iteration's id or a prefix that names one iteration, from the path
///
/// # Returns
///
/// The page as HTML. Fails with `web::Error::NotFound` when no iteration has the id, and with another `web::Error` when
/// the store or the template fails.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
pub async fn iteration_detail(
  State(state): State<AppState>,
  Path(id): Path<String>,
) -> Result<Html<String>, web::Error> {
  let data = load_iteration_detail(&state, &id).await?;
  let tmpl = IterationDetailTemplate {
    description_html: data.description_html,
    iteration: data.iteration,
    phases: data.phases,
    status_counts: data.status_counts,
    tags: data.tags,
    task_count: data.task_count,
    timeline_items: data.timeline_items,
  };
  Ok(Html(tmpl.render()?))
}

/// Iteration detail fragment (for SSE live reload).
///
/// Serves `GET /iterations/{id}/_detail`.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection
/// * `id` - the iteration's id or a prefix that names one iteration, from the path
///
/// # Returns
///
/// The fragment as HTML. Fails with `web::Error::NotFound` when no iteration has the id, and with another `web::Error`
/// when the store or the template fails.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
pub async fn iteration_detail_fragment(
  State(state): State<AppState>,
  Path(id): Path<String>,
) -> Result<Html<String>, web::Error> {
  let data = load_iteration_detail(&state, &id).await?;
  let tmpl = IterationDetailFragmentTemplate {
    description_html: data.description_html,
    iteration: data.iteration,
    phases: data.phases,
    status_counts: data.status_counts,
    tags: data.tags,
    task_count: data.task_count,
    timeline_items: data.timeline_items,
  };
  Ok(Html(tmpl.render()?))
}

/// Iteration list page.
///
/// Serves `GET /iterations`.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection
/// * `params` - the query, with the status tab to show
///
/// # Returns
///
/// The page as HTML. Fails with a `web::Error` when the store or the template fails.
pub async fn iteration_list(
  State(state): State<AppState>,
  Query(params): Query<IterationListParams>,
) -> Result<Html<String>, web::Error> {
  let data = load_iteration_list(&state, params.status).await?;
  let tmpl = IterationListTemplate {
    active_count: data.active_count,
    cancelled_count: data.cancelled_count,
    completed_count: data.completed_count,
    current_status: data.current_status,
    rows: data.rows,
  };
  Ok(Html(tmpl.render()?))
}

/// Iteration list fragment (for SSE live reload).
///
/// Serves `GET /iterations/_list`.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection
/// * `params` - the query, with the status tab to show
///
/// # Returns
///
/// The fragment as HTML. Fails with a `web::Error` when the store or the template fails.
pub async fn iteration_list_fragment(
  State(state): State<AppState>,
  Query(params): Query<IterationListParams>,
) -> Result<Html<String>, web::Error> {
  let data = load_iteration_list(&state, params.status).await?;
  let tmpl = IterationListFragmentTemplate {
    active_count: data.active_count,
    cancelled_count: data.cancelled_count,
    completed_count: data.completed_count,
    current_status: data.current_status,
    rows: data.rows,
  };
  Ok(Html(tmpl.render()?))
}

/// Build the enriched iteration rows for a page of iterations, batching tag
/// lookups into a single query.
///
/// The task counts and the highest phase are still one query each per iteration.
///
/// # Arguments
///
/// * `conn` - the store connection
/// * `iterations` - the iterations to show, in order
///
/// # Returns
///
/// One row for each iteration, in the same order. Fails with a `web::Error` when a store query fails.
async fn build_iteration_rows(
  conn: &Connection,
  iterations: Vec<iteration::Model>,
) -> Result<Vec<IterationRow>, web::Error> {
  if iterations.is_empty() {
    return Ok(Vec::new());
  }

  let ids: Vec<_> = iterations.iter().map(|i| i.id().clone()).collect();
  let tags_by_id = repo::tag::for_entities(conn, EntityType::Iteration, &ids).await?;

  let mut rows = Vec::with_capacity(iterations.len());
  for it in iterations {
    let tags = tags_by_id
      .get(it.id())
      .map(|ts| ts.iter().map(|t| t.label().to_owned()).collect::<Vec<_>>())
      .unwrap_or_default();
    let counts = repo::iteration::task_status_counts(conn, it.id()).await?;
    let max_phase = repo::iteration::max_phase(conn, it.id()).await?;
    rows.push(IterationRow {
      iteration: it,
      phase_count: max_phase.unwrap_or(0),
      tags,
      task_count: counts.total,
    });
  }

  Ok(rows)
}

/// Load the shared iteration board payload used by both the full page and the
/// fragment handler.
///
/// Tasks with status `in-progress`, `done` or `cancelled` go to those columns; any other status goes to the open
/// column.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection
/// * `id` - the iteration's id or a prefix that names one iteration
///
/// # Returns
///
/// The iteration and its tasks by column. Fails with `web::Error::NotFound` when no iteration has the id, and with
/// another `web::Error` when the store fails.
/// An ambiguous or malformed id prefix also fails, with `web::Error::Internal`.
async fn load_iteration_board(state: &AppState, id: &str) -> Result<IterationBoardData, web::Error> {
  let conn = state.store().connect().await?;
  let iter_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Iterations, id).await?;
  let iteration = repo::iteration::find_by_id(&conn, iter_id.clone())
    .await?
    .ok_or(web::Error::NotFound)?;

  let tasks = repo::iteration::tasks_with_phase(&conn, &iter_id).await?;

  let mut open_tasks = Vec::new();
  let mut in_progress_tasks = Vec::new();
  let mut done_tasks = Vec::new();
  let mut cancelled_tasks = Vec::new();
  for t in tasks {
    match t.status.as_str() {
      "in-progress" => in_progress_tasks.push(t),
      "done" => done_tasks.push(t),
      "cancelled" => cancelled_tasks.push(t),
      _ => open_tasks.push(t),
    }
  }

  Ok(IterationBoardData {
    cancelled_tasks,
    done_tasks,
    in_progress_tasks,
    iteration,
    open_tasks,
  })
}

/// Load the shared iteration detail payload used by both the full page and the
/// fragment handler.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection
/// * `id` - the iteration's id or a prefix that names one iteration
///
/// # Returns
///
/// The iteration with its rendered description, tags, phases, counts and timeline. Fails with `web::Error::NotFound`
/// when no iteration has the id, and with another `web::Error` when the store or the timeline fails.
async fn load_iteration_detail(state: &AppState, id: &str) -> Result<IterationDetailData, web::Error> {
  let conn = state.store().connect().await?;
  let iter_id = repo::resolve::resolve_id(&conn, repo::resolve::Table::Iterations, id).await?;
  let iteration = repo::iteration::find_by_id(&conn, iter_id.clone())
    .await?
    .ok_or(web::Error::NotFound)?;

  let tags = repo::tag::for_entity(&conn, EntityType::Iteration, &iter_id).await?;
  let tasks = repo::iteration::tasks_with_phase(&conn, &iter_id).await?;
  let status_counts = repo::iteration::task_status_counts(&conn, &iter_id).await?;

  let mut phase_map: BTreeMap<u32, Vec<IterationTaskRow>> = BTreeMap::new();
  for t in tasks {
    phase_map.entry(t.phase).or_default().push(t);
  }
  let phases: Vec<PhaseGroup> = phase_map
    .into_iter()
    .map(|(number, tasks)| PhaseGroup {
      number,
      tasks,
    })
    .collect();

  let task_count = status_counts.total;
  let description_html = markdown::render_markdown_to_html(iteration.description());
  let timeline_items = timeline::build_timeline(&conn, EntityType::Iteration, &iter_id)
    .await
    .map_err(web::Error::Internal)?;

  Ok(IterationDetailData {
    description_html,
    iteration,
    phases,
    status_counts,
    tags,
    task_count,
    timeline_items,
  })
}

/// Load the shared iteration list payload (rows, counts, current status filter)
/// used by both the full page and the fragment handler.
///
/// # Arguments
///
/// * `state` - the shared server state, for the store connection and the project id
/// * `status` - the status tab from the query, or `None` for `all`
///
/// # Returns
///
/// The rows for the tab and the count of iterations in each status. Fails with a `web::Error` when a store query fails.
async fn load_iteration_list(state: &AppState, status: Option<String>) -> Result<IterationListData, web::Error> {
  let conn = state.store().connect().await?;

  // Fetch all iterations to compute counts across every status.
  let all_iterations = repo::iteration::all(&conn, state.project_id(), &iteration::Filter::all()).await?;

  let active_count = all_iterations
    .iter()
    .filter(|i| i.status() == IterationStatus::Active)
    .count();
  let completed_count = all_iterations
    .iter()
    .filter(|i| i.status() == IterationStatus::Completed)
    .count();
  let cancelled_count = all_iterations
    .iter()
    .filter(|i| i.status() == IterationStatus::Cancelled)
    .count();

  let current_status = status.unwrap_or_else(|| "all".to_owned());

  // Filter iterations based on status param. Default (no param) shows every iteration;
  // a concrete status narrows to that status.
  let filter = match current_status.as_str() {
    "all" => iteration::Filter::all(),
    s => iteration::Filter {
      status: s.parse::<IterationStatus>().ok(),
      ..iteration::Filter::all()
    },
  };

  let iterations = repo::iteration::all(&conn, state.project_id(), &filter).await?;
  let rows = build_iteration_rows(&conn, iterations).await?;

  Ok(IterationListData {
    active_count,
    cancelled_count,
    completed_count,
    current_status,
    rows,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{self, model::Project};

  async fn setup() -> AppState {
    let (store, tmp) = store::open_temp().await.unwrap();
    let conn = store.connect().await.unwrap();
    let project = Project::new("/tmp/web-iter-test".into());
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

    // Seed two active, one completed, one cancelled.
    for (title, status) in [
      ("Active A", IterationStatus::Active),
      ("Active B", IterationStatus::Active),
      ("Completed", IterationStatus::Completed),
      ("Cancelled", IterationStatus::Cancelled),
    ] {
      let new = iteration::New {
        title: title.into(),
        ..Default::default()
      };
      let it = repo::iteration::create(&conn, &project_id, &new).await.unwrap();
      if status != IterationStatus::Active {
        repo::iteration::update(
          &conn,
          it.id(),
          &iteration::Patch {
            status: Some(status),
            ..Default::default()
          },
        )
        .await
        .unwrap();
      }
    }

    std::mem::forget(tmp);
    AppState::new(store, project_id)
  }

  mod load_iteration_board {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      model::{Project, iteration, primitives::Id},
      repo,
    };

    #[tokio::test]
    async fn it_buckets_tasks_by_status_including_in_progress() {
      let (store_arc, tmp) = store::open_temp().await.unwrap();
      let conn = store_arc.connect().await.unwrap();
      let project = Project::new("/tmp/web-iter-board".into());
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

      let iter = repo::iteration::create(
        &conn,
        &project_id,
        &iteration::New {
          title: "Sprint".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      for (title, status) in [
        ("Open task", "open"),
        ("In progress task", "in-progress"),
        ("Done task", "done"),
        ("Cancelled task", "cancelled"),
      ] {
        let task_id = Id::new();
        let params: [String; 4] = [
          task_id.to_string(),
          project_id.to_string(),
          title.to_string(),
          status.to_string(),
        ];
        conn
          .execute(
            "INSERT INTO tasks (id, project_id, title, status) VALUES (?1, ?2, ?3, ?4)",
            params,
          )
          .await
          .unwrap();
        repo::iteration::add_task(&conn, iter.id(), &task_id, 0).await.unwrap();
      }

      let state = AppState::new(store_arc, project_id);
      let data = load_iteration_board(&state, &iter.id().to_string()).await.unwrap();

      assert_eq!(data.open_tasks.len(), 1);
      assert_eq!(data.open_tasks[0].title, "Open task");

      assert_eq!(data.in_progress_tasks.len(), 1);
      assert_eq!(data.in_progress_tasks[0].title, "In progress task");

      assert_eq!(data.done_tasks.len(), 1);
      assert_eq!(data.done_tasks[0].title, "Done task");

      assert_eq!(data.cancelled_tasks.len(), 1);
      assert_eq!(data.cancelled_tasks[0].title, "Cancelled task");
    }

    #[tokio::test]
    async fn it_returns_not_found_for_an_unknown_iteration() {
      let state = setup().await;

      let result = load_iteration_board(&state, "kkkkkkkk").await;

      assert!(matches!(result, Err(web::Error::NotFound)));
    }
  }

  mod load_iteration_list {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_defaults_to_all_when_no_status_given() {
      let state = setup().await;

      let data = load_iteration_list(&state, None).await.unwrap();

      assert_eq!(data.current_status, "all");
      assert_eq!(data.rows.len(), 4);
      assert_eq!(
        (data.active_count, data.completed_count, data.cancelled_count),
        (2, 1, 1)
      );
    }

    #[tokio::test]
    async fn it_filters_to_a_specific_status() {
      let state = setup().await;

      let data = load_iteration_list(&state, Some("completed".into())).await.unwrap();

      assert_eq!(data.current_status, "completed");
      assert_eq!(data.rows.len(), 1);
      assert_eq!(data.rows[0].iteration.status(), IterationStatus::Completed);
    }

    #[tokio::test]
    async fn it_reports_counts_across_every_status_regardless_of_filter() {
      let state = setup().await;

      let a = load_iteration_list(&state, None).await.unwrap();
      let b = load_iteration_list(&state, Some("completed".into())).await.unwrap();

      assert_eq!((a.active_count, a.completed_count, a.cancelled_count), (2, 1, 1));
      assert_eq!((b.active_count, b.completed_count, b.cancelled_count), (2, 1, 1));
    }

    #[tokio::test]
    async fn it_returns_every_iteration_when_status_is_all() {
      let state = setup().await;

      let data = load_iteration_list(&state, Some("all".into())).await.unwrap();

      assert_eq!(data.current_status, "all");
      assert_eq!(data.rows.len(), 4);
      assert_eq!(
        (data.active_count, data.completed_count, data.cancelled_count),
        (2, 1, 1)
      );
    }
  }

  mod iteration_detail_timeline {
    use pretty_assertions::assert_eq;

    use crate::{
      store::{
        self,
        model::{Project, iteration, note, primitives::EntityType},
        repo,
      },
      web::timeline,
    };

    #[tokio::test]
    async fn it_merges_notes_and_semantic_events_in_chronological_order() {
      let (store_arc, tmp) = store::open_temp().await.unwrap();
      let conn = store_arc.connect().await.unwrap();
      let project = Project::new("/tmp/web-iter-timeline".into());
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

      let iter = repo::iteration::create(
        &conn,
        &project_id,
        &iteration::New {
          title: "Sprint".into(),
          ..Default::default()
        },
      )
      .await
      .unwrap();

      let tx = repo::transaction::begin(&conn, &project_id, "iteration create")
        .await
        .unwrap();
      repo::transaction::record_semantic_event(
        &conn,
        tx.id(),
        "iterations",
        &iter.id().to_string(),
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
        EntityType::Iteration,
        iter.id(),
        &note::New {
          body: "iteration note".into(),
          author_id: None,
        },
      )
      .await
      .unwrap();

      // Keep the store_arc alive for the query.
      let _ = store_arc.clone();
      let items = timeline::build_timeline(&conn, EntityType::Iteration, iter.id())
        .await
        .unwrap();

      assert_eq!(items.len(), 2);
      assert!(items[0].as_event().is_some());
      assert!(items[1].as_note().is_some());
    }
  }
}
