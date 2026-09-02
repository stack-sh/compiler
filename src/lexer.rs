use crate::diagnostic::{Diagnostic, SourcePosition, Span};

type LexResult<T> = Result<T, Box<Diagnostic>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Bare(String),
    String(String),
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    ForwardArrow,
    BidirectionalArrow,
    Association,
    End,
}

pub(crate) fn tokenize(source: &str) -> LexResult<Vec<Token>> {
    if source.starts_with('\u{feff}') {
        let start = SourcePosition::start();
        let end = SourcePosition {
            byte_offset: '\u{feff}'.len_utf8(),
            line: 1,
            column: 2,
        };
        return Err(Box::new(Diagnostic::error(
            "STK1002",
            "A byte order mark is not permitted.",
            Span { start, end },
        )));
    }

    Lexer::new(source).tokenize()
}

struct Lexer<'source> {
    source: &'source str,
    position: SourcePosition,
}

impl<'source> Lexer<'source> {
    fn new(source: &'source str) -> Self {
        Self {
            source,
            position: SourcePosition::start(),
        }
    }

    fn tokenize(mut self) -> LexResult<Vec<Token>> {
        let mut tokens = Vec::new();

        loop {
            self.skip_trivia();
            let start = self.position;
            let Some(character) = self.peek() else {
                tokens.push(Token {
                    kind: TokenKind::End,
                    span: Span::point(self.position),
                });
                return Ok(tokens);
            };

            let kind = match character {
                '{' => {
                    self.advance();
                    TokenKind::LeftBrace
                }
                '}' => {
                    self.advance();
                    TokenKind::RightBrace
                }
                '[' => {
                    self.advance();
                    TokenKind::LeftBracket
                }
                ']' => {
                    self.advance();
                    TokenKind::RightBracket
                }
                ',' => {
                    self.advance();
                    TokenKind::Comma
                }
                '.' => {
                    self.advance();
                    TokenKind::Dot
                }
                '"' => return self.lex_string(tokens),
                '<' if self.remaining().starts_with("<->") => {
                    self.advance();
                    self.advance();
                    self.advance();
                    TokenKind::BidirectionalArrow
                }
                '-' if self.remaining().starts_with("->") => {
                    self.advance();
                    self.advance();
                    TokenKind::ForwardArrow
                }
                '-' if self.remaining().starts_with("--") => {
                    self.advance();
                    self.advance();
                    TokenKind::Association
                }
                _ => self.lex_bare(),
            };

            tokens.push(Token {
                kind,
                span: Span {
                    start,
                    end: self.position,
                },
            });
        }
    }

    fn lex_string(mut self, mut tokens: Vec<Token>) -> LexResult<Vec<Token>> {
        let start = self.position;
        self.advance();
        let mut value = String::new();

        loop {
            let Some(character) = self.peek() else {
                return Err(Box::new(Diagnostic::error(
                    "STK2003",
                    "Input ended before the string was closed.",
                    Span {
                        start,
                        end: self.position,
                    },
                )));
            };

            match character {
                '"' => {
                    self.advance();
                    tokens.push(Token {
                        kind: TokenKind::String(value),
                        span: Span {
                            start,
                            end: self.position,
                        },
                    });
                    return self.finish_after_string(tokens);
                }
                '\\' => {
                    self.advance();
                    self.lex_escape(&mut value, start)?;
                }
                '\n' | '\r' | '\t' => {
                    let span = Span {
                        start: self.position,
                        end: self.position_after_current(),
                    };
                    return Err(Box::new(Diagnostic::error(
                        "STK1003",
                        "Strings cannot contain line breaks or tabs.",
                        span,
                    )));
                }
                value_character if value_character.is_control() => {
                    let span = Span {
                        start: self.position,
                        end: self.position_after_current(),
                    };
                    return Err(Box::new(Diagnostic::error(
                        "STK1003",
                        "Strings cannot contain control characters.",
                        span,
                    )));
                }
                value_character => {
                    value.push(value_character);
                    self.advance();
                }
            }
        }
    }

    fn finish_after_string(mut self, mut tokens: Vec<Token>) -> LexResult<Vec<Token>> {
        loop {
            self.skip_trivia();
            let start = self.position;
            let Some(character) = self.peek() else {
                tokens.push(Token {
                    kind: TokenKind::End,
                    span: Span::point(self.position),
                });
                return Ok(tokens);
            };

            let kind = match character {
                '{' => {
                    self.advance();
                    TokenKind::LeftBrace
                }
                '}' => {
                    self.advance();
                    TokenKind::RightBrace
                }
                '[' => {
                    self.advance();
                    TokenKind::LeftBracket
                }
                ']' => {
                    self.advance();
                    TokenKind::RightBracket
                }
                ',' => {
                    self.advance();
                    TokenKind::Comma
                }
                '.' => {
                    self.advance();
                    TokenKind::Dot
                }
                '"' => return self.lex_string(tokens),
                '<' if self.remaining().starts_with("<->") => {
                    self.advance();
                    self.advance();
                    self.advance();
                    TokenKind::BidirectionalArrow
                }
                '-' if self.remaining().starts_with("->") => {
                    self.advance();
                    self.advance();
                    TokenKind::ForwardArrow
                }
                '-' if self.remaining().starts_with("--") => {
                    self.advance();
                    self.advance();
                    TokenKind::Association
                }
                _ => self.lex_bare(),
            };

            tokens.push(Token {
                kind,
                span: Span {
                    start,
                    end: self.position,
                },
            });
        }
    }

    fn lex_escape(&mut self, value: &mut String, string_start: SourcePosition) -> LexResult<()> {
        let escape_start = self.position;
        let Some(character) = self.peek() else {
            return Err(Box::new(Diagnostic::error(
                "STK1003",
                "Input ended inside a string escape.",
                Span {
                    start: string_start,
                    end: self.position,
                },
            )));
        };

        match character {
            '"' => {
                value.push('"');
                self.advance();
            }
            '\\' => {
                value.push('\\');
                self.advance();
            }
            'u' => {
                self.advance();
                let high_or_scalar = self.lex_code_unit(escape_start)?;
                let scalar = if (0xd800..=0xdbff).contains(&high_or_scalar) {
                    if self.peek() != Some('\\') {
                        return Err(self.invalid_surrogate(escape_start));
                    }
                    self.advance();
                    if self.peek() != Some('u') {
                        return Err(self.invalid_surrogate(escape_start));
                    }
                    self.advance();
                    let low = self.lex_code_unit(escape_start)?;
                    if !(0xdc00..=0xdfff).contains(&low) {
                        return Err(self.invalid_surrogate(escape_start));
                    }
                    0x10000 + (((high_or_scalar - 0xd800) as u32) << 10) + (low - 0xdc00) as u32
                } else if (0xdc00..=0xdfff).contains(&high_or_scalar) {
                    return Err(self.invalid_surrogate(escape_start));
                } else {
                    high_or_scalar as u32
                };

                let Some(decoded) = char::from_u32(scalar) else {
                    return Err(self.invalid_escape(escape_start));
                };
                if decoded.is_control() || matches!(decoded, '\n' | '\r' | '\t') {
                    return Err(Box::new(Diagnostic::error(
                        "STK1003",
                        "A string escape decoded to a prohibited control value.",
                        Span {
                            start: escape_start,
                            end: self.position,
                        },
                    )));
                }
                value.push(decoded);
            }
            _ => return Err(self.invalid_escape(escape_start)),
        }

        Ok(())
    }

    fn lex_code_unit(&mut self, escape_start: SourcePosition) -> LexResult<u16> {
        let mut digits = String::with_capacity(4);
        for _ in 0..4 {
            let Some(character) = self.peek() else {
                return Err(self.invalid_escape(escape_start));
            };
            if !character.is_ascii_hexdigit() {
                return Err(self.invalid_escape(escape_start));
            }
            digits.push(character);
            self.advance();
        }

        u16::from_str_radix(&digits, 16).map_err(|_| self.invalid_escape(escape_start))
    }

    fn invalid_escape(&self, start: SourcePosition) -> Box<Diagnostic> {
        Box::new(Diagnostic::error(
            "STK1003",
            "The string contains an invalid escape.",
            Span {
                start,
                end: self.position,
            },
        ))
    }

    fn invalid_surrogate(&self, start: SourcePosition) -> Box<Diagnostic> {
        Box::new(Diagnostic::error(
            "STK1003",
            "The string contains an unpaired Unicode surrogate.",
            Span {
                start,
                end: self.position,
            },
        ))
    }

    fn lex_bare(&mut self) -> TokenKind {
        let start = self.position.byte_offset;

        while let Some(character) = self.peek() {
            if is_trivia(character)
                || matches!(character, '{' | '}' | '[' | ']' | ',' | '.' | '"')
                || self.remaining().starts_with("//")
                || self.remaining().starts_with("<->")
                || self.remaining().starts_with("->")
                || self.remaining().starts_with("--")
            {
                break;
            }
            self.advance();
        }

        if self.position.byte_offset == start {
            self.advance();
        }

        TokenKind::Bare(self.source[start..self.position.byte_offset].to_owned())
    }

    fn skip_trivia(&mut self) {
        loop {
            while self.peek().is_some_and(is_trivia) {
                self.advance();
            }

            if !self.remaining().starts_with("//") {
                return;
            }

            self.advance();
            self.advance();
            while self
                .peek()
                .is_some_and(|character| !matches!(character, '\n' | '\r'))
            {
                self.advance();
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn remaining(&self) -> &'source str {
        &self.source[self.position.byte_offset..]
    }

    fn advance(&mut self) {
        let Some(character) = self.peek() else {
            return;
        };

        if character == '\r' && self.remaining().starts_with("\r\n") {
            self.position.byte_offset += 2;
            self.position.line += 1;
            self.position.column = 1;
        } else {
            self.position.byte_offset += character.len_utf8();
            if matches!(character, '\n' | '\r') {
                self.position.line += 1;
                self.position.column = 1;
            } else {
                self.position.column += 1;
            }
        }
    }

    fn position_after_current(&self) -> SourcePosition {
        let mut copy = Self {
            source: self.source,
            position: self.position,
        };
        copy.advance();
        copy.position
    }
}

fn is_trivia(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r')
}

#[cfg(test)]
mod tests {
    use super::{Lexer, Token, TokenKind, tokenize};

    fn successful_tokens(source: &str) -> Vec<Token> {
        let result = tokenize(source);
        assert!(result.is_ok(), "{result:?}");
        result.into_iter().flatten().collect()
    }

    #[test]
    fn tokenizes_contextual_words_comments_and_operators() {
        let tokens = successful_tokens("node edge \"Label\" // note\n a->b a<->b a--b");
        let kinds: Vec<_> = tokens.into_iter().map(|token| token.kind).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Bare("node".into()),
                TokenKind::Bare("edge".into()),
                TokenKind::String("Label".into()),
                TokenKind::Bare("a".into()),
                TokenKind::ForwardArrow,
                TokenKind::Bare("b".into()),
                TokenKind::Bare("a".into()),
                TokenKind::BidirectionalArrow,
                TokenKind::Bare("b".into()),
                TokenKind::Bare("a".into()),
                TokenKind::Association,
                TokenKind::Bare("b".into()),
                TokenKind::End,
            ]
        );
    }

    #[test]
    fn decodes_supported_string_escapes_and_surrogate_pairs() {
        let tokens = successful_tokens(r#""quote: \" slash: \\ rocket: \uD83D\uDE80""#);

        assert_eq!(
            tokens[0].kind,
            TokenKind::String("quote: \" slash: \\ rocket: \u{1f680}".into())
        );
    }

    #[test]
    fn tracks_unicode_columns_and_crlf_lines() {
        let tokens = successful_tokens("node \u{65e5}\u{672c} \"x\"\r\nedge");

        assert_eq!(tokens[1].span.start.line, 1);
        assert_eq!(tokens[1].span.start.column, 6);
        assert_eq!(tokens[2].span.start.column, 9);
        assert_eq!(tokens[3].span.start.line, 2);
        assert_eq!(tokens[3].span.start.column, 1);
    }

    #[test]
    fn rejects_a_byte_order_mark() {
        let result = tokenize("\u{feff}stack 1.0");
        assert!(matches!(result, Err(diagnostic) if diagnostic.code == "STK1002"));
    }

    #[test]
    fn rejects_invalid_escapes_and_surrogates() {
        for source in [r#""\n""#, r#""\uD800""#, r#""\uDC00""#, r#""\uD800\u0041""#] {
            let result = tokenize(source);
            assert!(matches!(result, Err(diagnostic) if diagnostic.code == "STK1003"));
        }
    }

    #[test]
    fn reports_an_unterminated_string() {
        let result = tokenize("\"unfinished");
        assert!(matches!(result, Err(diagnostic) if diagnostic.code == "STK2003"));
    }

    #[test]
    fn tokenizes_punctuation_before_and_after_strings() {
        let tokens = successful_tokens("{}[],. \"label\" {}[],. <-> -> -- tail");
        let kinds: Vec<_> = tokens.into_iter().map(|token| token.kind).collect();

        assert!(kinds.starts_with(&[
            TokenKind::LeftBrace,
            TokenKind::RightBrace,
            TokenKind::LeftBracket,
            TokenKind::RightBracket,
            TokenKind::Comma,
            TokenKind::Dot,
        ]));
        assert!(kinds.contains(&TokenKind::BidirectionalArrow));
        assert!(kinds.contains(&TokenKind::ForwardArrow));
        assert!(kinds.contains(&TokenKind::Association));
        assert_eq!(kinds.last(), Some(&TokenKind::End));
    }

    #[test]
    fn rejects_raw_controls_and_incomplete_escapes() {
        for source in [
            "\"line\nbreak\"",
            "\"tab\tvalue\"",
            "\"control\u{1}value\"",
            "\"unfinished\\",
            r#""\u12"#,
            r#""\u12""#,
            r#""\uD800\x""#,
            r#""\u000A""#,
        ] {
            let result = tokenize(source);
            assert!(matches!(result, Err(diagnostic) if diagnostic.code == "STK1003"));
        }
    }

    #[test]
    fn handles_empty_input_and_unknown_operator_prefixes() {
        assert_eq!(
            successful_tokens("")
                .into_iter()
                .map(|token| token.kind)
                .collect::<Vec<_>>(),
            vec![TokenKind::End]
        );
        assert_eq!(
            successful_tokens("< -")
                .into_iter()
                .map(|token| token.kind)
                .collect::<Vec<_>>(),
            vec![
                TokenKind::Bare("<".into()),
                TokenKind::Bare("-".into()),
                TokenKind::End,
            ]
        );

        let mut lexer = Lexer::new("");
        lexer.advance();
        assert_eq!(lexer.position.byte_offset, 0);
    }
}
