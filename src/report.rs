//! Output formats: human text, JSON, and SARIF 2.1.0 (for GitHub code scanning).

use crate::lint::{Diagnostic, Severity, RULES};
use serde_json::json;

pub fn text(diags: &[Diagnostic]) -> String {
    let mut s = String::new();
    for d in diags {
        let sev = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        };
        s.push_str(&format!("{}:{}:{}: {sev}[{}]: {}\n", d.file.display(), d.line, d.column, d.rule, d.message));
    }
    let errors = diags.iter().filter(|d| d.severity == Severity::Error).count();
    let warnings = diags.iter().filter(|d| d.severity == Severity::Warning).count();
    s.push_str(&format!("{errors} error(s), {warnings} warning(s)\n"));
    s
}

pub fn json(diags: &[Diagnostic]) -> String {
    serde_json::to_string_pretty(diags).unwrap() + "\n"
}

pub fn sarif(diags: &[Diagnostic]) -> String {
    let rules: Vec<_> = RULES
        .iter()
        .map(|(id, sev, desc)| {
            json!({
                "id": id,
                "shortDescription": { "text": desc },
                "defaultConfiguration": { "level": level(*sev) },
            })
        })
        .collect();
    let results: Vec<_> = diags
        .iter()
        .map(|d| {
            json!({
                "ruleId": d.rule,
                "ruleIndex": RULES.iter().position(|r| r.0 == d.rule).unwrap_or(0),
                "level": level(d.severity),
                "message": { "text": d.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": d.file.to_string_lossy().replace('\\', "/") },
                        "region": { "startLine": d.line, "startColumn": d.column }
                    }
                }]
            })
        })
        .collect();
    let doc = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "arb-lint",
                "version": env!("CARGO_PKG_VERSION"),
                "rules": rules
            }},
            "results": results
        }]
    });
    serde_json::to_string_pretty(&doc).unwrap() + "\n"
}

fn level(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}
