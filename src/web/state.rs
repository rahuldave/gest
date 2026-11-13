//! Shared application state handed to every axum handler.

use std::sync::Arc;

use getset::Getters;
use tokio::sync::broadcast::Sender;

use crate::store::{Db, avatar_cache::AvatarCache, model::primitives::Id};

/// Shared state for the web server.
#[derive(Clone, Getters)]
pub struct AppState {
  /// The author recorded on notes added through the dashboard, resolved from git at startup.
  #[get = "pub"]
  author_id: Option<Id>,
  /// The on-disk cache that the `/avatars/{hash}` handler serves avatars from.
  #[get = "pub"]
  avatar_cache: Arc<AvatarCache>,
  /// The project whose entities the dashboard shows.
  #[get = "pub"]
  project_id: Id,
  /// The broadcast channel that wakes `/events` subscribers so browser tabs reload.
  #[get = "pub"]
  reload_tx: Sender<()>,
  /// The database the handlers read and write.
  #[get = "pub"]
  store: Arc<Db>,
}

impl AppState {
  /// Create a new web server state.
  ///
  /// The avatar cache defaults to a process-scoped temp directory; production
  /// callers should override it via [`Self::with_avatar_cache`] so entries are
  /// persisted under the user's configured `storage.cache_dir`.
  ///
  /// The state starts with no author and a new reload channel that buffers 16 signals.
  ///
  /// # Arguments
  ///
  /// * `store` - the database the handlers read and write
  /// * `project_id` - the project whose entities the dashboard shows
  ///
  /// # Returns
  ///
  /// The state, with the avatar cache in `gest-avatar-cache` under the system temp directory.
  pub fn new(store: Arc<Db>, project_id: Id) -> Self {
    let (reload_tx, _) = tokio::sync::broadcast::channel(16);
    let avatar_cache = Arc::new(AvatarCache::new(std::env::temp_dir().join("gest-avatar-cache")));
    Self {
      author_id: None,
      avatar_cache,
      project_id,
      reload_tx,
      store,
    }
  }

  /// Set the resolved author ID.
  ///
  /// # Arguments
  ///
  /// * `id` - the author recorded on notes added through the dashboard
  ///
  /// # Returns
  ///
  /// The state with the author set.
  pub fn with_author_id(mut self, id: Id) -> Self {
    self.author_id = Some(id);
    self
  }

  /// Set the avatar cache used by the `/avatars/:hash` handler.
  ///
  /// # Arguments
  ///
  /// * `cache` - the avatar cache to use instead of the default temp directory one
  ///
  /// # Returns
  ///
  /// The state with the cache replaced.
  pub fn with_avatar_cache(mut self, cache: Arc<AvatarCache>) -> Self {
    self.avatar_cache = cache;
    self
  }
}
