use crate::ast::{
    Diagram, DiagramMember, Document, Edge, EdgeOperator, EdgeProperty, Group, GroupMember,
    IdentifierList, Layout, LayoutStatement, Node, NodeProperty, Theme, Version,
};
use crate::diagnostic::{Diagnostic, Span, Spanned};
use crate::lexer::{Token, TokenKind, tokenize};

type ParseResult<T> = Result<T, Box<Diagnostic>>;

pub(crate) fn parse(source: &str) -> ParseResult<Document> {
    Parser::new(tokenize(source)?).parse_document()
}

struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    fn parse_document(mut self) -> ParseResult<Document> {
        let version = self.parse_version()?;
        let diagram = self.parse_diagram()?;
        self.expect_end()?;

        Ok(Document {
            span: Span::covering(version.span, diagram.span),
            version,
            diagram,
        })
    }

    fn parse_version(&mut self) -> ParseResult<Version> {
        let start = self.expect_keyword("stack")?.span;
        let (major, _) = self.expect_integer("language major version")?;
        self.expect_simple(|kind| matches!(kind, TokenKind::Dot), "'.'")?;
        let (minor, minor_span) = self.expect_integer("language minor version")?;

        Ok(Version {
            major,
            minor,
            span: Span::covering(start, minor_span),
        })
    }

    fn parse_diagram(&mut self) -> ParseResult<Diagram> {
        let start = self.expect_keyword("diagram")?.span;
        let title = self.expect_string("diagram title")?;
        self.expect_simple(|kind| matches!(kind, TokenKind::LeftBrace), "'{'")?;

        let mut members = Vec::new();
        while !self.at_simple(|kind| matches!(kind, TokenKind::RightBrace)) {
            self.reject_end("diagram")?;
            members.push(match self.current_bare() {
                Some("node") => DiagramMember::Node(self.parse_node()?),
                Some("group") => DiagramMember::Group(self.parse_group()?),
                Some("edge") => DiagramMember::Edge(self.parse_edge()?),
                Some("theme") => DiagramMember::Theme(self.parse_theme()?),
                Some("layout") => DiagramMember::Layout(self.parse_layout()?),
                _ => return Err(self.unexpected("a diagram declaration")),
            });
        }

        let end = self
            .expect_simple(|kind| matches!(kind, TokenKind::RightBrace), "'}'")?
            .span;

        Ok(Diagram {
            title,
            members,
            span: Span::covering(start, end),
        })
    }

    fn parse_group(&mut self) -> ParseResult<Group> {
        let start = self.expect_keyword("group")?.span;
        let identifier = self.expect_identifier("group identifier")?;
        let label = self.expect_string("group label")?;
        self.expect_simple(|kind| matches!(kind, TokenKind::LeftBrace), "'{'")?;

        let mut members = Vec::new();
        while !self.at_simple(|kind| matches!(kind, TokenKind::RightBrace)) {
            self.reject_end("group")?;
            members.push(match self.current_bare() {
                Some("node") => GroupMember::Node(self.parse_node()?),
                Some("group") => GroupMember::Group(self.parse_group()?),
                Some("layout") => GroupMember::Layout(self.parse_layout()?),
                _ => return Err(self.unexpected("a node, group, or layout declaration")),
            });
        }

        let end = self
            .expect_simple(|kind| matches!(kind, TokenKind::RightBrace), "'}'")?
            .span;

        Ok(Group {
            identifier,
            label,
            members,
            span: Span::covering(start, end),
        })
    }

    fn parse_node(&mut self) -> ParseResult<Node> {
        let start = self.expect_keyword("node")?.span;
        let identifier = self.expect_identifier("node identifier")?;
        let label = self.expect_string("node label")?;
        let mut end = label.span;
        let mut properties = Vec::new();

        if self
            .take_simple(|kind| matches!(kind, TokenKind::LeftBrace))
            .is_some()
        {
            if self.at_simple(|kind| matches!(kind, TokenKind::RightBrace)) {
                return Err(self.unexpected("at least one node property"));
            }

            while !self.at_simple(|kind| matches!(kind, TokenKind::RightBrace)) {
                self.reject_end("node block")?;
                properties.push(match self.current_bare() {
                    Some("kind") => {
                        self.advance();
                        NodeProperty::Kind(self.expect_identifier("node kind")?)
                    }
                    Some("icon") => {
                        self.advance();
                        NodeProperty::Icon(self.expect_string("icon identifier")?)
                    }
                    Some("detail") => {
                        self.advance();
                        NodeProperty::Detail(self.expect_string("node detail")?)
                    }
                    _ => return Err(self.unexpected("a node property")),
                });
            }

            end = self
                .expect_simple(|kind| matches!(kind, TokenKind::RightBrace), "'}'")?
                .span;
        }

        Ok(Node {
            identifier,
            label,
            properties,
            span: Span::covering(start, end),
        })
    }

    fn parse_edge(&mut self) -> ParseResult<Edge> {
        let start = self.expect_keyword("edge")?.span;
        let from = self.expect_identifier("edge endpoint")?;
        let operator_token = self.current_token().clone();
        let operator = match operator_token.kind {
            TokenKind::ForwardArrow => EdgeOperator::Forward,
            TokenKind::BidirectionalArrow => EdgeOperator::Bidirectional,
            TokenKind::Association => EdgeOperator::Association,
            _ => return Err(self.unexpected("an edge operator")),
        };
        self.advance();
        let operator = Spanned::new(operator, operator_token.span);
        let to = self.expect_identifier("edge endpoint")?;
        let label = if matches!(self.current_token().kind, TokenKind::String(_)) {
            Some(self.expect_string("edge label")?)
        } else {
            None
        };

        let mut end = label.as_ref().map_or(to.span, |label| label.span);
        let mut properties = Vec::new();
        if self
            .take_simple(|kind| matches!(kind, TokenKind::LeftBrace))
            .is_some()
        {
            if self.at_simple(|kind| matches!(kind, TokenKind::RightBrace)) {
                return Err(self.unexpected("at least one edge property"));
            }

            while !self.at_simple(|kind| matches!(kind, TokenKind::RightBrace)) {
                self.reject_end("edge block")?;
                properties.push(match self.current_bare() {
                    Some("kind") => {
                        self.advance();
                        EdgeProperty::Kind(self.expect_identifier("edge kind")?)
                    }
                    _ => return Err(self.unexpected("an edge property")),
                });
            }

            end = self
                .expect_simple(|kind| matches!(kind, TokenKind::RightBrace), "'}'")?
                .span;
        }

        Ok(Edge {
            from,
            operator,
            to,
            label,
            properties,
            span: Span::covering(start, end),
        })
    }

    fn parse_theme(&mut self) -> ParseResult<Theme> {
        let start = self.expect_keyword("theme")?.span;
        let identifier = self.expect_identifier("theme identifier")?;
        Ok(Theme {
            span: Span::covering(start, identifier.span),
            identifier,
        })
    }

    fn parse_layout(&mut self) -> ParseResult<Layout> {
        let start = self.expect_keyword("layout")?.span;
        self.expect_simple(|kind| matches!(kind, TokenKind::LeftBrace), "'{'")?;
        if self.at_simple(|kind| matches!(kind, TokenKind::RightBrace)) {
            return Err(self.unexpected("at least one layout statement"));
        }

        let mut statements = Vec::new();
        while !self.at_simple(|kind| matches!(kind, TokenKind::RightBrace)) {
            self.reject_end("layout block")?;
            statements.push(match self.current_bare() {
                Some("direction") => {
                    self.advance();
                    LayoutStatement::Direction(self.expect_identifier("layout direction")?)
                }
                Some("rank") => {
                    self.advance();
                    self.expect_keyword("same")?;
                    LayoutStatement::RankSame(self.parse_identifier_list()?)
                }
                Some("order") => {
                    self.advance();
                    LayoutStatement::Order(self.parse_identifier_list()?)
                }
                _ => return Err(self.unexpected("a layout statement")),
            });
        }

        let end = self
            .expect_simple(|kind| matches!(kind, TokenKind::RightBrace), "'}'")?
            .span;
        Ok(Layout {
            statements,
            span: Span::covering(start, end),
        })
    }

    fn parse_identifier_list(&mut self) -> ParseResult<IdentifierList> {
        let start = self
            .expect_simple(|kind| matches!(kind, TokenKind::LeftBracket), "'['")?
            .span;
        let mut identifiers = vec![self.expect_identifier("layout identifier")?];
        self.expect_simple(|kind| matches!(kind, TokenKind::Comma), "','")?;
        identifiers.push(self.expect_identifier("layout identifier")?);

        while self
            .take_simple(|kind| matches!(kind, TokenKind::Comma))
            .is_some()
        {
            identifiers.push(self.expect_identifier("layout identifier")?);
        }

        let end = self
            .expect_simple(|kind| matches!(kind, TokenKind::RightBracket), "']'")?
            .span;
        Ok(IdentifierList {
            identifiers,
            span: Span::covering(start, end),
        })
    }

    fn expect_integer(&mut self, description: &str) -> ParseResult<(u32, Span)> {
        let value = self.expect_identifier(description)?;
        if value.value.len() > 1 && value.value.starts_with('0') {
            return Err(Box::new(Diagnostic::error(
                "STK2002",
                format!("Expected {description} without leading zeroes."),
                value.span,
            )));
        }

        let parsed = value.value.parse::<u32>().map_err(|_| {
            Box::new(Diagnostic::error(
                "STK2002",
                format!("Expected {description}."),
                value.span,
            ))
        })?;
        Ok((parsed, value.span))
    }

    fn expect_identifier(&mut self, description: &str) -> ParseResult<Spanned<String>> {
        let token = self.current_token().clone();
        match token.kind {
            TokenKind::Bare(value) => {
                self.advance();
                Ok(Spanned::new(value, token.span))
            }
            _ => Err(self.unexpected(description)),
        }
    }

    fn expect_string(&mut self, description: &str) -> ParseResult<Spanned<String>> {
        let token = self.current_token().clone();
        match token.kind {
            TokenKind::String(value) => {
                self.advance();
                Ok(Spanned::new(value, token.span))
            }
            _ => Err(self.unexpected(description)),
        }
    }

    fn expect_keyword(&mut self, keyword: &str) -> ParseResult<Token> {
        if self.current_bare() != Some(keyword) {
            return Err(self.unexpected(&format!("'{keyword}'")));
        }
        let token = self.current_token().clone();
        self.advance();
        Ok(token)
    }

    fn expect_simple(
        &mut self,
        predicate: impl FnOnce(&TokenKind) -> bool,
        description: &str,
    ) -> ParseResult<Token> {
        if !predicate(&self.current_token().kind) {
            return Err(self.unexpected(description));
        }
        let token = self.current_token().clone();
        self.advance();
        Ok(token)
    }

    fn take_simple(&mut self, predicate: impl FnOnce(&TokenKind) -> bool) -> Option<Token> {
        if !predicate(&self.current_token().kind) {
            return None;
        }
        let token = self.current_token().clone();
        self.advance();
        Some(token)
    }

    fn expect_end(&self) -> ParseResult<()> {
        if matches!(self.current_token().kind, TokenKind::End) {
            Ok(())
        } else {
            Err(self.unexpected("the end of the document"))
        }
    }

    fn reject_end(&self, construct: &str) -> ParseResult<()> {
        if matches!(self.current_token().kind, TokenKind::End) {
            Err(Box::new(Diagnostic::error(
                "STK2003",
                format!("Input ended before the {construct} was complete."),
                self.current_token().span,
            )))
        } else {
            Ok(())
        }
    }

    fn unexpected(&self, expected: &str) -> Box<Diagnostic> {
        if matches!(self.current_token().kind, TokenKind::End) {
            Box::new(Diagnostic::error(
                "STK2003",
                format!("Input ended while expecting {expected}."),
                self.current_token().span,
            ))
        } else {
            Box::new(Diagnostic::error(
                "STK2002",
                format!("Expected {expected}."),
                self.current_token().span,
            ))
        }
    }

    fn current_bare(&self) -> Option<&str> {
        match &self.current_token().kind {
            TokenKind::Bare(value) => Some(value),
            _ => None,
        }
    }

    fn at_simple(&self, predicate: impl FnOnce(&TokenKind) -> bool) -> bool {
        predicate(&self.current_token().kind)
    }

    fn current_token(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn advance(&mut self) {
        if self.current + 1 < self.tokens.len() {
            self.current += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{DiagramMember, EdgeOperator, GroupMember, LayoutStatement};
    use crate::lexer::{Token, tokenize};

    use super::{Parser, parse};

    fn successful_tokens(source: &str) -> Vec<Token> {
        let result = tokenize(source);
        assert!(result.is_ok(), "{result:?}");
        result.into_iter().flatten().collect()
    }

    #[test]
    fn parses_a_minimal_document() {
        let result = parse(
            r#"stack 1.0
diagram "Hello Stack" {
  node web "Web app"
  node api "API"
  edge web -> api
}"#,
        );
        assert!(result.is_ok(), "{result:?}");
        let Some(document) = result.ok() else {
            return;
        };

        assert_eq!(document.version.major, 1);
        assert_eq!(document.version.minor, 0);
        assert_eq!(document.diagram.title.value, "Hello Stack");
        assert_eq!(document.diagram.members.len(), 3);
        assert!(matches!(
            &document.diagram.members[2],
            DiagramMember::Edge(edge) if edge.operator.value == EdgeOperator::Forward
        ));
    }

    #[test]
    fn parses_groups_properties_and_layout() {
        let result = parse(
            r#"stack 1.0
diagram "System" {
  theme dark
  group group "Group" {
    layout {
      direction down
      rank same [node, service]
      order [node, service]
    }
    node node "Node" { kind client icon "browser" detail "UI" }
    node service "Service"
  }
  edge node <-> service "RPC" { kind request }
}"#,
        );
        assert!(result.is_ok(), "{result:?}");
        let Some(document) = result.ok() else {
            return;
        };

        assert!(matches!(
            &document.diagram.members[1],
            DiagramMember::Group(group)
                if group.identifier.value == "group"
                    && matches!(
                        &group.members[0],
                        GroupMember::Layout(layout)
                            if matches!(layout.statements[1], LayoutStatement::RankSame(_))
                    )
        ));
    }

    #[test]
    fn rejects_unknown_and_incomplete_syntax() {
        let unknown = parse("stack 1.0 diagram \"x\" { server api \"API\" }");
        assert!(matches!(unknown, Err(diagnostic) if diagnostic.code == "STK2002"));

        let incomplete = parse("stack 1.0 diagram \"x\" { node api \"API\"");
        assert!(matches!(incomplete, Err(diagnostic) if diagnostic.code == "STK2003"));
    }

    #[test]
    fn requires_nonempty_property_and_layout_blocks() {
        for source in [
            "stack 1.0 diagram \"x\" { node api \"API\" {} }",
            "stack 1.0 diagram \"x\" { node a \"A\" node b \"B\" edge a -> b {} }",
            "stack 1.0 diagram \"x\" { node api \"API\" layout {} }",
        ] {
            assert!(matches!(
                parse(source),
                Err(diagnostic) if diagnostic.code == "STK2002"
            ));
        }
    }

    #[test]
    fn parses_all_edge_operators_and_long_layout_lists() {
        let result = parse(
            r#"stack 1.0
diagram "Operators" {
  node a "A"
  node b "B"
  node c "C"
  layout { order [a, b, c] }
  edge a -> b
  edge a <-> c
  edge b -- c
}"#,
        );

        assert!(result.is_ok(), "{result:?}");
        let Some(document) = result.ok() else {
            return;
        };
        let operators: Vec<_> = document
            .diagram
            .members
            .iter()
            .filter_map(|member| match member {
                DiagramMember::Edge(edge) => Some(edge.operator.value),
                _ => None,
            })
            .collect();
        assert_eq!(
            operators,
            vec![
                EdgeOperator::Forward,
                EdgeOperator::Bidirectional,
                EdgeOperator::Association,
            ]
        );
    }

    #[test]
    fn reports_errors_for_each_recursive_descent_boundary() {
        let cases = [
            ("", "STK2003"),
            ("diagram \"x\" {}", "STK2002"),
            ("stack 01.0 diagram \"x\" {}", "STK2002"),
            ("stack x.0 diagram \"x\" {}", "STK2002"),
            ("stack 1.x diagram \"x\" {}", "STK2002"),
            ("stack 1 0 diagram \"x\" {}", "STK2002"),
            ("stack 1.0 other \"x\" {}", "STK2002"),
            ("stack 1.0 diagram x {}", "STK2002"),
            ("stack 1.0 diagram \"x\" other", "STK2002"),
            ("stack 1.0 diagram \"x\" {} trailing", "STK2002"),
            ("stack 1.0 diagram \"x\" { group \"G\" {} }", "STK2002"),
            ("stack 1.0 diagram \"x\" { group g label {} }", "STK2002"),
            ("stack 1.0 diagram \"x\" { group g \"G\" other }", "STK2002"),
            ("stack 1.0 diagram \"x\" { group g \"G\" {", "STK2003"),
            (
                "stack 1.0 diagram \"x\" { group g \"G\" { edge a -> b } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { group g \"G\" { node } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { group g \"G\" { group } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { group g \"G\" { layout } }",
                "STK2002",
            ),
            ("stack 1.0 diagram \"x\" { node \"a\" \"A\" }", "STK2002"),
            ("stack 1.0 diagram \"x\" { node a label }", "STK2002"),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" { unknown value } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" { kind } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" { icon value } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" { detail value } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" { kind service",
                "STK2003",
            ),
            ("stack 1.0 diagram \"x\" { edge \"a\" -> b }", "STK2002"),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" node b \"B\" edge a ? b }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" edge a -> }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" node b \"B\" edge a -> b { unknown value } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" node b \"B\" edge a -> b { kind } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" node b \"B\" edge a -> b { kind flow",
                "STK2003",
            ),
            ("stack 1.0 diagram \"x\" { theme }", "STK2002"),
            ("stack 1.0 diagram \"x\" { layout other }", "STK2002"),
            ("stack 1.0 diagram \"x\" { layout {", "STK2003"),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" layout { unknown value } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" layout { direction } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" layout { rank other [a, b] } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" layout { order other } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" layout { rank same [a] } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" layout { order [, a] } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" layout { order [a b] } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" layout { order [a, ] } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" layout { order [a, b,] } }",
                "STK2002",
            ),
            (
                "stack 1.0 diagram \"x\" { node a \"A\" node b \"B\" layout { order [a, b } }",
                "STK2002",
            ),
        ];

        for (source, expected_code) in cases {
            let result = parse(source);
            assert!(
                matches!(result, Err(diagnostic) if diagnostic.code == expected_code),
                "source unexpectedly parsed: {source}"
            );
        }
    }

    #[test]
    fn production_parsers_reject_the_wrong_entry_keyword() {
        let tokens = successful_tokens("wrong");

        assert!(Parser::new(tokens.clone()).parse_group().is_err());
        assert!(Parser::new(tokens.clone()).parse_node().is_err());
        assert!(Parser::new(tokens.clone()).parse_edge().is_err());
        assert!(Parser::new(tokens.clone()).parse_theme().is_err());
        assert!(Parser::new(tokens).parse_layout().is_err());

        let mut end_parser = Parser::new(successful_tokens(""));
        end_parser.advance();
        assert_eq!(end_parser.current, 0);
    }
}
