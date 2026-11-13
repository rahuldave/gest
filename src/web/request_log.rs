//! Request logging middleware.
//!
//! Logs each HTTP request with method, path, status, and elapsed time.

use std::time::{Duration, Instant};

use axum::{extract::Request, middleware::Next, response::Response};

/// Middleware that logs each request on completion.
///
/// The line is logged at info level as `<method> <path> <status> <elapsed>`, such as
/// `GET /tasks 200 12ms`; the query string is not logged.
///
/// # Arguments
///
/// * `request` - the incoming request
/// * `next` - the rest of the middleware stack and the handler
///
/// # Returns
///
/// The handler's response, unchanged.
pub async fn log_request(request: Request, next: Next) -> Response {
  let method = request.method().clone();
  let path = request.uri().path().to_owned();
  let start = Instant::now();

  let response = next.run(request).await;

  let status = response.status().as_u16();
  let elapsed = format_duration(start.elapsed());
  log::info!("{method} {path} {status} {elapsed}");

  response
}

/// Format an elapsed time for the request log: whole milliseconds under a second, else seconds.
///
/// # Arguments
///
/// * `d` - the elapsed time
///
/// # Returns
///
/// Text such as `12ms` or `1.5s`.
fn format_duration(d: Duration) -> String {
  let ms = d.as_millis();
  if ms < 1000 {
    format!("{ms}ms")
  } else {
    format!("{:.1}s", d.as_secs_f64())
  }
}
