//! Normalized renderer-independent diagram representation.

/// A supported language version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageVersion {
    /// Language major version.
    pub major: u32,
    /// Language minor version.
    pub minor: u32,
}

/// A semantically valid normalized Stack diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagram {
    /// Declared language version.
    pub language_version: LanguageVersion,
    /// Visible diagram title.
    pub title: String,
    /// Effective theme identifier after language defaults.
    pub theme_id: String,
    /// Direct root children in declaration order.
    pub children: Vec<ElementId>,
    /// Nodes in declaration order.
    pub nodes: Vec<Node>,
    /// Groups in declaration order.
    pub groups: Vec<Group>,
    /// Edges in declaration order.
    pub edges: Vec<Edge>,
    /// Optional diagram-scoped layout input.
    pub layout: Option<Layout>,
}

/// A typed reference to one direct layout or containment child.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ElementId {
    /// Node identifier.
    Node(String),
    /// Group identifier.
    Group(String),
}

impl ElementId {
    /// Returns the underlying Stack identifier.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Node(identifier) | Self::Group(identifier) => identifier,
        }
    }
}

/// A normalized architectural node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// Globally unique source identifier.
    pub id: String,
    /// Visible node label.
    pub label: String,
    /// Effective semantic node kind.
    pub kind: NodeKind,
    /// Optional theme or namespaced provider icon identifier.
    pub icon_id: Option<String>,
    /// Optional visible detail.
    pub detail: Option<String>,
    /// Nearest containing group, if any.
    pub parent_group_id: Option<String>,
}

/// Coarse architectural meaning of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// Person, role, team, or autonomous participant.
    Actor,
    /// Browser, application, device, or other client.
    Client,
    /// Long-running application, API, gateway, or general component.
    Service,
    /// On-demand or serverless compute unit.
    Function,
    /// Background processor or scheduled job.
    Worker,
    /// Durable queryable datastore.
    Database,
    /// Disposable or derived datastore.
    Cache,
    /// Queue, stream, bus, or broker.
    Queue,
    /// Blob, object, file, or archival storage.
    Storage,
    /// System outside the architecture's control boundary.
    External,
}

/// A normalized containment group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// Globally unique source identifier.
    pub id: String,
    /// Visible group label.
    pub label: String,
    /// Nearest containing group, if any.
    pub parent_group_id: Option<String>,
    /// Direct children in declaration order.
    pub children: Vec<ElementId>,
    /// Optional group-scoped layout input.
    pub layout: Option<Layout>,
}

/// A normalized relationship between two nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// Left or source endpoint identifier.
    pub from: String,
    /// Right or target endpoint identifier.
    pub to: String,
    /// Effective directionality.
    pub direction: EdgeDirection,
    /// Effective semantic relationship kind.
    pub kind: EdgeKind,
    /// Optional visible edge label.
    pub label: Option<String>,
}

/// Normalized edge directionality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeDirection {
    /// Directed from `from` to `to`.
    Forward,
    /// Symmetric in both directions.
    Bidirectional,
    /// Directionless or intentionally unspecified.
    Association,
}

/// Semantic relationship kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// Generic runtime or conceptual flow.
    Flow,
    /// Synchronous request or call.
    Request,
    /// Asynchronous message or event delivery.
    Event,
    /// Data movement, replication, read, or write.
    Data,
    /// Build-time, deployment-time, or operational dependency.
    Dependency,
}

/// Normalized layout constraints and hints for one scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// Optional preferred flow direction.
    pub direction: Option<Direction>,
    /// Disjoint same-rank constraints.
    pub same_ranks: Vec<Vec<String>>,
    /// Optional relative-order hint.
    pub order: Option<Vec<String>>,
}

/// Preferred layout direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Prefer left-to-right progression.
    Right,
    /// Prefer top-to-bottom progression.
    Down,
}
