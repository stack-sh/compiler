use std::collections::HashMap;

use crate::CompileOutput;
use crate::ast::{self, DiagramMember, GroupMember, LayoutStatement, NodeProperty};
use crate::diagnostic::{Diagnostic, Severity, Span, Spanned};
use crate::ir;

const SUPPORTED_MAJOR: u32 = 1;
const SUPPORTED_MINOR: u32 = 0;
const NODE_KINDS: [&str; 10] = [
    "actor", "client", "service", "function", "worker", "database", "cache", "queue", "storage",
    "external",
];
const EDGE_KINDS: [&str; 5] = ["flow", "request", "event", "data", "dependency"];
const LAYOUT_DIRECTIONS: [&str; 2] = ["right", "down"];

pub(crate) fn validate(document: &ast::Document) -> CompileOutput {
    let mut validator = Validator::new(document);
    validator.run();
    validator.diagnostics.sort_by(|left, right| {
        left.span
            .start
            .byte_offset
            .cmp(&right.span.start.byte_offset)
            .then_with(|| severity_order(left.severity).cmp(&severity_order(right.severity)))
            .then_with(|| left.code.cmp(right.code))
            .then_with(|| left.message.cmp(&right.message))
    });

    let has_errors = validator
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    CompileOutput {
        diagram: (!has_errors).then(|| normalize(document)),
        diagnostics: validator.diagnostics,
    }
}

fn severity_order(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 0,
        Severity::Warning => 1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymbolKind {
    Node,
    Group,
}

#[derive(Debug, Clone, Copy)]
struct Symbol {
    kind: SymbolKind,
    span: Span,
}

struct Validator<'document> {
    document: &'document ast::Document,
    diagnostics: Vec<Diagnostic>,
    symbols: HashMap<&'document str, Symbol>,
    node_count: usize,
    group_count: usize,
}

impl<'document> Validator<'document> {
    fn new(document: &'document ast::Document) -> Self {
        Self {
            document,
            diagnostics: Vec::new(),
            symbols: HashMap::new(),
            node_count: 0,
            group_count: 0,
        }
    }

    fn run(&mut self) {
        self.validate_version();
        self.validate_text(&self.document.diagram.title, 1, 80, "diagram title");
        self.collect_declarations();
        self.validate_themes();
        self.validate_layout_scopes();
        self.validate_edges();
        self.validate_complexity();
    }

    fn validate_version(&mut self) {
        let version = &self.document.version;
        if version.major != SUPPORTED_MAJOR || version.minor > SUPPORTED_MINOR {
            self.diagnostics.push(
                Diagnostic::error(
                    "STK2001",
                    format!(
                        "Stack {}.{} is not supported by this compiler.",
                        version.major, version.minor
                    ),
                    version.span,
                )
                .with_expected([format!("{SUPPORTED_MAJOR}.{SUPPORTED_MINOR}")])
                .with_help(format!(
                    "Use Stack {SUPPORTED_MAJOR}.{SUPPORTED_MINOR} or an older compatible minor version."
                )),
            );
        }
    }

    fn collect_declarations(&mut self) {
        for member in &self.document.diagram.members {
            match member {
                DiagramMember::Node(node) => self.collect_node(node),
                DiagramMember::Group(group) => self.collect_group(group, 1),
                DiagramMember::Edge(_) | DiagramMember::Theme(_) | DiagramMember::Layout(_) => {}
            }
        }
    }

    fn collect_node(&mut self, node: &'document ast::Node) {
        self.node_count += 1;
        self.validate_identifier(&node.identifier);
        self.declare(&node.identifier, SymbolKind::Node);
        self.validate_text(&node.label, 1, 60, "node label");
        self.validate_node_properties(node);
    }

    fn collect_group(&mut self, group: &'document ast::Group, depth: usize) {
        self.group_count += 1;
        self.validate_identifier(&group.identifier);
        self.declare(&group.identifier, SymbolKind::Group);
        self.validate_text(&group.label, 1, 60, "group label");

        if depth > 3 {
            self.diagnostics.push(
                Diagnostic::error(
                    "STK3010",
                    "Group nesting exceeds three levels below the diagram.",
                    group.identifier.span,
                )
                .with_help("Move this group to the third level or higher."),
            );
        }

        if descendant_node_count(group) == 0 {
            self.diagnostics.push(
                Diagnostic::error(
                    "STK3009",
                    format!(
                        "Group '{}' does not contain a descendant node.",
                        group.identifier.value
                    ),
                    group.identifier.span,
                )
                .with_help("Add a node to this group or remove the empty boundary."),
            );
        }

        for member in &group.members {
            match member {
                GroupMember::Node(node) => self.collect_node(node),
                GroupMember::Group(child) => self.collect_group(child, depth + 1),
                GroupMember::Layout(_) => {}
            }
        }
    }

    fn declare(&mut self, identifier: &'document Spanned<String>, kind: SymbolKind) {
        if let Some(original) = self.symbols.get(identifier.value.as_str()).copied() {
            self.diagnostics.push(
                Diagnostic::error(
                    "STK3002",
                    format!(
                        "Identifier '{}' is declared more than once.",
                        identifier.value
                    ),
                    identifier.span,
                )
                .with_help("Rename or remove this duplicate declaration.")
                .with_related("The first declaration is here.", original.span),
            );
        } else {
            self.symbols.insert(
                identifier.value.as_str(),
                Symbol {
                    kind,
                    span: identifier.span,
                },
            );
        }
    }

    fn validate_node_properties(&mut self, node: &ast::Node) {
        let mut seen = HashMap::new();
        for property in &node.properties {
            let (name, value) = match property {
                NodeProperty::Kind(value) => ("kind", value),
                NodeProperty::Icon(value) => ("icon", value),
                NodeProperty::Detail(value) => ("detail", value),
            };
            self.reject_duplicate_property(name, property.span(), &mut seen);

            match property {
                NodeProperty::Kind(value) => {
                    self.validate_identifier(value);
                    if parse_node_kind(&value.value).is_none() {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "STK2002",
                                format!("Unknown node kind '{}'.", value.value),
                                value.span,
                            )
                            .with_expected(NODE_KINDS)
                            .with_help("Choose one of the supported node kinds."),
                        );
                    }
                }
                NodeProperty::Icon(value) => self.validate_icon_identifier(value),
                NodeProperty::Detail(value) => self.validate_text(value, 1, 80, "node detail"),
            }

            let _ = value;
        }
    }

    fn validate_themes(&mut self) {
        let mut first = None;
        for member in &self.document.diagram.members {
            let DiagramMember::Theme(theme) = member else {
                continue;
            };
            self.validate_identifier(&theme.identifier);
            if let Some(first_span) = first {
                self.diagnostics.push(
                    Diagnostic::error(
                        "STK3014",
                        "A diagram may contain only one theme statement.",
                        theme.span,
                    )
                    .with_help("Remove the duplicate theme statement.")
                    .with_related("The first theme statement is here.", first_span),
                );
            } else {
                first = Some(theme.span);
            }
        }
    }

    fn validate_layout_scopes(&mut self) {
        let root_children = direct_diagram_children(&self.document.diagram);
        let root_layouts: Vec<_> = self
            .document
            .diagram
            .members
            .iter()
            .filter_map(|member| match member {
                DiagramMember::Layout(layout) => Some(layout),
                _ => None,
            })
            .collect();
        self.validate_layout_blocks(&root_layouts, &root_children);

        for member in &self.document.diagram.members {
            if let DiagramMember::Group(group) = member {
                self.validate_group_layout_scopes(group);
            }
        }
    }

    fn validate_group_layout_scopes(&mut self, group: &ast::Group) {
        let children = direct_group_children(group);
        let layouts: Vec<_> = group
            .members
            .iter()
            .filter_map(|member| match member {
                GroupMember::Layout(layout) => Some(layout),
                _ => None,
            })
            .collect();
        self.validate_layout_blocks(&layouts, &children);

        for member in &group.members {
            if let GroupMember::Group(child) = member {
                self.validate_group_layout_scopes(child);
            }
        }
    }

    fn validate_layout_blocks(
        &mut self,
        layouts: &[&ast::Layout],
        direct_children: &HashMap<&str, Span>,
    ) {
        if let Some((first, rest)) = layouts.split_first() {
            for duplicate in rest {
                self.diagnostics.push(
                    Diagnostic::error(
                        "STK3012",
                        "A layout scope may contain only one layout block.",
                        duplicate.span,
                    )
                    .with_help("Remove the duplicate layout block.")
                    .with_related("The first layout block is here.", first.span),
                );
            }
        }

        for layout in layouts {
            self.validate_layout(layout, direct_children);
        }
    }

    fn validate_layout(&mut self, layout: &ast::Layout, direct_children: &HashMap<&str, Span>) {
        let mut first_direction = None;
        let mut first_order = None;
        let mut ranked_children = HashMap::new();

        for statement in &layout.statements {
            match statement {
                LayoutStatement::Direction(value) => {
                    self.validate_identifier(value);
                    if !matches!(value.value.as_str(), "right" | "down") {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "STK2002",
                                format!("Unknown layout direction '{}'.", value.value),
                                value.span,
                            )
                            .with_expected(LAYOUT_DIRECTIONS)
                            .with_help(
                                "Use 'right' for horizontal flow or 'down' for vertical flow.",
                            ),
                        );
                    }
                    self.reject_duplicate_singleton(
                        "direction statement",
                        statement.span(),
                        &mut first_direction,
                    );
                }
                LayoutStatement::RankSame(list) => {
                    self.validate_layout_list(list, direct_children);
                    for identifier in &list.identifiers {
                        if let Some(original) = ranked_children.get(identifier.value.as_str()) {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "STK3011",
                                    format!(
                                        "Layout child '{}' occurs in more than one same-rank statement.",
                                        identifier.value
                                    ),
                                    identifier.span,
                                )
                                .with_help("Keep this child in only one same-rank statement.")
                                .with_related("The child was first ranked here.", *original),
                            );
                        } else {
                            ranked_children.insert(identifier.value.as_str(), identifier.span);
                        }
                    }
                }
                LayoutStatement::Order(list) => {
                    self.validate_layout_list(list, direct_children);
                    self.reject_duplicate_singleton(
                        "order statement",
                        statement.span(),
                        &mut first_order,
                    );
                }
            }
        }
    }

    fn validate_layout_list(
        &mut self,
        list: &ast::IdentifierList,
        direct_children: &HashMap<&str, Span>,
    ) {
        let mut seen = HashMap::new();
        for identifier in &list.identifiers {
            let identifier_is_valid = self.validate_identifier(identifier);
            if identifier_is_valid && !direct_children.contains_key(identifier.value.as_str()) {
                let suggestions = identifier_suggestions(
                    &identifier.value,
                    direct_children.iter().map(|(name, span)| (*name, *span)),
                );
                let expected = suggestions
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect::<Vec<_>>();
                let mut diagnostic = Diagnostic::error(
                    "STK3011",
                    format!(
                        "Layout reference '{}' is not a direct child of this scope.",
                        identifier.value
                    ),
                    identifier.span,
                )
                .with_expected(expected.clone());
                diagnostic = if expected.is_empty() {
                    diagnostic.with_help(
                        "Reference a node or group declared directly in this layout scope.",
                    )
                } else {
                    diagnostic.with_help(format!(
                        "Use a direct child such as {}.",
                        expected.join(", ")
                    ))
                };
                for (name, span) in suggestions {
                    diagnostic = diagnostic
                        .with_related(format!("Direct child '{name}' is declared here."), span);
                }
                self.diagnostics.push(diagnostic);
            }

            if let Some(original) = seen.insert(identifier.value.as_str(), identifier.span) {
                self.diagnostics.push(
                    Diagnostic::error(
                        "STK3011",
                        format!(
                            "Layout reference '{}' occurs more than once in the same list.",
                            identifier.value
                        ),
                        identifier.span,
                    )
                    .with_help("Remove the repeated reference from this list.")
                    .with_related("The first occurrence is here.", original),
                );
            }
        }
    }

    fn validate_edges(&mut self) {
        let mut duplicate_edges = HashMap::new();
        let mut degree: HashMap<&str, usize> = HashMap::new();

        for member in &self.document.diagram.members {
            let DiagramMember::Edge(edge) = member else {
                continue;
            };

            let from_is_node = self.validate_edge_endpoint(&edge.from);
            let to_is_node = self.validate_edge_endpoint(&edge.to);
            self.validate_optional_text(edge.label.as_ref(), 1, 40, "edge label");
            let edge_kind = self.validate_edge_properties(edge);

            if from_is_node {
                *degree.entry(edge.from.value.as_str()).or_default() += 1;
            }
            if to_is_node && edge.to.value != edge.from.value {
                *degree.entry(edge.to.value.as_str()).or_default() += 1;
            }

            if from_is_node && to_is_node && edge.from.value == edge.to.value {
                self.diagnostics.push(
                    Diagnostic::error(
                        "STK3005",
                        format!("Edge connects node '{}' to itself.", edge.from.value),
                        edge.span,
                    )
                    .with_help("Connect two different nodes or remove this edge."),
                );
            }

            if let Some(edge_kind) =
                edge_kind.filter(|_| from_is_node && to_is_node && edge.from.value != edge.to.value)
            {
                let key = edge_key(edge, edge_kind);
                if let Some(original) = duplicate_edges.insert(key, edge.span) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "STK3006",
                            "An exact duplicate edge is declared.",
                            edge.span,
                        )
                        .with_help("Remove this edge or change its endpoints, kind, or label.")
                        .with_related("The first edge is here.", original),
                    );
                }
            }
        }

        self.warn_dense_nodes(&degree);
    }

    fn validate_edge_endpoint(&mut self, endpoint: &Spanned<String>) -> bool {
        if !self.validate_identifier(endpoint) {
            return false;
        }

        match self.symbols.get(endpoint.value.as_str()).copied() {
            Some(Symbol {
                kind: SymbolKind::Node,
                ..
            }) => true,
            Some(Symbol {
                kind: SymbolKind::Group,
                ..
            }) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "STK3004",
                        format!(
                            "Group '{}' cannot be used as an edge endpoint.",
                            endpoint.value
                        ),
                        endpoint.span,
                    )
                    .with_help("Connect the participating node inside the group."),
                );
                false
            }
            None => {
                let suggestions = identifier_suggestions(
                    &endpoint.value,
                    self.symbols.iter().filter_map(|(name, symbol)| {
                        (symbol.kind == SymbolKind::Node).then_some((*name, symbol.span))
                    }),
                );
                let expected = suggestions
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect::<Vec<_>>();
                let mut diagnostic = Diagnostic::error(
                    "STK3003",
                    format!("Unknown node '{}'.", endpoint.value),
                    endpoint.span,
                )
                .with_expected(expected.clone());
                diagnostic = if expected.is_empty() {
                    diagnostic.with_help(
                        "Declare this node or replace it with an existing node identifier.",
                    )
                } else {
                    diagnostic.with_help(format!(
                        "Use a declared node such as {}.",
                        expected.join(", ")
                    ))
                };
                for (name, span) in suggestions {
                    diagnostic =
                        diagnostic.with_related(format!("Node '{name}' is declared here."), span);
                }
                self.diagnostics.push(diagnostic);
                false
            }
        }
    }

    fn validate_edge_properties(&mut self, edge: &ast::Edge) -> Option<&'static str> {
        let mut seen = HashMap::new();
        let mut effective = Some("flow");
        for property in &edge.properties {
            let ast::EdgeProperty::Kind(value) = property;
            self.reject_duplicate_property("kind", property.span(), &mut seen);
            self.validate_identifier(value);
            if let Some(kind) = parse_edge_kind(&value.value) {
                effective = Some(kind.as_str());
            } else {
                self.diagnostics.push(
                    Diagnostic::error(
                        "STK2002",
                        format!("Unknown edge kind '{}'.", value.value),
                        value.span,
                    )
                    .with_expected(EDGE_KINDS)
                    .with_help("Choose one of the supported edge kinds."),
                );
                effective = None;
            }
        }
        effective
    }

    fn warn_dense_nodes(&mut self, degree: &HashMap<&str, usize>) {
        visit_nodes(&self.document.diagram, &mut |node| {
            if let Some(&count) = degree.get(node.identifier.value.as_str()) {
                if count > 12 {
                    self.diagnostics.push(
                        Diagnostic::warning(
                            "STK4002",
                            format!(
                                "Node '{}' has {count} incident edges; more than 12 may reduce legibility.",
                                node.identifier.value
                            ),
                            node.identifier.span,
                        )
                        .with_help("Consider splitting the diagram into more focused views."),
                    );
                }
            }
        });
    }

    fn validate_complexity(&mut self) {
        let edge_count = self
            .document
            .diagram
            .members
            .iter()
            .filter(|member| matches!(member, DiagramMember::Edge(_)))
            .count();
        if !(1..=40).contains(&self.node_count) {
            self.diagnostics.push(
                Diagnostic::error(
                    "STK4003",
                    format!(
                        "A diagram must contain between 1 and 40 nodes; found {}.",
                        self.node_count
                    ),
                    self.document.diagram.span,
                )
                .with_help("Add nodes or split the diagram to stay within 1 to 40 nodes."),
            );
        }
        if self.group_count > 12 {
            self.diagnostics.push(
                Diagnostic::error(
                    "STK4003",
                    format!(
                        "A diagram may contain at most 12 groups; found {}.",
                        self.group_count
                    ),
                    self.document.diagram.span,
                )
                .with_help("Remove groups or split the diagram into focused views."),
            );
        }

        let maximum_edges = 80.min(self.node_count.saturating_mul(2));
        if edge_count > maximum_edges {
            self.diagnostics.push(
                Diagnostic::error(
                    "STK4003",
                    format!(
                        "A diagram with {} nodes may contain at most {maximum_edges} edges; found {edge_count}.",
                        self.node_count
                    ),
                    self.document.diagram.span,
                )
                .with_help("Remove edges or split the diagram into focused views."),
            );
        }
    }

    fn validate_identifier(&mut self, identifier: &Spanned<String>) -> bool {
        if is_identifier(&identifier.value) {
            true
        } else {
            self.diagnostics.push(
                Diagnostic::error(
                    "STK3001",
                    format!("Identifier '{}' is invalid.", identifier.value),
                    identifier.span,
                )
                .with_help(
                    "Use 1 to 64 lowercase ASCII letters, digits, underscores, or hyphens, starting with a letter.",
                ),
            );
            false
        }
    }

    fn validate_icon_identifier(&mut self, identifier: &Spanned<String>) {
        if !is_icon_identifier(&identifier.value) {
            self.diagnostics.push(
                Diagnostic::error(
                    "STK3013",
                    format!("Icon identifier '{}' is malformed.", identifier.value),
                    identifier.span,
                )
                .with_help(
                    "Use an icon name of 1 to 64 lowercase ASCII letters, digits, or hyphens, optionally prefixed by a lowercase provider namespace and one colon.",
                ),
            );
        }
    }

    fn validate_optional_text(
        &mut self,
        value: Option<&Spanned<String>>,
        minimum: usize,
        maximum: usize,
        description: &str,
    ) {
        if let Some(value) = value {
            self.validate_text(value, minimum, maximum, description);
        }
    }

    fn validate_text(
        &mut self,
        value: &Spanned<String>,
        minimum: usize,
        maximum: usize,
        description: &str,
    ) {
        let length = value.value.chars().count();
        let boundary_whitespace = value.value.chars().next().is_some_and(char::is_whitespace)
            || value
                .value
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace);
        if !(minimum..=maximum).contains(&length) || boundary_whitespace {
            self.diagnostics.push(
                Diagnostic::error(
                    "STK3008",
                    format!(
                        "The {description} must contain {minimum} to {maximum} Unicode scalar values without leading or trailing whitespace."
                    ),
                    value.span,
                )
                .with_help(format!(
                    "Trim the text and keep its length between {minimum} and {maximum}."
                )),
            );
        }
    }

    fn reject_duplicate_property(
        &mut self,
        name: &'static str,
        span: Span,
        seen: &mut HashMap<&'static str, Span>,
    ) {
        if let Some(original) = seen.insert(name, span) {
            self.diagnostics.push(
                Diagnostic::error(
                    "STK3007",
                    format!("Property '{name}' occurs more than once in the same block."),
                    span,
                )
                .with_help("Remove the duplicate property.")
                .with_related("The first property is here.", original),
            );
        }
    }

    fn reject_duplicate_singleton(
        &mut self,
        description: &str,
        span: Span,
        first: &mut Option<Span>,
    ) {
        if let Some(original) = first {
            self.diagnostics.push(
                Diagnostic::error(
                    "STK3012",
                    format!("A layout block may contain only one {description}."),
                    span,
                )
                .with_help("Remove the duplicate layout statement.")
                .with_related("The first occurrence is here.", *original),
            );
        } else {
            *first = Some(span);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct EdgeKey {
    from: String,
    to: String,
    operator: ast::EdgeOperator,
    label: Option<String>,
    kind: &'static str,
}

fn edge_key(edge: &ast::Edge, kind: &'static str) -> EdgeKey {
    let (from, to) = match edge.operator.value {
        ast::EdgeOperator::Forward => (edge.from.value.clone(), edge.to.value.clone()),
        ast::EdgeOperator::Bidirectional | ast::EdgeOperator::Association => {
            if edge.from.value <= edge.to.value {
                (edge.from.value.clone(), edge.to.value.clone())
            } else {
                (edge.to.value.clone(), edge.from.value.clone())
            }
        }
    };
    EdgeKey {
        from,
        to,
        operator: edge.operator.value,
        label: edge.label.as_ref().map(|label| label.value.clone()),
        kind,
    }
}

fn identifier_suggestions<'candidate>(
    authored: &str,
    candidates: impl Iterator<Item = (&'candidate str, Span)>,
) -> Vec<(String, Span)> {
    let authored_length = authored.chars().count();
    let mut suggestions = candidates
        .filter_map(|(candidate, span)| {
            let distance = levenshtein(authored, candidate);
            let threshold = 1.max(authored_length.max(candidate.chars().count()) / 3);
            (distance <= threshold).then_some((distance, candidate, span))
        })
        .collect::<Vec<_>>();
    suggestions.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.as_bytes().cmp(right.1.as_bytes()))
    });
    suggestions.truncate(3);
    suggestions
        .into_iter()
        .map(|(_, name, span)| (name.to_owned(), span))
        .collect()
}

fn levenshtein(left: &str, right: &str) -> usize {
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];

    for (left_index, left_character) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_character) in right.iter().copied().enumerate() {
            let substitution =
                previous[right_index] + usize::from(left_character != right_character);
            current[right_index + 1] = (current[right_index] + 1)
                .min(previous[right_index + 1] + 1)
                .min(substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right.len()]
}

fn is_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes[1..].iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn is_icon_identifier(value: &str) -> bool {
    match value.split_once(':') {
        Some((provider, icon)) => {
            !icon.contains(':') && is_provider_namespace(provider) && is_icon_name(icon)
        }
        None => is_icon_name(value),
    }
}

fn is_provider_namespace(value: &str) -> bool {
    let bytes = value.as_bytes();
    (2..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn is_icon_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=64).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn parse_node_kind(value: &str) -> Option<ir::NodeKind> {
    Some(match value {
        "actor" => ir::NodeKind::Actor,
        "client" => ir::NodeKind::Client,
        "service" => ir::NodeKind::Service,
        "function" => ir::NodeKind::Function,
        "worker" => ir::NodeKind::Worker,
        "database" => ir::NodeKind::Database,
        "cache" => ir::NodeKind::Cache,
        "queue" => ir::NodeKind::Queue,
        "storage" => ir::NodeKind::Storage,
        "external" => ir::NodeKind::External,
        _ => return None,
    })
}

fn parse_edge_kind(value: &str) -> Option<ir::EdgeKind> {
    Some(match value {
        "flow" => ir::EdgeKind::Flow,
        "request" => ir::EdgeKind::Request,
        "event" => ir::EdgeKind::Event,
        "data" => ir::EdgeKind::Data,
        "dependency" => ir::EdgeKind::Dependency,
        _ => return None,
    })
}

impl ir::EdgeKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::Request => "request",
            Self::Event => "event",
            Self::Data => "data",
            Self::Dependency => "dependency",
        }
    }
}

fn descendant_node_count(group: &ast::Group) -> usize {
    group
        .members
        .iter()
        .map(|member| match member {
            GroupMember::Node(_) => 1,
            GroupMember::Group(group) => descendant_node_count(group),
            GroupMember::Layout(_) => 0,
        })
        .sum()
}

fn direct_diagram_children(diagram: &ast::Diagram) -> HashMap<&str, Span> {
    diagram
        .members
        .iter()
        .filter_map(|member| match member {
            DiagramMember::Node(node) => {
                Some((node.identifier.value.as_str(), node.identifier.span))
            }
            DiagramMember::Group(group) => {
                Some((group.identifier.value.as_str(), group.identifier.span))
            }
            DiagramMember::Edge(_) | DiagramMember::Theme(_) | DiagramMember::Layout(_) => None,
        })
        .collect()
}

fn direct_group_children(group: &ast::Group) -> HashMap<&str, Span> {
    group
        .members
        .iter()
        .filter_map(|member| match member {
            GroupMember::Node(node) => Some((node.identifier.value.as_str(), node.identifier.span)),
            GroupMember::Group(group) => {
                Some((group.identifier.value.as_str(), group.identifier.span))
            }
            GroupMember::Layout(_) => None,
        })
        .collect()
}

fn visit_nodes<'ast>(diagram: &'ast ast::Diagram, visitor: &mut impl FnMut(&'ast ast::Node)) {
    for member in &diagram.members {
        match member {
            DiagramMember::Node(node) => visitor(node),
            DiagramMember::Group(group) => visit_group_nodes(group, visitor),
            DiagramMember::Edge(_) | DiagramMember::Theme(_) | DiagramMember::Layout(_) => {}
        }
    }
}

fn visit_group_nodes<'ast>(group: &'ast ast::Group, visitor: &mut impl FnMut(&'ast ast::Node)) {
    for member in &group.members {
        match member {
            GroupMember::Node(node) => visitor(node),
            GroupMember::Group(group) => visit_group_nodes(group, visitor),
            GroupMember::Layout(_) => {}
        }
    }
}

fn normalize(document: &ast::Document) -> ir::Diagram {
    let mut nodes = Vec::new();
    let mut groups = Vec::new();
    let children = document
        .diagram
        .members
        .iter()
        .filter_map(|member| match member {
            DiagramMember::Node(node) => Some(ir::ElementId::Node(node.identifier.value.clone())),
            DiagramMember::Group(group) => {
                Some(ir::ElementId::Group(group.identifier.value.clone()))
            }
            DiagramMember::Edge(_) | DiagramMember::Theme(_) | DiagramMember::Layout(_) => None,
        })
        .collect();

    for member in &document.diagram.members {
        match member {
            DiagramMember::Node(node) => nodes.push(normalize_node(node, None)),
            DiagramMember::Group(group) => normalize_group(group, None, &mut nodes, &mut groups),
            DiagramMember::Edge(_) | DiagramMember::Theme(_) | DiagramMember::Layout(_) => {}
        }
    }

    let edges = document
        .diagram
        .members
        .iter()
        .filter_map(|member| match member {
            DiagramMember::Edge(edge) => Some(normalize_edge(edge)),
            _ => None,
        })
        .collect();
    let selected_theme = document
        .diagram
        .members
        .iter()
        .find_map(|member| match member {
            DiagramMember::Theme(theme) => Some(theme.identifier.value.clone()),
            _ => None,
        });
    let theme_id = match selected_theme {
        Some(theme_id) => theme_id,
        None => "default".to_owned(),
    };
    let layout = document
        .diagram
        .members
        .iter()
        .find_map(|member| match member {
            DiagramMember::Layout(layout) => Some(normalize_layout(layout)),
            _ => None,
        });

    ir::Diagram {
        language_version: ir::LanguageVersion {
            major: document.version.major,
            minor: document.version.minor,
        },
        title: document.diagram.title.value.clone(),
        theme_id,
        children,
        nodes,
        groups,
        edges,
        layout,
    }
}

fn normalize_node(node: &ast::Node, parent_group_id: Option<&str>) -> ir::Node {
    let mut kind = ir::NodeKind::Service;
    let mut icon_id = None;
    let mut detail = None;
    for property in &node.properties {
        match property {
            NodeProperty::Kind(value) => {
                if let Some(parsed_kind) = parse_node_kind(&value.value) {
                    kind = parsed_kind;
                }
            }
            NodeProperty::Icon(value) => icon_id = Some(value.value.clone()),
            NodeProperty::Detail(value) => detail = Some(value.value.clone()),
        }
    }
    ir::Node {
        id: node.identifier.value.clone(),
        label: node.label.value.clone(),
        kind,
        icon_id,
        detail,
        parent_group_id: parent_group_id.map(str::to_owned),
    }
}

fn normalize_group(
    group: &ast::Group,
    parent_group_id: Option<&str>,
    nodes: &mut Vec<ir::Node>,
    groups: &mut Vec<ir::Group>,
) {
    let children = group
        .members
        .iter()
        .filter_map(|member| match member {
            GroupMember::Node(node) => Some(ir::ElementId::Node(node.identifier.value.clone())),
            GroupMember::Group(group) => Some(ir::ElementId::Group(group.identifier.value.clone())),
            GroupMember::Layout(_) => None,
        })
        .collect();
    let layout = group.members.iter().find_map(|member| match member {
        GroupMember::Layout(layout) => Some(normalize_layout(layout)),
        _ => None,
    });
    groups.push(ir::Group {
        id: group.identifier.value.clone(),
        label: group.label.value.clone(),
        parent_group_id: parent_group_id.map(str::to_owned),
        children,
        layout,
    });

    for member in &group.members {
        match member {
            GroupMember::Node(node) => {
                nodes.push(normalize_node(node, Some(&group.identifier.value)))
            }
            GroupMember::Group(child) => {
                normalize_group(child, Some(&group.identifier.value), nodes, groups)
            }
            GroupMember::Layout(_) => {}
        }
    }
}

fn normalize_edge(edge: &ast::Edge) -> ir::Edge {
    let authored_kind = edge.properties.iter().find_map(|property| match property {
        ast::EdgeProperty::Kind(value) => parse_edge_kind(&value.value),
    });
    let kind = match authored_kind {
        Some(kind) => kind,
        None => ir::EdgeKind::Flow,
    };
    let direction = match edge.operator.value {
        ast::EdgeOperator::Forward => ir::EdgeDirection::Forward,
        ast::EdgeOperator::Bidirectional => ir::EdgeDirection::Bidirectional,
        ast::EdgeOperator::Association => ir::EdgeDirection::Association,
    };
    ir::Edge {
        from: edge.from.value.clone(),
        to: edge.to.value.clone(),
        direction,
        kind,
        label: edge.label.as_ref().map(|label| label.value.clone()),
    }
}

fn normalize_layout(layout: &ast::Layout) -> ir::Layout {
    let mut direction = None;
    let mut same_ranks = Vec::new();
    let mut order = None;
    for statement in &layout.statements {
        match statement {
            LayoutStatement::Direction(value) => {
                direction = match value.value.as_str() {
                    "right" => Some(ir::Direction::Right),
                    "down" => Some(ir::Direction::Down),
                    _ => None,
                };
            }
            LayoutStatement::RankSame(list) => same_ranks.push(
                list.identifiers
                    .iter()
                    .map(|identifier| identifier.value.clone())
                    .collect(),
            ),
            LayoutStatement::Order(list) => {
                order = Some(
                    list.identifiers
                        .iter()
                        .map(|identifier| identifier.value.clone())
                        .collect(),
                );
            }
        }
    }
    ir::Layout {
        direction,
        same_ranks,
        order,
    }
}

#[cfg(test)]
mod tests;
