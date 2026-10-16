//! A deliberately small, non-executing JavaScript data reader. Only literals
//! are accepted; calls, property access, computed keys and functions are errors.
use serde_json::{Map, Value};

pub fn read(source: &str, start: char) -> Result<Value, String> {
    let mut p = Parser { source, at: 0 };
    // Skip wrapper code, but never mistake brackets inside strings/comments
    // for the registry's literal argument.
    loop {
        p.space()?;
        match p.peek() {
            Some(c) if c == start => return p.value(0),
            Some('\'' | '"') => {
                p.string()?;
            }
            Some(_) => {
                p.bump();
            }
            None => return Err("Missing Echo-Live data literal".into()),
        }
    }
}

struct Parser<'a> {
    source: &'a str,
    at: usize,
}
impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.source[self.at..].chars().next()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += c.len_utf8();
        Some(c)
    }
    fn space(&mut self) -> Result<(), String> {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }
            if self.source[self.at..].starts_with("//") {
                while self.bump().is_some_and(|c| c != '\n') {}
            } else if self.source[self.at..].starts_with("/*") {
                let end = self.source[self.at + 2..]
                    .find("*/")
                    .ok_or("Unclosed comment")?;
                self.at += end + 4;
            } else {
                return Ok(());
            }
        }
    }
    fn expect(&mut self, c: char) -> Result<(), String> {
        self.space()?;
        if self.bump() == Some(c) {
            Ok(())
        } else {
            Err(format!("Expected '{c}' at byte {}", self.at))
        }
    }
    fn word(&mut self) -> String {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
        {
            self.bump();
        }
        self.source[start..self.at].into()
    }
    fn string(&mut self) -> Result<String, String> {
        let quote = self.bump().ok_or("Missing string")?;
        let mut out = String::new();
        while let Some(c) = self.bump() {
            if c == quote {
                return Ok(out);
            }
            if c == '\\' {
                let c = self.bump().ok_or("Unclosed escape")?;
                out.push(match c {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    'b' => '\u{8}',
                    'f' => '\u{c}',
                    '\\' | '\'' | '"' | '/' => c,
                    'u' | 'x' => {
                        let n = if c == 'u' { 4 } else { 2 };
                        let mut hex = String::new();
                        for _ in 0..n {
                            hex.push(self.bump().ok_or("Invalid Unicode escape")?);
                        }
                        let first =
                            u32::from_str_radix(&hex, 16).map_err(|_| "Invalid Unicode escape")?;
                        let scalar = if (0xd800..=0xdbff).contains(&first) && c == 'u' {
                            if self.bump() != Some('\\') || self.bump() != Some('u') {
                                return Err("Invalid surrogate pair".into());
                            }
                            let mut low = String::new();
                            for _ in 0..4 {
                                low.push(self.bump().ok_or("Invalid surrogate pair")?);
                            }
                            let low = u32::from_str_radix(&low, 16)
                                .map_err(|_| "Invalid surrogate pair")?;
                            if !(0xdc00..=0xdfff).contains(&low) {
                                return Err("Invalid surrogate pair".into());
                            }
                            0x10000 + ((first - 0xd800) << 10) + low - 0xdc00
                        } else {
                            first
                        };
                        char::from_u32(scalar).ok_or("Invalid Unicode scalar")?
                    }
                    _ => return Err("Unsupported string escape".into()),
                });
            } else if c == '\n' || c == '\r' {
                return Err("Unclosed string".into());
            } else {
                out.push(c);
            }
        }
        Err("Unclosed string".into())
    }
    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > 64 {
            return Err("Echo-Live data nesting exceeds limit".into());
        }
        self.space()?;
        match self.peek().ok_or("Missing value")? {
            '\'' | '"' => Ok(Value::String(self.string()?)),
            '{' | '[' => {
                let object = self.bump() == Some('{');
                let close = if object { '}' } else { ']' };
                let mut map = Map::new();
                let mut list = Vec::new();
                loop {
                    self.space()?;
                    if self.peek() == Some(close) {
                        self.bump();
                        break;
                    }
                    let key = if object {
                        let key = if matches!(self.peek(), Some('\'' | '"')) {
                            self.string()?
                        } else {
                            self.word()
                        };
                        if key.is_empty() {
                            return Err("Only literal object keys are supported".into());
                        }
                        self.expect(':')?;
                        Some(key)
                    } else {
                        None
                    };
                    let v = self.value(depth + 1)?;
                    if let Some(key) = key {
                        map.insert(key, v);
                    } else {
                        list.push(v);
                    }
                    self.space()?;
                    if self.peek() == Some(',') {
                        self.bump();
                    } else if self.peek() != Some(close) {
                        return Err(format!(
                            "Unsupported JavaScript expression at byte {}",
                            self.at
                        ));
                    }
                }
                Ok(if object {
                    Value::Object(map)
                } else {
                    Value::Array(list)
                })
            }
            '-' | '0'..='9' => {
                let start = self.at;
                while self
                    .peek()
                    .is_some_and(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))
                {
                    self.bump();
                }
                serde_json::from_str(&self.source[start..self.at]).map_err(|e| e.to_string())
            }
            _ => match self.word().as_str() {
                "true" => Ok(Value::Bool(true)),
                "false" => Ok(Value::Bool(false)),
                "null" => Ok(Value::Null),
                _ => Err(format!(
                    "Only literal data is supported at byte {}",
                    self.at
                )),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literals_and_wrapper_comments() {
        let v = read("load('emoji', e => e.meta.name, /* [ */ [{name:'羊', x:[1,true,], s:'\\uD83D\\uDE03'},]);", '[').unwrap();
        assert_eq!(v[0]["name"], "羊");
        assert_eq!(v[0]["s"], "😃");
    }
    #[test]
    fn expressions_are_never_executed() {
        for source in [
            "[{x: fetch('bad')}]",
            "[{x:global.secret}]",
            "[{['key']:1}]",
            "[{x:`template`}]",
            "[{x:()=>1}]",
        ] {
            assert!(read(source, '[').is_err(), "{source}");
        }
    }
}
