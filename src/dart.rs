//! Find which localization getters the Dart code uses.
//!
//! Heuristic: collect every identifier that follows a `.` in `.dart` files
//! (e.g. `AppLocalizations.of(context)!.helloWorld`, `l10n.helloWorld`),
//! skipping generated files. A template key that never appears is reported.

use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

fn is_generated(name: &str) -> bool {
    name.ends_with(".g.dart")
        || name.ends_with(".freezed.dart")
        || (name.starts_with("app_localizations") && name.ends_with(".dart"))
        || name.starts_with("l10n") && name.ends_with(".dart") && name.contains('_')
}

fn walk(dir: &Path, re: &Regex, out: &mut HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !name.starts_with('.') && name != "build" {
                walk(&path, re, out);
            }
        } else if name.ends_with(".dart") && !is_generated(&name) {
            if let Ok(src) = std::fs::read_to_string(&path) {
                for c in re.captures_iter(&src) {
                    out.insert(c[1].to_string());
                }
            }
        }
    }
}

pub fn member_identifiers(root: &Path) -> HashSet<String> {
    let re = Regex::new(r"\.\s*([A-Za-z_$][A-Za-z0-9_$]*)").unwrap();
    let mut out = HashSet::new();
    walk(root, &re, &mut out);
    out
}
