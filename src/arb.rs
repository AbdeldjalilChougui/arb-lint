//! Loading ARB files (JSON objects with message keys, `@key` metadata and
//! `@@` global attributes), keeping key order.

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ArbFile {
    pub path: PathBuf,
    pub text: String,
    pub locale: String,
    /// `@@locale` if present.
    pub declared_locale: Option<String>,
    pub map: Map<String, Value>,
}

impl ArbFile {
    pub fn parse(path: &Path, text: String) -> Result<ArbFile, (usize, usize, String)> {
        let value: Value = serde_json::from_str(&text).map_err(|e| (e.line(), e.column(), e.to_string()))?;
        let Value::Object(map) = value else {
            return Err((1, 1, "an ARB file must be a JSON object".into()));
        };
        let declared_locale = map.get("@@locale").and_then(Value::as_str).map(String::from);
        let from_name = locale_from_filename(path);
        let locale = declared_locale.clone().or(from_name).unwrap_or_default();
        Ok(ArbFile { path: path.to_path_buf(), text, locale, declared_locale, map })
    }

    /// Message keys (not `@key` / `@@attr`), in file order.
    pub fn messages(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.map.iter().filter(|(k, _)| !k.starts_with('@'))
    }

    pub fn message(&self, key: &str) -> Option<&str> {
        self.map.get(key).and_then(Value::as_str)
    }

    pub fn metadata(&self, key: &str) -> Option<&Map<String, Value>> {
        self.map.get(&format!("@{key}")).and_then(Value::as_object)
    }

    /// Declared placeholders of a message (from `@key.placeholders`).
    pub fn declared_placeholders(&self, key: &str) -> Option<Vec<String>> {
        let p = self.metadata(key)?.get("placeholders")?.as_object()?;
        Some(p.keys().cloned().collect())
    }

    /// 1-based (line, column) of `"key":` in the source, for diagnostics.
    pub fn position_of(&self, key: &str) -> (usize, usize) {
        let needle = format!("{}:", serde_json::to_string(key).unwrap_or_default());
        let compact = self.text.find(&needle);
        let pos = compact.or_else(|| {
            let quoted = serde_json::to_string(key).unwrap_or_default();
            self.text.match_indices(&quoted).find(|(i, _)| self.text[i + quoted.len()..].trim_start().starts_with(':')).map(|(i, _)| i)
        });
        match pos {
            Some(i) => {
                let before = &self.text[..i];
                let line = before.matches('\n').count() + 1;
                let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
                (line, col)
            }
            None => (1, 1),
        }
    }
}

/// `app_en.arb` → `en`, `intl_pt_BR.arb` → `pt_BR`.
pub fn locale_from_filename(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let parts: Vec<&str> = stem.split('_').collect();
    // Last one or two segments: a 2-3 letter language, optionally a region/script.
    let is_lang = |s: &str| (2..=3).contains(&s.len()) && s.chars().all(|c| c.is_ascii_lowercase());
    let is_region = |s: &str| (s.len() == 2 && s.chars().all(|c| c.is_ascii_uppercase())) || (s.len() == 4 && s.chars().next().is_some_and(|c| c.is_ascii_uppercase())) || (s.len() == 3 && s.chars().all(|c| c.is_ascii_digit()));
    match parts.as_slice() {
        [.., l, r] if is_lang(l) && is_region(r) => Some(format!("{l}_{r}")),
        [.., l] if is_lang(l) => Some(l.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_from_names() {
        assert_eq!(locale_from_filename(Path::new("lib/l10n/app_en.arb")).as_deref(), Some("en"));
        assert_eq!(locale_from_filename(Path::new("intl_pt_BR.arb")).as_deref(), Some("pt_BR"));
        assert_eq!(locale_from_filename(Path::new("app_zh_Hant.arb")).as_deref(), Some("zh_Hant"));
        assert_eq!(locale_from_filename(Path::new("strings.arb")), None);
    }

    #[test]
    fn positions() {
        let text = "{\n  \"@@locale\": \"en\",\n  \"hello\": \"Hi\",\n  \"@hello\": {}\n}\n".to_string();
        let f = ArbFile::parse(Path::new("app_en.arb"), text).unwrap();
        assert_eq!(f.position_of("hello"), (3, 3));
        assert_eq!(f.position_of("@hello"), (4, 3));
        assert_eq!(f.messages().count(), 1);
    }
}
