//! Just enough JSON for `--script` (reading) and `--tree` (writing), without a dependency.

use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn num(&self) -> Option<f64> {
        match self {
            Self::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// `[x, y]`.
    pub fn point(&self) -> Option<(f32, f32)> {
        match self {
            Self::Arr(v) if v.len() == 2 => Some((v[0].num()? as f32, v[1].num()? as f32)),
            _ => None,
        }
    }
}

pub(crate) fn parse(text: &str) -> Result<Json> {
    let mut p = Parser {
        bytes: text.as_bytes(),
        at: 0,
    };
    let value = p.value()?;
    p.space();
    if p.at != p.bytes.len() {
        bail!("trailing characters at byte {}", p.at);
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn space(&mut self) {
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    fn eat(&mut self, byte: u8) -> bool {
        self.space();
        let hit = self.bytes.get(self.at) == Some(&byte);
        self.at += usize::from(hit);
        hit
    }

    fn word(&mut self, word: &str, value: Json) -> Result<Json> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            bail!("unexpected input at byte {}", self.at)
        }
    }

    fn value(&mut self) -> Result<Json> {
        self.space();
        match self.bytes.get(self.at) {
            Some(b'{') => {
                self.at += 1;
                let mut fields = Vec::new();
                if !self.eat(b'}') {
                    loop {
                        self.space();
                        let Json::Str(key) = self.string()? else { unreachable!() };
                        if !self.eat(b':') {
                            bail!("expected ':' at byte {}", self.at);
                        }
                        fields.push((key, self.value()?));
                        if self.eat(b'}') {
                            break;
                        }
                        if !self.eat(b',') {
                            bail!("expected ',' or '}}' at byte {}", self.at);
                        }
                    }
                }
                Ok(Json::Obj(fields))
            }
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                if !self.eat(b']') {
                    loop {
                        items.push(self.value()?);
                        if self.eat(b']') {
                            break;
                        }
                        if !self.eat(b',') {
                            bail!("expected ',' or ']' at byte {}", self.at);
                        }
                    }
                }
                Ok(Json::Arr(items))
            }
            Some(b'"') => self.string(),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'n') => self.word("null", Json::Null),
            Some(_) => {
                let start = self.at;
                while self
                    .bytes
                    .get(self.at)
                    .is_some_and(|b| b.is_ascii_digit() || b"+-.eE".contains(b))
                {
                    self.at += 1;
                }
                let number = std::str::from_utf8(&self.bytes[start..self.at])?;
                match number.parse() {
                    Ok(n) => Ok(Json::Num(n)),
                    Err(_) => bail!("bad value at byte {start}"),
                }
            }
            None => bail!("unexpected end"),
        }
    }

    fn string(&mut self) -> Result<Json> {
        if self.bytes.get(self.at) != Some(&b'"') {
            bail!("expected a string at byte {}", self.at);
        }
        self.at += 1;
        let mut out = Vec::new();
        loop {
            match self.bytes.get(self.at) {
                None => bail!("unterminated string"),
                Some(b'"') => break,
                Some(b'\\') => {
                    let escaped = self.bytes.get(self.at + 1).copied();
                    self.at += 2;
                    match escaped {
                        Some(b'n') => out.push(b'\n'),
                        Some(b't') => out.push(b'\t'),
                        Some(b'r') => out.push(b'\r'),
                        Some(b'u') => {
                            let hex = self.bytes.get(self.at..self.at + 4).unwrap_or_default();
                            let code = u32::from_str_radix(std::str::from_utf8(hex)?, 16)?;
                            let c = char::from_u32(code).unwrap_or('\u{FFFD}');
                            out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                            self.at += 4;
                        }
                        Some(other) => out.push(other),
                        None => bail!("unterminated string"),
                    }
                    continue;
                }
                Some(&b) => out.push(b),
            }
            self.at += 1;
        }
        self.at += 1;
        Ok(Json::Str(String::from_utf8(out)?))
    }
}

/// `s` as a JSON string literal.
pub(crate) fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_script_and_round_trips_strings() {
        let v = parse(r#" [ {"at": 0.5, "down": [10, -2.5e1]}, {"key": "Shift+Tab", "x": null, "ok": true} ] "#)
            .unwrap();
        let Json::Arr(items) = &v else { panic!() };
        assert_eq!(items[0].get("at").and_then(Json::num), Some(0.5));
        assert_eq!(items[0].get("down").and_then(Json::point), Some((10.0, -25.0)));
        assert_eq!(items[1].get("key").and_then(Json::str), Some("Shift+Tab"));
        let tricky = "a \"b\" \\ \n é";
        assert_eq!(parse(&quote(tricky)).unwrap(), Json::Str(tricky.into()));
        assert!(parse("[1, 2").is_err());
    }
}
