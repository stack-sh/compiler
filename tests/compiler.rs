use stack_compiler::{compile, compile_bytes, diagnostic::Severity, ir};

const VALID_EXAMPLES: &[(&str, &str)] = &[
    ("minimal", include_str!("fixtures/valid/01-minimal.stack")),
    (
        "node semantics",
        include_str!("fixtures/valid/02-node-semantics.stack"),
    ),
    (
        "groups and layout",
        include_str!("fixtures/valid/03-groups-and-layout.stack"),
    ),
    (
        "commerce platform",
        include_str!("fixtures/valid/04-commerce-platform.stack"),
    ),
];

#[test]
fn canonical_examples_compile_without_diagnostics() {
    for (name, source) in VALID_EXAMPLES {
        let output = compile(source);
        assert!(
            output.diagnostics.is_empty(),
            "{name}: {:?}",
            output.diagnostics
        );
        assert!(output.diagram.is_some(), "{name}");
    }
}

#[test]
fn canonical_commerce_example_has_expected_normalized_shape() {
    let output = compile(VALID_EXAMPLES[3].1);
    assert!(output.diagram.is_some(), "{:?}", output.diagnostics);
    let Some(diagram) = output.diagram else {
        return;
    };

    assert_eq!(diagram.theme_id, "default");
    assert_eq!(diagram.nodes.len(), 13);
    assert_eq!(diagram.groups.len(), 5);
    assert_eq!(diagram.edges.len(), 12);
    assert_eq!(
        diagram.layout.as_ref().and_then(|layout| layout.direction),
        Some(ir::Direction::Right),
    );
}

#[test]
fn compiler_errors_prevent_ir() {
    let output = compile(
        r#"stack 1.0
diagram "Invalid" {
  group empty "Empty" {}
  node api "API"
  edge api -> missing
}"#,
    );

    assert!(output.diagram.is_none());
    assert!(
        output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity == Severity::Error)
    );
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "STK3003")
    );
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "STK3009")
    );
}

#[test]
fn byte_api_reports_encoding_and_bom_errors() {
    let invalid_utf8 = compile_bytes(b"stack 1.0\n\xff");
    assert_eq!(invalid_utf8.diagnostics[0].code, "STK1001");
    assert!(invalid_utf8.diagram.is_none());

    let bom = compile_bytes("\u{feff}stack 1.0 diagram \"x\" { node x \"X\" }".as_bytes());
    assert_eq!(bom.diagnostics[0].code, "STK1002");
    assert!(bom.diagram.is_none());
}

#[test]
fn complexity_errors_and_degree_warnings_have_distinct_outcomes() {
    let no_nodes = compile("stack 1.0 diagram \"Empty\" {}");
    assert!(no_nodes.diagram.is_none());
    assert_eq!(no_nodes.diagnostics[0].code, "STK4003");

    let mut dense = String::from("stack 1.0\ndiagram \"Dense\" {\n  node hub \"Hub\"\n");
    for index in 0..13 {
        dense.push_str(&format!("  node n{index} \"N {index}\"\n"));
        dense.push_str(&format!("  edge hub -> n{index}\n"));
    }
    dense.push('}');
    let warning = compile(&dense);
    assert!(warning.diagram.is_some());
    assert_eq!(warning.diagnostics[0].code, "STK4002");
    assert_eq!(warning.diagnostics[0].severity, Severity::Warning);
}
