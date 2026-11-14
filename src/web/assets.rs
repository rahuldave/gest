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

#[cfg(test)]
mod tests {
  use axum::body::to_bytes;

  use super::*;

  #[tokio::test]
  async fn it_serves_the_scripts_that_base_html_loads() {
    for (path, needle) in [("forms.js", "md-preview-btn"), ("live.js", "EventSource")] {
      let response = serve(Path(path.to_owned())).await;

      assert_eq!(response.status(), StatusCode::OK, "{path}");
      assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/javascript; charset=utf-8"
      );
      let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
      assert!(String::from_utf8_lossy(&body).contains(needle), "{path}");
    }
  }
}
