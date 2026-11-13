//! Miscellaneous JSON/HTML API endpoints that are not tied to a single domain.

use axum::{
  Json,
  http::header::CONTENT_TYPE,
  response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::web::markdown;

/// Request body for the render-markdown endpoint.
#[derive(Deserialize)]
pub struct RenderMarkdownBody {
  /// Raw Markdown source to render.
  pub body: String,
}

/// POST /api/render-markdown — render Markdown to HTML.
///
/// The editor's live preview calls it with the draft body as JSON. That call carries no `_csrf` form field, so the
/// CSRF layer in front of the router rejects it with `403` before this handler runs.
///
/// # Arguments
///
/// * `payload` - the JSON body holding the Markdown source
///
/// # Returns
///
/// A `200` response with the rendered HTML as `text/html; charset=utf-8`. The `Json` extractor rejects a body that
/// is not JSON of this shape before the handler runs.
pub async fn api_render_markdown(Json(payload): Json<RenderMarkdownBody>) -> Response {
  let html_output = markdown::render_markdown_to_html(&payload.body);
  ([(CONTENT_TYPE, "text/html; charset=utf-8")], html_output).into_response()
}
