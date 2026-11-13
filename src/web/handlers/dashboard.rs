//! Dashboard page handlers.

use askama::Template;
use axum::{extract::State, response::Html};

use crate::{
  store::{
    model::{
      artifact, iteration,
      primitives::{IterationStatus, TaskStatus},
      task,
    },
    repo,
  },
  web::{self, AppState},
};

/// The dashboard's content alone, which the live-reload script swaps in.
#[derive(Template)]
#[template(path = "dashboard_content.html")]
struct DashboardFragmentTemplate {
  /// Iterations whose status is active.
  active_iteration_count: usize,
  /// Artifacts in the project that are not archived.
  artifact_count: usize,
  /// Tasks whose status is cancelled.
  cancelled_count: usize,
  /// Iterations whose status is cancelled.
  cancelled_iteration_count: usize,
  /// Iterations whose status is completed.
  completed_iteration_count: usize,
  /// Tasks whose status is done.
  done_count: usize,
  /// Tasks whose status is in progress.
  in_progress_count: usize,
  /// Iterations shown in the summary card; the active ones only, the same number as `active_iteration_count`.
  iteration_count: usize,
  /// Tasks whose status is open.
  open_count: usize,
  /// Tasks still to finish: the open ones plus the in-progress ones.
  task_count: usize,
}

/// The full dashboard page, with the layout around the counts.
#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
  /// Iterations whose status is active.
  active_iteration_count: usize,
  /// Artifacts in the project that are not archived.
  artifact_count: usize,
  /// Tasks whose status is cancelled.
  cancelled_count: usize,
  /// Iterations whose status is cancelled.
  cancelled_iteration_count: usize,
  /// Iterations whose status is completed.
  completed_iteration_count: usize,
  /// Tasks whose status is done.
  done_count: usize,
  /// Tasks whose status is in progress.
  in_progress_count: usize,
  /// Iterations shown in the summary card; the active ones only, the same number as `active_iteration_count`.
  iteration_count: usize,
  /// Tasks whose status is open.
  open_count: usize,
  /// Tasks still to finish: the open ones plus the in-progress ones.
  task_count: usize,
}

/// Dashboard page showing project summary.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store and the project id
///
/// # Returns
///
/// The rendered page, or a [`web::Error`] when the store or the template fails.
pub async fn dashboard(State(state): State<AppState>) -> Result<Html<String>, web::Error> {
  let conn = state.store().connect().await?;
  let pid = state.project_id();

  let tasks = repo::task::all(&conn, pid, &task::Filter::all()).await?;
  let artifacts = repo::artifact::all(&conn, pid, &Default::default()).await?;
  let iterations = repo::iteration::all(&conn, pid, &iteration::Filter::all()).await?;

  let (
    active_iteration_count,
    artifact_count,
    cancelled_count,
    cancelled_iteration_count,
    completed_iteration_count,
    done_count,
    in_progress_count,
    iteration_count,
    open_count,
    task_count,
  ) = dashboard_counts(&tasks, &artifacts, &iterations);

  let tmpl = DashboardTemplate {
    active_iteration_count,
    artifact_count,
    cancelled_count,
    cancelled_iteration_count,
    completed_iteration_count,
    done_count,
    in_progress_count,
    iteration_count,
    open_count,
    task_count,
  };
  Ok(Html(tmpl.render()?))
}

/// Dashboard content fragment for live reload.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store and the project id
///
/// # Returns
///
/// The rendered fragment, without the page layout, or a [`web::Error`] when the store or the template fails.
pub async fn dashboard_fragment(State(state): State<AppState>) -> Result<Html<String>, web::Error> {
  let conn = state.store().connect().await?;
  let pid = state.project_id();

  let tasks = repo::task::all(&conn, pid, &task::Filter::all()).await?;
  let artifacts = repo::artifact::all(&conn, pid, &Default::default()).await?;
  let iterations = repo::iteration::all(&conn, pid, &iteration::Filter::all()).await?;

  let (
    active_iteration_count,
    artifact_count,
    cancelled_count,
    cancelled_iteration_count,
    completed_iteration_count,
    done_count,
    in_progress_count,
    iteration_count,
    open_count,
    task_count,
  ) = dashboard_counts(&tasks, &artifacts, &iterations);

  let tmpl = DashboardFragmentTemplate {
    active_iteration_count,
    artifact_count,
    cancelled_count,
    cancelled_iteration_count,
    completed_iteration_count,
    done_count,
    in_progress_count,
    iteration_count,
    open_count,
    task_count,
  };
  Ok(Html(tmpl.render()?))
}

/// Compute dashboard status counts from task, artifact, and iteration lists.
///
/// # Arguments
///
/// * `tasks` - every task of the project
/// * `artifacts` - the artifacts of the project that are not archived
/// * `iterations` - every iteration of the project
///
/// # Returns
///
/// The counts in the order of the template fields: active iterations, artifacts, cancelled tasks, cancelled
/// iterations, completed iterations, done tasks, in-progress tasks, iterations (the active count again), open
/// tasks, and unfinished tasks (open plus in progress).
fn dashboard_counts(
  tasks: &[task::Model],
  artifacts: &[artifact::Model],
  iterations: &[iteration::Model],
) -> (usize, usize, usize, usize, usize, usize, usize, usize, usize, usize) {
  let mut open_count = 0;
  let mut in_progress_count = 0;
  let mut done_count = 0;
  let mut cancelled_count = 0;
  for t in tasks {
    match t.status() {
      TaskStatus::Open => open_count += 1,
      TaskStatus::InProgress => in_progress_count += 1,
      TaskStatus::Done => done_count += 1,
      TaskStatus::Cancelled => cancelled_count += 1,
    }
  }

  let mut active_iteration_count = 0;
  let mut completed_iteration_count = 0;
  let mut cancelled_iteration_count = 0;
  for i in iterations {
    match i.status() {
      IterationStatus::Active => active_iteration_count += 1,
      IterationStatus::Completed => completed_iteration_count += 1,
      IterationStatus::Cancelled => cancelled_iteration_count += 1,
    }
  }

  (
    active_iteration_count,
    artifacts.len(),
    cancelled_count,
    cancelled_iteration_count,
    completed_iteration_count,
    done_count,
    in_progress_count,
    active_iteration_count,
    open_count,
    open_count + in_progress_count,
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{
    self,
    model::{Project, iteration as iter_model, task as task_model},
  };

  mod dashboard_counts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_includes_terminal_state_tasks_and_iterations() {
      let (store_arc, tmp) = store::open_temp().await.unwrap();
      let conn = store_arc.connect().await.unwrap();
      let project = Project::new("/tmp/web-dashboard-counts".into());
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
      let pid = project.id().clone();
      std::mem::forget(tmp);

      for (title, status) in [
        ("Open", TaskStatus::Open),
        ("In progress", TaskStatus::InProgress),
        ("Done", TaskStatus::Done),
        ("Cancelled", TaskStatus::Cancelled),
      ] {
        let new = task_model::New {
          title: title.into(),
          status: Some(status),
          ..Default::default()
        };
        repo::task::create(&conn, &pid, &new).await.unwrap();
      }

      for (title, status) in [
        ("Active", IterationStatus::Active),
        ("Completed", IterationStatus::Completed),
        ("Cancelled", IterationStatus::Cancelled),
      ] {
        let it = repo::iteration::create(
          &conn,
          &pid,
          &iter_model::New {
            title: title.into(),
            ..Default::default()
          },
        )
        .await
        .unwrap();
        if status != IterationStatus::Active {
          repo::iteration::update(
            &conn,
            it.id(),
            &iter_model::Patch {
              status: Some(status),
              ..Default::default()
            },
          )
          .await
          .unwrap();
        }
      }

      let tasks = repo::task::all(&conn, &pid, &task::Filter::all()).await.unwrap();
      let artifacts = repo::artifact::all(&conn, &pid, &Default::default()).await.unwrap();
      let iterations = repo::iteration::all(&conn, &pid, &iteration::Filter::all())
        .await
        .unwrap();

      let (
        active_iteration_count,
        _artifact_count,
        cancelled_count,
        cancelled_iteration_count,
        completed_iteration_count,
        done_count,
        in_progress_count,
        iteration_count,
        open_count,
        task_count,
      ) = dashboard_counts(&tasks, &artifacts, &iterations);

      assert_eq!(task_count, 2);
      assert_eq!(open_count, 1);
      assert_eq!(in_progress_count, 1);
      assert_eq!(done_count, 1);
      assert_eq!(cancelled_count, 1);
      assert_eq!(iteration_count, 1);
      assert_eq!(active_iteration_count, 1);
      assert_eq!(completed_iteration_count, 1);
      assert_eq!(cancelled_iteration_count, 1);
    }
  }
}
