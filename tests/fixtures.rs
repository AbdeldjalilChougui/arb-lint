use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_arb-lint")).args(args).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

#[test]
fn good_project_is_clean() {
    let (code, out, err) = run(&[fixture("good").to_str().unwrap(), "--require-descriptions", "--deny-warnings"]);
    assert_eq!(out, "0 error(s), 0 warning(s)\n", "{err}");
    assert_eq!(code, 0);
}

/// (file, rule, key) triples reported for the bad fixture.
fn bad_findings() -> Vec<(String, String, Option<String>)> {
    let (code, out, _) = run(&[fixture("bad").to_str().unwrap(), "--format", "json"]);
    assert_eq!(code, 1, "errors must fail the run");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    v.as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let file = Path::new(d["file"].as_str().unwrap()).file_name().unwrap().to_string_lossy().into_owned();
            (file, d["rule"].as_str().unwrap().to_string(), d["key"].as_str().map(String::from))
        })
        .collect()
}

#[test]
fn bad_project_reports_each_problem() {
    let got = bad_findings();
    let has = |file: &str, rule: &str, key: Option<&str>| {
        assert!(
            got.iter().any(|(f, r, k)| f == file && r == rule && k.as_deref() == key),
            "missing {file} {rule} {key:?}\nall: {got:#?}"
        );
    };
    has("app_es.arb", "invalid-json", None);
    has("app_en.arb", "duplicate-key", Some("title"));
    has("app_en.arb", "orphan-metadata", Some("@ghost"));
    has("app_en.arb", "undeclared-placeholder", Some("welcome"));
    has("app_en.arb", "unused-placeholder", Some("welcome"));
    has("app_en.arb", "icu-syntax", Some("broken"));
    has("app_en.arb", "icu-syntax", Some("noOther"));
    has("app_en.arb", "unused-key", Some("neverUsed"));
    has("app_ar.arb", "rtl-untranslated", Some("title"));
    has("app_ar.arb", "unknown-placeholder", Some("welcome"));
    has("app_ar.arb", "missing-placeholder", Some("welcome"));
    has("app_ar.arb", "plural-missing-category", Some("cartItems"));
    has("app_ar.arb", "rtl-bidi-controls", Some("neverUsed"));
    has("app_ar.arb", "extra-key", Some("onlyHere"));
    has("app_ar.arb", "missing-translation", None);
    has("app_fr.arb", "locale-mismatch", Some("@@locale"));
    has("app_fr.arb", "empty-message", Some("title"));
    // Keys used in Dart are not reported as unused.
    assert!(!got.iter().any(|(_, r, k)| r == "unused-key" && k.as_deref() == Some("title")));
    // French needs only one/other: no plural warning there.
    assert!(!got.iter().any(|(f, r, _)| f == "app_fr.arb" && r == "plural-missing-category"));
}

#[test]
fn text_output_has_positions() {
    let (_, out, _) = run(&[fixture("bad").to_str().unwrap()]);
    let line = out.lines().find(|l| l.contains("orphan-metadata")).unwrap();
    assert!(line.contains("app_en.arb:16:3: warning[orphan-metadata]"), "{line}");
}

#[test]
fn sarif_output() {
    let (_, out, _) = run(&[fixture("bad").to_str().unwrap(), "--format", "sarif", "--no-unused"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["version"], "2.1.0");
    let run = &v["runs"][0];
    assert_eq!(run["tool"]["driver"]["name"], "arb-lint");
    let results = run["results"].as_array().unwrap();
    assert!(results.iter().any(|r| r["ruleId"] == "icu-syntax" && r["level"] == "error"));
    assert!(results.iter().all(|r| r["ruleId"] != "unused-key"));
    let loc = &results[0]["locations"][0]["physicalLocation"];
    assert!(loc["region"]["startLine"].as_u64().unwrap() >= 1);
}

#[test]
fn fix_formats_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("app_en.arb"),
        r#"{"b": "B {x}", "a": "A", "@b": {"placeholders": {"x": {}}}}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("app_fr.arb"), r#"{"b": "B fr {x}", "zz": "extra", "a": "A fr"}"#).unwrap();
    let (_, _, err) = run(&[dir.path().to_str().unwrap(), "--fix"]);
    assert!(err.contains("formatted 2 file(s)"), "{err}");
    let en = std::fs::read_to_string(dir.path().join("app_en.arb")).unwrap();
    assert_eq!(
        en,
        "{\n  \"@@locale\": \"en\",\n  \"b\": \"B {x}\",\n  \"@b\": {\n    \"placeholders\": {\n      \"x\": {}\n    }\n  },\n  \"a\": \"A\"\n}\n"
    );
    let fr = std::fs::read_to_string(dir.path().join("app_fr.arb")).unwrap();
    assert_eq!(fr, "{\n  \"@@locale\": \"fr\",\n  \"b\": \"B fr {x}\",\n  \"a\": \"A fr\",\n  \"zz\": \"extra\"\n}\n");
    // Second run changes nothing.
    let (_, _, err) = run(&[dir.path().to_str().unwrap(), "--fix"]);
    assert!(err.contains("formatted 0 file(s)"));
}
