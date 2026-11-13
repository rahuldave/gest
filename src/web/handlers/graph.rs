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

/// The task columns, in the order `task::Model::try_from` reads a row, for the raw task query.
const TASK_SELECT_COLUMNS: &str = "\
  id, project_id, title, priority, status, description, \
  assigned_to, metadata, resolved_at, created_at, updated_at";

/// Query parameters shared by graph pages.
#[derive(Deserialize)]
pub struct GraphParams {
    /// `1`, `true`, `yes` or `all` includes completed and cancelled iterations and resolved tasks.
    all: Option<String>,
    /// The Mermaid flow direction: `TD`, `BT`, `LR` or `RL`; anything else gives `TB`.
    direction: Option<String>,
    /// `1`, `true`, `yes` or `all` keeps the relationship page to one diagram, without per-iteration sections.
    single: Option<String>,
}

/// The project relationship graph page: one diagram of the shown iterations, their tasks and related tasks, and phase
/// sections per iteration when there are several.
#[derive(Template)]
#[template(path = "graphs/relationships.html")]
struct RelationshipGraphTemplate {
    /// The Mermaid flow direction, such as `TB` or `LR`, kept in the page's links.
    direction: String,
    /// The diagram of the whole project.
    graph: MermaidGraph,
    /// Whether finished iterations and resolved tasks are shown, kept in the page's links.
    include_all: bool,
    /// The per-iteration phase diagrams; empty in single-diagram mode or with one iteration or none.
    sections: Vec<PhaseGraphSection>,
    /// Whether the page shows only the project diagram, kept in the page's links.
    single_diagram: bool,
}

/// The phase graph page: one diagram per phase of each iteration.
#[derive(Template)]
#[template(path = "graphs/phases.html")]
struct PhaseGraphTemplate {
    /// The Mermaid flow direction, such as `TB` or `LR`, kept in the page's links.
    direction: String,
    /// Whether finished iterations and resolved tasks are shown, kept in the page's links.
    include_all: bool,
    /// One section per iteration, with its phase diagrams.
    sections: Vec<PhaseGraphSection>,
}

/// The iterations, tasks and edges that the graph pages draw, loaded once per request.
struct GraphData {
    /// Phase memberships of the loaded tasks in the loaded iterations.
    iteration_tasks: Vec<IterationTaskEdge>,
    /// The iterations shown, oldest first.
    iterations: Vec<IterationNode>,
    /// Relationships that touch a loaded task, with any task end among the loaded tasks.
    relationships: Vec<RelationshipEdge>,
    /// The tasks shown, by full id.
    tasks: BTreeMap<String, TaskNode>,
}

/// An iteration as a graph node.
struct IterationNode {
    /// The full iteration id.
    id: String,
    /// The iteration status as text, such as `active`.
    status: String,
    /// The iteration title, wrapped into the node's label.
    title: String,
}

/// A task's membership in one phase of an iteration, drawn as an edge from the iteration.
struct IterationTaskEdge {
    /// The full iteration id.
    iteration_id: String,
    /// The phase number within the iteration.
    phase: u32,
    /// The full task id.
    task_id: String,
}

/// One diagram, as Mermaid flowchart source with its caption.
struct MermaidGraph {
    /// The Mermaid flowchart source.
    mermaid: String,
    /// How many task nodes the diagram holds.
    task_count: usize,
    /// The caption, such as `project relationships` or `phase 2`.
    title: String,
}

/// One iteration's section on a graph page, with a diagram per phase.
struct PhaseGraphSection {
    /// The full iteration id, for its link.
    iteration_id: String,
    /// The first eight characters of the iteration id, for display.
    iteration_short_id: String,
    /// The iteration status as text.
    iteration_status: String,
    /// The iteration title, shown as the section heading.
    iteration_title: String,
    /// The iteration's phases in ascending order.
    phases: Vec<PhaseGraph>,
}

/// The diagram of one phase of an iteration.
struct PhaseGraph {
    /// The phase's tasks and the tasks related to them, plus any task with a relationship whose source is not a task.
    graph: MermaidGraph,
    /// The phase number; the sections are in ascending phase order.
    number: u32,
}

/// A stored relationship between two entities, as a graph edge.
struct RelationshipEdge {
    /// The kind of relationship, such as blocks or child-of.
    rel_type: RelationshipType,
    /// The full id of the source entity.
    source_id: String,
    /// The entity type of the source; only a task source is checked against the selected tasks.
    source_type: EntityType,
    /// The full id of the target entity.
    target_id: String,
    /// The entity type of the target; only a task target is checked against the selected tasks.
    target_type: EntityType,
}

/// A task as a graph node.
struct TaskNode {
    /// The full task id.
    id: String,
    /// The task priority, shown as `P<n>`, if set.
    priority: Option<u8>,
    /// The task status as text, such as `in-progress`; it picks the node's colour class.
    status: String,
    /// The task title, wrapped into the node's label.
    title: String,
}

/// Project relationship graph page.
///
/// Handles `GET /graphs` and `GET /graphs/relationships`. With more than one iteration, and without `single`, it
/// adds a phase section per iteration under the project diagram.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store and the project id
/// * `params` - the query string: `all`, `direction` and `single`
///
/// # Returns
///
/// The rendered page, or a [`web::Error`] when the store or the template fails.
pub async fn graph_relationships(
    State(state): State<AppState>,
    Query(params): Query<GraphParams>,
) -> Result<Html<String>, web::Error> {
    let include_all = parse_include_all(params.all.as_deref());
    let direction = parse_direction(params.direction.as_deref());
    let single_diagram = parse_bool(params.single.as_deref());
    let data = load_graph_data(&state, include_all).await?;
    let sections = if !single_diagram && data.iterations.len() > 1 {
        build_phase_sections(&data, &direction)
    } else {
        Vec::new()
    };
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
        sections,
        single_diagram,
    };
    Ok(Html(tmpl.render()?))
}

/// Per-iteration phase relationship graph page.
///
/// Handles `GET /graphs/phases`; the `single` parameter is ignored.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store and the project id
/// * `params` - the query string: `all` and `direction`
///
/// # Returns
///
/// The rendered page, or a [`web::Error`] when the store or the template fails.
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

/// Accept a Mermaid flow direction from the query string.
///
/// # Arguments
///
/// * `direction` - the `direction` query value, if given
///
/// # Returns
///
/// `TD`, `BT`, `LR` or `RL` as given, or `TB` for anything else.
fn parse_direction(direction: Option<&str>) -> String {
    match direction {
        Some("TD" | "BT" | "LR" | "RL") => direction.unwrap().to_owned(),
        _ => "TB".to_owned(),
    }
}

/// Read the `all` query value, which includes finished iterations and resolved tasks.
///
/// # Arguments
///
/// * `value` - the `all` query value, if given
///
/// # Returns
///
/// Whether the value is one that [`parse_bool`] accepts as true.
fn parse_include_all(value: Option<&str>) -> bool {
    parse_bool(value)
}

/// Read a query flag.
///
/// # Arguments
///
/// * `value` - the query value, if given
///
/// # Returns
///
/// `true` for `1`, `true`, `yes` or `all`, and `false` for anything else or nothing.
fn parse_bool(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true" | "yes" | "all"))
}

/// Load the iterations, their tasks and the tasks' relationships for a graph page.
///
/// Without `include_all` it skips completed and cancelled iterations and done and cancelled tasks. Tasks reached
/// only through a relationship are added, and edges to tasks that were filtered out are dropped.
///
/// # Arguments
///
/// * `state` - the shared app state, for the store and the project id
/// * `include_all` - whether to include finished iterations and resolved tasks
///
/// # Returns
///
/// The graph data, or a [`web::Error`] when the store fails.
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

/// Read the phase memberships of the given iterations.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `iteration_ids` - the iterations to read
///
/// # Returns
///
/// One edge per task in each iteration, ordered by iteration, phase and the time it was added; empty for no ids.
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

/// Read every relationship whose source or target is one of the given tasks.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `task_ids` - the full ids of the tasks
///
/// # Returns
///
/// The relationships, ordered by type and creation time; empty for no ids.
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

/// Read the given tasks of the project as graph nodes.
///
/// # Arguments
///
/// * `conn` - the database connection
/// * `project_id` - the project the tasks must belong to
/// * `task_ids` - the full ids of the tasks
/// * `include_all` - whether to keep done and cancelled tasks
///
/// # Returns
///
/// The tasks found, by full id; empty for no ids.
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

/// Write numbered SQL placeholders for an `IN (...)` list.
///
/// # Arguments
///
/// * `count` - how many placeholders to write
/// * `start` - the number of the first one
///
/// # Returns
///
/// The placeholders joined by commas, such as `?2, ?3, ?4`.
fn placeholders(count: usize, start: usize) -> String {
    (start..start + count)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Build one section per iteration, with a diagram for each of its phases.
///
/// # Arguments
///
/// * `data` - the loaded graph data
/// * `direction` - the Mermaid flow direction
///
/// # Returns
///
/// The sections, in the order of `data.iterations`; an iteration with no tasks has no phases.
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
        .map(|iteration| {
            let phase_numbers = phase_map
                .get(iteration.id.as_str())
                .map(|phases| phases.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default();
            let phases = phase_numbers
                .into_iter()
                .map(|phase| {
                    let graph = related_phase_graph(data, &iteration.id, phase, direction);
                    PhaseGraph {
                        graph,
                        number: phase,
                    }
                })
                .collect::<Vec<_>>();
            PhaseGraphSection {
                iteration_id: iteration.id.clone(),
                iteration_short_id: iteration.id.chars().take(8).collect(),
                iteration_status: iteration.status.clone(),
                iteration_title: iteration.title.clone(),
                phases,
            }
        })
        .collect()
}

/// Draw one phase of an iteration: its tasks and the tasks directly related to them.
///
/// Tasks whose relationship has a non-task source are also included.
///
/// # Arguments
///
/// * `data` - the loaded graph data
/// * `iteration_id` - the full id of the iteration
/// * `phase` - the phase number
/// * `direction` - the Mermaid flow direction
///
/// # Returns
///
/// The diagram, captioned `phase <n>`, without iteration nodes.
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

/// Write Mermaid flowchart source for iterations, tasks and their edges.
///
/// Each node links to its page. Task-to-task relationships are drawn in their parent or blocker direction, and an
/// edge with an end outside `tasks` is skipped.
///
/// # Arguments
///
/// * `iterations` - the iteration nodes, in display order
/// * `tasks` - the task nodes, by full id
/// * `iteration_tasks` - the phase memberships, drawn as iteration-to-task edges
/// * `relationships` - the relationships; only task-to-task ones are drawn
/// * `direction` - the Mermaid flow direction
/// * `stack_iterations` - whether to join consecutive iterations with a dotted `next iteration` edge
///
/// # Returns
///
/// The flowchart source, one statement per line.
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
        "  classDef iteration fill:#17324d,stroke:#67b7dc,color:#f6fbff;".to_owned(),
        "  classDef active fill:#15391f,stroke:#6fcf97,color:#f7fff9;".to_owned(),
        "  classDef open fill:#2f2f2f,stroke:#d6d6d6,color:#ffffff;".to_owned(),
        "  classDef progress fill:#463814,stroke:#f2c94c,color:#fff8db;".to_owned(),
        "  classDef done fill:#20382f,stroke:#7bd88f,color:#f1fff3;".to_owned(),
        "  classDef cancelled fill:#3d2525,stroke:#eb5757,color:#fff4f4;".to_owned(),
        "  classDef legend fill:#11161c,stroke:#6b7280,color:#e5e7eb;".to_owned(),
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

/// Orient a relationship for drawing, so inverse types read the same way.
///
/// # Arguments
///
/// * `rel` - the relationship
///
/// # Returns
///
/// The source id, the target id and the edge label: `parent of`, `blocks` or `relates to`.
fn relationship_display(rel: &RelationshipEdge) -> (&str, &str, &'static str) {
    match rel.rel_type {
        RelationshipType::ChildOf => (&rel.target_id, &rel.source_id, "parent of"),
        RelationshipType::ParentOf => (&rel.source_id, &rel.target_id, "parent of"),
        RelationshipType::BlockedBy => (&rel.target_id, &rel.source_id, "blocks"),
        RelationshipType::Blocks => (&rel.source_id, &rel.target_id, "blocks"),
        RelationshipType::RelatesTo => (&rel.source_id, &rel.target_id, "relates to"),
    }
}

/// Make a Mermaid node identifier from an entity id.
///
/// # Arguments
///
/// * `kind` - the prefix, `I` for an iteration or `T` for a task
/// * `entity_id` - the full entity id
///
/// # Returns
///
/// The prefix, an underscore and the first twelve characters of the id.
fn node_id(kind: &str, entity_id: &str) -> String {
    let short = entity_id.chars().take(12).collect::<String>();
    format!("{kind}_{short}")
}

/// Join the non-empty parts of a node label.
///
/// # Arguments
///
/// * `parts` - the parts, such as the short id, title, status and priority
///
/// # Returns
///
/// The escaped parts joined by ` | `.
fn label(parts: impl IntoIterator<Item = String>) -> String {
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .map(|part| escape_label(&part))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Escape text for a quoted Mermaid label.
///
/// # Arguments
///
/// * `text` - the label text
///
/// # Returns
///
/// The text with backslashes and double quotes escaped and newlines turned into spaces.
fn escape_label(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}

/// Wrap a long title for a node, four words to a line.
///
/// # Arguments
///
/// * `title` - the entity title
///
/// # Returns
///
/// The title as given when it has four words or fewer, or else its words in lines of four joined by `<br/>`.
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

/// Pick the Mermaid class that colours a node by its status.
///
/// # Arguments
///
/// * `status` - the status as text
///
/// # Returns
///
/// `active`, `open`, `progress`, `done` (for done or completed) or `cancelled`; `open` for anything else.
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

    #[tokio::test]
    async fn it_keeps_empty_iteration_sections_like_the_python_exporter() {
        let state = setup_state().await;
        let conn = state.store().connect().await.unwrap();
        let empty_iter = repo::iteration::create(
            &conn,
            state.project_id(),
            &iteration::New {
                title: "Empty iteration".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let task_iter = repo::iteration::create(
            &conn,
            state.project_id(),
            &iteration::New {
                title: "Task iteration".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let task = repo::task::create(
            &conn,
            state.project_id(),
            &task::New {
                title: "Graph task".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        repo::iteration::add_task(&conn, task_iter.id(), task.id(), 1)
            .await
            .unwrap();

        let data = load_graph_data(&state, false).await.unwrap();
        let sections = build_phase_sections(&data, "TB");

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].iteration_id, empty_iter.id().to_string());
        assert_eq!(sections[0].phases.len(), 0);
        assert_eq!(sections[1].iteration_id, task_iter.id().to_string());
        assert_eq!(sections[1].phases.len(), 1);
    }

    #[test]
    fn it_sanitizes_graph_options() {
        assert_eq!(parse_direction(Some("LR")), "LR");
        assert_eq!(parse_direction(Some("sideways")), "TB");
        assert!(parse_include_all(Some("1")));
        assert!(!parse_include_all(Some("0")));
    }
}
