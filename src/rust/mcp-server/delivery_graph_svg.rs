use std::collections::BTreeMap;

use crate::delivery_graph::{DeliveryGraph, GraphNode};

const LANES: &[(&str, &[&str], &str)] = &[
    ("Acceptance", &["acceptance"], "#277768"),
    ("Tasks", &["task"], "#366eb5"),
    ("Architecture", &["module", "flow"], "#8260a3"),
    ("Execution", &["result"], "#366eb5"),
    ("Evidence", &["file", "verification"], "#277768"),
    ("Review", &["review"], "#ad6825"),
];

const ARCHITECTURE_LANES: &[(&str, &[&str], &str)] = &[
    ("Applications", &["application"], "#277768"),
    ("Modules", &["module"], "#366eb5"),
    (
        "Interfaces & interactions",
        &["interface", "interaction"],
        "#8260a3",
    ),
    ("Data", &["entity", "datastore"], "#ad6825"),
    ("Flows", &["flow"], "#277768"),
];

fn xml(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t' | '\r'))
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// Fixed-width wrapping also breaks identifiers and paths with no word boundaries.
fn lines(value: &str) -> Vec<String> {
    let mut result = vec![];
    let mut current = String::new();
    let mut width = 0;
    for word in value.split_whitespace() {
        if !current.is_empty() {
            if width + 1 + word.chars().count() > 26 {
                result.push(std::mem::take(&mut current));
                width = 0;
            } else {
                current.push(' ');
                width += 1;
            }
        }
        for ch in word.chars() {
            if width >= 26 {
                result.push(std::mem::take(&mut current));
                width = 0;
            }
            current.push(ch);
            width += if ch.is_ascii() { 1 } else { 2 };
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn card_height(node: &GraphNode) -> usize {
    92 + lines(&node.label).len().max(1) * 20 + summary_lines(node).len() * 17
}

fn summary_lines(node: &GraphNode) -> Vec<String> {
    let mut summary = node.summary.as_deref().map(lines).unwrap_or_default();
    if summary.len() > 3 {
        summary.truncate(3);
        summary[2] = format!("{}...", summary[2].chars().take(23).collect::<String>());
    }
    summary
}

/// Local, deterministic SVG export. Coordinates are derived from graph data only.
pub fn render(graph: &DeliveryGraph) -> String {
    render_view(graph, false)
}

pub fn render_architecture(graph: &DeliveryGraph) -> String {
    render_view(graph, true)
}

fn render_view(graph: &DeliveryGraph, architecture: bool) -> String {
    let all_lanes = if architecture {
        ARCHITECTURE_LANES
    } else {
        LANES
    };
    let lanes: Vec<_> = all_lanes
        .iter()
        .filter(|(_, kinds, _)| {
            !architecture
                || graph.nodes.is_empty()
                || graph
                    .nodes
                    .iter()
                    .any(|node| kinds.contains(&node.kind.as_str()))
        })
        .collect();
    let width = 40 + lanes.len() * 284;
    let mut positions = BTreeMap::new();
    let mut bottom = 310;
    for (lane, (_, kinds, _)) in lanes.iter().enumerate() {
        let mut y = 208;
        for node in graph
            .nodes
            .iter()
            .filter(|node| kinds.contains(&node.kind.as_str()))
        {
            let height = card_height(node);
            positions.insert(node.id.as_str(), (40 + lane * 284, y, height));
            y += height + 30;
        }
        bottom = bottom.max(y);
    }
    let long_edges = graph
        .edges
        .iter()
        .filter(|edge| {
            matches!((positions.get(edge.from.as_str()), positions.get(edge.to.as_str())),
            (Some(&(x1, _, _)), Some(&(x2, _, _))) if x2 != x1 + 284)
        })
        .count();
    let height = bottom + (long_edges * 7 + 70).max(120);
    let title = if architecture {
        "Technical architecture"
    } else {
        "From acceptance to reviewed delivery"
    };
    let category = if architecture {
        "TECHNICAL ARCHITECTURE"
    } else {
        "DELIVERY EVIDENCE"
    };
    let description = if architecture {
        "Declared applications, modules, interfaces, data ownership and flows."
    } else {
        "Explicit links between acceptance, tasks, architecture, execution, evidence and review."
    };
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">
<title id="title">Loom {title}</title><desc id="desc">{description}</desc>
<defs><marker id="arrow" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0L7 3.5L0 7" fill="#83939d"/></marker></defs>
<rect width="100%" height="100%" fill="#f7f9fa"/>
<g font-family="Arial, Helvetica, sans-serif" letter-spacing="0">
<text x="40" y="48" font-size="14" fill="#277768">LOOM / {category}</text>
<text x="40" y="88" font-size="30" font-weight="700" fill="#172b35">{title}</text>
<text x="40" y="118" font-size="14" fill="#536773">{} / {} · {} nodes · {} relationships</text>
"##,
        xml(graph.delivery_id.as_deref().unwrap_or("No delivery")),
        xml(graph.phase_id.as_deref().unwrap_or("No phase")),
        graph.nodes.len(),
        graph.edges.len()
    );
    for (lane, (label, _, color)) in lanes.iter().enumerate() {
        let x = 40 + lane * 284;
        let label = xml(label);
        svg.push_str(&format!(r##"<rect x="{x}" y="153" width="244" height="3" fill="{color}"/><text x="{x}" y="183" font-size="16" font-weight="700" fill="#273e49">{label}</text>"##));
    }
    // Long connections use channels below the cards so they never pass through text.
    let mut channel = 0;
    for edge in &graph.edges {
        let (Some(&(x1, y1, h1)), Some(&(x2, y2, h2))) = (
            positions.get(edge.from.as_str()),
            positions.get(edge.to.as_str()),
        ) else {
            continue;
        };
        let planned = matches!(
            edge.relation.as_str(),
            "covers" | "planned artifact" | "planned by" | "precedes"
        );
        let dash = if planned {
            "stroke-dasharray=\"4 5\""
        } else {
            ""
        };
        let d = if x2 == x1 + 284 {
            let a = x1 + 244;
            let b = x2;
            let mid = a + 20;
            format!("M{a} {} H{mid} V{} H{b}", y1 + h1 / 2, y2 + h2 / 2)
        } else {
            let route = bottom + 12 + channel * 7;
            channel += 1;
            let a = x1 + 244;
            let gutter_a = x1 + 254;
            let gutter_b = x2 - 10;
            format!(
                "M{a} {} H{gutter_a} V{route} H{gutter_b} V{} H{x2}",
                y1 + h1 / 2,
                y2 + h2 / 2
            )
        };
        svg.push_str(&format!(r##"<path d="{d}" fill="none" stroke="#83939d" stroke-width="1.5" {dash} marker-end="url(#arrow)"><title>{}</title></path>"##, xml(&format!("{} → {}: {}", edge.from, edge.to, edge.relation))));
    }
    for node in &graph.nodes {
        let Some(&(x, y, h)) = positions.get(node.id.as_str()) else {
            continue;
        };
        let color = lanes
            .iter()
            .find(|(_, kinds, _)| kinds.contains(&node.kind.as_str()))
            .unwrap()
            .2;
        let tooltip = format!(
            "{}{}",
            node.id,
            node.summary
                .as_deref()
                .map(|summary| format!("\n{summary}"))
                .unwrap_or_default()
        );
        svg.push_str(&format!(r##"<g><title>{}</title><rect x="{x}" y="{y}" width="244" height="{h}" rx="6" fill="#ffffff" stroke="#d5dfe4"/><rect x="{x}" y="{y}" width="4" height="{h}" rx="2" fill="{color}"/><text x="{}" y="{}" font-size="11" fill="#687c87">{}</text>"##, xml(&tooltip), x + 18, y + 25, xml(&node.kind.to_uppercase())));
        for (line, text) in lines(&node.label).iter().enumerate() {
            svg.push_str(&format!(r##"<text x="{}" y="{}" font-size="14" font-weight="600" fill="#203742">{}</text>"##, x + 18, y + 52 + line * 20, xml(text)));
        }
        let summary_y = y + 52 + lines(&node.label).len().max(1) * 20;
        for (line, text) in summary_lines(node).iter().enumerate() {
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" font-size="12" fill="#536773">{}</text>"##,
                x + 18,
                summary_y + line * 17,
                xml(text)
            ));
        }
        if let Some(status) = &node.status {
            let (ink, bg) = match status.as_str() {
                "passed" | "approved" | "completed" => ("#17664b", "#e5f4ec"),
                "failed" | "blocked" | "rejected" => ("#a63235", "#fbeaec"),
                _ => ("#526571", "#eef2f4"),
            };
            let status = if status.chars().count() > 24 {
                format!("{}...", status.chars().take(21).collect::<String>())
            } else {
                status.clone()
            };
            svg.push_str(&format!(r##"<rect x="{}" y="{}" width="208" height="24" rx="4" fill="{bg}"/><text x="{}" y="{}" font-size="12" fill="{ink}">{}</text>"##, x + 18, y + h - 38, x + 27, y + h - 21, xml(&status)));
        }
        svg.push_str("</g>");
    }
    if graph.nodes.is_empty() {
        svg.push_str(r##"<text x="40" y="240" font-size="16" fill="#536773">No delivery records available.</text>"##);
    }
    let legend = if architecture {
        "Declared contract relationships: providers, consumers, ownership and data references. Hover for relation details."
    } else {
        "Solid: execution and evidence links. Dashed: planned coverage. Statuses reflect recorded results."
    };
    svg.push_str(&format!(
        r##"<text x="40" y="{}" font-size="12" fill="#687c87">{legend}</text></g></svg>"##,
        height - 24
    ));
    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_untrusted_labels_and_preserves_recorded_status() {
        let graph = DeliveryGraph {
            schema_version: 1,
            delivery_id: None,
            phase_id: None,
            nodes: vec![GraphNode {
                id: "result:1".into(),
                kind: "result".into(),
                label: "<script>&\"hello\"".into(),
                status: Some("blocked".into()),
                summary: None,
            }],
            edges: vec![],
            mermaid: String::new(),
        };
        let svg = render(&graph);
        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&lt;script&gt;&amp;&quot;hello&quot;"));
        assert!(svg.contains(">blocked</text>"));
        assert_eq!(svg, render(&graph));
        assert!(lines(&"a".repeat(200)).iter().all(|line| line.len() <= 26));
        let architecture = render_architecture(&graph);
        assert!(!architecture.contains("Interfaces & interactions"));
        let empty = DeliveryGraph {
            nodes: vec![],
            ..graph
        };
        assert!(render_architecture(&empty).contains("Interfaces &amp; interactions"));
    }
}
