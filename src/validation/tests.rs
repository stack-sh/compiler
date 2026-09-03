use crate::diagnostic::Severity;
use crate::ir::{Direction, EdgeDirection, EdgeKind, ElementId, NodeKind};

use super::levenshtein;

fn compile(source: &str) -> crate::CompileOutput {
    crate::compile(source)
}

fn codes(output: &crate::CompileOutput) -> Vec<&'static str> {
    output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn validation_applies_defaults_and_normalizes_structure() {
    let output = compile(
        r#"stack 1.0
diagram "API" {
  group platform "Platform" {
node app "Application"
node db "Database" { kind database }
  }
  edge app -> db
}"#,
    );

    assert!(output.diagnostics.is_empty());
    let Some(diagram) = output.diagram else {
        return;
    };
    assert_eq!(diagram.theme_id, "default");
    assert_eq!(diagram.children, vec![ElementId::Group("platform".into())]);
    assert_eq!(diagram.nodes[0].kind, NodeKind::Service);
    assert_eq!(
        diagram.nodes[0].parent_group_id.as_deref(),
        Some("platform")
    );
    assert_eq!(diagram.nodes[1].kind, NodeKind::Database);
    assert_eq!(diagram.edges[0].kind, EdgeKind::Flow);
    assert_eq!(diagram.edges[0].direction, EdgeDirection::Forward);
}

#[test]
fn validation_collects_independent_semantic_errors() {
    let output = compile(
        r#"stack 2.0
diagram " Bad " {
  group empty "Empty" {}
  node Bad ""
  node Bad "Duplicate"
  edge missing -> empty
}"#,
    );

    let codes = codes(&output);
    assert!(output.diagram.is_none());
    for expected in [
        "STK2001", "STK3001", "STK3002", "STK3003", "STK3004", "STK3008", "STK3009",
    ] {
        assert!(codes.contains(&expected), "missing {expected}: {codes:?}");
    }
}

#[test]
fn validation_rejects_duplicate_properties_edges_and_layout_singletons() {
    let output = compile(
        r#"stack 1.0
diagram "Duplicates" {
  layout { direction right direction down order [a, b] order [b, a] }
  node a "A" { kind service kind worker }
  node b "B"
  edge a -- b { kind flow }
  edge b -- a
}"#,
    );

    let codes = codes(&output);
    assert!(codes.contains(&"STK3007"));
    assert!(codes.contains(&"STK3012"));
    assert!(codes.contains(&"STK3006"));
}

#[test]
fn validation_checks_layout_scope_and_rank_membership() {
    let output = compile(
        r#"stack 1.0
diagram "Layout" {
  node outside "Outside"
  group platform "Platform" {
layout {
  rank same [a, b]
  rank same [a, outside]
  order [a, a]
}
node a "A"
node b "B"
  }
}"#,
    );

    assert!(
        codes(&output)
            .iter()
            .filter(|code| **code == "STK3011")
            .count()
            >= 3
    );
}

#[test]
fn validation_checks_icons_and_text() {
    let output = compile(
        r#"stack 1.0
diagram "Icons" {
  node app "App" { icon "Bad_icon" detail " detail" }
}"#,
    );

    let codes = codes(&output);
    assert!(codes.contains(&"STK3013"));
    assert!(codes.contains(&"STK3008"));
}

#[test]
fn validation_warns_when_node_degree_exceeds_twelve() {
    let mut source = String::from("stack 1.0\ndiagram \"Dense\" {\n  node hub \"Hub\"\n");
    for index in 0..13 {
        source.push_str(&format!("  node n{index} \"N {index}\"\n"));
        source.push_str(&format!("  edge hub -> n{index}\n"));
    }
    source.push('}');

    let output = compile(&source);
    assert!(output.diagram.is_some());
    assert_eq!(codes(&output), vec!["STK4002"]);
}

#[test]
fn normalization_covers_all_node_edge_and_containment_variants() {
    let output = compile(
        r#"stack 1.0
diagram "Complete IR" {
  theme light
  group outer "Outer" {
group inner "Inner" {
  node actor "Actor" { kind actor }
  node client "Client" { kind client }
  node function "Function" { kind function }
  node worker "Worker" { kind worker }
  node database "Database" { kind database }
  node cache "Cache" { kind cache }
  node queue "Queue" { kind queue }
  node storage "Storage" { kind storage }
  node external "External" { kind external }
}
  }
  node service "Service" { kind service icon "api" detail "Public API" }
  layout {
direction down
rank same [outer, service]
order [outer, service]
  }
  edge actor <-> client "Request" { kind request }
  edge function -- worker "Dependency" { kind dependency }
  edge database -> cache "Data" { kind data }
  edge queue -> storage "Event" { kind event }
  edge external -> service "Flow" { kind flow }
}"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let Some(diagram) = output.diagram else {
        return;
    };
    assert_eq!(diagram.theme_id, "light");
    assert_eq!(diagram.groups.len(), 2);
    assert_eq!(diagram.nodes.len(), 10);
    assert_eq!(diagram.edges.len(), 5);
    assert_eq!(diagram.children[0].as_str(), "outer");
    assert_eq!(diagram.children[1].as_str(), "service");
    assert_eq!(
        diagram
            .nodes
            .iter()
            .map(|node| node.kind)
            .collect::<Vec<_>>(),
        vec![
            NodeKind::Actor,
            NodeKind::Client,
            NodeKind::Function,
            NodeKind::Worker,
            NodeKind::Database,
            NodeKind::Cache,
            NodeKind::Queue,
            NodeKind::Storage,
            NodeKind::External,
            NodeKind::Service,
        ]
    );
    assert_eq!(diagram.edges[0].direction, EdgeDirection::Bidirectional);
    assert_eq!(diagram.edges[0].kind, EdgeKind::Request);
    assert_eq!(diagram.edges[1].direction, EdgeDirection::Association);
    assert_eq!(diagram.edges[1].kind, EdgeKind::Dependency);
    assert_eq!(diagram.edges[2].kind, EdgeKind::Data);
    assert_eq!(diagram.edges[3].kind, EdgeKind::Event);
    assert_eq!(diagram.edges[4].kind, EdgeKind::Flow);
    assert_eq!(
        diagram.layout.as_ref().and_then(|layout| layout.direction),
        Some(Direction::Down)
    );
}

#[test]
fn validation_reports_less_common_semantic_failures() {
    let output = compile(
        r#"stack 1.0
diagram "Invalid variants" {
  theme first
  theme second
  node a "A" { kind unknown }
  node b "B"
  layout { direction sideways }
  layout { direction right }
  edge Bad -> a
  edge a -> a
  edge a -> b { kind unknown }
}"#,
    );
    let codes = codes(&output);

    assert!(output.diagram.is_none());
    for expected in ["STK2002", "STK3001", "STK3005", "STK3012", "STK3014"] {
        assert!(codes.contains(&expected), "missing {expected}: {codes:?}");
    }
}

#[test]
fn validation_reports_closed_set_expectations() {
    let output = compile(
        r#"stack 1.0
diagram "Expected values" {
  node a "A" { kind process }
  node b "B"
  layout { direction sideways }
  edge a -> b { kind command }
}"#,
    );

    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "STK2002")
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        diagnostics[0].expected,
        [
            "actor", "client", "service", "function", "worker", "database", "cache", "queue",
            "storage", "external",
        ]
    );
    assert_eq!(diagnostics[1].expected, ["right", "down"]);
    assert_eq!(
        diagnostics[2].expected,
        ["flow", "request", "event", "data", "dependency"]
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.help.is_some())
    );
}

#[test]
fn validation_suggests_nearby_nodes_deterministically() {
    let output = compile(
        r#"stack 1.0
diagram "Suggestions" {
  node api "API"
  node payment "Payment"
  node paymant "Paymant"
  node paymont "Paymont"
  node database "Database"
  edge api -> paymnt
}"#,
    );

    let Some(diagnostic) = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "STK3003")
    else {
        return;
    };
    assert_eq!(diagnostic.expected, ["paymant", "payment", "paymont"]);
    assert_eq!(diagnostic.related.len(), 3);
    assert_eq!(
        diagnostic.help.as_deref(),
        Some("Use a declared node such as paymant, payment, paymont.")
    );
}

#[test]
fn suggestion_distance_counts_unicode_scalars() {
    assert_eq!(levenshtein("café", "cafe"), 1);
    assert_eq!(levenshtein("図", "図表"), 1);
    assert_eq!(levenshtein("right", "down"), 5);
}

#[test]
fn validation_rejects_excessive_group_depth() {
    let output = compile(
        r#"stack 1.0
diagram "Deep" {
  group one "One" {
group two "Two" {
  group three "Three" {
    group four "Four" {
      node leaf "Leaf"
    }
  }
}
  }
}"#,
    );

    assert!(output.diagram.is_none());
    assert!(codes(&output).contains(&"STK3010"));
}

#[test]
fn complexity_limits_cover_nodes_groups_and_edges() {
    let mut too_many_nodes = String::from("stack 1.0\ndiagram \"Nodes\" {\n");
    for index in 0..41 {
        too_many_nodes.push_str(&format!("  node n{index} \"Node {index}\"\n"));
    }
    too_many_nodes.push('}');
    assert!(codes(&compile(&too_many_nodes)).contains(&"STK4003"));

    let mut too_many_groups = String::from("stack 1.0\ndiagram \"Groups\" {\n");
    for index in 0..13 {
        too_many_groups.push_str(&format!(
            "  group g{index} \"Group {index}\" {{ node n{index} \"Node {index}\" }}\n"
        ));
    }
    too_many_groups.push('}');
    assert!(codes(&compile(&too_many_groups)).contains(&"STK4003"));

    let mut too_many_edges =
        String::from("stack 1.0\ndiagram \"Edges\" {\n  node a \"A\"\n  node b \"B\"\n");
    for index in 0..5 {
        too_many_edges.push_str(&format!("  edge a -> b \"Edge {index}\"\n"));
    }
    too_many_edges.push('}');
    assert!(codes(&compile(&too_many_edges)).contains(&"STK4003"));
}

#[test]
fn diagnostics_sort_errors_and_warnings_deterministically() {
    let mut source = String::from("stack 1.0\ndiagram \" Dense\" {\n  node hub \"Hub\"\n");
    for index in 0..13 {
        source.push_str(&format!("  node n{index} \"N {index}\"\n"));
        source.push_str(&format!("  edge hub -> n{index}\n"));
    }
    source.push('}');
    let output = compile(&source);

    assert!(output.diagram.is_none());
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    );
    assert!(
        output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Warning)
    );
}
