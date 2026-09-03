use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};
use stack_compiler::{compile_bytes, diagnostic, ir};

#[test]
fn valid_cases_match_normalized_ir() -> Result<(), Box<dyn Error>> {
    let root = specification_root()?.join("conformance/valid");
    for case in case_directories(&root)? {
        let source = fs::read(case.join("source.stack"))?;
        let expected_ir = read_json(&case.join("expected.ir.json"))?;
        let output = compile_bytes(&source);

        let Some(diagram) = output.diagram else {
            return Err(test_error(format!(
                "{} did not produce IR: {:?}",
                case.display(),
                output.diagnostics
            )));
        };
        assert_eq!(
            diagram_json(&diagram),
            expected_ir,
            "IR mismatch in {}",
            case.display()
        );

        let expected_diagnostics_path = case.join("expected.diagnostics.json");
        let expected_diagnostics = if expected_diagnostics_path.exists() {
            read_json(&expected_diagnostics_path)?
        } else {
            json!({ "schemaVersion": "1.0", "diagnostics": [] })
        };
        assert_eq!(
            diagnostics_json(&output.diagnostics, &expected_diagnostics),
            expected_diagnostics,
            "diagnostic mismatch in {}",
            case.display()
        );
    }
    Ok(())
}

#[test]
fn invalid_cases_match_portable_diagnostics() -> Result<(), Box<dyn Error>> {
    let root = specification_root()?.join("conformance/invalid");
    for case in case_directories(&root)? {
        let source = fs::read(case.join("source.stack"))?;
        let expected = read_json(&case.join("expected.diagnostics.json"))?;
        let output = compile_bytes(&source);

        assert!(
            output.diagram.is_none(),
            "invalid case produced IR: {}",
            case.display()
        );
        assert_eq!(
            diagnostics_json(&output.diagnostics, &expected),
            expected,
            "diagnostic mismatch in {}",
            case.display()
        );
    }
    Ok(())
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
            "no conformance cases found in {}",
            root.display()
        )));
    }
    Ok(cases)
}

fn read_json(path: &Path) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn test_error(message: String) -> Box<dyn Error> {
    Box::new(std::io::Error::other(message))
}

fn diagnostics_json(diagnostics: &[diagnostic::Diagnostic], expected: &Value) -> Value {
    let expected_diagnostics = expected.get("diagnostics").and_then(Value::as_array);
    json!({
        "schemaVersion": "1.0",
        "diagnostics": diagnostics
            .iter()
            .enumerate()
            .map(|(index, diagnostic)| {
                let compare_expected = expected_diagnostics
                    .and_then(|items| items.get(index))
                    .and_then(|item| item.get("expected"))
                    .is_some();
                diagnostic_expectation_json(diagnostic, compare_expected)
            })
            .collect::<Vec<_>>(),
    })
}

fn diagnostic_expectation_json(
    diagnostic: &diagnostic::Diagnostic,
    compare_expected: bool,
) -> Value {
    let mut value = json!({
        "code": diagnostic.code,
        "severity": severity_name(diagnostic.severity),
        "range": range_json(diagnostic.span),
    });
    if compare_expected {
        value["expected"] = json!(diagnostic.expected);
    }
    value
}

fn severity_name(severity: diagnostic::Severity) -> &'static str {
    match severity {
        diagnostic::Severity::Error => "error",
        diagnostic::Severity::Warning => "warning",
    }
}

fn range_json(span: diagnostic::Span) -> Value {
    json!({
        "start": position_json(span.start),
        "end": position_json(span.end),
    })
}

fn position_json(position: diagnostic::SourcePosition) -> Value {
    json!({
        "byteOffset": position.byte_offset,
        "line": position.line,
        "column": position.column,
    })
}

fn diagram_json(diagram: &ir::Diagram) -> Value {
    json!({
        "schemaVersion": "1.0",
        "languageVersion": {
            "major": diagram.language_version.major,
            "minor": diagram.language_version.minor,
        },
        "title": diagram.title,
        "themeId": diagram.theme_id,
        "children": diagram.children.iter().map(element_json).collect::<Vec<_>>(),
        "nodes": diagram.nodes.iter().map(node_json).collect::<Vec<_>>(),
        "groups": diagram.groups.iter().map(group_json).collect::<Vec<_>>(),
        "edges": diagram.edges.iter().map(edge_json).collect::<Vec<_>>(),
        "layout": diagram.layout.as_ref().map(layout_json),
    })
}

fn element_json(element: &ir::ElementId) -> Value {
    match element {
        ir::ElementId::Node(id) => json!({ "type": "node", "id": id }),
        ir::ElementId::Group(id) => json!({ "type": "group", "id": id }),
    }
}

fn node_json(node: &ir::Node) -> Value {
    json!({
        "id": node.id,
        "label": node.label,
        "kind": node_kind_name(node.kind),
        "iconId": node.icon_id,
        "detail": node.detail,
        "parentGroupId": node.parent_group_id,
    })
}

fn node_kind_name(kind: ir::NodeKind) -> &'static str {
    match kind {
        ir::NodeKind::Actor => "actor",
        ir::NodeKind::Client => "client",
        ir::NodeKind::Service => "service",
        ir::NodeKind::Function => "function",
        ir::NodeKind::Worker => "worker",
        ir::NodeKind::Database => "database",
        ir::NodeKind::Cache => "cache",
        ir::NodeKind::Queue => "queue",
        ir::NodeKind::Storage => "storage",
        ir::NodeKind::External => "external",
    }
}

fn group_json(group: &ir::Group) -> Value {
    json!({
        "id": group.id,
        "label": group.label,
        "parentGroupId": group.parent_group_id,
        "children": group.children.iter().map(element_json).collect::<Vec<_>>(),
        "layout": group.layout.as_ref().map(layout_json),
    })
}

fn edge_json(edge: &ir::Edge) -> Value {
    json!({
        "from": edge.from,
        "to": edge.to,
        "direction": edge_direction_name(edge.direction),
        "kind": edge_kind_name(edge.kind),
        "label": edge.label,
    })
}

fn edge_direction_name(direction: ir::EdgeDirection) -> &'static str {
    match direction {
        ir::EdgeDirection::Forward => "forward",
        ir::EdgeDirection::Bidirectional => "bidirectional",
        ir::EdgeDirection::Association => "association",
    }
}

fn edge_kind_name(kind: ir::EdgeKind) -> &'static str {
    match kind {
        ir::EdgeKind::Flow => "flow",
        ir::EdgeKind::Request => "request",
        ir::EdgeKind::Event => "event",
        ir::EdgeKind::Data => "data",
        ir::EdgeKind::Dependency => "dependency",
    }
}

fn layout_json(layout: &ir::Layout) -> Value {
    json!({
        "direction": layout.direction.map(direction_name),
        "sameRanks": layout.same_ranks,
        "order": layout.order,
    })
}

fn direction_name(direction: ir::Direction) -> &'static str {
    match direction {
        ir::Direction::Right => "right",
        ir::Direction::Down => "down",
    }
}

fn specification_root() -> Result<PathBuf, Box<dyn Error>> {
    let path = std::env::var_os("STACK_SPECIFICATION_DIR").ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "STACK_SPECIFICATION_DIR must identify a specification checkout",
        )
    })?;
    Ok(PathBuf::from(path))
}
