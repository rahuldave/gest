//! Server-sent events endpoint that pushes reload pings to connected browser tabs.

use std::convert::Infallible;

use axum::{
  extract::State,
  response::sse::{Event, KeepAlive, Sse},
};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use super::AppState;

/// SSE endpoint that streams named `ping` events to connected clients.
///
/// Each item from the state's broadcast channel, including a lag notice, becomes one `ping` event with the data
/// `reload`; the stream also sends keep-alive comments.
///
/// # Arguments
///
/// * `state` - the shared state, whose reload channel the stream subscribes to
///
/// # Returns
///
/// The event stream for `/events`.
pub async fn events(State(state): State<AppState>) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
  let rx = state.reload_tx().subscribe();
  let stream = BroadcastStream::new(rx).map(|_| Ok(Event::default().event("ping").data("reload")));
  Sse::new(stream).keep_alive(KeepAlive::default())
}
