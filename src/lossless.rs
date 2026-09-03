//! Lossless lexical source model for formatters and editor tooling.

use crate::diagnostic::{SourcePosition, Span};
use crate::lexer;

/// A syntactically valid Stack document represented as authored lexemes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    tokens: Vec<Token>,
}

impl Document {
    pub(crate) fn from_lexer_tokens(source: &str, tokens: Vec<lexer::Token>) -> Self {
        let mut lossless_tokens = Vec::new();
        let mut cursor = SourcePosition::start();

        for token in tokens {
            append_trivia(source, cursor, token.span.start, &mut lossless_tokens);

            let span = token.span;
            let text = source[span.start.byte_offset..span.end.byte_offset].to_owned();
            let kind = match token.kind {
                lexer::TokenKind::Bare(_) => TokenKind::Bare,
                lexer::TokenKind::String(value) => TokenKind::String(value),
                lexer::TokenKind::LeftBrace => TokenKind::LeftBrace,
                lexer::TokenKind::RightBrace => TokenKind::RightBrace,
                lexer::TokenKind::LeftBracket => TokenKind::LeftBracket,
                lexer::TokenKind::RightBracket => TokenKind::RightBracket,
                lexer::TokenKind::Comma => TokenKind::Comma,
                lexer::TokenKind::Dot => TokenKind::Dot,
                lexer::TokenKind::ForwardArrow => TokenKind::ForwardArrow,
                lexer::TokenKind::BidirectionalArrow => TokenKind::BidirectionalArrow,
                lexer::TokenKind::Association => TokenKind::Association,
                lexer::TokenKind::End => TokenKind::End,
            };
            lossless_tokens.push(Token { kind, text, span });
            cursor = span.end;
        }

        Self {
            tokens: lossless_tokens,
        }
    }

    /// Returns every authored token and trivia segment in source order.
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Reconstructs the original UTF-8 source byte-for-byte.
    pub fn reconstruct(&self) -> String {
        let mut source = String::new();
        for token in &self.tokens {
            source.push_str(&token.text);
        }
        source
    }
}

/// One authored lexeme or trivia segment with its exact source text and span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Lexical category. String tokens also expose their decoded value.
    pub kind: TokenKind,
    /// Exact authored text, including escapes and original line endings.
    pub text: String,
    /// End-exclusive span in the original source.
    pub span: Span,
}

/// Lexical category in a lossless Stack document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// One or more spaces, tabs, LF line endings, or CRLF line endings.
    Whitespace,
    /// A `//` comment without its following line ending.
    LineComment,
    /// An identifier, contextual keyword, integer, or unknown bare token.
    Bare,
    /// A source string and its decoded value.
    String(String),
    /// `{`.
    LeftBrace,
    /// `}`.
    RightBrace,
    /// `[`.
    LeftBracket,
    /// `]`.
    RightBracket,
    /// `,`.
    Comma,
    /// `.`.
    Dot,
    /// `->`.
    ForwardArrow,
    /// `<->`.
    BidirectionalArrow,
    /// `--`.
    Association,
    /// The zero-width end of the source.
    End,
}

fn append_trivia(
    source: &str,
    mut position: SourcePosition,
    end: SourcePosition,
    tokens: &mut Vec<Token>,
) {
    while position.byte_offset < end.byte_offset {
        let start = position;
        let is_comment = source[position.byte_offset..end.byte_offset].starts_with("//");

        if is_comment {
            while position.byte_offset < end.byte_offset
                && next_character(source, position.byte_offset)
                    .is_some_and(|character| !matches!(character, '\n' | '\r'))
            {
                advance_position(source, &mut position);
            }
        } else {
            while position.byte_offset < end.byte_offset
                && !source[position.byte_offset..end.byte_offset].starts_with("//")
            {
                advance_position(source, &mut position);
            }
        }

        let text = source[start.byte_offset..position.byte_offset].to_owned();
        tokens.push(Token {
            kind: if is_comment {
                TokenKind::LineComment
            } else {
                TokenKind::Whitespace
            },
            text,
            span: Span {
                start,
                end: position,
            },
        });
    }
}

fn next_character(source: &str, offset: usize) -> Option<char> {
    source[offset..].chars().next()
}

fn advance_position(source: &str, position: &mut SourcePosition) {
    let Some(character) = next_character(source, position.byte_offset) else {
        return;
    };

    if character == '\r' && source[position.byte_offset..].starts_with("\r\n") {
        position.byte_offset += 2;
        position.line += 1;
        position.column = 1;
    } else {
        position.byte_offset += character.len_utf8();
        if matches!(character, '\n' | '\r') {
            position.line += 1;
            position.column = 1;
        } else {
            position.column += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Document, TokenKind};
    use crate::lexer::tokenize;

    #[test]
    fn preserves_every_lexeme_and_decoded_string_value() {
        let source = " \t// lead\r\nword \"\\u0041\" {}[],. a->b a<->b a--b // tail";
        let result = tokenize(source);
        assert!(result.is_ok(), "{result:?}");
        let tokens: Vec<_> = result.into_iter().flatten().collect();
        let document = Document::from_lexer_tokens(source, tokens);

        assert_eq!(document.reconstruct(), source);
        assert_eq!(document.tokens[0].kind, TokenKind::Whitespace);
        assert_eq!(document.tokens[0].text, " \t");
        assert_eq!(document.tokens[1].kind, TokenKind::LineComment);
        assert_eq!(document.tokens[1].text, "// lead");
        assert_eq!(document.tokens[1].span.start.line, 1);
        assert_eq!(document.tokens[2].text, "\r\n");
        assert_eq!(document.tokens[2].span.end.line, 2);
        assert!(document.tokens.iter().any(
            |token| matches!(&token.kind, TokenKind::String(value) if value == "A")
                && token.text == "\"\\u0041\""
        ));

        for expected in [
            TokenKind::LeftBrace,
            TokenKind::RightBrace,
            TokenKind::LeftBracket,
            TokenKind::RightBracket,
            TokenKind::Comma,
            TokenKind::Dot,
            TokenKind::ForwardArrow,
            TokenKind::BidirectionalArrow,
            TokenKind::Association,
            TokenKind::End,
        ] {
            assert!(document.tokens.iter().any(|token| token.kind == expected));
        }

        let mut byte_offset = 0;
        for token in document.tokens() {
            assert_eq!(token.span.start.byte_offset, byte_offset);
            byte_offset = token.span.end.byte_offset;
        }
        assert_eq!(byte_offset, source.len());
    }

    #[test]
    fn defensive_position_advance_stops_at_end_of_source() {
        let mut position = crate::diagnostic::SourcePosition {
            byte_offset: 0,
            line: 1,
            column: 1,
        };
        super::advance_position("", &mut position);
        assert_eq!(position.byte_offset, 0);
    }
}
