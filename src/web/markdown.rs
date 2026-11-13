//! Markdown-to-HTML renderer for the web layer using pulldown-cmark.

use pulldown_cmark::{Options, Parser, html};

use crate::web::{csrf::CSRF_TOKEN_PLACEHOLDER, nonce::NONCE_PLACEHOLDER};

/// Convert a markdown string to HTML suitable for template rendering.
///
/// Tables, strikethrough and task lists are on. Raw HTML in the input passes through as pulldown-cmark writes
/// it. The page middleware later replaces [`NONCE_PLACEHOLDER`] and [`CSRF_TOKEN_PLACEHOLDER`] in every HTML
/// response, so this function writes the first `_` of `__CSP_NONCE` and `__CSRF_TOKEN` in the output as `&#95;`.
/// The reader still sees the same text, but a `<script nonce="__CSP_NONCE__">` in a note can no longer receive the
/// page's nonce and get past the Content-Security-Policy, and the CSRF token does not appear in rendered Markdown.
/// Other user fields, such as titles and edit forms, are not covered here.
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
  neutralize_placeholders(html_output)
}

/// Write the first `_` of each rewrite placeholder as `&#95;`, so the page middleware does not replace it.
///
/// The pattern replaced is the placeholder without its closing `__`, such as `__CSP_NONCE`. Two placeholders
/// can share underscores (`__CSP_NONCE__CSP_NONCE__`), and [`str::replace`] skips overlapping matches. The
/// shorter pattern cannot overlap itself, and its replacement starts with `&`, so no new match can form.
///
/// # Arguments
///
/// * `html` - rendered user content
///
/// # Returns
///
/// The same HTML, with no exact copy of a placeholder left in it.
fn neutralize_placeholders(html: String) -> String {
  [NONCE_PLACEHOLDER, CSRF_TOKEN_PLACEHOLDER]
    .iter()
    .fold(html, |out, placeholder| {
      let pattern = placeholder.trim_end_matches('_');
      out.replace(pattern, &format!("&#95;{}", &pattern[1..]))
    })
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
  use crate::web::{csrf::CSRF_TOKEN_PLACEHOLDER, nonce, nonce::NONCE_PLACEHOLDER, security_headers};

  const HOSTILE: &str = "<script nonce=\"__CSP_NONCE__\">alert(1)</script>\n\nform token: __CSRF_TOKEN__";

  #[test]
  fn it_leaves_no_rewrite_placeholder_in_user_content() {
    let html = render_markdown_to_html(HOSTILE);

    assert!(!html.contains(NONCE_PLACEHOLDER), "{html}");
    assert!(!html.contains(CSRF_TOKEN_PLACEHOLDER), "{html}");
  }

  #[test]
  fn it_leaves_no_placeholder_when_two_share_their_underscores() {
    let html = render_markdown_to_html(
      "<script nonce=\"__CSP_NONCE__CSP_NONCE__\">x</script>\n\n<p title=\"__CSRF_TOKEN__CSRF_TOKEN__\">y</p>",
    );

    assert!(!html.contains(NONCE_PLACEHOLDER), "{html}");
    assert!(!html.contains(CSRF_TOKEN_PLACEHOLDER), "{html}");
  }

  #[test]
  fn it_still_shows_the_placeholder_text_to_the_reader() {
    let html = render_markdown_to_html("`__CSP_NONCE__`");

    assert!(html.contains("&#95;_CSP_NONCE__"), "{html}");
  }

  #[tokio::test]
  async fn it_does_not_give_a_script_in_user_content_the_page_nonce() {
    async fn page() -> Html<String> {
      Html(format!(
        "<html><body><script nonce=\"__CSP_NONCE__\">1;</script>{}</body></html>",
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
