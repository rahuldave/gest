//! Tags and their attachment to entities through the `entity_tags` join table.
//!
//! Tags are global, keyed by a unique label, and shared by every project. Detaching a tag removes only
//! the join row, never the tag.

use std::{collections::HashMap, str::FromStr};

use chrono::Utc;
use libsql::{Connection, Value};

use crate::store::{
  Error,
  model::{
    Tag,
    primitives::{EntityType, Id},
  },
};

/// The columns of `tags` in the order that the tag model's `TryFrom<Row>` reads them.
const SELECT_COLUMNS: &str = "id, label, created_at, updated_at";

/// Return all tags ordered by label.
///
/// # Arguments
///
/// * `conn` - the database connection
///
/// # Returns
///
/// The tags, attached or not; an error when the query or row decoding fails.
pub async fn all(conn: &Connection) -> Result<Vec<Tag>, Error> {
  log::debug!("repo::tag::all");
  let mut rows = conn
    .query(&format!("SELECT {SELECT_COLUMNS} FROM tags ORDER BY label"), ())
    .await?;

  let mut tags = Vec::new();
  while let Some(row) = rows.next().await? {
    tags.push(Tag::try_from(row)?);
  }
  Ok(tags)
}

/// Attach a tag to an entity. Creates the tag if it doesn't exist.
///
/// Attaching a tag that is already attached is a no-op (`INSERT OR IGNORE`).
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `entity_type` - the kind of the entity
/// * `entity_id` - the entity to tag
/// * `label` - the tag's label
///
/// # Returns
///
/// The existing or newly created tag, or an error when a query fails.
pub async fn attach(conn: &Connection, entity_type: EntityType, entity_id: &Id, label: &str) -> Result<Tag, Error> {
  log::debug!("repo::tag::attach");
  Ok(insert_link(conn, entity_type, entity_id, label).await?.0)
}

/// Attach a tag to an entity as part of a transaction, so that undo removes the link.
///
/// The `created` event on `entity_tags` is recorded only when the link is new, with the link's key columns as
/// its `before_data`. A link that was already there is left out of the transaction, so undo keeps it.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `transaction_id` - the transaction the event belongs to
/// * `entity_type` - the kind of the entity
/// * `entity_id` - the entity to tag
/// * `label` - the tag's label
///
/// # Returns
///
/// The existing or newly created tag, or an error when a query fails.
pub async fn attach_recorded(
  conn: &Connection,
  transaction_id: &Id,
  entity_type: EntityType,
  entity_id: &Id,
  label: &str,
) -> Result<Tag, Error> {
  log::debug!("repo::tag::attach_recorded");
  let (tag, inserted) = insert_link(conn, entity_type, entity_id, label).await?;
  if inserted {
    let key = serde_json::json!({
      "entity_type": entity_type.to_string(),
      "entity_id": entity_id.to_string(),
      "tag_id": tag.id().to_string(),
    });
    super::transaction::record_event(
      conn,
      transaction_id,
      "entity_tags",
      &tag.id().to_string(),
      "created",
      Some(&key),
    )
    .await?;
  }
  Ok(tag)
}

/// Finds or creates the tag and inserts the link with `INSERT OR IGNORE`.
///
/// # Returns
///
/// The tag, and whether a new link row was inserted.
async fn insert_link(
  conn: &Connection,
  entity_type: EntityType,
  entity_id: &Id,
  label: &str,
) -> Result<(Tag, bool), Error> {
  let tag = find_or_create(conn, label).await?;
  let inserted = conn
    .execute(
      "INSERT OR IGNORE INTO entity_tags (entity_type, entity_id, tag_id, created_at) VALUES (?1, ?2, ?3, ?4)",
      [
        entity_type.to_string(),
        entity_id.to_string(),
        tag.id().to_string(),
        Utc::now().to_rfc3339(),
      ],
    )
    .await?;
  Ok((tag, inserted > 0))
}

/// Return all distinct tags attached to entities of the given type, ordered by label.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `entity_type` - the kind of entity whose tags to collect
///
/// # Returns
///
/// The tags; an error when the query or row decoding fails.
pub async fn by_entity_type(conn: &Connection, entity_type: EntityType) -> Result<Vec<Tag>, Error> {
  log::debug!("repo::tag::by_entity_type");
  let mut rows = conn
    .query(
      "SELECT DISTINCT t.id, t.label, t.created_at, t.updated_at FROM tags t \
        INNER JOIN entity_tags et ON et.tag_id = t.id \
        WHERE et.entity_type = ?1 \
        ORDER BY t.label",
      [entity_type.to_string()],
    )
    .await?;

  let mut tags = Vec::new();
  while let Some(row) = rows.next().await? {
    tags.push(Tag::try_from(row)?);
  }
  Ok(tags)
}

/// Create a new tag with the given label.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `tag` - the tag to insert, with its id, label and timestamps
///
/// # Returns
///
/// The inserted tag as read back from the database, or an error when the insert fails (for example on a
/// duplicate label) or the read fails.
pub async fn create(conn: &Connection, tag: &Tag) -> Result<Tag, Error> {
  log::debug!("repo::tag::create");
  conn
    .execute(
      &format!("INSERT INTO tags ({SELECT_COLUMNS}) VALUES (?1, ?2, ?3, ?4)"),
      [
        tag.id().to_string(),
        tag.label().clone(),
        tag.created_at().to_rfc3339(),
        tag.updated_at().to_rfc3339(),
      ],
    )
    .await?;

  find_by_id(conn, tag.id().clone())
    .await?
    .ok_or_else(|| Error::InvalidValue("tag not found after insert".into()))
}

/// Detach a tag from an entity. Does not delete the tag itself.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `entity_type` - the kind of the entity
/// * `entity_id` - the entity to untag
/// * `label` - the tag's label
///
/// # Returns
///
/// `true` when a join row was removed, `false` when no tag has that label or it was not attached, or an
/// error when a query fails.
pub async fn detach(conn: &Connection, entity_type: EntityType, entity_id: &Id, label: &str) -> Result<bool, Error> {
  log::debug!("repo::tag::detach");
  let Some(tag) = find_by_label(conn, label).await? else {
    return Ok(false);
  };
  let affected = conn
    .execute(
      "DELETE FROM entity_tags WHERE entity_type = ?1 AND entity_id = ?2 AND tag_id = ?3",
      [entity_type.to_string(), entity_id.to_string(), tag.id().to_string()],
    )
    .await?;
  Ok(affected > 0)
}

/// Detach all tags from an entity. Does not delete the tags themselves.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `entity_type` - the kind of the entity
/// * `entity_id` - the entity to untag
///
/// # Returns
///
/// The number of join rows removed, or an error when the delete fails.
pub async fn detach_all(conn: &Connection, entity_type: EntityType, entity_id: &Id) -> Result<u64, Error> {
  log::debug!("repo::tag::detach_all");
  let affected = conn
    .execute(
      "DELETE FROM entity_tags WHERE entity_type = ?1 AND entity_id = ?2",
      [entity_type.to_string(), entity_id.to_string()],
    )
    .await?;
  Ok(affected)
}

/// Detach all tags from an entity as part of a transaction, so that undo puts them back.
///
/// Each removed link is recorded as a `deleted` event on `entity_tags`, with the whole row as its
/// `before_data`. Does not delete the tags themselves.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `transaction_id` - the transaction the events belong to
/// * `entity_type` - the kind of the entity
/// * `entity_id` - the entity to untag
///
/// # Returns
///
/// The number of join rows removed, or an error when a query fails.
pub async fn detach_all_recorded(
  conn: &Connection,
  transaction_id: &Id,
  entity_type: EntityType,
  entity_id: &Id,
) -> Result<u64, Error> {
  log::debug!("repo::tag::detach_all_recorded");
  let mut rows = conn
    .query(
      "SELECT tag_id, created_at FROM entity_tags WHERE entity_type = ?1 AND entity_id = ?2",
      [entity_type.to_string(), entity_id.to_string()],
    )
    .await?;
  let mut links = Vec::new();
  while let Some(row) = rows.next().await? {
    links.push((row.get::<String>(0)?, row.get::<String>(1)?));
  }
  let removed = detach_all(conn, entity_type, entity_id).await?;
  for (tag_id, created_at) in links {
    let before = serde_json::json!({
      "entity_type": entity_type.to_string(),
      "entity_id": entity_id.to_string(),
      "tag_id": tag_id,
      "created_at": created_at,
    });
    super::transaction::record_event(conn, transaction_id, "entity_tags", &tag_id, "deleted", Some(&before)).await?;
  }
  Ok(removed)
}

/// Find a tag by its [`Id`].
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `id` - the full tag id
///
/// # Returns
///
/// The tag, `None` when no tag has that id, or an error when the query or row decoding fails.
pub async fn find_by_id(conn: &Connection, id: impl Into<Id>) -> Result<Option<Tag>, Error> {
  log::debug!("repo::tag::find_by_id");
  let id = id.into();
  let mut rows = conn
    .query(
      &format!("SELECT {SELECT_COLUMNS} FROM tags WHERE id = ?1"),
      [id.to_string()],
    )
    .await?;

  match rows.next().await? {
    Some(row) => Ok(Some(Tag::try_from(row)?)),
    None => Ok(None),
  }
}

/// Find a tag by its label.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `label` - the exact label to match
///
/// # Returns
///
/// The tag, `None` when no tag has that label, or an error when the query or row decoding fails.
pub async fn find_by_label(conn: &Connection, label: &str) -> Result<Option<Tag>, Error> {
  log::debug!("repo::tag::find_by_label");
  let mut rows = conn
    .query(
      &format!("SELECT {SELECT_COLUMNS} FROM tags WHERE label = ?1"),
      [label.to_string()],
    )
    .await?;

  match rows.next().await? {
    Some(row) => Ok(Some(Tag::try_from(row)?)),
    None => Ok(None),
  }
}

/// Find an existing tag by label or create a new one.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `label` - the tag's label
///
/// # Returns
///
/// The existing tag, or the newly created one, or an error when a query fails.
pub async fn find_or_create(conn: &Connection, label: &str) -> Result<Tag, Error> {
  log::debug!("repo::tag::find_or_create");
  if let Some(existing) = find_by_label(conn, label).await? {
    return Ok(existing);
  }
  let tag = Tag::new(label);
  create(conn, &tag).await
}

/// Return all tags attached to each of the given entities in a single query.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `entity_type` - the kind of the entities
/// * `entity_ids` - the entities whose tags to load
///
/// # Returns
///
/// Entities with no tags are absent from the returned map. Within each entry
/// the tags are ordered by label. Passing an empty `entity_ids` slice returns
/// an empty map without issuing a query. It is an error when the query fails or a stored id does not parse.
pub async fn for_entities(
  conn: &Connection,
  entity_type: EntityType,
  entity_ids: &[Id],
) -> Result<HashMap<Id, Vec<Tag>>, Error> {
  log::debug!("repo::tag::for_entities");
  let mut map: HashMap<Id, Vec<Tag>> = HashMap::new();
  if entity_ids.is_empty() {
    return Ok(map);
  }

  let placeholders = (2..entity_ids.len() + 2)
    .map(|i| format!("?{i}"))
    .collect::<Vec<_>>()
    .join(", ");
  let sql = format!(
    "SELECT t.id, t.label, t.created_at, t.updated_at, et.entity_id \
      FROM entity_tags et \
      INNER JOIN tags t ON t.id = et.tag_id \
      WHERE et.entity_type = ?1 AND et.entity_id IN ({placeholders}) \
      ORDER BY et.entity_id, t.label"
  );

  let mut params: Vec<Value> = Vec::with_capacity(entity_ids.len() + 1);
  params.push(Value::from(entity_type.to_string()));
  for id in entity_ids {
    params.push(Value::from(id.to_string()));
  }

  let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
  while let Some(row) = rows.next().await? {
    let entity_id_str: String = row.get(4)?;
    let tag = Tag::try_from(row)?;
    let entity_id = Id::from_str(&entity_id_str).map_err(Error::InvalidValue)?;
    map.entry(entity_id).or_default().push(tag);
  }

  Ok(map)
}

/// Return all tag labels for a specific entity.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `entity_type` - the kind of the entity
/// * `entity_id` - the entity whose labels to read
///
/// # Returns
///
/// The labels, ordered alphabetically, or an error when the query fails.
pub async fn for_entity(conn: &Connection, entity_type: EntityType, entity_id: &Id) -> Result<Vec<String>, Error> {
  log::debug!("repo::tag::for_entity");
  let mut rows = conn
    .query(
      "SELECT t.label FROM tags t \
        INNER JOIN entity_tags et ON et.tag_id = t.id \
        WHERE et.entity_type = ?1 AND et.entity_id = ?2 \
        ORDER BY t.label",
      [entity_type.to_string(), entity_id.to_string()],
    )
    .await?;

  let mut labels = Vec::new();
  while let Some(row) = rows.next().await? {
    let label: String = row.get(0)?;
    labels.push(label);
  }
  Ok(labels)
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use tempfile::TempDir;

  use super::*;
  use crate::store::{self, Db, model::Project};

  async fn setup() -> (Arc<Db>, Connection, TempDir) {
    let (store, tmp) = store::open_temp().await.unwrap();
    let conn = store.connect().await.unwrap();
    (store, conn, tmp)
  }

  async fn create_project(conn: &Connection) -> Id {
    let project = Project::new("/tmp/test".into());
    conn
      .execute(
        "INSERT INTO projects (id, root, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        [
          project.id().to_string(),
          project.root().to_string_lossy().into_owned(),
          project.created_at().to_rfc3339(),
          project.updated_at().to_rfc3339(),
        ],
      )
      .await
      .unwrap();
    project.id().clone()
  }

  async fn create_task(conn: &Connection, project_id: &Id) -> Id {
    let task_id = Id::new();
    conn
      .execute(
        "INSERT INTO tasks (id, project_id, title) VALUES (?1, ?2, ?3)",
        [task_id.to_string(), project_id.to_string(), "Test task".to_string()],
      )
      .await
      .unwrap();
    task_id
  }

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_tags_sorted_by_label() {
      let (_store, conn, _tmp) = setup().await;

      let z = Tag::new("zebra");
      let a = Tag::new("alpha");
      create(&conn, &z).await.unwrap();
      create(&conn, &a).await.unwrap();

      let tags = all(&conn).await.unwrap();
      assert_eq!(tags.len(), 2);
      assert_eq!(tags[0].label(), "alpha");
      assert_eq!(tags[1].label(), "zebra");
    }
  }

  mod attach_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_tag_and_attaches() {
      let (_store, conn, _tmp) = setup().await;
      let project_id = create_project(&conn).await;
      let task_id = create_task(&conn, &project_id).await;

      let tag = attach(&conn, EntityType::Task, &task_id, "urgent").await.unwrap();
      assert_eq!(tag.label(), "urgent");

      let labels = for_entity(&conn, EntityType::Task, &task_id).await.unwrap();
      assert_eq!(labels, vec!["urgent"]);
    }

    #[tokio::test]
    async fn it_does_not_duplicate_attachment() {
      let (_store, conn, _tmp) = setup().await;
      let project_id = create_project(&conn).await;
      let task_id = create_task(&conn, &project_id).await;

      attach(&conn, EntityType::Task, &task_id, "urgent").await.unwrap();
      attach(&conn, EntityType::Task, &task_id, "urgent").await.unwrap();

      let labels = for_entity(&conn, EntityType::Task, &task_id).await.unwrap();
      assert_eq!(labels, vec!["urgent"]);
    }
  }

  mod detach_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_removes_tag_from_entity() {
      let (_store, conn, _tmp) = setup().await;
      let project_id = create_project(&conn).await;
      let task_id = create_task(&conn, &project_id).await;

      attach(&conn, EntityType::Task, &task_id, "remove-me").await.unwrap();
      let removed = detach(&conn, EntityType::Task, &task_id, "remove-me").await.unwrap();

      assert!(removed);

      let labels = for_entity(&conn, EntityType::Task, &task_id).await.unwrap();
      assert_eq!(labels.len(), 0);
    }

    #[tokio::test]
    async fn it_returns_false_when_tag_not_found() {
      let (_store, conn, _tmp) = setup().await;
      let project_id = create_project(&conn).await;
      let task_id = create_task(&conn, &project_id).await;

      let removed = detach(&conn, EntityType::Task, &task_id, "nonexistent").await.unwrap();
      assert!(!removed);
    }
  }

  mod for_entities_fn {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_groups_tags_by_entity_in_one_query() {
      let (_store, conn, _tmp) = setup().await;
      let project_id = create_project(&conn).await;
      let t1 = create_task(&conn, &project_id).await;
      let t2 = create_task(&conn, &project_id).await;
      let t3 = create_task(&conn, &project_id).await;

      attach(&conn, EntityType::Task, &t1, "alpha").await.unwrap();
      attach(&conn, EntityType::Task, &t1, "zebra").await.unwrap();
      attach(&conn, EntityType::Task, &t2, "alpha").await.unwrap();
      // t3 has no tags

      let map = for_entities(&conn, EntityType::Task, &[t1.clone(), t2.clone(), t3.clone()])
        .await
        .unwrap();

      assert_eq!(map.len(), 2);
      let t1_tags = map.get(&t1).unwrap();
      assert_eq!(t1_tags.len(), 2);
      assert_eq!(t1_tags[0].label(), "alpha");
      assert_eq!(t1_tags[1].label(), "zebra");
      let t2_tags = map.get(&t2).unwrap();
      assert_eq!(t2_tags.len(), 1);
      assert_eq!(t2_tags[0].label(), "alpha");
      assert!(!map.contains_key(&t3));
    }

    #[tokio::test]
    async fn it_returns_empty_map_for_empty_input() {
      let (_store, conn, _tmp) = setup().await;

      let map = for_entities(&conn, EntityType::Task, &[]).await.unwrap();

      assert!(map.is_empty());
    }

    #[tokio::test]
    async fn it_scopes_by_entity_type() {
      let (_store, conn, _tmp) = setup().await;
      let project_id = create_project(&conn).await;
      let task_id = create_task(&conn, &project_id).await;

      attach(&conn, EntityType::Task, &task_id, "task-tag").await.unwrap();
      // Attach the same id as an artifact tag would go to a different type, so
      // the task lookup should ignore it. Simulate by attaching a different
      // type with the same id.
      attach(&conn, EntityType::Artifact, &task_id, "artifact-tag")
        .await
        .unwrap();

      let map = for_entities(&conn, EntityType::Task, std::slice::from_ref(&task_id))
        .await
        .unwrap();

      let tags = map.get(&task_id).unwrap();
      assert_eq!(tags.len(), 1);
      assert_eq!(tags[0].label(), "task-tag");
    }
  }
}
