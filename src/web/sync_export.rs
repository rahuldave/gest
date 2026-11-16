//! Export to `.gest/` after each web write.
//!
//! Every CLI command imports `.gest/` into the database before it runs, and exports the database back when it exits.
//! `gest serve` runs as one long command, so without this layer its writes reach only the database, and the next CLI
//! command's import puts the older `.gest/` files back over them.

use axum::{
  extract::{Request, State},
  http::{Method, StatusCode},
  middleware::Next,
  response::{IntoResponse, Response},
};

use super::state::AppState;

/// Middleware that keeps `.gest/` and the database in step around each web write.
///
/// A write is any request that is not `GET` or `HEAD`, except the Markdown preview, which only renders. For a write it
/// holds the store's web write lock, imports `.gest/` first, so files a `git pull` added while serve runs are in the
/// database, runs the handler, and then exports. It exports whatever the handler answered, because a handler can fail
/// after saving part of a write. When the import or export fails, it answers 500 instead of the handler's response, so
/// the browser does not show a save that the next CLI command would undo. It does nothing more when sync is off.
///
/// # Arguments
///
/// * `state` - the shared state, whose store holds the sync settings
/// * `request` - the incoming request
/// * `next` - the rest of the middleware stack and the handler
///
/// # Returns
///
/// The handler's response, or a 500 response when syncing `.gest/` failed.
pub async fn export_after_write(State(state): State<AppState>, request: Request, next: Next) -> Response {
  let writes =
    !matches!(*request.method(), Method::GET | Method::HEAD) && request.uri().path() != "/api/render-markdown";
  if !writes {
    return next.run(request).await;
  }
  let store = state.store();
  let _guard = store.web_write_lock().await;
  if let Err(e) = store.import_now().await {
    log::warn!("sync import before web write failed: {e}");
    return sync_failed("read", &e);
  }
  let response = next.run(request).await;
  if let Err(e) = store.export_now().await {
    log::warn!("sync export after web write failed: {e}");
    return sync_failed("write", &e);
  }
  response
}

/// A 500 response saying that syncing `.gest/` failed.
fn sync_failed(verb: &str, error: &dyn std::fmt::Display) -> Response {
  (
    StatusCode::INTERNAL_SERVER_ERROR,
    format!("gest could not {verb} the .gest/ directory: {error}. The change may be lost on the next gest command."),
  )
    .into_response()
}
