//! Reference compiler frontend for the Stack diagram language.
//!
//! The public pipeline deliberately stops at normalized, renderer-independent
//! diagram IR. Theme resolution, layout, and rendering belong to downstream
//! crates and applications.

#![forbid(unsafe_code)]

pub mod ast;
pub mod diagnostic;
pub mod ir;

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
        Err(error) => {
            let position = position_after_valid_prefix(&source[..error.valid_up_to()]);
            ParseOutput {
                document: None,
                diagnostics: vec![Diagnostic::error(
                    "STK1001",
                    "Input is not valid UTF-8.",
                    diagnostic::Span::point(position),
                )],
            }
        }
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

#[cfg(test)]
mod tests {
    use super::{
        compile, compile_bytes, parse, parse_bytes, position_after_valid_prefix, validate,
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
}
