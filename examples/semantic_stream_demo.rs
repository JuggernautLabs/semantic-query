use semantic_query::json_utils::{deserialize_stream_map, ParsedOrUnknown};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Finding { message: String, severity: String }

fn main() {
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
    // Mixed text + multiple JSON objects and arrays
    let blob = r#"
Intro text...
{"message":"Be careful","severity":"low"}
Some more text
{"message":"Oops","severity":"high","extra":123}
And even arrays: [{"message":"A","severity":"low"},{"message":"B","severity":"medium"}]
Trailing text
"#;

    let items: Vec<ParsedOrUnknown<Finding>> = deserialize_stream_map::<Finding>(blob);
    for (idx, item) in items.into_iter().enumerate() {
        match item {
            ParsedOrUnknown::Parsed(f) => println!(
                "{}: Parsed => message='{}', severity='{}'",
                idx, f.message, f.severity
            ),
            ParsedOrUnknown::Unknown(coords) => println!(
                "{}: Unknown => kind={:?} span=[{}..={}]",
                idx, coords.kind, coords.start, coords.end
            ),
        }
    }
}
