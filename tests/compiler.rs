use stack_compiler::{
    compile, compile_bytes, diagnostic::Severity, ir, lossless::TokenKind, parse_lossless,
};

#[test]
fn public_api_applies_defaults_to_a_valid_document() {
    let output = compile("stack 1.0 diagram \"API\" { node api \"API\" }");
    assert!(output.diagram.is_some(), "{:?}", output.diagnostics);
    let Some(diagram) = output.diagram else {
        return;
    };

    assert_eq!(diagram.theme_id, "default");
    assert_eq!(diagram.nodes.len(), 1);
    let Some(node) = diagram.nodes.first() else {
        return;
    };
    assert_eq!(node.kind, ir::NodeKind::Service);
    assert!(diagram.edges.is_empty());
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
fn public_lossless_api_reconstructs_authored_source() {
    let source = concat!(
        "// leading\r\n",
        "stack 1.0\r\n",
        "diagram \"\\u0041\" { node api \"API\" } // trailing\r\n",
    );
    let output = parse_lossless(source);
    assert!(output.diagnostics.is_empty());
    let Some(document) = output.document else {
        return;
    };

    assert_eq!(document.reconstruct(), source);
    assert!(
        document
            .tokens()
            .iter()
            .any(|token| token.kind == TokenKind::LineComment)
    );
    assert!(document.tokens().iter().any(
        |token| matches!(&token.kind, TokenKind::String(value) if value == "A")
            && token.text == "\"\\u0041\""
    ));
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
