//! The checks. Each produces [`Diagnostic`]s with a stable rule id.

use crate::arb::{locale_from_filename, ArbFile};
use crate::{dart, icu, plural};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Note,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub rule: &'static str,
    pub severity: Severity,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub key: Option<String>,
    pub message: String,
}

/// Every rule: id, default severity, short description (also used for SARIF).
pub const RULES: &[(&str, Severity, &str)] = &[
    ("invalid-json", Severity::Error, "The file is not a valid JSON object"),
    ("duplicate-key", Severity::Error, "A key appears more than once in the same file"),
    ("invalid-message", Severity::Error, "A message value is not a string"),
    ("invalid-metadata", Severity::Error, "An @key metadata entry is malformed"),
    ("icu-syntax", Severity::Error, "ICU MessageFormat syntax error (braces, plural/select, cases)"),
    ("undeclared-placeholder", Severity::Error, "The template uses a placeholder missing from @key.placeholders"),
    ("unused-placeholder", Severity::Warning, "A declared placeholder is not used by the template message"),
    ("unknown-placeholder", Severity::Error, "A translation uses a placeholder the template does not have"),
    ("missing-placeholder", Severity::Warning, "A translation never uses one of the template's placeholders"),
    ("missing-translation", Severity::Warning, "A template key is missing from a translation"),
    ("extra-key", Severity::Warning, "A translation has a key that is not in the template"),
    ("orphan-metadata", Severity::Warning, "An @key entry has no matching message"),
    ("missing-description", Severity::Warning, "A template message has no @key.description"),
    ("plural-missing-category", Severity::Warning, "A plural lacks a CLDR category required by the locale"),
    ("locale-mismatch", Severity::Warning, "@@locale does not match the file name"),
    ("empty-message", Severity::Warning, "A translation is empty while the template is not"),
    ("unused-key", Severity::Warning, "A template key is never referenced in the Dart code"),
    ("rtl-untranslated", Severity::Warning, "An RTL-locale message contains Latin text and no RTL letters"),
    ("rtl-bidi-controls", Severity::Warning, "Unbalanced Unicode bidi embedding/isolate controls"),
];

pub fn default_severity(rule: &str) -> Severity {
    RULES.iter().find(|(id, ..)| *id == rule).map_or(Severity::Warning, |r| r.1)
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub require_descriptions: bool,
    /// Scan this directory's `.dart` files for unused keys.
    pub dart_root: Option<PathBuf>,
}

pub struct Project {
    pub files: Vec<ArbFile>,
    pub template: usize,
}

struct Sink<'a> {
    out: &'a mut Vec<Diagnostic>,
}

impl Sink<'_> {
    fn at(&mut self, rule: &'static str, f: &ArbFile, key: Option<&str>, message: String) {
        let (line, column) = key.map_or((1, 1), |k| f.position_of(k));
        self.out.push(Diagnostic {
            rule,
            severity: default_severity(rule),
            file: f.path.clone(),
            line,
            column,
            key: key.map(String::from),
            message,
        });
    }
}

/// Keys that appear more than once at the top level of a JSON object.
pub fn duplicate_keys(text: &str) -> Vec<String> {
    use serde::de::{Deserializer, IgnoredAny, MapAccess, Visitor};
    struct Keys(Vec<String>);
    impl<'de> Visitor<'de> for &mut Keys {
        type Value = ();
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a JSON object")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<(), A::Error> {
            while let Some(k) = m.next_key::<String>()? {
                m.next_value::<IgnoredAny>()?;
                self.0.push(k);
            }
            Ok(())
        }
    }
    let mut keys = Keys(Vec::new());
    let mut de = serde_json::Deserializer::from_str(text);
    if de.deserialize_map(&mut keys).is_err() {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    let mut dups = Vec::new();
    for k in keys.0 {
        if !seen.insert(k.clone()) && !dups.contains(&k) {
            dups.push(k);
        }
    }
    dups
}

/// Load `.arb` files; files that fail to parse become diagnostics.
pub fn load(paths: &[PathBuf], diags: &mut Vec<Diagnostic>) -> Vec<ArbFile> {
    let mut files = Vec::new();
    for p in paths {
        let text = match std::fs::read_to_string(p) {
            Ok(t) => t,
            Err(e) => {
                diags.push(Diagnostic {
                    rule: "invalid-json",
                    severity: Severity::Error,
                    file: p.clone(),
                    line: 1,
                    column: 1,
                    key: None,
                    message: format!("cannot read file: {e}"),
                });
                continue;
            }
        };
        match ArbFile::parse(p, text) {
            Ok(f) => files.push(f),
            Err((line, column, msg)) => diags.push(Diagnostic {
                rule: "invalid-json",
                severity: Severity::Error,
                file: p.clone(),
                line,
                column,
                key: None,
                message: msg,
            }),
        }
    }
    files
}

fn parse_message(sink: &mut Sink, f: &ArbFile, key: &str, value: &Value) -> Option<Vec<icu::Node>> {
    let Some(s) = value.as_str() else {
        sink.at("invalid-message", f, Some(key), format!("message '{key}' must be a string"));
        return None;
    };
    match icu::parse(s) {
        Ok(n) => Some(n),
        Err(e) => {
            sink.at("icu-syntax", f, Some(key), format!("'{key}': {} (at character {})", e.msg, e.offset));
            None
        }
    }
}

fn check_plural_categories(sink: &mut Sink, f: &ArbFile, key: &str, nodes: &[icu::Node]) {
    let Some(required) = plural::required(&f.locale) else { return };
    for (var, cases) in icu::plural_cases(nodes) {
        let has = |c: &str| {
            cases.iter().any(|k| k == c)
                || matches!((c, cases.iter().any(|k| k == "=0"), cases.iter().any(|k| k == "=1"), cases.iter().any(|k| k == "=2")),
                    ("zero", true, _, _) | ("one", _, true, _) | ("two", _, _, true))
        };
        let missing: Vec<&str> = required.iter().copied().filter(|c| !has(c)).collect();
        if !missing.is_empty() {
            sink.at(
                "plural-missing-category",
                f,
                Some(key),
                format!("'{key}': plural '{var}' for locale '{}' lacks: {}", f.locale, missing.join(", ")),
            );
        }
    }
}

fn is_rtl_letter(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
}

fn check_rtl(sink: &mut Sink, f: &ArbFile, key: &str, raw: &str, nodes: &[icu::Node]) {
    let text = icu::text_content(nodes);
    let latin = text.chars().any(|c| c.is_ascii_alphabetic());
    let rtl = text.chars().any(is_rtl_letter);
    if latin && !rtl {
        sink.at("rtl-untranslated", f, Some(key), format!("'{key}' in RTL locale '{}' has Latin text but no RTL letters", f.locale));
    }
    let (mut emb, mut iso) = (0i32, 0i32);
    let mut bad = false;
    for c in raw.chars() {
        match c {
            '\u{202A}'..='\u{202B}' | '\u{202D}'..='\u{202E}' => emb += 1,
            '\u{202C}' => emb -= 1,
            '\u{2066}'..='\u{2068}' => iso += 1,
            '\u{2069}' => iso -= 1,
            _ => {}
        }
        bad |= emb < 0 || iso < 0;
    }
    if bad || emb != 0 || iso != 0 {
        sink.at("rtl-bidi-controls", f, Some(key), format!("'{key}' has unbalanced bidi control characters"));
    }
}

/// Run every check over a project.
pub fn lint(project: &Project, opts: &Options, diags: &mut Vec<Diagnostic>) {
    let mut sink = Sink { out: diags };
    let t = &project.files[project.template];

    for f in &project.files {
        for dup in duplicate_keys(&f.text) {
            sink.at("duplicate-key", f, Some(&dup), format!("key '{dup}' is defined more than once (the last one wins)"));
        }
        for (k, v) in f.map.iter().filter(|(k, _)| k.starts_with('@') && !k.starts_with("@@")) {
            let base = &k[1..];
            if !f.map.contains_key(base) {
                sink.at("orphan-metadata", f, Some(k), format!("'{k}' has no message '{base}'"));
            }
            match v {
                Value::Object(m) => {
                    if let Some(p) = m.get("placeholders") {
                        if !p.is_object() {
                            sink.at("invalid-metadata", f, Some(k), format!("'{k}.placeholders' must be an object"));
                        }
                    }
                }
                _ => sink.at("invalid-metadata", f, Some(k), format!("'{k}' must be an object")),
            }
        }
        if let (Some(decl), Some(from_name)) = (&f.declared_locale, locale_from_filename(&f.path)) {
            if decl.replace('-', "_") != from_name {
                sink.at("locale-mismatch", f, Some("@@locale"), format!("@@locale is '{decl}' but the file name says '{from_name}'"));
            }
        }
    }

    // Template.
    let mut template_ph: Vec<(String, BTreeSet<String>)> = Vec::new();
    for (key, value) in t.messages() {
        let Some(nodes) = parse_message(&mut sink, t, key, value) else {
            template_ph.push((key.clone(), BTreeSet::new()));
            continue;
        };
        let used = icu::placeholders(&nodes);
        let declared: BTreeSet<String> = t.declared_placeholders(key).unwrap_or_default().into_iter().collect();
        for p in used.difference(&declared) {
            sink.at("undeclared-placeholder", t, Some(key), format!("'{key}' uses {{{p}}} but @{key}.placeholders does not declare it"));
        }
        for p in declared.difference(&used) {
            sink.at("unused-placeholder", t, Some(key), format!("'{key}' declares placeholder '{p}' but never uses it"));
        }
        if opts.require_descriptions && t.metadata(key).and_then(|m| m.get("description")).and_then(Value::as_str).is_none_or(str::is_empty) {
            sink.at("missing-description", t, Some(key), format!("'{key}' has no @{key}.description"));
        }
        check_plural_categories(&mut sink, t, key, &nodes);
        if plural::is_rtl(&t.locale) {
            check_rtl(&mut sink, t, key, value.as_str().unwrap_or(""), &nodes);
        }
        template_ph.push((key.clone(), used));
    }

    // Translations.
    for (i, f) in project.files.iter().enumerate() {
        if i == project.template {
            continue;
        }
        for (key, used_t) in &template_ph {
            if !f.map.contains_key(key) {
                sink.at("missing-translation", f, None, format!("missing translation for '{key}'"));
                continue;
            }
            let value = &f.map[key];
            let Some(nodes) = parse_message(&mut sink, f, key, value) else { continue };
            let used = icu::placeholders(&nodes);
            for p in used.difference(used_t) {
                sink.at("unknown-placeholder", f, Some(key), format!("'{key}' uses {{{p}}}, which the template does not have"));
            }
            for p in used_t.difference(&used) {
                sink.at("missing-placeholder", f, Some(key), format!("'{key}' never uses {{{p}}} from the template"));
            }
            let raw = value.as_str().unwrap_or("");
            if raw.trim().is_empty() && !t.message(key).unwrap_or("").trim().is_empty() {
                sink.at("empty-message", f, Some(key), format!("'{key}' is empty"));
            }
            check_plural_categories(&mut sink, f, key, &nodes);
            if plural::is_rtl(&f.locale) {
                check_rtl(&mut sink, f, key, raw, &nodes);
            }
        }
        for (key, _) in f.messages() {
            if !t.map.contains_key(key) {
                sink.at("extra-key", f, Some(key), format!("'{key}' is not in the template {}", file_name(&t.path)));
            }
        }
    }

    // Unused keys.
    if let Some(root) = &opts.dart_root {
        let used = dart::member_identifiers(root);
        for (key, _) in t.messages() {
            if !used.contains(key.as_str()) {
                sink.at("unused-key", t, Some(key), format!("'{key}' is not referenced in Dart code under {}", root.display()));
            }
        }
    }

    sink.out.sort_by(|a, b| (&a.file, a.line, a.column, a.rule).cmp(&(&b.file, b.line, b.column, b.rule)));
}

pub fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}
