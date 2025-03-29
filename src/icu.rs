//! A small parser for the ICU MessageFormat subset Flutter's gen-l10n
//! accepts: `{name}`, `{n, plural, =0{..} one{..} other{..}}`,
//! `{g, select, male{..} other{..}}`, and `#` inside plural cases.
//! Apostrophes are literal, as in gen-l10n's default (`use-escaping: false`).

use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Text(String),
    Arg(String),
    Plural { name: String, cases: Vec<(String, Vec<Node>)> },
    Select { name: String, cases: Vec<(String, Vec<Node>)> },
    /// `#` inside a plural case.
    Hash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcuError {
    /// Char offset in the message.
    pub offset: usize,
    pub msg: String,
}

pub const PLURAL_KEYWORDS: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

struct P<'a> {
    s: &'a [char],
    i: usize,
}

pub fn parse(message: &str) -> Result<Vec<Node>, IcuError> {
    let chars: Vec<char> = message.chars().collect();
    let mut p = P { s: &chars, i: 0 };
    let nodes = p.nodes(false, 0)?;
    if p.i < chars.len() {
        return Err(IcuError { offset: p.i, msg: "unmatched '}'".into() });
    }
    Ok(nodes)
}

impl P<'_> {
    fn err<T>(&self, msg: impl Into<String>) -> Result<T, IcuError> {
        Err(IcuError { offset: self.i, msg: msg.into() })
    }

    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }

    fn ident(&mut self) -> String {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '=' || c == '-') {
            self.i += 1;
        }
        self.s[start..self.i].iter().collect()
    }

    /// Parse until an unmatched `}` (not consumed) or the end.
    fn nodes(&mut self, in_plural: bool, depth: usize) -> Result<Vec<Node>, IcuError> {
        if depth > 20 {
            return self.err("message nesting too deep");
        }
        let mut out = Vec::new();
        let mut text = String::new();
        while let Some(c) = self.peek() {
            match c {
                '}' => break,
                '{' => {
                    if !text.is_empty() {
                        out.push(Node::Text(std::mem::take(&mut text)));
                    }
                    out.push(self.argument(depth)?);
                }
                '#' if in_plural => {
                    if !text.is_empty() {
                        out.push(Node::Text(std::mem::take(&mut text)));
                    }
                    out.push(Node::Hash);
                    self.i += 1;
                }
                _ => {
                    text.push(c);
                    self.i += 1;
                }
            }
        }
        if !text.is_empty() {
            out.push(Node::Text(text));
        }
        Ok(out)
    }

    fn argument(&mut self, depth: usize) -> Result<Node, IcuError> {
        let open = self.i;
        self.i += 1; // '{'
        self.skip_ws();
        let name = self.ident();
        if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) && !name.chars().all(|c| c.is_ascii_digit()) {
            return self.err("expected a placeholder name after '{'");
        }
        self.skip_ws();
        match self.peek() {
            Some('}') => {
                self.i += 1;
                Ok(Node::Arg(name))
            }
            Some(',') => {
                self.i += 1;
                self.skip_ws();
                let kind = self.ident();
                self.skip_ws();
                if self.peek() != Some(',') {
                    return self.err(format!("expected ',' after '{kind}'"));
                }
                self.i += 1;
                match kind.as_str() {
                    "plural" | "select" => {
                        let plural = kind == "plural";
                        let cases = self.cases(plural, depth)?;
                        if !cases.iter().any(|(k, _)| k == "other") {
                            return Err(IcuError { offset: open, msg: format!("{kind} for '{name}' has no 'other' case") });
                        }
                        Ok(if plural { Node::Plural { name, cases } } else { Node::Select { name, cases } })
                    }
                    "" => self.err("expected 'plural' or 'select'"),
                    k => self.err(format!("unsupported argument type '{k}' (expected plural or select)")),
                }
            }
            None => Err(IcuError { offset: open, msg: "unclosed '{'".into() }),
            Some(c) => self.err(format!("unexpected '{c}' in placeholder")),
        }
    }

    fn cases(&mut self, plural: bool, depth: usize) -> Result<Vec<(String, Vec<Node>)>, IcuError> {
        let mut cases: Vec<(String, Vec<Node>)> = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some('}') => {
                    self.i += 1;
                    return Ok(cases);
                }
                None => return self.err("unclosed plural/select"),
                _ => {}
            }
            let key = self.ident();
            if key.is_empty() {
                return self.err("expected a case keyword");
            }
            if plural {
                let explicit = key.strip_prefix('=').is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
                if !explicit && !PLURAL_KEYWORDS.contains(&key.as_str()) {
                    return self.err(format!("invalid plural category '{key}' (use zero, one, two, few, many, other or =N)"));
                }
            } else if key.starts_with('=') {
                return self.err(format!("invalid select case '{key}'"));
            }
            if cases.iter().any(|(k, _)| *k == key) {
                return self.err(format!("duplicate case '{key}'"));
            }
            self.skip_ws();
            if self.peek() != Some('{') {
                return self.err(format!("expected '{{' after case '{key}'"));
            }
            self.i += 1;
            let body = self.nodes(plural, depth + 1)?;
            if self.peek() != Some('}') {
                return self.err(format!("unclosed case '{key}'"));
            }
            self.i += 1;
            cases.push((key, body));
        }
    }
}

/// All placeholder names a message refers to (arguments, plural and select variables).
pub fn placeholders(nodes: &[Node]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    collect(nodes, &mut out);
    out
}

fn collect(nodes: &[Node], out: &mut BTreeSet<String>) {
    for n in nodes {
        match n {
            Node::Arg(a) => {
                out.insert(a.clone());
            }
            Node::Plural { name, cases } | Node::Select { name, cases } => {
                out.insert(name.clone());
                for (_, body) in cases {
                    collect(body, out);
                }
            }
            Node::Text(_) | Node::Hash => {}
        }
    }
}

/// Plural cases used anywhere in the message, per variable.
pub fn plural_cases(nodes: &[Node]) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    fn walk(nodes: &[Node], out: &mut Vec<(String, Vec<String>)>) {
        for n in nodes {
            match n {
                Node::Plural { name, cases } => {
                    out.push((name.clone(), cases.iter().map(|(k, _)| k.clone()).collect()));
                    for (_, b) in cases {
                        walk(b, out);
                    }
                }
                Node::Select { cases, .. } => {
                    for (_, b) in cases {
                        walk(b, out);
                    }
                }
                _ => {}
            }
        }
    }
    walk(nodes, &mut out);
    out
}

/// Concatenated literal text (used for RTL heuristics).
pub fn text_content(nodes: &[Node]) -> String {
    let mut s = String::new();
    fn walk(nodes: &[Node], s: &mut String) {
        for n in nodes {
            match n {
                Node::Text(t) => s.push_str(t),
                Node::Plural { cases, .. } | Node::Select { cases, .. } => {
                    for (_, b) in cases {
                        walk(b, s);
                        s.push(' ');
                    }
                }
                _ => {}
            }
        }
    }
    walk(nodes, &mut s);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_and_nested() {
        let n = parse("Hello {name}!").unwrap();
        assert_eq!(n, vec![Node::Text("Hello ".into()), Node::Arg("name".into()), Node::Text("!".into())]);
        let n = parse("{count, plural, =0{No items} one{One item for {user}} other{# items}}").unwrap();
        assert_eq!(placeholders(&n).into_iter().collect::<Vec<_>>(), vec!["count", "user"]);
        assert_eq!(plural_cases(&n), vec![("count".to_string(), vec!["=0".to_string(), "one".into(), "other".into()])]);
        let n = parse("{g, select, male{He} female{She} other{They}} has {n, plural, one{# cat} other{# cats}}").unwrap();
        assert_eq!(placeholders(&n).len(), 2);
    }

    #[test]
    fn literal_hash_and_apostrophe_outside_plural() {
        let n = parse("Item #1 isn't {x}").unwrap();
        assert_eq!(n[0], Node::Text("Item #1 isn't ".into()));
    }

    #[test]
    fn errors() {
        let bad = [
            ("Hello {name", "unclosed"),
            ("Hello }", "unmatched"),
            ("{}", "placeholder name"),
            ("{n, plural, one{x}}", "no 'other'"),
            ("{n, plural, single{x} other{y}}", "invalid plural category"),
            ("{n, plural, one{x} one{y} other{z}}", "duplicate"),
            ("{n, plural, one x other{y}}", "expected '{'"),
            ("{n, number}", "expected ','"),
            ("{n, date, short}", "unsupported"),
            ("{g, select, male{x} other{y}", "unclosed"),
        ];
        for (msg, want) in bad {
            let e = parse(msg).unwrap_err();
            assert!(e.msg.contains(want), "{msg}: {}", e.msg);
        }
    }
}
