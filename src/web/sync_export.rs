//! Export to `.gest/` after each web write.
//!
//! Every CLI command imports `.gest/` into the database before it runs, and exports the database back when it exits.
//! `gest serve` runs as one long command, so without this layer its writes reach only the database, and the next CLI
//! command's import puts the older `.gest/` files back over them.

use axum::{
  extract::{Request, State},
  http::Method,
  middleware::Next,
  response::Response,
};

use super::state::AppState;

/// Middleware that exports the database to `.gest/` after a write request succeeds.
///
/// A write is any request that is not `GET` or `HEAD`, except the Markdown preview, which only renders. It succeeds
/// when the handler answers with a success or redirect status; a rejected write saves nothing, so it exports nothing.
/// The export runs before the response is sent, so a CLI command started after the browser shows the result sees the
/// edit. It does nothing when sync is off or the project has no `.gest/` directory, and a failed export is logged as a
/// warning, as at the end of a CLI command.
///
/// # Arguments
///
/// * `state` - the shared state, whose store holds the sync settings
/// * `request` - the incoming request
/// * `next` - the rest of the middleware stack and the handler
///
/// # Returns
///
/// The handler's response, unchanged.
pub async fn export_after_write(State(state): State<AppState>, request: Request, next: Next) -> Response {
  let writes =
    !matches!(*request.method(), Method::GET | Method::HEAD) && request.uri().path() != "/api/render-markdown";
  let response = next.run(request).await;
  let status = response.status();
  if writes
    && (status.is_success() || status.is_redirection())
    && let Err(e) = state.store().export_if_needed().await
  {
    log::warn!("sync export after web write failed: {e}");
  }
  response
}
