//! Miscellaneous JSON/HTML API endpoints that are not tied to a single domain.

use axum::{
  Form,
  http::header::CONTENT_TYPE,
  response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::web::markdown;

/// Request body for the render-markdown endpoint, sent as a form.
///
/// The form also carries the `_csrf` field, which the CSRF layer checks before this handler runs; serde ignores it.
#[derive(Deserialize)]
pub struct RenderMarkdownBody {
  /// Raw Markdown source to render.
  pub body: String,
}

/// POST /api/render-markdown — render Markdown to HTML.
///
/// The editor's live preview posts the draft as an `application/x-www-form-urlencoded` body with a `body` field and
/// the `_csrf` field, read from the page's `csrf-token` meta tag, so the CSRF layer accepts it.
///
/// # Arguments
///
/// * `payload` - the form holding the Markdown source
///
/// # Returns
///
/// A `200` response with the rendered HTML as `text/html; charset=utf-8`. The `Form` extractor rejects a body that
/// is not a form with a `body` field before the handler runs.
pub async fn api_render_markdown(Form(payload): Form<RenderMarkdownBody>) -> Response {
  let html_output = markdown::render_markdown_to_html(&payload.body);
  ([(CONTENT_TYPE, "text/html; charset=utf-8")], html_output).into_response()
}
