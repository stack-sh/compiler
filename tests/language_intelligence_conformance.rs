use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};
use stack_compiler::{
    diagnostic::{self, SourcePosition},
    language_intelligence::{
        CompletionCatalog, CompletionCatalogEntry, CompletionItem, CompletionKind, DocumentSymbol,
        DocumentSymbolKind, Hover, HoverKind, TextEdit, completion, diagnostics, document_symbols,
        hover,
    },
};

#[test]
fn compiler_owned_operations_match_canonical_fixtures() -> Result<(), Box<dyn Error>> {
    let root = specification_root()?.join("conformance/language-intelligence");
    let mut checked = 0_usize;
    for case in case_directories(&root)? {
        let fixture = read_json(&case.join("fixture.json"))?;
        let source_name = required_string(&fixture, "source")?;
        let source = fs::read_to_string(case.join(source_name))?;
        for operation in required_array(&fixture, "operations")? {
            let request = required_value(operation, "request")?;
            let feature = required_string(request, "feature")?;
            let Some(actual) = compiler_response(&source, request)? else {
                assert_eq!(feature, "format", "unknown delegated operation");
                continue;
            };
            let expected = required_value(operation, "response")?;
            assert_eq!(
                actual,
                *expected,
                "language-intelligence mismatch in {} operation {}",
                case.display(),
                required_string(operation, "id")?
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked, 8,
        "canonical compiler-owned operation count changed"
    );
    Ok(())
}

#[test]
fn formatter_owned_operations_are_explicitly_delegated() -> Result<(), Box<dyn Error>> {
    let root = specification_root()?.join("conformance/language-intelligence");
    let mut format_operations = 0_usize;
    for case in case_directories(&root)? {
        let fixture = read_json(&case.join("fixture.json"))?;
        for operation in required_array(&fixture, "operations")? {
            let request = required_value(operation, "request")?;
            if required_string(request, "feature")? == "format" {
                let response = required_value(operation, "response")?;
                assert!(
                    required_array(response, "edits")?.len() == 1,
                    "canonical formatter fixture must contain one whole-document edit"
                );
                format_operations += 1;
            }
        }
    }
    assert_eq!(
        format_operations, 1,
        "canonical formatter ownership fixture count changed"
    );
    Ok(())
}

fn compiler_response(source: &str, request: &Value) -> Result<Option<Value>, Box<dyn Error>> {
    let version = required_u64(request, "documentVersion")?;
    let response = match required_string(request, "feature")? {
        "diagnostics" => {
            let output = diagnostics(source, version);
            json!({
                "schemaVersion": output.schema_version,
                "kind": "response",
                "documentVersion": output.document_version,
                "feature": "diagnostics",
                "diagnostics": diagnostics_json(&output.diagnostics),
            })
        }
        "completion" => {
            let position = position_from_json(required_value(request, "position")?)?;
            let catalog = catalog_from_json(required_value(request, "completionCatalog")?)?;
            let output = completion(source, version, position, &catalog)?;
            json!({
                "schemaVersion": output.schema_version,
                "kind": "response",
                "documentVersion": output.document_version,
                "feature": "completion",
                "diagnostics": diagnostics_json(&output.diagnostics),
                "isIncomplete": output.is_incomplete,
                "items": output.items.iter().map(completion_item_json).collect::<Vec<_>>(),
            })
        }
        "hover" => {
            let position = position_from_json(required_value(request, "position")?)?;
            let output = hover(source, version, position)?;
            json!({
                "schemaVersion": output.schema_version,
                "kind": "response",
                "documentVersion": output.document_version,
                "feature": "hover",
                "diagnostics": diagnostics_json(&output.diagnostics),
                "hover": output.hover.as_ref().map(hover_json),
            })
        }
        "documentSymbols" => {
            let output = document_symbols(source, version);
            json!({
                "schemaVersion": output.schema_version,
                "kind": "response",
                "documentVersion": output.document_version,
                "feature": "documentSymbols",
                "diagnostics": diagnostics_json(&output.diagnostics),
                "symbols": output.symbols.iter().map(document_symbol_json).collect::<Vec<_>>(),
            })
        }
        "format" => return Ok(None),
        feature => return Err(test_error(format!("unsupported feature: {feature}"))),
    };
    Ok(Some(response))
}

fn catalog_from_json(value: &Value) -> Result<CompletionCatalog, Box<dyn Error>> {
    let icons = required_array(value, "icons")?
        .iter()
        .map(|entry| {
            Ok(CompletionCatalogEntry {
                id: required_string(entry, "id")?.to_owned(),
                label: required_string(entry, "label")?.to_owned(),
                detail: optional_string(entry, "detail")?,
                documentation: optional_string(entry, "documentation")?,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    Ok(CompletionCatalog { icons })
}

fn position_from_json(value: &Value) -> Result<SourcePosition, Box<dyn Error>> {
    Ok(SourcePosition {
        byte_offset: required_usize(value, "byteOffset")?,
        line: required_usize(value, "line")?,
        column: required_usize(value, "column")?,
    })
}

fn diagnostics_json(diagnostics: &[diagnostic::Diagnostic]) -> Vec<Value> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "code": diagnostic.code,
                "severity": severity_name(diagnostic.severity),
                "message": diagnostic.message,
                "range": range_json(diagnostic.span),
                "expected": diagnostic.expected,
                "help": diagnostic.help,
                "related": diagnostic.related.iter().map(|related| json!({
                    "message": related.message,
                    "range": range_json(related.span),
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn completion_item_json(item: &CompletionItem) -> Value {
    json!({
        "label": item.label,
        "kind": completion_kind_name(item.kind),
        "detail": item.detail,
        "documentation": item.documentation,
        "filterText": item.filter_text,
        "sortText": item.sort_text,
        "edit": text_edit_json(&item.edit),
    })
}

fn hover_json(hover: &Hover) -> Value {
    json!({
        "range": range_json(hover.range),
        "kind": hover_kind_name(hover.kind),
        "label": hover.label,
        "detail": hover.detail,
        "documentation": hover.documentation,
    })
}

fn document_symbol_json(symbol: &DocumentSymbol) -> Value {
    json!({
        "name": symbol.name,
        "kind": document_symbol_kind_name(symbol.kind),
        "detail": symbol.detail,
        "range": range_json(symbol.range),
        "selectionRange": range_json(symbol.selection_range),
        "children": symbol.children.iter().map(document_symbol_json).collect::<Vec<_>>(),
    })
}

fn text_edit_json(edit: &TextEdit) -> Value {
    json!({
        "range": range_json(edit.range),
        "newText": edit.new_text,
    })
}

fn range_json(span: diagnostic::Span) -> Value {
    json!({
        "start": position_json(span.start),
        "end": position_json(span.end),
    })
}

fn position_json(position: SourcePosition) -> Value {
    json!({
        "byteOffset": position.byte_offset,
        "line": position.line,
        "column": position.column,
    })
}

fn completion_kind_name(kind: CompletionKind) -> &'static str {
    match kind {
        CompletionKind::Keyword => "keyword",
        CompletionKind::Property => "property",
        CompletionKind::EnumValue => "enumValue",
        CompletionKind::Identifier => "identifier",
        CompletionKind::Icon => "icon",
    }
}

fn hover_kind_name(kind: HoverKind) -> &'static str {
    match kind {
        HoverKind::Diagram => "diagram",
        HoverKind::Group => "group",
        HoverKind::Node => "node",
        HoverKind::Edge => "edge",
        HoverKind::Property => "property",
    }
}

fn document_symbol_kind_name(kind: DocumentSymbolKind) -> &'static str {
    match kind {
        DocumentSymbolKind::Diagram => "diagram",
        DocumentSymbolKind::Group => "group",
        DocumentSymbolKind::Node => "node",
        DocumentSymbolKind::Edge => "edge",
    }
}

fn severity_name(severity: diagnostic::Severity) -> &'static str {
    match severity {
        diagnostic::Severity::Error => "error",
        diagnostic::Severity::Warning => "warning",
    }
}

fn case_directories(root: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut cases = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            cases.push(entry.path());
        }
    }
    cases.sort();
    if cases.is_empty() {
        return Err(test_error(format!(
            "no language-intelligence cases found in {}",
            root.display()
        )));
    }
    Ok(cases)
}

fn read_json(path: &Path) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn required_value<'value>(
    value: &'value Value,
    key: &str,
) -> Result<&'value Value, Box<dyn Error>> {
    value
        .get(key)
        .ok_or_else(|| test_error(format!("missing JSON property: {key}")))
}

fn required_array<'value>(
    value: &'value Value,
    key: &str,
) -> Result<&'value Vec<Value>, Box<dyn Error>> {
    required_value(value, key)?
        .as_array()
        .ok_or_else(|| test_error(format!("JSON property is not an array: {key}")))
}

fn required_string<'value>(value: &'value Value, key: &str) -> Result<&'value str, Box<dyn Error>> {
    required_value(value, key)?
        .as_str()
        .ok_or_else(|| test_error(format!("JSON property is not a string: {key}")))
}

fn optional_string(value: &Value, key: &str) -> Result<Option<String>, Box<dyn Error>> {
    match required_value(value, key)? {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text.clone())),
        _ => Err(test_error(format!(
            "JSON property is not a string or null: {key}"
        ))),
    }
}

fn required_u64(value: &Value, key: &str) -> Result<u64, Box<dyn Error>> {
    required_value(value, key)?
        .as_u64()
        .ok_or_else(|| test_error(format!("JSON property is not an unsigned integer: {key}")))
}

fn required_usize(value: &Value, key: &str) -> Result<usize, Box<dyn Error>> {
    usize::try_from(required_u64(value, key)?)
        .map_err(|_| test_error(format!("JSON property does not fit usize: {key}")))
}

fn specification_root() -> Result<PathBuf, Box<dyn Error>> {
    let directory = std::env::var_os("STACK_SPECIFICATION_DIR").ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "STACK_SPECIFICATION_DIR must identify a specification checkout",
        )
    })?;
    Ok(PathBuf::from(directory))
}

fn test_error(message: String) -> Box<dyn Error> {
    Box::new(std::io::Error::other(message))
}
