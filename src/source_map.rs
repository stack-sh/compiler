//! Rust source-map sidecar for post-compiler diagnostics.

use crate::ast::{self, DiagramMember, GroupMember, LayoutStatement, NodeProperty};
use crate::diagnostic::Span;
use crate::lossless::{self, TokenKind};

/// Whether a semantic value was authored or supplied by a language default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOrigin {
    /// The value was written in source at this end-exclusive span.
    Authored(Span),
    /// The value was omitted from source.
    Omitted,
}

impl SourceOrigin {
    /// Returns the authored span, or `None` for an omitted value.
    pub const fn span(self) -> Option<Span> {
        match self {
            Self::Authored(span) => Some(span),
            Self::Omitted => None,
        }
    }
}

/// Source origin for one node's semantic icon value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeIconSource {
    /// Globally unique node identifier.
    pub node_id: String,
    /// Authored icon-string span or omitted default.
    pub origin: SourceOrigin,
}

/// Semantic scope of a layout block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutScope {
    /// The root diagram layout.
    Diagram,
    /// The layout of the group with this globally unique identifier.
    Group(String),
}

/// Source origin for one scope's semantic order hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutOrderSource {
    /// Diagram or group identity.
    pub scope: LayoutScope,
    /// Complete authored `order` statement span or omitted value.
    pub origin: SourceOrigin,
}

/// Deterministic source locations associated with normalized semantic identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMap {
    theme: SourceOrigin,
    node_icons: Vec<NodeIconSource>,
    layout_orders: Vec<LayoutOrderSource>,
}

impl SourceMap {
    pub(crate) fn from_document(document: &ast::Document, lossless: &lossless::Document) -> Self {
        let theme = document
            .diagram
            .members
            .iter()
            .find_map(|member| match member {
                DiagramMember::Theme(theme) => Some(SourceOrigin::Authored(theme.identifier.span)),
                _ => None,
            })
            .unwrap_or(SourceOrigin::Omitted);

        let mut node_icons = Vec::new();
        let mut layout_orders = vec![LayoutOrderSource {
            scope: LayoutScope::Diagram,
            origin: layout_order_origin(
                document
                    .diagram
                    .members
                    .iter()
                    .find_map(diagram_member_layout),
                lossless,
            ),
        }];

        for member in &document.diagram.members {
            match member {
                DiagramMember::Node(node) => node_icons.push(node_icon_source(node)),
                DiagramMember::Group(group) => {
                    append_group_sources(group, lossless, &mut node_icons, &mut layout_orders);
                }
                DiagramMember::Edge(_) | DiagramMember::Theme(_) | DiagramMember::Layout(_) => {}
            }
        }

        Self {
            theme,
            node_icons,
            layout_orders,
        }
    }

    /// Returns the authored diagram theme identifier or omitted default.
    pub const fn theme(&self) -> SourceOrigin {
        self.theme
    }

    /// Returns node icon entries in depth-first declaration order.
    pub fn node_icons(&self) -> &[NodeIconSource] {
        &self.node_icons
    }

    /// Finds a node's authored icon string or omitted default by node identifier.
    pub fn node_icon(&self, node_id: &str) -> Option<SourceOrigin> {
        self.node_icons
            .iter()
            .find(|entry| entry.node_id == node_id)
            .map(|entry| entry.origin)
    }

    /// Returns layout order entries with the diagram first, then groups in depth-first order.
    pub fn layout_orders(&self) -> &[LayoutOrderSource] {
        &self.layout_orders
    }

    /// Returns the diagram's authored order statement or omitted value.
    pub fn diagram_order(&self) -> SourceOrigin {
        self.layout_orders
            .first()
            .map_or(SourceOrigin::Omitted, |entry| entry.origin)
    }

    /// Finds a group's authored order statement or omitted value by group identifier.
    pub fn group_order(&self, group_id: &str) -> Option<SourceOrigin> {
        self.layout_orders
            .iter()
            .find_map(|entry| match &entry.scope {
                LayoutScope::Group(identifier) if identifier == group_id => Some(entry.origin),
                LayoutScope::Diagram | LayoutScope::Group(_) => None,
            })
    }
}

fn diagram_member_layout(member: &DiagramMember) -> Option<&ast::Layout> {
    match member {
        DiagramMember::Layout(layout) => Some(layout),
        _ => None,
    }
}

fn group_member_layout(member: &GroupMember) -> Option<&ast::Layout> {
    match member {
        GroupMember::Layout(layout) => Some(layout),
        _ => None,
    }
}

fn node_icon_source(node: &ast::Node) -> NodeIconSource {
    let origin = node
        .properties
        .iter()
        .find_map(|property| match property {
            NodeProperty::Icon(icon) => Some(SourceOrigin::Authored(icon.span)),
            NodeProperty::Kind(_) | NodeProperty::Detail(_) => None,
        })
        .unwrap_or(SourceOrigin::Omitted);

    NodeIconSource {
        node_id: node.identifier.value.clone(),
        origin,
    }
}

fn append_group_sources(
    group: &ast::Group,
    lossless: &lossless::Document,
    node_icons: &mut Vec<NodeIconSource>,
    layout_orders: &mut Vec<LayoutOrderSource>,
) {
    layout_orders.push(LayoutOrderSource {
        scope: LayoutScope::Group(group.identifier.value.clone()),
        origin: layout_order_origin(group.members.iter().find_map(group_member_layout), lossless),
    });

    for member in &group.members {
        match member {
            GroupMember::Node(node) => node_icons.push(node_icon_source(node)),
            GroupMember::Group(child) => {
                append_group_sources(child, lossless, node_icons, layout_orders);
            }
            GroupMember::Layout(_) => {}
        }
    }
}

fn layout_order_origin(
    layout: Option<&ast::Layout>,
    lossless: &lossless::Document,
) -> SourceOrigin {
    let Some(list) = layout.and_then(|layout| {
        layout
            .statements
            .iter()
            .find_map(|statement| match statement {
                LayoutStatement::Order(list) => Some(list),
                LayoutStatement::Direction(_) | LayoutStatement::RankSame(_) => None,
            })
    }) else {
        return SourceOrigin::Omitted;
    };

    SourceOrigin::Authored(order_statement_span(list.span, lossless.tokens()))
}

fn order_statement_span(list_span: Span, tokens: &[lossless::Token]) -> Span {
    let keyword = tokens
        .iter()
        .take_while(|token| token.span.end.byte_offset <= list_span.start.byte_offset)
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::LineComment))
        .last();

    keyword.map_or(list_span, |keyword| Span::covering(keyword.span, list_span))
}

#[cfg(test)]
mod tests {
    use super::SourceOrigin;

    #[test]
    fn source_origin_returns_only_authored_spans() {
        let span = crate::diagnostic::Span::point(crate::diagnostic::SourcePosition::start());
        assert_eq!(SourceOrigin::Authored(span).span(), Some(span));
        assert_eq!(SourceOrigin::Omitted.span(), None);
    }
}
