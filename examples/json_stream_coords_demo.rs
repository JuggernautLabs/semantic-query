//! Demonstrate the structure-aware JSON scanner over a mixed blob.
use semantic_query::json_utils::find_json_structures;

fn main() {
    let text = r#"Hello {"a":1} world [1, {"b":2}, 3] tail"#;
    println!("input: {}", text);
    let coords = find_json_structures(text);
    println!("found {} root structures", coords.len());
    for (i, node) in coords.iter().enumerate() {
        let end = node.end + 1;
        println!("#{} {:?} {}..{} => {}", i, node.kind, node.start, end, &text[node.start..end]);
        println!("  children: {}", node.children.len());
        for (j, ch) in node.children.iter().enumerate() {
            let end = ch.end + 1;
            println!("    - child#{} {:?} {}..{} => {}", j, ch.kind, ch.start, end, &text[ch.start..end]);
        }
    }
}

