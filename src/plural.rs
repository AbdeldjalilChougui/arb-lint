//! CLDR cardinal plural categories (besides `other`) for common locales.
//! Source: Unicode CLDR plural rules. Unknown locales are not checked.

pub fn categories(locale: &str) -> Option<&'static [&'static str]> {
    let lang = locale.split(['_', '-']).next().unwrap_or(locale).to_ascii_lowercase();
    Some(match lang.as_str() {
        "ar" => &["zero", "one", "two", "few", "many", "other"],
        "cy" => &["zero", "one", "two", "few", "many", "other"],
        "ga" => &["one", "two", "few", "many", "other"],
        "he" | "iw" => &["one", "two", "other"],
        "ru" | "uk" | "be" => &["one", "few", "many", "other"],
        "pl" => &["one", "few", "many", "other"],
        "cs" | "sk" => &["one", "few", "many", "other"],
        "lt" => &["one", "few", "many", "other"],
        "ro" => &["one", "few", "other"],
        "hr" | "sr" | "bs" => &["one", "few", "other"],
        "sl" => &["one", "two", "few", "other"],
        "fr" | "es" | "it" | "pt" | "ca" => &["one", "many", "other"],
        "en" | "de" | "nl" | "sv" | "da" | "nb" | "no" | "fi" | "et" | "el" | "hu" | "tr" | "bg" | "fa" | "ur" | "hi"
        | "bn" | "sw" | "kab" | "ber" => &["one", "other"],
        "ja" | "zh" | "ko" | "vi" | "th" | "id" | "ms" => &["other"],
        _ => return None,
    })
}

/// Categories a translation should provide, ignoring `many` for the Romance
/// languages (CLDR uses it only for numbers like 1 000 000, and most apps omit it).
pub fn required(locale: &str) -> Option<Vec<&'static str>> {
    let lang = locale.split(['_', '-']).next().unwrap_or(locale).to_ascii_lowercase();
    let cats = categories(locale)?;
    let romance = matches!(lang.as_str(), "fr" | "es" | "it" | "pt" | "ca");
    Some(cats.iter().copied().filter(|c| !(romance && *c == "many")).collect())
}

/// Right-to-left locales.
pub fn is_rtl(locale: &str) -> bool {
    let lang = locale.split(['_', '-']).next().unwrap_or(locale).to_ascii_lowercase();
    matches!(lang.as_str(), "ar" | "fa" | "he" | "iw" | "ur" | "ps" | "sd" | "ug" | "yi" | "dv" | "ckb")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookups() {
        assert_eq!(categories("ar_DZ").unwrap().len(), 6);
        assert_eq!(required("fr").unwrap(), vec!["one", "other"]);
        assert_eq!(required("en-US").unwrap(), vec!["one", "other"]);
        assert!(categories("xx").is_none());
        assert!(is_rtl("ar") && is_rtl("fa_IR") && !is_rtl("fr"));
    }
}
