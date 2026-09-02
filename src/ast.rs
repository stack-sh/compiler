//! Source-oriented abstract syntax tree.

use crate::diagnostic::{Span, Spanned};

/// A parsed Stack document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Authored language version.
    pub version: Version,
    /// The document's single diagram.
    pub diagram: Diagram,
    /// Span of the complete document.
    pub span: Span,
}

/// A `major.minor` language version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// Authored major number.
    pub major: u32,
    /// Authored minor number.
    pub minor: u32,
    /// Span of the complete version directive.
    pub span: Span,
}

/// The root diagram declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagram {
    /// Visible diagram title.
    pub title: Spanned<String>,
    /// Authored declarations in source order.
    pub members: Vec<DiagramMember>,
    /// Span of the complete declaration.
    pub span: Span,
}

/// A declaration allowed directly inside a diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagramMember {
    /// Node declaration.
    Node(Node),
    /// Group declaration.
    Group(Group),
    /// Edge declaration.
    Edge(Edge),
    /// Theme selection.
    Theme(Theme),
    /// Layout block.
    Layout(Layout),
}

/// A theme selection statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// Authored theme identifier.
    pub identifier: Spanned<String>,
    /// Span of the complete statement.
    pub span: Span,
}

/// A labeled containment group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// Source identifier.
    pub identifier: Spanned<String>,
    /// Visible group label.
    pub label: Spanned<String>,
    /// Authored group members in source order.
    pub members: Vec<GroupMember>,
    /// Span of the complete declaration.
    pub span: Span,
}

/// A declaration allowed directly inside a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupMember {
    /// Node declaration.
    Node(Node),
    /// Nested group declaration.
    Group(Group),
    /// Scoped layout block.
    Layout(Layout),
}

/// An architectural node declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// Source identifier.
    pub identifier: Spanned<String>,
    /// Visible node label.
    pub label: Spanned<String>,
    /// Authored properties in source order.
    pub properties: Vec<NodeProperty>,
    /// Span of the complete declaration.
    pub span: Span,
}

/// A property in a node block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeProperty {
    /// Authored node kind.
    Kind(Spanned<String>),
    /// Authored theme-local icon identifier.
    Icon(Spanned<String>),
    /// Authored visible detail.
    Detail(Spanned<String>),
}

impl NodeProperty {
    /// Returns this property's value span.
    pub fn span(&self) -> Span {
        match self {
            Self::Kind(value) | Self::Icon(value) | Self::Detail(value) => value.span,
        }
    }
}

/// An edge declaration between two identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// Left endpoint reference.
    pub from: Spanned<String>,
    /// Authored edge operator.
    pub operator: Spanned<EdgeOperator>,
    /// Right endpoint reference.
    pub to: Spanned<String>,
    /// Optional visible edge label.
    pub label: Option<Spanned<String>>,
    /// Authored edge properties in source order.
    pub properties: Vec<EdgeProperty>,
    /// Span of the complete declaration.
    pub span: Span,
}

/// Directionality expressed by an edge operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeOperator {
    /// `->`
    Forward,
    /// `<->`
    Bidirectional,
    /// `--`
    Association,
}

/// A property in an edge block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeProperty {
    /// Authored relationship kind.
    Kind(Spanned<String>),
}

impl EdgeProperty {
    /// Returns this property's value span.
    pub fn span(&self) -> Span {
        match self {
            Self::Kind(value) => value.span,
        }
    }
}

/// A scoped collection of layout statements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// Authored statements in source order.
    pub statements: Vec<LayoutStatement>,
    /// Span of the complete block.
    pub span: Span,
}

/// A layout constraint or hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutStatement {
    /// Preferred flow direction.
    Direction(Spanned<String>),
    /// Same-rank constraint.
    RankSame(IdentifierList),
    /// Relative-order hint.
    Order(IdentifierList),
}

impl LayoutStatement {
    /// Returns the complete statement span.
    pub fn span(&self) -> Span {
        match self {
            Self::Direction(value) => value.span,
            Self::RankSame(list) | Self::Order(list) => list.span,
        }
    }
}

/// A bracketed list of identifier references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifierList {
    /// Authored identifiers in source order.
    pub identifiers: Vec<Spanned<String>>,
    /// Span including the list brackets.
    pub span: Span,
}
