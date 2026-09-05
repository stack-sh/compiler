//! Stateless, protocol-neutral language intelligence for one source snapshot.

use std::{collections::BTreeSet, error::Error, fmt};

use crate::{
    compile_bytes,
    diagnostic::{Diagnostic, SourcePosition, Span},
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

fn validate_position(source: &str, position: SourcePosition) -> Result<(), IntelligenceError> {
    if position.byte_offset > source.len()
        || !source.is_char_boundary(position.byte_offset)
        || (position.byte_offset > 0
            && source.as_bytes()[position.byte_offset - 1] == b'\r'
            && source.as_bytes().get(position.byte_offset) == Some(&b'\n'))
    {
        return Err(IntelligenceError::InvalidPosition);
    }

    let mut line = 1;
    let mut column = 1;
    let mut characters = source[..position.byte_offset].chars().peekable();
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

    if position.line != line || position.column != column {
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
        CompletionCatalog, CompletionCatalogEntry, IntelligenceError, MAX_COMPLETION_ICONS,
        diagnostics, diagnostics_bytes, validate_catalog, validate_position,
    };
    use crate::diagnostic::SourcePosition;

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
}
