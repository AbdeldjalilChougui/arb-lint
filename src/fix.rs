//! `--fix`: canonical formatting. Keys follow the template's order, each
//! `@key` sits right after its message, `@@` attributes come first,
//! `@@locale` is added when missing, 2-space indentation and a final newline.
//! Values are never changed and nothing is added or removed besides `@@locale`.

use crate::arb::ArbFile;
use serde_json::{Map, Value};

pub fn format(file: &ArbFile, template_order: &[String]) -> String {
    let src = &file.map;
    let mut out = Map::new();
    if !src.contains_key("@@locale") && !file.locale.is_empty() {
        out.insert("@@locale".into(), Value::String(file.locale.clone()));
    }
    for (k, v) in src.iter().filter(|(k, _)| k.starts_with("@@")) {
        out.insert(k.clone(), v.clone());
    }
    let push = |out: &mut Map<String, Value>, key: &str| {
        if let Some(v) = src.get(key) {
            if !out.contains_key(key) {
                out.insert(key.to_string(), v.clone());
            }
        }
        let meta = format!("@{key}");
        if let Some(v) = src.get(&meta) {
            if !out.contains_key(&meta) {
                out.insert(meta, v.clone());
            }
        }
    };
    for key in template_order {
        if src.contains_key(key) {
            push(&mut out, key);
        }
    }
    // Keys not in the template keep their relative order, then leftovers (orphan metadata).
    for (k, _) in src.iter().filter(|(k, _)| !k.starts_with('@')) {
        push(&mut out, k);
    }
    for (k, v) in src.iter() {
        if !out.contains_key(k) {
            out.insert(k.clone(), v.clone());
        }
    }
    let mut s = serde_json::to_string_pretty(&Value::Object(out)).expect("serializable");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn reorders_and_adds_locale() {
        let text = r#"{"b": "B", "@a": {"description": "x"}, "z": "extra", "a": "A"}"#.to_string();
        let f = ArbFile::parse(Path::new("app_fr.arb"), text).unwrap();
        let out = format(&f, &["a".into(), "b".into()]);
        assert_eq!(
            out,
            "{\n  \"@@locale\": \"fr\",\n  \"a\": \"A\",\n  \"@a\": {\n    \"description\": \"x\"\n  },\n  \"b\": \"B\",\n  \"z\": \"extra\"\n}\n"
        );
        // Formatting is idempotent.
        let again = ArbFile::parse(Path::new("app_fr.arb"), out.clone()).unwrap();
        assert_eq!(format(&again, &["a".into(), "b".into()]), out);
    }
}
