//! Structured diagnostics emitted by compiler stages.

/// A one-based source position with its zero-based UTF-8 byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourcePosition {
    /// Zero-based byte offset in the original UTF-8 source.
    pub byte_offset: usize,
    /// One-based source line.
    pub line: usize,
    /// One-based Unicode scalar column.
    pub column: usize,
}

impl SourcePosition {
    /// Creates the first position in a source document.
    pub const fn start() -> Self {
        Self {
            byte_offset: 0,
            line: 1,
            column: 1,
        }
    }
}

/// An end-exclusive source span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    /// Inclusive start position.
    pub start: SourcePosition,
    /// Exclusive end position.
    pub end: SourcePosition,
}

impl Span {
    /// Creates an empty span at one position.
    pub const fn point(position: SourcePosition) -> Self {
        Self {
            start: position,
            end: position,
        }
    }

    /// Creates a span covering both input spans.
    pub fn covering(start: Self, end: Self) -> Self {
        Self {
            start: start.start,
            end: end.end,
        }
    }
}

/// A value paired with the source span that authored it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    /// Decoded or parsed value.
    pub value: T,
    /// Source span for the value.
    pub span: Span,
}

impl<T> Spanned<T> {
    /// Creates a spanned value.
    pub const fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}

/// Diagnostic severity defined by the Stack specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The source cannot produce normalized IR.
    Error,
    /// The source remains valid but deserves attention.
    Warning,
}

/// Additional source context related to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedInformation {
    /// Description of the related source location.
    pub message: String,
    /// Related source span.
    pub span: Span,
}

/// A portable compiler diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable diagnostic identifier.
    pub code: &'static str,
    /// Error or warning severity.
    pub severity: Severity,
    /// Concise human-readable description.
    pub message: String,
    /// Primary source span.
    pub span: Span,
    /// Ordered source values or constructs valid at the primary span.
    pub expected: Vec<String>,
    /// Optional corrective guidance.
    pub help: Option<String>,
    /// Other declarations or references involved in the problem.
    pub related: Vec<RelatedInformation>,
}

impl Diagnostic {
    pub(crate) fn error(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            span,
            expected: Vec::new(),
            help: None,
            related: Vec::new(),
        }
    }

    pub(crate) fn warning(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            span,
            expected: Vec::new(),
            help: None,
            related: Vec::new(),
        }
    }

    pub(crate) fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub(crate) fn with_expected<I, S>(mut self, expected: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.expected = expected.into_iter().map(Into::into).collect();
        self
    }

    pub(crate) fn with_related(mut self, message: impl Into<String>, span: Span) -> Self {
        self.related.push(RelatedInformation {
            message: message.into(),
            span,
        });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, SourcePosition, Span};

    #[test]
    fn diagnostics_default_to_no_expected_values() {
        let diagnostic = Diagnostic::error(
            "STK2002",
            "Unexpected value.",
            Span::point(SourcePosition::start()),
        );

        assert!(diagnostic.expected.is_empty());
    }

    #[test]
    fn diagnostics_preserve_expected_value_order() {
        let diagnostic = Diagnostic::error(
            "STK2002",
            "Unknown direction.",
            Span::point(SourcePosition::start()),
        )
        .with_expected(["right", "down"]);

        assert_eq!(diagnostic.expected, ["right", "down"]);
    }
}
