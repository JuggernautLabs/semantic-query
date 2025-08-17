**Compile-Time Prompt System (YAML + Alias-Checked Constants) — Implementation Blueprint**

- Goal: Externalize all prompts into YAML files under `prompts/`, reference by alias in code, and fail compilation if an alias is unknown. At build time, generate alias-checked `&'static str` constants (like `include_str!`) and a macro for ergonomic access. Only a `&'static str` remains at call sites; no runtime parsing/mapping.

—

1) High-Level Architecture
- Inputs: YAML prompt files in `prompts/**/{*.yaml,*.yml}`.
- Build step: `build.rs` scans, parses, validates, and emits Rust code into `OUT_DIR/prompts_gen.rs`.
- Outputs: Public nested modules with `pub const` `&'static str` per alias, and a `prompt!()` macro that expands an alias literal to the constant path; unknown aliases yield a compile error.
- Consumption: Library calls `prompt!("namespace.alias")` to get a `&'static str` at compile time; minimal runtime interpolation (simple placeholder replacement) applies where needed.

—

2) File/Module Layout (Target State)
- prompts/
  - core/
    - schema.yaml            # example YAML, can define multiple prompts
  - ...                      # additional namespaces/files
- build.rs                   # scans prompts/, generates OUT_DIR/prompts_gen.rs
- src/prompts/mod.rs         # includes generated file
- src/util/template.rs       # small helper for placeholder substitution (string replace)
- Cargo.toml                 # declares build.rs and build-dependencies

—

3) Prompt YAML Spec (Authoring Contract)
- File format: YAML; each file contains a root key `prompts:`, with an array of entries.
- Entry schema (validated by build.rs):
  - name (string, optional): human-readable; not required to be unique globally.
  - aliases (array<string>, required): at least one alias per entry; each alias must be globally unique across the entire tree.
    - Convention: dot-separated, lower_snake segments (e.g., `core.schema.only_json`).
  - description (string, optional): for maintainers; not used by codegen.
  - template (string, required): the prompt body; use `|` block scalars for multi-line content.
- Placeholders: `{user_prompt}`, `{schema_json}`, etc. Interpreted by runtime string replacement, not by the build script. Unknown placeholders are left intact.
- Reserved characters: YAML supports quotes; avoid unintended `{`/`}` used for placeholders unless desired. For literal `{`/`}`, consider doubling (`{{`/`}}`) and documenting, or simply rephrasing.

Example (prompts/core/schema.yaml):
```
prompts:
  - name: schema_guided
    aliases: ["core.schema.only_json", "schema.guided"]
    description: Enforce JSON-only response with provided JSON Schema
    template: |
      {user_prompt}

      You must output only a single JSON value that strictly conforms to the following JSON Schema. Do not include explanations, prose, code fences labels, or additional text — only the JSON value itself.
      {schema_json}

  - name: semantic_stream
    aliases: ["core.semantic.stream"]
    template: |
      {user_prompt}

      Return a single JSON array of items in order. Each item must be one of:
      - Text: {"kind":"Text","content":{"text":"..."}}
      - Data: {"kind":"Data","content": <object matching the provided schema>}
      Do not include any text outside the JSON array. No code fences.
      {schema_json}
```

—

4) Code Generation Rules (Deterministic, Sanitized)
- Input collection:
  - Walk `prompts/` recursively (use `walkdir`).
  - Consider files with extensions `.yaml` or `.yml` only.
  - Sort files lexicographically for deterministic ordering in output.
- Parsing:
  - Use `serde_yaml` to parse each file into `Root { prompts: Vec<Prompt> }`.
  - On parse error, panic with: `Failed to parse YAML: <path>: <serde_yaml_error>` (serde_yaml includes line/col).
- Validation:
  - Each Prompt must have non-empty `aliases` and a non-empty `template`.
  - Aliases must be globally unique. Maintain a `HashMap<String, AliasMeta { file, index }>`; on duplicate, panic with: `Duplicate alias '<alias>' in <path1> and <path2>`.
  - Optionally warn (via println! to stderr) if an alias uses unexpected casing or characters.
- Alias → Module/Constant mapping:
  - Split alias by '.' into segments. For all but the last segment, create nested modules; for the last segment, create a `pub const`.
  - Sanitize each module identifier:
    - Convert to snake_case (replace hyphens and invalid chars with '_').
    - If identifier is a Rust keyword (e.g., `type`, `mod`, `crate`), append `_`.
    - Ensure it starts with a letter or `_`; if not, prefix with `_`.
  - Sanitize the const identifier from the last segment:
    - Convert to SCREAMING_SNAKE_CASE (non-alphanumeric → `_`).
    - Apply the same keyword/start-char rules as above.
  - Store a mapping of alias → const path (e.g., `prompts::core::schema::ONLY_JSON`).
- Raw string literal generation:
  - For each `template` value, create a raw-string literal using `r#"..."#` with enough `#` to avoid any `"#` collisions.
  - Algorithm: Compute the maximum consecutive `#` count that appears in `template` adjacent to an ending quote pattern and use `n + 1` (or simpler, always use e.g. 16 `#`s).
  - Normalize line endings to `\n` (optional); do not alter content otherwise.
- Emitted file structure (`prompts_gen.rs`):
  - Header comment: GENERATED FILE - DO NOT EDIT MANUALLY.
  - `#[allow(dead_code)] pub mod prompts { ... }` with nested modules and `pub const` definitions.
  - `#[macro_export] macro_rules! prompt { ... }` with one arm per alias: `("core.schema.only_json") => { $crate::prompts::core::schema::ONLY_JSON };`
  - A final fallback: `($other:literal) => { compile_error!(concat!("Unknown prompt alias: ", $other)); }`.
  - Optionally `pub const ALL_ALIASES: &[&str] = &["core.schema.only_json", ...];` for tooling.
- Rerun triggers:
  - Emit `cargo:rerun-if-changed=prompts` and for each file `cargo:rerun-if-changed=<path>`.
  - Also `cargo:rerun-if-env-changed=FORCE_PROMPT_REGEN` (optional override).

—

5) Cargo.toml Changes (Exact)
- Add build script:
  - `[package] build = "build.rs"`
- Add build dependencies (prefer stable, minimal versions):
```
[build-dependencies]
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
walkdir = "2"
heck = "0.5" # for case conversions (snake_case, SHOUTY_SNAKE)
```
- No runtime dependencies added for this feature; generation is build-time only.

—

6) Source Tree Additions
- `build.rs` skeleton (pseudocode with exact structure):
```
// build.rs
use std::{env, fs, path::{Path, PathBuf}};
use walkdir::WalkDir;
use serde::Deserialize;
use heck::{ToSnakeCase, ToShoutySnakeCase};

#[derive(Deserialize)]
struct Root { prompts: Vec<Prompt> }

#[derive(Deserialize)]
struct Prompt {
    name: Option<String>,
    aliases: Vec<String>,
    description: Option<String>,
    template: String,
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:rerun-if-changed=prompts");

    let mut files: Vec<PathBuf> = Vec::new();
    if Path::new("prompts").exists() {
        for entry in WalkDir::new("prompts").into_iter().filter_map(Result::ok) {
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if matches!(ext, "yaml" | "yml") { files.push(p.to_path_buf()); }
                }
            }
        }
    }
    files.sort();
    for f in &files { println!("cargo:rerun-if-changed={}", f.display()); }

    let mut alias_map: std::collections::BTreeMap<String, (String, String)> = std::collections::BTreeMap::new();
    // alias -> (module_path, const_name)
    let mut items: Vec<(String, String, String)> = Vec::new(); // (alias, module_path, literal)

    for file in &files {
        let s = fs::read_to_string(file).expect("read prompts file");
        let root: Root = serde_yaml::from_str(&s)
            .unwrap_or_else(|e| panic!("Failed to parse YAML: {}: {}", file.display(), e));
        for (idx, pr) in root.prompts.iter().enumerate() {
            if pr.aliases.is_empty() { panic!("Prompt missing aliases: {} (entry #{})", file.display(), idx); }
            if pr.template.trim().is_empty() { panic!("Prompt missing template: {} (entry #{})", file.display(), idx); }
            for alias in &pr.aliases {
                if let Some((prev_file, _)) = alias_map.get(alias) {
                    panic!("Duplicate alias '{}' in {} and {}", alias, prev_file, file.display());
                }
                let (mod_path, const_ident) = compute_path_idents(alias);
                alias_map.insert(alias.clone(), (file.display().to_string(), mod_path.clone()));
                items.push((alias.clone(), mod_path, pr.template.clone()));
            }
        }
    }

    let gen = render(items);
    let out = Path::new(&out_dir).join("prompts_gen.rs");
    fs::write(&out, gen).expect("write prompts_gen.rs");
}

fn compute_path_idents(alias: &str) -> (String, String) {
    // returns (module path like "prompts::core::schema", const ident like "ONLY_JSON")
    let mut segs: Vec<String> = alias.split('.')
        .map(|s| sanitize_mod(s))
        .collect();
    let last = segs.pop().unwrap_or_else(|| "prompt".into());
    let const_ident = sanitize_const(&last);
    let mut path = String::from("prompts");
    for s in segs { path.push_str("::"); path.push_str(&s); }
    (path, const_ident)
}

fn sanitize_mod(s: &str) -> String {
    let mut id = s.to_snake_case();
    if id.is_empty() || !id.chars().next().unwrap().is_ascii_alphabetic() { id.insert(0, '_'); }
    match id.as_str() {
        // rust keywords
        "as"|"break"|"const"|"continue"|"crate"|"else"|"enum"|"extern"|"false"|"fn"|"for"|"if"|"impl"|"in"|"let"|"loop"|"match"|"mod"|"move"|"mut"|"pub"|"ref"|"return"|"self"|"Self"|"static"|"struct"|"super"|"trait"|"true"|"type"|"unsafe"|"use"|"where"|"while"|"async"|"await"|"dyn" => { id.push('_'); }
        _ => {}
    }
    id
}

fn sanitize_const(s: &str) -> String {
    let mut id = s.to_shouty_snake_case();
    if id.is_empty() || !id.chars().next().unwrap().is_ascii_alphabetic() { id.insert(0, '_'); }
    id
}

fn render(items: Vec<(String, String, String)>) -> String {
    let mut out = String::new();
    out.push_str("// @generated by build.rs — DO NOT EDIT\n");
    out.push_str("#[allow(dead_code, non_snake_case, non_upper_case_globals)]\n");
    out.push_str("pub mod prompts {\n");
    // build a tree of modules -> consts
    use std::collections::BTreeMap as Map;
    let mut tree: Map<String, Vec<(String, String)>> = Map::new(); // mod_path -> Vec<(const_ident, literal)>
    for (alias, mod_path, lit) in items {
        let (_alias, const_ident) = { let parts: Vec<_> = alias.rsplitn(2, '.').collect(); (alias, sanitize_const(parts[0])) };
        tree.entry(mod_path).or_default().push((const_ident, lit));
    }
    // Emit modules and consts in sorted order
    let mut paths: Vec<_> = tree.keys().cloned().collect();
    paths.sort();
    for p in paths {
        // p like "prompts::core::schema" — we are already inside `pub mod prompts`, so strip leading "prompts::"
        let rel = p.strip_prefix("prompts::").unwrap_or(&p);
        let mods: Vec<&str> = if rel.is_empty() { vec![] } else { rel.split("::").collect() };
        emit_mods(&mut out, &mods, &tree[&p]);
    }
    out.push_str("}\n\n");
    // Macro arms
    out.push_str("#[macro_export]\nmacro_rules! prompt {\n");
    // regenerate alias-to-path mapping in a stable order
    // (omitted here for brevity in pseudocode)
    out.push_str("    ($other:literal) => { compile_error!(concat!(\"Unknown prompt alias: \", $other)); };\n}\n");
    out
}

fn emit_mods(out: &mut String, mods: &[&str], consts: &Vec<(String, String)>) {
    for m in mods { out.push_str(&format!("    pub mod {} {{\n", m)); }
    for (ident, lit) in consts {
        let raw = raw_string_literal(lit);
        out.push_str(&format!("        pub const {}: &str = {} ;\n", ident, raw));
    }
    for _ in mods { out.push_str("    }\n"); }
}

fn raw_string_literal(s: &str) -> String {
    // naive: always 16 hashes
    format!("r################\"{}\"################", s)
}
```

Notes:
- The above is a compact blueprint; the actual implementation should compute macro arms that map each alias literal to the corresponding `$crate::prompts::<mods>::<CONST>`.
- Determinism: keep alias and module emission stable (sort keys) to minimize diff churn.
- Windows paths: use `display()` only in messages; we only embed aliases and literals in generated code.

—

7) Library Integration (Code Touch Points)
- New module exposing generated artifacts: `src/prompts/mod.rs`
  - Contents:
    - `include!(concat!(env!("OUT_DIR"), "/prompts_gen.rs"));`
  - Re-export in `src/lib.rs`: add `pub mod prompts;` so consumers can refer to `crate::prompts::...`.
- Template fill helper: `src/util/template.rs`
```
pub fn fill_template(mut template: String, vars: &[(&str, &str)]) -> String {
    for (k, v) in vars {
        let needle = format!("{{{}}}", k);
        template = template.replace(&needle, v);
    }
    template
}
```
- Update `src/core.rs`:
  - `augment_prompt_with_schema<T>`:
    - Generate `schema_json` (already present).
    - `let tpl = prompt!("core.schema.only_json");`
    - `fill_template(tpl.to_string(), &[("user_prompt", &prompt), ("schema_json", &schema_json)])`
  - `augment_prompt_with_semantic_schema<T>`:
    - `let tpl = prompt!("core.semantic.stream");`
    - Same `fill_template` usage.

—

8) Error Messages (Exact Wording)
- YAML parse error: `Failed to parse YAML: <PATH>: <serde_yaml_err>`
- Missing aliases: `Prompt missing aliases: <PATH> (entry #<index>)`
- Missing template: `Prompt missing template: <PATH> (entry #<index>)`
- Duplicate alias: `Duplicate alias '<alias>' in <PATH_A> and <PATH_B>`
- Unknown alias at call site: compile error from macro: `Unknown prompt alias: <alias>`

—

9) Edge Cases and Policies
- Alias sanitization collisions: Two different aliases that sanitize to identical module/const paths (e.g., `x-y` and `x_y`) are not a problem because we index by original alias. However, when emitting module/const identifiers, the same sanitized id under the same module path would collide. To avoid this:
  - If a collision is detected at the identifier level (same module path and same const ident), append a short hash suffix derived from the original alias (e.g., `_A1B2`).
- Extremely long templates: Constants hold strings in the binary; large prompts increase binary size. This is by design and OK.
- Extremely long aliases: Rare; still supported. If too long for identifier, keep hashing suffix strategy.
- Braces in templates: For literal braces, authors can double them (`{{`, `}}`) and the code that fills placeholders should not try to interpret doubles. We are not implementing advanced templating in v1.
- Non-ASCII aliases: Discouraged; sanitize into ASCII identifier with `_` replacement; macro uses the literal alias string, so macro arm must match the exact literal including non-ASCII; acceptable but not recommended.
- Deterministic generation: Sort files and aliases before emission.

—

10) Testing Plan (Actionable)
- Unit tests (runtime):
  - Template substitution: multiple replacements, overlapping keys, unknown keys remain.
- Build-time checks (manual and CI):
  - Introduce a duplicate alias in a temp file under `prompts/` and assert build fails with the duplicate error.
  - Introduce a missing `template` or empty `aliases` to assert build fails with proper error.
  - Add/modify a YAML and verify build re-runs (observe timestamps or use `--message-format=json` to see rerun message).
- Optional compile-fail tests (future):
  - Use `trybuild` to verify that `prompt!("unknown.alias")` fails the compilation with expected message.

—

11) Migration Plan (Step-by-Step for an Agent)
1. Create directory `prompts/` and add `prompts/core/schema.yaml` with two entries (`core.schema.only_json`, `core.semantic.stream`) derived from `core.rs` (remove code fences and match current intent).
2. Add `build.rs` as per blueprint; wire up scanning, parsing, validation, and codegen into `OUT_DIR/prompts_gen.rs`.
3. Add `src/prompts/mod.rs` with `include!(concat!(env!("OUT_DIR"), "/prompts_gen.rs"));`.
4. In `src/lib.rs`, add `pub mod prompts;`.
5. Add `src/util/template.rs` with `fill_template` helper. Re-export it if needed (e.g., `pub mod util;`).
6. Modify `src/core.rs`:
   - In `augment_prompt_with_schema<T>`, fetch `tpl = prompt!("core.schema.only_json")` and apply `fill_template` with `{user_prompt}` and `{schema_json}`.
   - In `augment_prompt_with_semantic_schema<T>`, fetch `tpl = prompt!("core.semantic.stream")` similarly.
7. Build locally. If generation fails, fix YAML or build scripts per error.
8. Run unit tests (`cargo test`); fix any string differences if tests assert prompt structure.
9. Update README with a new “Prompt Modules” section: authoring, aliasing, compile-time checks, and examples.
10. Commit with message referencing this blueprint.

—

12) Acceptance Criteria (Verifiable)
- Code references to known aliases compile and evaluate to `&'static str` constants — verified by inspecting expanded macros (`cargo expand`) or simply type-check success.
- Using a non-existent alias in `prompt!("...")` causes a compile error, not a runtime failure.
- Duplicate aliases across YAML files fail the build with a clear message containing both file paths.
- `augment_prompt_*` use the YAML-backed templates and produce the same behavior as before (minus code fences) when exercised in examples/tests.
- Changing a YAML prompt causes `cargo` to rebuild and update the generated constants.

—

13) Future Enhancements (Not in v1)
- Placeholder validation: Static analysis to warn when a template uses a `{key}` not supplied by code paths.
- Rich templating: Optional handlebars/minijinja for conditionals/loops; would reintroduce runtime parsing, so consider codegen-based expansion if ever needed.
- Feature flags per provider to include/exclude prompt namespaces.
- Prompt documentation export: Generate a markdown index from YAML metadata for docs sites.

—

14) Gotchas & Pitfalls Checklist
- Ensure macro paths use `$crate::prompts::...` so they work from downstream crates.
- Keep generation idempotent and stable: sorted inputs, stable sanitation.
- Be careful with raw string delimiters; if using dynamic `#` count, test against edge-case templates containing `"#` sequences.
- Avoid including file paths or environment data in generated constants to keep reproducible builds.
- Sanitize module and const names to valid Rust identifiers; resolve collisions with hashed suffixes when necessary.

—

15) Concrete Examples (for Bot Implementation)
- Calling from code:
```
use crate::prompts; // re-exported from lib.rs

let tpl = prompt!("core.schema.only_json");
let final_prompt = crate::util::template::fill_template(
    tpl.to_string(),
    &[("user_prompt", &prompt), ("schema_json", &schema_json)],
);
```
- Direct constant path (no macro):
```
let tpl = crate::prompts::core::schema::ONLY_JSON;
```
- Compile error (intentional):
```
let _ = prompt!("core.schema.not_there"); // error: Unknown prompt alias: core.schema.not_there
```

This blueprint contains all decisions, interfaces, and step-by-step tasks for a coding agent to implement the compile-time prompt system without further clarification.
