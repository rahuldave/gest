//! Embedded static assets served via rust-embed.

use axum::{
  extract::Path,
  http::{StatusCode, header},
  response::{IntoResponse, Response},
};
use rust_embed::Embed;

/// The `.css` and `.js` files under `templates/`, embedded in the binary at build time.
#[derive(Embed)]
#[folder = "templates/"]
#[include = "*.css"]
#[include = "*.js"]
struct StaticAssets;

/// Serve an embedded static asset by path.
///
/// # Arguments
///
/// * `path` - the asset's path under `templates/`, from the `/static/{*path}` route
///
/// # Returns
///
/// The asset's bytes with a `Content-Type` from its extension, or an empty 404 response when no
/// embedded asset has that path.
pub async fn serve(Path(path): Path<String>) -> Response {
  match StaticAssets::get(&path) {
    Some(file) => {
      let mime = mime_for(&path);
      ([(header::CONTENT_TYPE, mime)], file.data).into_response()
    }
    None => StatusCode::NOT_FOUND.into_response(),
  }
}

/// Infer a MIME type from the file extension.
///
/// # Arguments
///
/// * `path` - the asset's path
///
/// # Returns
///
/// The UTF-8 CSS or JavaScript type for `.css` and `.js`, and `application/octet-stream` for any
/// other extension.
fn mime_for(path: &str) -> &'static str {
  if path.ends_with(".css") {
    "text/css; charset=utf-8"
  } else if path.ends_with(".js") {
    "application/javascript; charset=utf-8"
  } else {
    "application/octet-stream"
  }
}
