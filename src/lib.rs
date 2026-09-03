//! Reference compiler frontend for the Stack diagram language.
//!
//! The public pipeline deliberately stops at normalized, renderer-independent
//! diagram IR. Theme resolution, layout, and rendering belong to downstream
//! crates and applications.

#![forbid(unsafe_code)]

pub mod ast;
pub mod diagnostic;
pub mod ir;
pub mod lossless;
pub mod source_map;

mod lexer;
mod parser;
mod validation;

use diagnostic::Diagnostic;

/// Output of lexical and syntax parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOutput {
    /// Parsed syntax tree, present only when no lexical or syntax error occurred.
    pub document: Option<ast::Document>,
    /// Lexical and syntax diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

/// Output of the complete compiler frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    /// Normalized diagram, present only when no compiler-stage error occurred.
    pub diagram: Option<ir::Diagram>,
    /// Lexical, syntax, semantic, and complexity diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

/// Output of syntactic parsing into the lossless source model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LosslessParseOutput {
    /// Lossless document, present only when lexical and syntax parsing succeeds.
    pub document: Option<lossless::Document>,
    /// Lexical and syntax diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

/// Output of compilation with the Rust-only source-map sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMappedCompileOutput {
    /// Normalized diagram, present only when no compiler-stage error occurred.
    pub diagram: Option<ir::Diagram>,
    /// Source map corresponding to `diagram`, absent whenever `diagram` is absent.
    pub source_map: Option<source_map::SourceMap>,
    /// Lexical, syntax, semantic, and complexity diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses UTF-8 Stack source into a source-oriented AST.
pub fn parse(source: &str) -> ParseOutput {
    match parser::parse(source) {
        Ok(document) => ParseOutput {
            document: Some(document),
            diagnostics: Vec::new(),
        },
        Err(diagnostic) => ParseOutput {
            document: None,
            diagnostics: vec![*diagnostic],
        },
    }
}

/// Decodes and parses Stack source bytes into a source-oriented AST.
pub fn parse_bytes(source: &[u8]) -> ParseOutput {
    match std::str::from_utf8(source) {
        Ok(source) => parse(source),
        Err(error) => ParseOutput {
            document: None,
            diagnostics: vec![invalid_utf8_diagnostic(source, error)],
        },
    }
}

/// Parses UTF-8 Stack source into exact authored tokens and trivia.
pub fn parse_lossless(source: &str) -> LosslessParseOutput {
    let tokens = match lexer::tokenize(source) {
        Ok(tokens) => tokens,
        Err(diagnostic) => {
            return LosslessParseOutput {
                document: None,
                diagnostics: vec![*diagnostic],
            };
        }
    };

    match parser::parse_tokens(tokens.clone()) {
        Ok(_) => LosslessParseOutput {
            document: Some(lossless::Document::from_lexer_tokens(source, tokens)),
            diagnostics: Vec::new(),
        },
        Err(diagnostic) => LosslessParseOutput {
            document: None,
            diagnostics: vec![*diagnostic],
        },
    }
}

/// Decodes and parses Stack source bytes into exact authored tokens and trivia.
pub fn parse_lossless_bytes(source: &[u8]) -> LosslessParseOutput {
    match std::str::from_utf8(source) {
        Ok(source) => parse_lossless(source),
        Err(error) => LosslessParseOutput {
            document: None,
            diagnostics: vec![invalid_utf8_diagnostic(source, error)],
        },
    }
}

/// Validates a parsed document and produces normalized IR when it is valid.
pub fn validate(document: &ast::Document) -> CompileOutput {
    validation::validate(document)
}

/// Parses, validates, and normalizes UTF-8 Stack source.
pub fn compile(source: &str) -> CompileOutput {
    let parsed = parse(source);
    match parsed.document {
        Some(document) => validate(&document),
        None => CompileOutput {
            diagram: None,
            diagnostics: parsed.diagnostics,
        },
    }
}

/// Decodes, parses, validates, and normalizes Stack source bytes.
pub fn compile_bytes(source: &[u8]) -> CompileOutput {
    let parsed = parse_bytes(source);
    match parsed.document {
        Some(document) => validate(&document),
        None => CompileOutput {
            diagram: None,
            diagnostics: parsed.diagnostics,
        },
    }
}

/// Parses, validates, and normalizes source with an engine-facing source map.
pub fn compile_with_source_map(source: &str) -> SourceMappedCompileOutput {
    let tokens = match lexer::tokenize(source) {
        Ok(tokens) => tokens,
        Err(diagnostic) => {
            return SourceMappedCompileOutput {
                diagram: None,
                source_map: None,
                diagnostics: vec![*diagnostic],
            };
        }
    };
    let document = match parser::parse_tokens(tokens.clone()) {
        Ok(document) => document,
        Err(diagnostic) => {
            return SourceMappedCompileOutput {
                diagram: None,
                source_map: None,
                diagnostics: vec![*diagnostic],
            };
        }
    };
    let compiled = validate(&document);
    let source_map = compiled.diagram.as_ref().map(|_| {
        let lossless = lossless::Document::from_lexer_tokens(source, tokens);
        source_map::SourceMap::from_document(&document, &lossless)
    });

    SourceMappedCompileOutput {
        diagram: compiled.diagram,
        source_map,
        diagnostics: compiled.diagnostics,
    }
}

/// Decodes and compiles source bytes with an engine-facing source map.
pub fn compile_bytes_with_source_map(source: &[u8]) -> SourceMappedCompileOutput {
    match std::str::from_utf8(source) {
        Ok(source) => compile_with_source_map(source),
        Err(error) => SourceMappedCompileOutput {
            diagram: None,
            source_map: None,
            diagnostics: vec![invalid_utf8_diagnostic(source, error)],
        },
    }
}

fn position_after_valid_prefix(prefix: &[u8]) -> diagnostic::SourcePosition {
    let source = match std::str::from_utf8(prefix) {
        Ok(source) => source,
        Err(_) => return diagnostic::SourcePosition::start(),
    };
    let mut position = diagnostic::SourcePosition::start();
    let mut characters = source.chars().peekable();

    while let Some(character) = characters.next() {
        position.byte_offset += character.len_utf8();
        if character == '\r' && characters.peek() == Some(&'\n') {
            if let Some(newline) = characters.next() {
                position.byte_offset += newline.len_utf8();
            }
            position.line += 1;
            position.column = 1;
        } else if matches!(character, '\n' | '\r') {
            position.line += 1;
            position.column = 1;
        } else {
            position.column += 1;
        }
    }

    position
}

fn invalid_utf8_diagnostic(source: &[u8], error: std::str::Utf8Error) -> Diagnostic {
    let position = position_after_valid_prefix(&source[..error.valid_up_to()]);
    Diagnostic::error(
        "STK1001",
        "Input is not valid UTF-8.",
        diagnostic::Span::point(position),
    )
}

#[cfg(test)]
mod tests {
    use crate::lossless::TokenKind;
    use crate::source_map::{LayoutScope, SourceOrigin};

    use super::{
        compile, compile_bytes, compile_bytes_with_source_map, compile_with_source_map, parse,
        parse_bytes, parse_lossless, parse_lossless_bytes, position_after_valid_prefix, validate,
    };

    #[test]
    fn reports_invalid_utf8_at_the_decoded_prefix_position() {
        let output = parse_bytes(b"stack 1.0\ndiagram \"x\" {\n\xff}");

        assert!(output.document.is_none());
        assert_eq!(output.diagnostics[0].code, "STK1001");
        assert_eq!(output.diagnostics[0].span.start.line, 3);
        assert_eq!(output.diagnostics[0].span.start.column, 1);
    }

    #[test]
    fn public_entry_points_cover_success_and_syntax_failure() {
        let source = "stack 1.0 diagram \"API\" { node api \"API\" }";
        let parsed = parse(source);
        assert!(parsed.diagnostics.is_empty());
        let Some(document) = parsed.document else {
            return;
        };
        assert!(validate(&document).diagram.is_some());
        assert!(parse_bytes(source.as_bytes()).document.is_some());
        assert!(compile_bytes(source.as_bytes()).diagram.is_some());

        let syntax_error = compile("stack 1.0 diagram \"API\" {");
        assert!(syntax_error.diagram.is_none());
        assert_eq!(syntax_error.diagnostics[0].code, "STK2003");
    }

    #[test]
    fn utf8_error_positions_handle_crlf_and_defensive_invalid_prefixes() {
        let output = parse_bytes(b"stack 1.0\r\n\xff");
        assert_eq!(output.diagnostics[0].span.start.line, 2);
        assert_eq!(output.diagnostics[0].span.start.column, 1);
        assert_eq!(
            position_after_valid_prefix(b"\xff"),
            crate::diagnostic::SourcePosition::start()
        );
    }

    #[test]
    fn lossless_entry_points_preserve_trivia_escapes_and_crlf() {
        let source = concat!(
            "// leading\r\n",
            "stack 1.0\r\n",
            "diagram \"\\u56F3\" {\r\n",
            "\tnode api \"API\" // trailing\r\n",
            "}\r\n",
        );
        let output = parse_lossless(source);
        assert!(output.diagnostics.is_empty());
        let Some(document) = output.document else {
            return;
        };

        assert_eq!(document.reconstruct(), source);
        assert!(document.tokens().iter().any(|token| {
            matches!(&token.kind, TokenKind::String(value) if value == "図")
                && token.text == "\"\\u56F3\""
        }));
        assert!(
            document.tokens().iter().any(|token| {
                token.kind == TokenKind::Whitespace && token.text.contains("\r\n")
            })
        );
        assert!(
            document.tokens().iter().any(|token| {
                token.kind == TokenKind::LineComment && token.text == "// trailing"
            })
        );

        let bytes_output = parse_lossless_bytes(source.as_bytes());
        assert_eq!(bytes_output.document, Some(document));
    }

    #[test]
    fn lossless_entry_points_report_lexical_syntax_and_encoding_errors() {
        let bom = parse_lossless("\u{feff}stack 1.0");
        assert!(bom.document.is_none());
        assert_eq!(bom.diagnostics[0].code, "STK1002");

        let syntax = parse_lossless("stack 1.0 diagram \"x\" {");
        assert!(syntax.document.is_none());
        assert_eq!(syntax.diagnostics[0].code, "STK2003");

        let encoding = parse_lossless_bytes(b"stack 1.0\r\n\xff");
        assert!(encoding.document.is_none());
        assert_eq!(encoding.diagnostics[0].code, "STK1001");
        assert_eq!(encoding.diagnostics[0].span.start.line, 2);
    }

    #[test]
    fn lossless_syntax_model_keeps_semantically_invalid_source() {
        let source = concat!(
            "stack 1.0\n",
            "diagram \"Duplicate\" {\n",
            "  node api \"First\"\n",
            "  node api \"Second\"\n",
            "}\n",
        );

        assert!(parse_lossless(source).document.is_some());
        assert!(compile(source).diagram.is_none());
    }

    #[test]
    fn source_map_resolves_authored_values_by_semantic_identity() {
        let source = concat!(
            "stack 1.0\n",
            "diagram \"Mapped\" {\n",
            "  node root \"Root\"\n",
            "  group services \"Services\" {\n",
            "    node api \"API\" { icon \"service\" }\n",
            "    node worker \"Worker\"\n",
            "    layout {\n",
            "      order // Group order\n",
            "        [api, worker]\n",
            "    }\n",
            "  }\n",
            "  theme dark\n",
            "  layout { order [root, services] }\n",
            "}\n",
        );

        let mapped = compile_with_source_map(source);
        let plain = compile(source);
        assert_eq!(mapped.diagram, plain.diagram);
        assert_eq!(mapped.diagnostics, plain.diagnostics);
        let Some(source_map) = mapped.source_map else {
            return;
        };

        assert_eq!(authored_text(source, source_map.theme()), Some("dark"));
        assert_eq!(source_map.node_icon("root"), Some(SourceOrigin::Omitted));
        assert_eq!(source_map.node_icon("worker"), Some(SourceOrigin::Omitted));
        assert_eq!(source_map.node_icon("missing"), None);
        let Some(api_icon) = source_map.node_icon("api") else {
            return;
        };
        assert_eq!(authored_text(source, api_icon), Some("\"service\""));
        assert_eq!(
            source_map
                .node_icons()
                .iter()
                .map(|entry| entry.node_id.as_str())
                .collect::<Vec<_>>(),
            vec!["root", "api", "worker"]
        );

        assert_eq!(
            authored_text(source, source_map.diagram_order()),
            Some("order [root, services]")
        );
        let Some(group_order) = source_map.group_order("services") else {
            return;
        };
        assert_eq!(
            authored_text(source, group_order),
            Some("order // Group order\n        [api, worker]")
        );
        assert_eq!(source_map.group_order("missing"), None);
        assert!(matches!(
            source_map.layout_orders()[0].scope,
            LayoutScope::Diagram
        ));
        assert!(matches!(
            &source_map.layout_orders()[1].scope,
            LayoutScope::Group(identifier) if identifier == "services"
        ));

        assert_eq!(
            compile_with_source_map(source).source_map,
            Some(source_map.clone())
        );
        assert_eq!(
            compile_bytes_with_source_map(source.as_bytes()).source_map,
            Some(source_map)
        );
    }

    #[test]
    fn source_map_distinguishes_defaults_and_rejects_error_results() {
        let source = concat!(
            "stack 1.0 ",
            "diagram \"Default\" { ",
            "group services \"Services\" { node api \"API\" } ",
            "}",
        );
        let output = compile_with_source_map(source);
        let Some(source_map) = output.source_map else {
            return;
        };
        assert_eq!(source_map.theme(), SourceOrigin::Omitted);
        assert_eq!(source_map.node_icon("api"), Some(SourceOrigin::Omitted));
        assert_eq!(source_map.diagram_order(), SourceOrigin::Omitted);
        assert_eq!(
            source_map.group_order("services"),
            Some(SourceOrigin::Omitted)
        );
        assert_eq!(source_map.layout_orders().len(), 2);

        for invalid in [
            "\u{feff}stack 1.0",
            "stack 1.0 diagram \"Incomplete\" {",
            "stack 1.0 diagram \"Duplicate\" { node api \"A\" node api \"B\" }",
        ] {
            let output = compile_with_source_map(invalid);
            assert!(output.diagram.is_none());
            assert!(output.source_map.is_none());
            assert!(!output.diagnostics.is_empty());
        }

        let encoding = compile_bytes_with_source_map(b"stack 1.0\n\xff");
        assert!(encoding.diagram.is_none());
        assert!(encoding.source_map.is_none());
        assert_eq!(encoding.diagnostics[0].code, "STK1001");
    }

    fn authored_text(source: &str, origin: SourceOrigin) -> Option<&str> {
        origin
            .span()
            .map(|span| &source[span.start.byte_offset..span.end.byte_offset])
    }
}
