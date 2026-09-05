//! Stateless, protocol-neutral language intelligence for one source snapshot.

use std::{collections::BTreeSet, error::Error, fmt};

use crate::{
    ast, compile, compile_bytes,
    diagnostic::{Diagnostic, SourcePosition, Span},
    lexer::{self, Token, TokenKind},
    parse,
};

/// Portable language-intelligence schema version implemented by this crate.
pub const SCHEMA_VERSION: &str = "1.0";

/// Maximum number of caller-owned icons accepted for one completion request.
pub const MAX_COMPLETION_ICONS: usize = 4_096;

/// Caller-owned icon metadata available to completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCatalogEntry {
    /// Exact unnamespaced or provider-namespaced Stack icon identifier.
    pub id: String,
    /// User-visible plain-text label.
    pub label: String,
    /// Optional plain-text secondary label.
    pub detail: Option<String>,
    /// Optional plain-text documentation.
    pub documentation: Option<String>,
}

/// Explicit, caller-owned catalog data for one completion request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletionCatalog {
    /// Available icon entries. The compiler never loads these from storage or a network.
    pub icons: Vec<CompletionCatalogEntry>,
}

/// A source replacement interpreted against the unchanged input snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// End-exclusive source range replaced by this edit.
    pub range: Span,
    /// Literal Stack source inserted in place of the range.
    pub new_text: String,
}

/// Semantic category of one completion item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// A grammatical Stack keyword.
    Keyword,
    /// A property or layout statement valid in the current block.
    Property,
    /// A closed value from the Stack language specification.
    EnumValue,
    /// A document-local semantic identifier.
    Identifier,
    /// An icon supplied by the caller-owned completion catalog.
    Icon,
}

/// One literal, protocol-neutral source completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    /// User-visible plain-text label.
    pub label: String,
    /// Semantic completion category.
    pub kind: CompletionKind,
    /// Optional plain-text secondary label.
    pub detail: Option<String>,
    /// Optional plain-text documentation.
    pub documentation: Option<String>,
    /// Plain string used by consumers for filtering.
    pub filter_text: String,
    /// Stable ordering key.
    pub sort_text: String,
    /// Literal source replacement for this item.
    pub edit: TextEdit,
}

/// Completion result for one caller-owned document version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionOutput {
    /// Portable schema version.
    pub schema_version: &'static str,
    /// Document version supplied by the caller.
    pub document_version: u64,
    /// Ordered compiler diagnostics for the same source snapshot.
    pub diagnostics: Vec<Diagnostic>,
    /// Whether more source context may materially change the list.
    pub is_incomplete: bool,
    /// Deterministically ordered completion items.
    pub items: Vec<CompletionItem>,
}

/// Compiler diagnostics for one caller-owned document version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsOutput {
    /// Portable schema version.
    pub schema_version: &'static str,
    /// Document version supplied by the caller.
    pub document_version: u64,
    /// Ordered compiler diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

/// Operational failure at the language-intelligence input boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntelligenceError {
    /// The supplied source position is outside the source or its coordinates disagree.
    InvalidPosition,
    /// The caller supplied more icon entries than the bounded contract permits.
    CompletionCatalogTooLarge,
    /// One catalog entry violates the portable identifier or text bounds.
    InvalidCompletionCatalogEntry {
        /// Zero-based catalog entry index.
        index: usize,
    },
    /// An icon identifier occurs more than once in the catalog.
    DuplicateCompletionCatalogId {
        /// Zero-based index of the repeated entry.
        index: usize,
    },
}

impl fmt::Display for IntelligenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPosition => formatter.write_str("source position is invalid"),
            Self::CompletionCatalogTooLarge => {
                formatter.write_str("completion catalog exceeds the item limit")
            }
            Self::InvalidCompletionCatalogEntry { index } => {
                write!(formatter, "completion catalog entry {index} is invalid")
            }
            Self::DuplicateCompletionCatalogId { index } => {
                write!(
                    formatter,
                    "completion catalog entry {index} repeats an icon id"
                )
            }
        }
    }
}

impl Error for IntelligenceError {}

/// Compiles UTF-8 Stack source and returns portable diagnostics for one snapshot.
pub fn diagnostics(source: &str, document_version: u64) -> DiagnosticsOutput {
    diagnostics_bytes(source.as_bytes(), document_version)
}

/// Decodes and compiles Stack source bytes for one snapshot.
pub fn diagnostics_bytes(source: &[u8], document_version: u64) -> DiagnosticsOutput {
    DiagnosticsOutput {
        schema_version: SCHEMA_VERSION,
        document_version,
        diagnostics: compile_bytes(source).diagnostics,
    }
}

/// Computes context-aware completion for one complete UTF-8 source snapshot.
pub fn completion(
    source: &str,
    document_version: u64,
    position: SourcePosition,
    catalog: &CompletionCatalog,
) -> Result<CompletionOutput, IntelligenceError> {
    validate_position(source, position)?;
    validate_catalog(catalog)?;

    let parsed = parse(source);
    let is_incomplete = parsed.document.is_none();
    let compiled = compile(source);
    let Ok(tokens) = lexer::tokenize(source) else {
        return Ok(CompletionOutput {
            schema_version: SCHEMA_VERSION,
            document_version,
            diagnostics: compiled.diagnostics,
            is_incomplete: true,
            items: Vec::new(),
        });
    };

    let cursor = position.byte_offset;
    let active_index = active_token_index(&tokens, cursor);
    let current_index = active_index.unwrap_or_else(|| token_index_at_or_after(&tokens, cursor));
    let (replacement, prefix) = replacement_and_prefix(source, &tokens, active_index, position)?;
    let scopes = scopes_before(&tokens, current_index);
    let context = completion_context(&tokens, current_index, scopes.last().copied());
    let nodes = parsed
        .document
        .as_ref()
        .map(document_nodes)
        .unwrap_or_default();
    let candidates = candidates_for(context, catalog, &nodes);
    let mut items: Vec<_> = candidates
        .into_iter()
        .filter(|candidate| candidate.filter_text.starts_with(&prefix))
        .map(|candidate| candidate.into_item(replacement))
        .collect();
    items.sort_by(|left, right| {
        left.sort_text
            .cmp(&right.sort_text)
            .then_with(|| left.label.cmp(&right.label))
    });

    Ok(CompletionOutput {
        schema_version: SCHEMA_VERSION,
        document_version,
        diagnostics: compiled.diagnostics,
        is_incomplete,
        items,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Diagram,
    Group,
    Node,
    Edge,
    Layout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CompletionContext {
    Root,
    DiagramMember,
    GroupMember,
    NodeProperty,
    EdgeProperty,
    LayoutStatement,
    NodeKind,
    EdgeKind,
    Direction,
    RankRelation,
    Icon,
    EdgeEndpoint { excluded: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    label: String,
    kind: CompletionKind,
    detail: Option<String>,
    documentation: Option<String>,
    filter_text: String,
    sort_text: String,
    new_text: String,
}

impl Candidate {
    fn literal(label: &str, kind: CompletionKind, detail: &str, order: usize) -> Self {
        Self {
            label: label.into(),
            kind,
            detail: Some(detail.into()),
            documentation: None,
            filter_text: label.into(),
            sort_text: format!("{order:04}:{label}"),
            new_text: label.into(),
        }
    }

    fn into_item(self, range: Span) -> CompletionItem {
        CompletionItem {
            label: self.label,
            kind: self.kind,
            detail: self.detail,
            documentation: self.documentation,
            filter_text: self.filter_text,
            sort_text: self.sort_text,
            edit: TextEdit {
                range,
                new_text: self.new_text,
            },
        }
    }
}

fn candidates_for(
    context: CompletionContext,
    catalog: &CompletionCatalog,
    nodes: &[(String, String)],
) -> Vec<Candidate> {
    match context {
        CompletionContext::Root => literals(
            &["stack", "diagram"],
            CompletionKind::Keyword,
            "document keyword",
        ),
        CompletionContext::DiagramMember => literals(
            &["node", "group", "edge", "theme", "layout"],
            CompletionKind::Keyword,
            "diagram member",
        ),
        CompletionContext::GroupMember => literals(
            &["node", "group", "layout"],
            CompletionKind::Keyword,
            "group member",
        ),
        CompletionContext::NodeProperty => literals(
            &["kind", "icon", "detail"],
            CompletionKind::Property,
            "node property",
        ),
        CompletionContext::EdgeProperty => {
            literals(&["kind"], CompletionKind::Property, "edge property")
        }
        CompletionContext::LayoutStatement => literals(
            &["direction", "rank", "order"],
            CompletionKind::Property,
            "layout statement",
        ),
        CompletionContext::NodeKind => literals(
            &[
                "actor", "client", "service", "function", "worker", "database", "cache", "queue",
                "storage", "external",
            ],
            CompletionKind::EnumValue,
            "node kind",
        ),
        CompletionContext::EdgeKind => literals(
            &["flow", "request", "event", "data", "dependency"],
            CompletionKind::EnumValue,
            "edge kind",
        ),
        CompletionContext::Direction => literals(
            &["right", "down"],
            CompletionKind::EnumValue,
            "layout direction",
        ),
        CompletionContext::RankRelation => {
            literals(&["same"], CompletionKind::Keyword, "rank relation")
        }
        CompletionContext::Icon => {
            let mut icons: Vec<_> = catalog.icons.iter().collect();
            icons.sort_by(|left, right| left.id.cmp(&right.id));
            icons
                .into_iter()
                .map(|entry| Candidate {
                    label: entry.label.clone(),
                    kind: CompletionKind::Icon,
                    detail: entry.detail.clone(),
                    documentation: entry.documentation.clone(),
                    filter_text: entry.id.clone(),
                    sort_text: entry.id.clone(),
                    new_text: entry.id.clone(),
                })
                .collect()
        }
        CompletionContext::EdgeEndpoint { excluded } => nodes
            .iter()
            .enumerate()
            .filter(|(_, (identifier, _))| excluded.as_ref() != Some(identifier))
            .map(|(index, (identifier, label))| Candidate {
                label: identifier.clone(),
                kind: CompletionKind::Identifier,
                detail: Some(format!("node · {label}")),
                documentation: None,
                filter_text: identifier.clone(),
                sort_text: format!("{:04}:{identifier}", index + 1),
                new_text: identifier.clone(),
            })
            .collect(),
    }
}

fn literals(values: &[&str], kind: CompletionKind, detail: &str) -> Vec<Candidate> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| Candidate::literal(value, kind, detail, index + 1))
        .collect()
}

fn completion_context(
    tokens: &[Token],
    current_index: usize,
    scope: Option<Scope>,
) -> CompletionContext {
    let previous_index = current_index.checked_sub(1);
    let previous = previous_index.and_then(|index| tokens.get(index));
    if bare_value(previous) == Some("kind") {
        return match scope {
            Some(Scope::Edge) => CompletionContext::EdgeKind,
            _ => CompletionContext::NodeKind,
        };
    }
    if bare_value(previous) == Some("direction") {
        return CompletionContext::Direction;
    }
    if bare_value(previous) == Some("rank") {
        return CompletionContext::RankRelation;
    }
    if bare_value(previous) == Some("icon") {
        return CompletionContext::Icon;
    }
    if previous.is_some_and(|token| {
        matches!(
            token.kind,
            TokenKind::ForwardArrow | TokenKind::BidirectionalArrow | TokenKind::Association
        )
    }) {
        let excluded = previous_index
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| tokens.get(index))
            .and_then(|token| bare_value(Some(token)))
            .map(str::to_owned);
        return CompletionContext::EdgeEndpoint { excluded };
    }
    if bare_value(previous) == Some("edge") {
        return CompletionContext::EdgeEndpoint { excluded: None };
    }

    match scope {
        Some(Scope::Diagram) => CompletionContext::DiagramMember,
        Some(Scope::Group) => CompletionContext::GroupMember,
        Some(Scope::Node) => CompletionContext::NodeProperty,
        Some(Scope::Edge) => CompletionContext::EdgeProperty,
        Some(Scope::Layout) => CompletionContext::LayoutStatement,
        None => CompletionContext::Root,
    }
}

fn token_index_at_or_after(tokens: &[Token], cursor: usize) -> usize {
    tokens
        .iter()
        .position(|token| token.span.start.byte_offset >= cursor)
        .unwrap_or(tokens.len())
}

fn active_token_index(tokens: &[Token], cursor: usize) -> Option<usize> {
    tokens.iter().enumerate().find_map(|(index, token)| {
        (!matches!(token.kind, TokenKind::End)
            && token.span.start.byte_offset <= cursor
            && cursor <= token.span.end.byte_offset)
            .then_some(index)
    })
}

fn replacement_and_prefix(
    source: &str,
    tokens: &[Token],
    active_index: Option<usize>,
    position: SourcePosition,
) -> Result<(Span, String), IntelligenceError> {
    let Some(token) = active_index.and_then(|index| tokens.get(index)) else {
        return Ok((Span::point(position), String::new()));
    };
    match token.kind {
        TokenKind::Bare(_) => Ok((
            token.span,
            source[token.span.start.byte_offset..position.byte_offset].to_owned(),
        )),
        TokenKind::String(_) if token.span.end.byte_offset > token.span.start.byte_offset + 1 => {
            let start_offset = token.span.start.byte_offset + 1;
            let end_offset = token.span.end.byte_offset - 1;
            if position.byte_offset < start_offset || position.byte_offset > end_offset {
                return Ok((Span::point(position), String::new()));
            }
            Ok((
                Span {
                    start: position_at_offset(source, start_offset)?,
                    end: position_at_offset(source, end_offset)?,
                },
                source[start_offset..position.byte_offset].to_owned(),
            ))
        }
        _ => Ok((Span::point(position), String::new())),
    }
}

fn scopes_before(tokens: &[Token], current_index: usize) -> Vec<Scope> {
    let mut scopes = Vec::new();
    for (index, token) in tokens.iter().take(current_index).enumerate() {
        match token.kind {
            TokenKind::LeftBrace => {
                if let Some(scope) = scope_for_left_brace(tokens, index) {
                    scopes.push(scope);
                }
            }
            TokenKind::RightBrace => {
                scopes.pop();
            }
            _ => {}
        }
    }
    scopes
}

fn scope_for_left_brace(tokens: &[Token], index: usize) -> Option<Scope> {
    let previous = index.checked_sub(1).and_then(|item| tokens.get(item));
    if bare_value(previous) == Some("layout") {
        return Some(Scope::Layout);
    }
    if index >= 5
        && bare_value(tokens.get(index - 5)) == Some("edge")
        && is_edge_operator(tokens.get(index - 3))
        && matches!(tokens[index - 1].kind, TokenKind::String(_))
    {
        return Some(Scope::Edge);
    }
    if index >= 4
        && bare_value(tokens.get(index - 4)) == Some("edge")
        && is_edge_operator(tokens.get(index - 2))
    {
        return Some(Scope::Edge);
    }
    if index >= 3 && matches!(tokens[index - 1].kind, TokenKind::String(_)) {
        match bare_value(tokens.get(index - 3)) {
            Some("node") => return Some(Scope::Node),
            Some("group") => return Some(Scope::Group),
            _ => {}
        }
    }
    if index >= 2
        && bare_value(tokens.get(index - 2)) == Some("diagram")
        && matches!(tokens[index - 1].kind, TokenKind::String(_))
    {
        return Some(Scope::Diagram);
    }
    None
}

fn is_edge_operator(token: Option<&Token>) -> bool {
    token.is_some_and(|item| {
        matches!(
            item.kind,
            TokenKind::ForwardArrow | TokenKind::BidirectionalArrow | TokenKind::Association
        )
    })
}

fn bare_value(token: Option<&Token>) -> Option<&str> {
    match token.map(|item| &item.kind) {
        Some(TokenKind::Bare(value)) => Some(value),
        _ => None,
    }
}

fn document_nodes(document: &ast::Document) -> Vec<(String, String)> {
    let mut nodes = Vec::new();
    collect_diagram_nodes(&document.diagram.members, &mut nodes);
    nodes
}

fn collect_diagram_nodes(members: &[ast::DiagramMember], nodes: &mut Vec<(String, String)>) {
    for member in members {
        match member {
            ast::DiagramMember::Node(node) => {
                nodes.push((node.identifier.value.clone(), node.label.value.clone()));
            }
            ast::DiagramMember::Group(group) => collect_group_nodes(&group.members, nodes),
            ast::DiagramMember::Edge(_)
            | ast::DiagramMember::Theme(_)
            | ast::DiagramMember::Layout(_) => {}
        }
    }
}

fn collect_group_nodes(members: &[ast::GroupMember], nodes: &mut Vec<(String, String)>) {
    for member in members {
        match member {
            ast::GroupMember::Node(node) => {
                nodes.push((node.identifier.value.clone(), node.label.value.clone()));
            }
            ast::GroupMember::Group(group) => collect_group_nodes(&group.members, nodes),
            ast::GroupMember::Layout(_) => {}
        }
    }
}

fn position_at_offset(
    source: &str,
    byte_offset: usize,
) -> Result<SourcePosition, IntelligenceError> {
    if byte_offset > source.len()
        || !source.is_char_boundary(byte_offset)
        || (byte_offset > 0
            && source.as_bytes()[byte_offset - 1] == b'\r'
            && source.as_bytes().get(byte_offset) == Some(&b'\n'))
    {
        return Err(IntelligenceError::InvalidPosition);
    }

    let mut line = 1;
    let mut column = 1;
    let mut characters = source[..byte_offset].chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\r' && characters.peek() == Some(&'\n') {
            characters.next();
            line += 1;
            column = 1;
        } else if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    Ok(SourcePosition {
        byte_offset,
        line,
        column,
    })
}

fn validate_position(source: &str, position: SourcePosition) -> Result<(), IntelligenceError> {
    if position_at_offset(source, position.byte_offset)? != position {
        return Err(IntelligenceError::InvalidPosition);
    }
    Ok(())
}

fn validate_catalog(catalog: &CompletionCatalog) -> Result<(), IntelligenceError> {
    if catalog.icons.len() > MAX_COMPLETION_ICONS {
        return Err(IntelligenceError::CompletionCatalogTooLarge);
    }

    let mut identifiers = BTreeSet::new();
    for (index, entry) in catalog.icons.iter().enumerate() {
        let valid = valid_icon_id(&entry.id)
            && text_within(&entry.label, 120)
            && optional_text_within(entry.detail.as_deref(), 240)
            && optional_text_within(entry.documentation.as_deref(), 1_000);
        if !valid {
            return Err(IntelligenceError::InvalidCompletionCatalogEntry { index });
        }
        if !identifiers.insert(entry.id.as_str()) {
            return Err(IntelligenceError::DuplicateCompletionCatalogId { index });
        }
    }
    Ok(())
}

fn valid_icon_id(identifier: &str) -> bool {
    let mut segments = identifier.split(':');
    let first = segments.next();
    let second = segments.next();
    if segments.next().is_some() {
        return false;
    }
    match (first, second) {
        (Some(segment), None) => valid_icon_segment(segment),
        (Some(namespace), Some(icon)) => valid_icon_segment(namespace) && valid_icon_segment(icon),
        _ => false,
    }
}

fn valid_icon_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn text_within(value: &str, maximum: usize) -> bool {
    let length = value.chars().count();
    (1..=maximum).contains(&length)
}

fn optional_text_within(value: Option<&str>, maximum: usize) -> bool {
    value.is_none_or(|text| text_within(text, maximum))
}

#[cfg(test)]
mod tests {
    use super::{
        CompletionCatalog, CompletionCatalogEntry, CompletionContext, CompletionKind,
        IntelligenceError, MAX_COMPLETION_ICONS, Scope, candidates_for, completion, diagnostics,
        diagnostics_bytes, position_at_offset, scope_for_left_brace, validate_catalog,
        validate_position,
    };
    use crate::{diagnostic::SourcePosition, lexer};

    fn catalog_entry(id: &str) -> CompletionCatalogEntry {
        CompletionCatalogEntry {
            id: id.into(),
            label: id.into(),
            detail: None,
            documentation: None,
        }
    }

    #[test]
    fn diagnostics_echo_the_snapshot_version_for_text_and_bytes() {
        let source = "stack 1.0 diagram \"A\" { node a \"A\" }";
        let output = diagnostics(source, 42);
        assert_eq!(output.schema_version, "1.0");
        assert_eq!(output.document_version, 42);
        assert!(output.diagnostics.is_empty());

        let invalid_utf8 = diagnostics_bytes(&[0xff], 43);
        assert_eq!(invalid_utf8.document_version, 43);
        assert_eq!(invalid_utf8.diagnostics[0].code, "STK1001");
    }

    #[test]
    fn positions_require_matching_utf8_scalar_coordinates() {
        let source = "a😀\r\nb";
        assert_eq!(
            validate_position(
                source,
                SourcePosition {
                    byte_offset: 7,
                    line: 2,
                    column: 1,
                },
            ),
            Ok(())
        );
        for position in [
            SourcePosition {
                byte_offset: 2,
                line: 1,
                column: 3,
            },
            SourcePosition {
                byte_offset: 6,
                line: 1,
                column: 3,
            },
            SourcePosition {
                byte_offset: 8,
                line: 2,
                column: 3,
            },
            SourcePosition {
                byte_offset: 1,
                line: 2,
                column: 1,
            },
        ] {
            assert_eq!(
                validate_position(source, position),
                Err(IntelligenceError::InvalidPosition)
            );
        }
    }

    #[test]
    fn catalog_validation_bounds_and_deduplicates_untrusted_entries() {
        let valid = CompletionCatalog {
            icons: vec![catalog_entry("database"), catalog_entry("aws:s3")],
        };
        assert_eq!(validate_catalog(&valid), Ok(()));

        let duplicate = CompletionCatalog {
            icons: vec![catalog_entry("aws:s3"), catalog_entry("aws:s3")],
        };
        assert_eq!(
            validate_catalog(&duplicate),
            Err(IntelligenceError::DuplicateCompletionCatalogId { index: 1 })
        );

        for id in ["", "AWS:s3", "aws:", "aws:s3:extra", "aws:s_3"] {
            let invalid = CompletionCatalog {
                icons: vec![catalog_entry(id)],
            };
            assert_eq!(
                validate_catalog(&invalid),
                Err(IntelligenceError::InvalidCompletionCatalogEntry { index: 0 })
            );
        }

        let invalid_text = CompletionCatalog {
            icons: vec![CompletionCatalogEntry {
                id: "aws:s3".into(),
                label: String::new(),
                detail: Some(String::new()),
                documentation: Some("x".repeat(1_001)),
            }],
        };
        assert_eq!(
            validate_catalog(&invalid_text),
            Err(IntelligenceError::InvalidCompletionCatalogEntry { index: 0 })
        );

        let too_large = CompletionCatalog {
            icons: (0..=MAX_COMPLETION_ICONS)
                .map(|index| catalog_entry(&format!("icon-{index}")))
                .collect(),
        };
        assert_eq!(
            validate_catalog(&too_large),
            Err(IntelligenceError::CompletionCatalogTooLarge)
        );
    }

    #[test]
    fn intelligence_errors_have_actionable_display_text() {
        let errors = [
            IntelligenceError::InvalidPosition,
            IntelligenceError::CompletionCatalogTooLarge,
            IntelligenceError::InvalidCompletionCatalogEntry { index: 2 },
            IntelligenceError::DuplicateCompletionCatalogId { index: 3 },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn completion_matches_semantic_context_and_caller_catalog() {
        let source = concat!(
            "stack 1.0\n\n",
            "diagram \"Checkout\" {\n",
            "  theme dark\n\n",
            "  node api \"API\" {\n",
            "    kind service\n",
            "    icon \"aws:s3\"\n",
            "  }\n\n",
            "  group data \"Data\" {\n",
            "    node database \"Database\" {\n",
            "      kind database\n",
            "    }\n",
            "  }\n\n",
            "  edge api -> database \"SQL\" {\n",
            "    kind data\n",
            "  }\n",
            "}\n",
        );
        let catalog = CompletionCatalog {
            icons: vec![
                CompletionCatalogEntry {
                    id: "aws:sqs".into(),
                    label: "Amazon SQS".into(),
                    detail: Some("AWS provider icon".into()),
                    documentation: None,
                },
                CompletionCatalogEntry {
                    id: "aws:s3".into(),
                    label: "Amazon S3".into(),
                    detail: Some("AWS provider icon".into()),
                    documentation: None,
                },
            ],
        };

        let node_kind = completion(
            source,
            7,
            position_at_offset(source, 78).unwrap_or(SourcePosition::start()),
            &CompletionCatalog::default(),
        );
        assert!(
            matches!(node_kind, Ok(ref output) if output.schema_version == "1.0"
            && output.document_version == 7
            && output.diagnostics.is_empty()
            && !output.is_incomplete
            && output.items.len() == 1
            && output.items[0].label == "service"
            && output.items[0].kind == CompletionKind::EnumValue
            && output.items[0].sort_text == "0003:service"
            && output.items[0].edit.range.start.byte_offset == 74
            && output.items[0].edit.range.end.byte_offset == 81)
        );

        let icons = completion(
            source,
            7,
            position_at_offset(source, 97).unwrap_or(SourcePosition::start()),
            &catalog,
        );
        assert!(
            matches!(icons, Ok(ref output) if output.items.iter().map(|item| item.label.as_str()).collect::<Vec<_>>() == ["Amazon S3", "Amazon SQS"]
            && output.items.iter().all(|item| item.kind == CompletionKind::Icon)
            && output.items[0].edit.new_text == "aws:s3"
            && output.items[1].edit.new_text == "aws:sqs")
        );

        let endpoint = completion(
            source,
            7,
            position_at_offset(source, 206).unwrap_or(SourcePosition::start()),
            &CompletionCatalog::default(),
        );
        assert!(matches!(endpoint, Ok(ref output) if output.items.len() == 1
            && output.items[0].label == "database"
            && output.items[0].detail.as_deref() == Some("node · Database")
            && output.items[0].sort_text == "0002:database"));
    }

    #[test]
    fn completion_recovers_a_partial_node_property() {
        let source = "stack 1.0\n\ndiagram \"Partial\" {\n  node api \"API\" {\n    ki\n";
        let output = completion(
            source,
            9,
            position_at_offset(source, 56).unwrap_or(SourcePosition::start()),
            &CompletionCatalog::default(),
        );
        assert!(matches!(output, Ok(ref value) if value.is_incomplete
            && value.diagnostics.len() == 1
            && value.diagnostics[0].code == "STK2002"
            && value.items.len() == 1
            && value.items[0].label == "kind"
            && value.items[0].kind == CompletionKind::Property
            && value.items[0].edit.range.start.byte_offset == 54
            && value.items[0].edit.range.end.byte_offset == 56));
    }

    #[test]
    fn completion_rejects_boundary_errors_and_stops_after_lexical_failure() {
        let source = "stack 1.0 diagram \"A\" { node a \"A\" }";
        let invalid_position = completion(
            source,
            1,
            SourcePosition {
                byte_offset: 1,
                line: 9,
                column: 9,
            },
            &CompletionCatalog::default(),
        );
        assert_eq!(invalid_position, Err(IntelligenceError::InvalidPosition));

        let invalid_catalog = completion(
            source,
            1,
            SourcePosition::start(),
            &CompletionCatalog {
                icons: vec![catalog_entry("INVALID")],
            },
        );
        assert_eq!(
            invalid_catalog,
            Err(IntelligenceError::InvalidCompletionCatalogEntry { index: 0 })
        );

        let lexical_source = "\u{feff}stack";
        let lexical = completion(
            lexical_source,
            2,
            SourcePosition::start(),
            &CompletionCatalog::default(),
        );
        assert!(matches!(lexical, Ok(ref output) if output.is_incomplete
            && output.items.is_empty()
            && output.diagnostics[0].code == "STK1002"));
    }

    #[test]
    fn every_completion_category_has_a_deterministic_candidate_shape() {
        let catalog = CompletionCatalog {
            icons: vec![CompletionCatalogEntry {
                id: "aws:s3".into(),
                label: "Amazon S3".into(),
                detail: Some("Object storage".into()),
                documentation: Some("Caller-owned documentation".into()),
            }],
        };
        let nodes = vec![
            ("api".into(), "API".into()),
            ("db".into(), "Database".into()),
        ];
        let contexts = [
            CompletionContext::Root,
            CompletionContext::DiagramMember,
            CompletionContext::GroupMember,
            CompletionContext::NodeProperty,
            CompletionContext::EdgeProperty,
            CompletionContext::LayoutStatement,
            CompletionContext::NodeKind,
            CompletionContext::EdgeKind,
            CompletionContext::Direction,
            CompletionContext::RankRelation,
            CompletionContext::Icon,
            CompletionContext::EdgeEndpoint {
                excluded: Some("api".into()),
            },
            CompletionContext::EdgeEndpoint { excluded: None },
        ];
        for context in contexts {
            let candidates = candidates_for(context, &catalog, &nodes);
            assert!(!candidates.is_empty());
            assert!(
                candidates
                    .iter()
                    .all(|candidate| !candidate.filter_text.is_empty())
            );
        }
    }

    #[test]
    fn scope_detection_distinguishes_every_braced_construct() {
        let source = concat!(
            "stack 1.0 diagram \"D\" {",
            "node n \"N\" { kind service }",
            "group g \"G\" { node c \"C\" }",
            "edge n -> c \"E\" { kind flow }",
            "layout { direction right }",
            "}",
        );
        let tokens = lexer::tokenize(source).unwrap_or_default();
        let scopes: Vec<_> = tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| matches!(token.kind, crate::lexer::TokenKind::LeftBrace))
            .filter_map(|(index, _)| scope_for_left_brace(&tokens, index))
            .collect();
        assert_eq!(
            scopes,
            [
                Scope::Diagram,
                Scope::Node,
                Scope::Group,
                Scope::Edge,
                Scope::Layout,
            ]
        );
    }
}
