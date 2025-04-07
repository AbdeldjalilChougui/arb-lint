//! arb-lint: lint and format Flutter ARB localization files.

pub mod arb;
pub mod dart;
pub mod fix;
pub mod icu;
pub mod lint;
pub mod plural;
pub mod report;

use std::path::{Path, PathBuf};

/// Where the ARB files, the template and the Dart sources are.
#[derive(Debug, Clone)]
pub struct Layout {
    pub arb_dir: PathBuf,
    pub template: Option<String>,
    pub dart_root: Option<PathBuf>,
}

/// Minimal reader for the two `l10n.yaml` keys we need (no YAML dependency).
pub fn read_l10n_yaml(text: &str) -> (Option<String>, Option<String>) {
    let mut arb_dir = None;
    let mut template = None;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some((k, v)) = line.split_once(':') {
            let v = v.trim().trim_matches(['"', '\'']).to_string();
            match k.trim() {
                "arb-dir" => arb_dir = Some(v),
                "template-arb-file" => template = Some(v),
                _ => {}
            }
        }
    }
    (arb_dir, template)
}

/// Resolve a path given on the command line: a Flutter project root (with
/// `l10n.yaml` or `lib/l10n`), or a directory of `.arb` files.
pub fn discover(path: &Path) -> Layout {
    let yaml = path.join("l10n.yaml");
    if let Ok(text) = std::fs::read_to_string(&yaml) {
        let (dir, template) = read_l10n_yaml(&text);
        return Layout {
            arb_dir: path.join(dir.unwrap_or_else(|| "lib/l10n".into())),
            template: Some(template.unwrap_or_else(|| "app_en.arb".into())),
            dart_root: path.join("lib").is_dir().then(|| path.join("lib")),
        };
    }
    if path.join("lib/l10n").is_dir() {
        return Layout { arb_dir: path.join("lib/l10n"), template: None, dart_root: Some(path.join("lib")) };
    }
    Layout { arb_dir: path.to_path_buf(), template: None, dart_root: None }
}

pub fn arb_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "arb"))
        .collect();
    v.sort();
    Ok(v)
}

/// Pick the template: the configured name, else `app_en.arb`, else any
/// `*_en.arb`, else the file with the most `@key` metadata entries.
pub fn pick_template(files: &[arb::ArbFile], configured: Option<&str>) -> Option<usize> {
    let by_name = |name: &str| files.iter().position(|f| f.path.file_name().is_some_and(|n| n == name));
    if let Some(c) = configured {
        return by_name(c);
    }
    by_name("app_en.arb")
        .or_else(|| files.iter().position(|f| f.locale == "en"))
        .or_else(|| (0..files.len()).max_by_key(|&i| files[i].map.keys().filter(|k| k.starts_with('@') && !k.starts_with("@@")).count()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l10n_yaml() {
        let (d, t) = read_l10n_yaml("arb-dir: lib/i18n # comment\ntemplate-arb-file: 'intl_fr.arb'\noutput-localization-file: x.dart\n");
        assert_eq!(d.as_deref(), Some("lib/i18n"));
        assert_eq!(t.as_deref(), Some("intl_fr.arb"));
    }
}
