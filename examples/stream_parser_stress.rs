//! Stress the streaming parser with tricky content boundaries.
use semantic_query::json_utils::{JsonStreamParser, NodeType};

fn main() {
    let chunks = vec![
        "Lead {\"x\": ",
        "{\"y\": [1,2,3]}",
        ", \"z\": 3}",
        " tail [{\"ok\":true}, {\"k\":\"v\"}] end",
    ];

    let mut p = JsonStreamParser::new();
    let mut accum = String::new();
    for c in chunks {
        println!("feeding chunk: {:?}", c);
        accum.push_str(c);
        for node in p.feed(c) {
            let end = node.end + 1;
            println!("CLOSED {:?} {}..{} => {}", node.kind, node.start, end, &accum[node.start..end]);
            if node.kind == NodeType::Array { println!("  array children: {}", node.children.len()); }
        }
    }
}

