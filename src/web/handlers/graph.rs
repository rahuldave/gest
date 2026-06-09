//! Mermaid graph handlers for the web dashboard.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt::Write as _,
};

use askama::Template;
use axum::{
    extract::{Query, State},
    response::Html,
};
use libsql::{Connection, Value};
use serde::Deserialize;

use crate::{
    store::{
        Error as StoreError,
        model::{
            iteration,
            primitives::{EntityType, Id, RelationshipType},
            relationship, task,
        },
        repo,
    },
    web::{self, AppState},
};

const TASK_SELECT_COLUMNS: &str = "\
  id, project_id, title, priority, status, description, \
  assigned_to, metadata, resolved_at, created_at, updated_at";

/// Query parameters shared by graph pages.
#[derive(Deserialize)]
pub struct GraphParams {
    all: Option<String>,
    direction: Option<String>,
}

#[derive(Template)]
#[template(path = "graphs/relationships.html")]
struct RelationshipGraphTemplate {
    direction: String,
    graph: MermaidGraph,
    include_all: bool,
}

#[derive(Template)]
#[template(path = "graphs/phases.html")]
struct PhaseGraphTemplate {
    direction: String,
    include_all: bool,
    sections: Vec<PhaseGraphSection>,
}

struct GraphData {
    iteration_tasks: Vec<IterationTaskEdge>,
    iterations: Vec<IterationNode>,
    relationships: Vec<RelationshipEdge>,
    tasks: BTreeMap<String, TaskNode>,
}

struct IterationNode {
    id: String,
    status: String,
    title: String,
}

struct IterationTaskEdge {
    iteration_id: String,
    phase: u32,
    task_id: String,
}

struct MermaidGraph {
    mermaid: String,
    task_count: usize,
    title: String,
}

struct PhaseGraphSection {
    iteration_id: String,
    iteration_short_id: String,
    iteration_status: String,
    iteration_title: String,
    phases: Vec<PhaseGraph>,
}

struct PhaseGraph {
    graph: MermaidGraph,
    number: u32,
}

struct RelationshipEdge {
    rel_type: RelationshipType,
    source_id: String,
    source_type: EntityType,
    target_id: String,
    target_type: EntityType,
}

struct TaskNode {
    id: String,
    priority: Option<u8>,
    status: String,
    title: String,
}

/// Project relationship graph page.
pub async fn graph_relationships(
    State(state): State<AppState>,
    Query(params): Query<GraphParams>,
) -> Result<Html<String>, web::Error> {
    let include_all = parse_include_all(params.all.as_deref());
    let direction = parse_direction(params.direction.as_deref());
    let data = load_graph_data(&state, include_all).await?;
    let graph = MermaidGraph {
        task_count: data.tasks.len(),
        title: "project relationships".to_owned(),
        mermaid: build_mermaid(
            &data.iterations,
            &data.tasks,
            &data.iteration_tasks,
            &data.relationships,
            &direction,
            true,
        ),
    };
    let tmpl = RelationshipGraphTemplate {
        direction,
        graph,
        include_all,
    };
    Ok(Html(tmpl.render()?))
}

/// Per-iteration phase relationship graph page.
pub async fn graph_phases(
    State(state): State<AppState>,
    Query(params): Query<GraphParams>,
) -> Result<Html<String>, web::Error> {
    let include_all = parse_include_all(params.all.as_deref());
    let direction = parse_direction(params.direction.as_deref());
    let data = load_graph_data(&state, include_all).await?;
    let sections = build_phase_sections(&data, &direction);
    let tmpl = PhaseGraphTemplate {
        direction,
        include_all,
        sections,
    };
    Ok(Html(tmpl.render()?))
}

fn parse_direction(direction: Option<&str>) -> String {
    match direction {
        Some("TD" | "BT" | "LR" | "RL") => direction.unwrap().to_owned(),
        _ => "TB".to_owned(),
    }
}

fn parse_include_all(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true" | "yes" | "all"))
}

async fn load_graph_data(state: &AppState, include_all: bool) -> Result<GraphData, web::Error> {
    let conn = state.store().connect().await?;
    let filter = if include_all {
        iteration::Filter::all()
    } else {
        iteration::Filter::default()
    };
    let mut iterations = repo::iteration::all(&conn, state.project_id(), &filter).await?;
    iterations.sort_by_key(|i| *i.created_at());
    let iteration_ids: Vec<Id> = iterations.iter().map(|i| i.id().clone()).collect();
    let iteration_nodes = iterations
        .into_iter()
        .map(|i| IterationNode {
            id: i.id().to_string(),
            status: i.status().to_string(),
            title: i.title().to_owned(),
        })
        .collect::<Vec<_>>();

    let iteration_tasks = fetch_iteration_task_edges(&conn, &iteration_ids).await?;
    let mut task_ids: HashSet<String> = iteration_tasks
        .iter()
        .map(|edge| edge.task_id.clone())
        .collect();
    let mut relationships = fetch_task_relationship_edges(&conn, &task_ids).await?;
    for rel in &relationships {
        if rel.source_type == EntityType::Task {
            task_ids.insert(rel.source_id.clone());
        }
        if rel.target_type == EntityType::Task {
            task_ids.insert(rel.target_id.clone());
        }
    }

    let tasks = fetch_task_nodes(&conn, state.project_id(), &task_ids, include_all).await?;
    let task_ids: HashSet<String> = tasks.keys().cloned().collect();
    let iteration_tasks = iteration_tasks
        .into_iter()
        .filter(|edge| task_ids.contains(&edge.task_id))
        .collect::<Vec<_>>();
    relationships.retain(|rel| {
        (rel.source_type != EntityType::Task || task_ids.contains(&rel.source_id))
            && (rel.target_type != EntityType::Task || task_ids.contains(&rel.target_id))
    });

    Ok(GraphData {
        iteration_tasks,
        iterations: iteration_nodes,
        relationships,
        tasks,
    })
}

async fn fetch_iteration_task_edges(
    conn: &Connection,
    iteration_ids: &[Id],
) -> Result<Vec<IterationTaskEdge>, StoreError> {
    if iteration_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = placeholders(iteration_ids.len(), 1);
    let sql = format!(
        "SELECT iteration_id, task_id, phase FROM iteration_tasks \
      WHERE iteration_id IN ({placeholders}) ORDER BY iteration_id, phase, created_at"
    );
    let params = iteration_ids
        .iter()
        .map(|id| Value::from(id.to_string()))
        .collect::<Vec<_>>();

    let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
    let mut edges = Vec::new();
    while let Some(row) = rows.next().await? {
        let iteration_id: String = row.get(0)?;
        let task_id: String = row.get(1)?;
        let phase: i64 = row.get(2)?;
        edges.push(IterationTaskEdge {
            iteration_id,
            task_id,
            phase: phase as u32,
        });
    }
    Ok(edges)
}

async fn fetch_task_relationship_edges(
    conn: &Connection,
    task_ids: &HashSet<String>,
) -> Result<Vec<RelationshipEdge>, StoreError> {
    if task_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut ids = task_ids.iter().cloned().collect::<Vec<_>>();
    ids.sort();
    let placeholders = placeholders(ids.len(), 1);
    let sql = format!(
        "SELECT id, rel_type, source_id, source_type, target_id, target_type, created_at, updated_at \
      FROM relationships \
      WHERE (source_type = 'task' AND source_id IN ({placeholders})) \
         OR (target_type = 'task' AND target_id IN ({placeholders})) \
      ORDER BY rel_type, created_at"
    );
    let params = ids
        .iter()
        .chain(ids.iter())
        .map(|id| Value::from(id.clone()))
        .collect::<Vec<_>>();

    let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
    let mut edges = Vec::new();
    while let Some(row) = rows.next().await? {
        let rel = relationship::Model::try_from(row)?;
        edges.push(RelationshipEdge {
            rel_type: rel.rel_type(),
            source_id: rel.source_id().to_string(),
            source_type: rel.source_type(),
            target_id: rel.target_id().to_string(),
            target_type: rel.target_type(),
        });
    }
    Ok(edges)
}

async fn fetch_task_nodes(
    conn: &Connection,
    project_id: &Id,
    task_ids: &HashSet<String>,
    include_all: bool,
) -> Result<BTreeMap<String, TaskNode>, StoreError> {
    if task_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut ids = task_ids.iter().cloned().collect::<Vec<_>>();
    ids.sort();
    let placeholders = placeholders(ids.len(), 2);
    let task_filter = if include_all {
        ""
    } else {
        " AND status NOT IN ('done', 'cancelled')"
    };
    let sql = format!(
        "SELECT {} FROM tasks WHERE project_id = ?1 AND id IN ({placeholders}){task_filter} ORDER BY created_at, title",
        TASK_SELECT_COLUMNS,
    );
    let params = std::iter::once(Value::from(project_id.to_string()))
        .chain(ids.iter().map(|id| Value::from(id.clone())))
        .collect::<Vec<_>>();

    let mut rows = conn.query(&sql, libsql::params_from_iter(params)).await?;
    let mut tasks = BTreeMap::new();
    while let Some(row) = rows.next().await? {
        let task = task::Model::try_from(row)?;
        tasks.insert(
            task.id().to_string(),
            TaskNode {
                id: task.id().to_string(),
                priority: task.priority(),
                status: task.status().to_string(),
                title: task.title().to_owned(),
            },
        );
    }
    Ok(tasks)
}

fn placeholders(count: usize, start: usize) -> String {
    (start..start + count)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn build_phase_sections(data: &GraphData, direction: &str) -> Vec<PhaseGraphSection> {
    let mut phase_map: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
    for edge in &data.iteration_tasks {
        phase_map
            .entry(&edge.iteration_id)
            .or_default()
            .insert(edge.phase);
    }

    data.iterations
        .iter()
        .filter_map(|iteration| {
            let phases = phase_map
                .get(iteration.id.as_str())?
                .iter()
                .map(|phase| {
                    let graph = related_phase_graph(data, &iteration.id, *phase, direction);
                    PhaseGraph {
                        graph,
                        number: *phase,
                    }
                })
                .collect::<Vec<_>>();
            Some(PhaseGraphSection {
                iteration_id: iteration.id.clone(),
                iteration_short_id: iteration.id.chars().take(8).collect(),
                iteration_status: iteration.status.clone(),
                iteration_title: iteration.title.clone(),
                phases,
            })
        })
        .collect()
}

fn related_phase_graph(
    data: &GraphData,
    iteration_id: &str,
    phase: u32,
    direction: &str,
) -> MermaidGraph {
    let mut selected_task_ids = data
        .iteration_tasks
        .iter()
        .filter(|edge| edge.iteration_id == iteration_id && edge.phase == phase)
        .map(|edge| edge.task_id.clone())
        .collect::<HashSet<_>>();

    let selected_relationships = data
        .relationships
        .iter()
        .filter(|rel| {
            (rel.source_type != EntityType::Task || selected_task_ids.contains(&rel.source_id))
                || (rel.target_type != EntityType::Task
                    || selected_task_ids.contains(&rel.target_id))
        })
        .map(|rel| RelationshipEdge {
            rel_type: rel.rel_type,
            source_id: rel.source_id.clone(),
            source_type: rel.source_type,
            target_id: rel.target_id.clone(),
            target_type: rel.target_type,
        })
        .collect::<Vec<_>>();

    for rel in &selected_relationships {
        if rel.source_type == EntityType::Task {
            selected_task_ids.insert(rel.source_id.clone());
        }
        if rel.target_type == EntityType::Task {
            selected_task_ids.insert(rel.target_id.clone());
        }
    }

    let selected_tasks = data
        .tasks
        .iter()
        .filter(|(id, _)| selected_task_ids.contains(*id))
        .map(|(id, task)| {
            (
                id.clone(),
                TaskNode {
                    id: task.id.clone(),
                    priority: task.priority,
                    status: task.status.clone(),
                    title: task.title.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    let relationships = selected_relationships
        .into_iter()
        .filter(|rel| {
            (rel.source_type != EntityType::Task || selected_tasks.contains_key(&rel.source_id))
                && (rel.target_type != EntityType::Task
                    || selected_tasks.contains_key(&rel.target_id))
        })
        .collect::<Vec<_>>();

    MermaidGraph {
        task_count: selected_tasks.len(),
        title: format!("phase {phase}"),
        mermaid: build_mermaid(&[], &selected_tasks, &[], &relationships, direction, false),
    }
}

fn build_mermaid(
    iterations: &[IterationNode],
    tasks: &BTreeMap<String, TaskNode>,
    iteration_tasks: &[IterationTaskEdge],
    relationships: &[RelationshipEdge],
    direction: &str,
    stack_iterations: bool,
) -> String {
    let mut lines = vec![
        format!("flowchart {direction}"),
        "  classDef iteration fill:#17324d,stroke:#4EA8E0,color:#f6fbff;".to_owned(),
        "  classDef active fill:#17324d,stroke:#4EA8E0,color:#f6fbff;".to_owned(),
        "  classDef open fill:#2f2f2f,stroke:#C4C8D4,color:#ffffff;".to_owned(),
        "  classDef progress fill:#463814,stroke:#CC9820,color:#fff8db;".to_owned(),
        "  classDef done fill:#20382f,stroke:#36BE78,color:#f1fff3;".to_owned(),
        "  classDef cancelled fill:#3d2525,stroke:#D05830,color:#fff4f4;".to_owned(),
    ];

    let mut previous_iteration_id: Option<&str> = None;
    for iteration in iterations {
        let ident = node_id("I", &iteration.id);
        lines.push(format!(
            "  {ident}[\"{}\"]",
            label([
                iteration.id.chars().take(8).collect::<String>(),
                wrapped_title(&iteration.title),
                iteration.status.clone(),
            ])
        ));
        lines.push(format!("  class {ident} iteration;"));
        lines.push(format!(
            "  click {ident} \"/iterations/{}\" _self",
            iteration.id
        ));
        if stack_iterations && let Some(previous) = previous_iteration_id {
            lines.push(format!(
                "  {} -. \"next iteration\" .-> {ident}",
                node_id("I", previous)
            ));
        }
        previous_iteration_id = Some(&iteration.id);
    }

    for task in tasks.values() {
        let ident = node_id("T", &task.id);
        let priority = task.priority.map(|p| format!("P{p}"));
        lines.push(format!(
            "  {ident}[\"{}\"]",
            label([
                task.id.chars().take(8).collect::<String>(),
                wrapped_title(&task.title),
                task.status.clone(),
                priority.unwrap_or_default(),
            ])
        ));
        lines.push(format!(
            "  class {ident} {};",
            mermaid_class_for_status(&task.status)
        ));
        lines.push(format!("  click {ident} \"/tasks/{}\" _self", task.id));
    }

    for edge in iteration_tasks {
        if tasks.contains_key(&edge.task_id) {
            lines.push(format!(
                "  {} -- \"phase {}\" --> {}",
                node_id("I", &edge.iteration_id),
                edge.phase,
                node_id("T", &edge.task_id)
            ));
        }
    }

    for rel in relationships {
        if rel.source_type != EntityType::Task || rel.target_type != EntityType::Task {
            continue;
        }
        if !tasks.contains_key(&rel.source_id) || !tasks.contains_key(&rel.target_id) {
            continue;
        }
        let (source, target, label) = relationship_display(rel);
        lines.push(format!(
            "  {} -- \"{}\" --> {}",
            node_id("T", source),
            escape_label(label),
            node_id("T", target)
        ));
    }

    lines.join("\n")
}

fn relationship_display(rel: &RelationshipEdge) -> (&str, &str, &'static str) {
    match rel.rel_type {
        RelationshipType::ChildOf => (&rel.target_id, &rel.source_id, "parent of"),
        RelationshipType::ParentOf => (&rel.source_id, &rel.target_id, "parent of"),
        RelationshipType::BlockedBy => (&rel.target_id, &rel.source_id, "blocks"),
        RelationshipType::Blocks => (&rel.source_id, &rel.target_id, "blocks"),
        RelationshipType::RelatesTo => (&rel.source_id, &rel.target_id, "relates to"),
    }
}

fn node_id(kind: &str, entity_id: &str) -> String {
    let short = entity_id.chars().take(12).collect::<String>();
    format!("{kind}_{short}")
}

fn label(parts: impl IntoIterator<Item = String>) -> String {
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .map(|part| escape_label(&part))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn escape_label(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}

fn wrapped_title(title: &str) -> String {
    const MAX_WORDS_PER_LINE: usize = 4;
    let words = title.split_whitespace().collect::<Vec<_>>();
    if words.len() <= MAX_WORDS_PER_LINE {
        return title.to_owned();
    }

    let mut out = String::new();
    for (index, chunk) in words.chunks(MAX_WORDS_PER_LINE).enumerate() {
        if index > 0 {
            let _ = write!(out, "<br/>");
        }
        let _ = write!(out, "{}", chunk.join(" "));
    }
    out
}

fn mermaid_class_for_status(status: &str) -> &'static str {
    match status {
        "active" => "active",
        "open" => "open",
        "in-progress" => "progress",
        "done" | "completed" => "done",
        "cancelled" => "cancelled",
        _ => "open",
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
        self,
        model::{
            Project, iteration,
            primitives::{RelationshipType, TaskStatus},
            task,
        },
    };

    async fn setup_state() -> AppState {
        let (store_arc, tmp) = store::open_temp().await.unwrap();
        let conn = store_arc.connect().await.unwrap();
        let project = Project::new("/tmp/web-graph-test".into());
        conn.execute(
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
        let project_id = project.id().clone();
        std::mem::forget(tmp);
        AppState::new(store_arc, project_id)
    }

    #[tokio::test]
    async fn it_builds_a_project_relationship_graph_from_iterations_tasks_and_links() {
        let state = setup_state().await;
        let conn = state.store().connect().await.unwrap();
        let iter = repo::iteration::create(
            &conn,
            state.project_id(),
            &iteration::New {
                title: "Graph iteration".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let parent = repo::task::create(
            &conn,
            state.project_id(),
            &task::New {
                title: "Parent task".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let child = repo::task::create(
            &conn,
            state.project_id(),
            &task::New {
                title: "Child task".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        repo::iteration::add_task(&conn, iter.id(), child.id(), 2)
            .await
            .unwrap();
        repo::relationship::create(
            &conn,
            RelationshipType::ChildOf,
            EntityType::Task,
            child.id(),
            EntityType::Task,
            parent.id(),
        )
        .await
        .unwrap();

        let data = load_graph_data(&state, false).await.unwrap();
        let graph = build_mermaid(
            &data.iterations,
            &data.tasks,
            &data.iteration_tasks,
            &data.relationships,
            "TB",
            true,
        );

        assert!(graph.contains("Graph iteration"));
        assert!(graph.contains("Parent task"));
        assert!(graph.contains("Child task"));
        assert!(graph.contains("-- \"phase 2\" -->"));
        assert!(graph.contains("-- \"parent of\" -->"));
    }

    #[tokio::test]
    async fn it_excludes_terminal_tasks_until_all_is_requested() {
        let state = setup_state().await;
        let conn = state.store().connect().await.unwrap();
        let iter = repo::iteration::create(
            &conn,
            state.project_id(),
            &iteration::New {
                title: "Graph iteration".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let done = repo::task::create(
            &conn,
            state.project_id(),
            &task::New {
                status: Some(TaskStatus::Done),
                title: "Done task".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        repo::iteration::add_task(&conn, iter.id(), done.id(), 1)
            .await
            .unwrap();

        let active_only = load_graph_data(&state, false).await.unwrap();
        let with_all = load_graph_data(&state, true).await.unwrap();

        assert_eq!(active_only.tasks.len(), 0);
        assert_eq!(with_all.tasks.len(), 1);
    }

    #[test]
    fn it_sanitizes_graph_options() {
        assert_eq!(parse_direction(Some("LR")), "LR");
        assert_eq!(parse_direction(Some("sideways")), "TB");
        assert!(parse_include_all(Some("1")));
        assert!(!parse_include_all(Some("0")));
    }
}
