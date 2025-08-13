use serde::Deserialize;
use semantic_query::json_utils::extract_all;

#[derive(Debug, Deserialize, PartialEq)]
struct Item { x: i32 }

#[test]
fn extract_all_from_array() {
    let s = r#"[{"x":1},{"x":2},{"x":3}]"#;
    let v: Vec<Item> = extract_all(s);
    assert_eq!(v.len(), 3);
    assert_eq!(v[0].x, 1);
    assert_eq!(v[2].x, 3);
}

#[test]
fn extract_all_mixed_text_and_objects() {
    let s = r#"prefix {"x":10} middle {"y":99} tail {"x":20} end"#;
    let v: Vec<Item> = extract_all(s);
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].x, 10);
    assert_eq!(v[1].x, 20);
}

#[test]
fn extract_all_nested_array_in_text() {
    let s = r#"noise [{"x":7},{"x":8}] more {"x":9}"#;
    let v: Vec<Item> = extract_all(s);
    assert_eq!(v.len(), 3);
    assert_eq!(v[0].x, 7);
    assert_eq!(v[1].x, 8);
    assert_eq!(v[2].x, 9);
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type")]
enum Event {
    Tool { name: String, args: serde_json::Value },
    Notice { message: String },
}

#[test]
fn extract_all_enums_mixed_text_and_arrays() {
    let s = r#"
Start
{"type":"Notice","message":"warming up"}
[ {"type":"Tool","name":"open","args":{"path":"/tmp"}}, {"type":"Notice","message":"opened"} ]
End
{"type":"Tool","name":"close","args":{}}
"#;
    let events: Vec<Event> = extract_all(s);
    assert_eq!(events.len(), 4);
    match &events[0] { Event::Notice { message } => assert_eq!(message, "warming up"), _ => panic!("expected Notice") }
    match &events[1] { Event::Tool { name, args } => { assert_eq!(name, "open"); assert_eq!(args["path"], "/tmp"); }, _ => panic!("expected Tool") }
    match &events[2] { Event::Notice { message } => assert_eq!(message, "opened"), _ => panic!("expected Notice") }
    match &events[3] { Event::Tool { name, .. } => assert_eq!(name, "close"), _ => panic!("expected Tool") }
}

#[derive(Debug, Deserialize, PartialEq)]
struct ComplexItem { id: u32, name: String, meta: Meta }

#[derive(Debug, Deserialize, PartialEq)]
struct Meta { tags: Vec<String>, flags: Flags }

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "kind")]
enum Flags { A { fast: bool }, B { safe: bool } }

#[test]
fn extract_all_complex_structs_arrays_and_singletons() {
    let s = r#"
prefix
[{"id":1,"name":"alpha","meta":{"tags":["x","y"],"flags":{"kind":"A","fast":true}}},
 {"id":2,"name":"beta","meta":{"tags":["z"],"flags":{"kind":"B","safe":false}}}]
tail {"id":3,"name":"gamma","meta":{"tags":[],"flags":{"kind":"A","fast":false}}}
"#;
    let items: Vec<ComplexItem> = extract_all(s);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].id, 1);
    assert_eq!(items[1].meta.tags[0], "z");
    match items[2].meta.flags { Flags::A { fast } => assert!(!fast), _ => panic!("expected Flags::A") }
}
