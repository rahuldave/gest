//! Markdown-to-HTML renderer for the web layer using pulldown-cmark.

use pulldown_cmark::{Options, Parser, html};

/// Convert a markdown string to HTML suitable for template rendering.
///
/// Tables, strikethrough and task lists are on. Raw HTML in the input passes through as pulldown-cmark writes
/// it. The page middleware does not write the nonce or the CSRF token into user text, because the placeholders it
/// replaces are random in each process; see [`crate::web::nonce::nonce_placeholder`]. Raw HTML is not sanitized,
/// so a script that the Content-Security-Policy allows can still run; task lxpqmxto tracks that.
///
/// # Arguments
///
/// * `text` - the Markdown source
///
/// # Returns
///
/// The rendered HTML fragment.
pub fn render_markdown_to_html(text: &str) -> String {
  let mut opts = Options::empty();
  opts.insert(Options::ENABLE_TABLES);
  opts.insert(Options::ENABLE_STRIKETHROUGH);
  opts.insert(Options::ENABLE_TASKLISTS);
  let parser = Parser::new_ext(text, opts);
  let mut html_output = String::new();
  html::push_html(&mut html_output, parser);
  html_output
}

#[cfg(test)]
mod tests {
  use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request as HttpRequest, header},
    middleware,
    response::Html,
    routing::get,
  };
  use tower::ServiceExt;

  use super::*;
  use crate::web::{nonce, security_headers};

  const HOSTILE: &str = "<script nonce=\"__CSP_NONCE__\">alert(1)</script>\n\nform token: __CSRF_TOKEN__";

  #[test]
  fn it_shows_placeholder_like_text_as_written() {
    let html = render_markdown_to_html("`__CSP_NONCE__`");

    assert!(html.contains("__CSP_NONCE__"), "{html}");
  }

  #[tokio::test]
  async fn it_does_not_give_a_script_in_user_content_the_page_nonce() {
    async fn page() -> Html<String> {
      Html(format!(
        "<html><body><script nonce=\"{}\">1;</script>{}</body></html>",
        nonce::nonce_placeholder(),
        render_markdown_to_html(HOSTILE)
      ))
    }
    let app = Router::new()
      .route("/", get(page))
      .layer(middleware::from_fn(security_headers::add_security_headers))
      .layer(middleware::from_fn(nonce::attach_nonce));

    let response = app
      .oneshot(HttpRequest::builder().uri("/").body(Body::empty()).unwrap())
      .await
      .unwrap();
    let csp = response
      .headers()
      .get(header::CONTENT_SECURITY_POLICY)
      .unwrap()
      .to_str()
      .unwrap()
      .to_owned();
    let start = csp.find("'nonce-").unwrap() + "'nonce-".len();
    let nonce = &csp[start..start + csp[start..].find('\'').unwrap()];
    let body = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();

    assert_eq!(
      body.matches(nonce).count(),
      1,
      "only the page's own script carries the nonce: {body}"
    );
  }
}
