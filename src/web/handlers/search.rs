//! Search handlers (HTML page + JSON API).

use askama::Template;
use axum::{
  extract::{Query, State},
  http::StatusCode,
  response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::{
  store::{
    model::{artifact, iteration, task},
    repo, search_query,
  },
  web::{self, AppState},
};

/// Query parameters for the JSON search API.
#[derive(Deserialize)]
pub struct ApiSearchParams {
  /// The search text in gest's query syntax; empty or missing gives an empty list.
  q: Option<String>,
}

/// Query parameters for the HTML search page.
#[derive(Deserialize)]
pub struct SearchQuery {
  /// The search text in gest's query syntax; empty or missing shows the page with no results.
  q: Option<String>,
}

/// JSON result returned from `/api/search`.
#[derive(Serialize)]
struct ApiSearchResult {
  /// The full id of the entity.
  id: String,
  /// The entity type, `task`, `artifact` or `iteration`, serialized as `type`.
  #[serde(rename = "type")]
  kind: String,
  /// The short id prefix that the UI displays.
  short_id: String,
  /// The entity's title, shown in the relationship picker.
  title: String,
}

/// The search page, with the query box and its results grouped by entity type.
#[derive(Template)]
#[template(path = "search.html")]
struct SearchTemplate {
  /// Artifacts that match the query; empty when the query is empty.
  artifacts: Vec<artifact::Model>,
  /// Iterations that match the query; empty when the query is empty.
  iterations: Vec<iteration::Model>,
  /// The search text, shown back in the query box.
  query: String,
  /// Tasks that match the query; empty when the query is empty.
  tasks: Vec<task::Model>,
}

/// GET /api/search?q=... — JSON search results for the relationship picker.
///
/// The search passes `show_all`, so resolved tasks, archived artifacts and completed or cancelled iterations match too.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store and the project id
/// * `params` - the query string, holding the search text
///
/// # Returns
///
/// A JSON array of matches, tasks first, then artifacts, then iterations; an empty array for empty text, and an
/// empty array with `500` when the store fails.
pub async fn api_search(State(state): State<AppState>, Query(params): Query<ApiSearchParams>) -> Response {
  let query = params.q.unwrap_or_default();
  if query.is_empty() {
    return axum::Json(Vec::<ApiSearchResult>::new()).into_response();
  }

  let conn = match state.store().connect().await {
    Ok(c) => c,
    Err(e) => {
      log::error!("api search connect failed: {e}");
      return (
        StatusCode::INTERNAL_SERVER_ERROR,
        axum::Json(Vec::<ApiSearchResult>::new()),
      )
        .into_response();
    }
  };

  let parsed = search_query::parse(&query);
  let results = match repo::search::query(&conn, state.project_id(), &parsed, true).await {
    Ok(r) => r,
    Err(e) => {
      log::error!("api search failed: {e}");
      return (
        StatusCode::INTERNAL_SERVER_ERROR,
        axum::Json(Vec::<ApiSearchResult>::new()),
      )
        .into_response();
    }
  };

  let mut items: Vec<ApiSearchResult> = Vec::new();
  for task in results.tasks {
    items.push(ApiSearchResult {
      id: task.id().to_string(),
      kind: "task".to_string(),
      short_id: task.id().short(),
      title: task.title().to_owned(),
    });
  }
  for artifact in results.artifacts {
    items.push(ApiSearchResult {
      id: artifact.id().to_string(),
      kind: "artifact".to_string(),
      short_id: artifact.id().short(),
      title: artifact.title().to_owned(),
    });
  }
  for iteration in results.iterations {
    items.push(ApiSearchResult {
      id: iteration.id().to_string(),
      kind: "iteration".to_string(),
      short_id: iteration.id().short(),
      title: iteration.title().to_owned(),
    });
  }

  axum::Json(items).into_response()
}

/// Search page.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store and the project id
/// * `params` - the query string, holding the search text
///
/// # Returns
///
/// The rendered page with the matching tasks, artifacts and iterations, or a [`web::Error`] when the store or the
/// template fails.
pub async fn search(
  State(state): State<AppState>,
  Query(params): Query<SearchQuery>,
) -> Result<Html<String>, web::Error> {
  let conn = state.store().connect().await?;
  let query = params.q.unwrap_or_default();

  let (tasks, artifacts, iterations) = if query.is_empty() {
    (Vec::new(), Vec::new(), Vec::new())
  } else {
    let parsed = search_query::parse(&query);
    let results = repo::search::query(&conn, state.project_id(), &parsed, true).await?;
    (results.tasks, results.artifacts, results.iterations)
  };

  let tmpl = SearchTemplate {
    artifacts,
    iterations,
    query,
    tasks,
  };
  Ok(Html(tmpl.render()?))
}
