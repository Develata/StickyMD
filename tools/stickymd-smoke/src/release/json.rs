//! Strict, depth-bounded JSON for Cargo metadata and native adapter facts.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::collections::BTreeMap;

#[derive(Debug, PartialEq)]
pub(super) enum Value {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn field(&self, key: &str) -> Result<&Self, String> {
        match self {
            Self::Object(fields) => fields
                .get(key)
                .ok_or_else(|| format!("missing JSON field {key}")),
            _ => Err("expected JSON object".to_owned()),
        }
    }
    pub fn string(&self) -> Result<&str, String> {
        match self {
            Self::String(value) => Ok(value),
            _ => Err("expected JSON string".to_owned()),
        }
    }
    pub fn array(&self) -> Result<&[Self], String> {
        match self {
            Self::Array(value) => Ok(value),
            _ => Err("expected JSON array".to_owned()),
        }
    }
    pub fn optional_string(&self) -> Result<Option<&str>, String> {
        if *self == Self::Null {
            Ok(None)
        } else {
            self.string().map(Some)
        }
    }
    pub fn unsigned(&self) -> Result<u64, String> {
        match self {
            Self::Number(value) => value
                .parse()
                .map_err(|_| "expected unsigned JSON integer".to_owned()),
            _ => Err("expected JSON number".to_owned()),
        }
    }
}

pub(super) fn parse(text: &str) -> Result<Value, String> {
    let mut parser = Parser {
        text: text.trim_start_matches('\u{feff}'),
        offset: 0,
    };
    let value = parser.value(0)?;
    parser.whitespace();
    if parser.offset != parser.text.len() {
        return Err("trailing JSON input".to_owned());
    }
    Ok(value)
}

struct Parser<'a> {
    text: &'a str,
    offset: usize,
}

impl Parser<'_> {
    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\r' | b'\n' | b'\t')) {
            self.offset += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.offset).copied()
    }
    fn consume(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.offset += 1;
            true
        } else {
            false
        }
    }
    fn require(&mut self, byte: u8) -> Result<(), String> {
        if self.consume(byte) {
            Ok(())
        } else {
            Err(format!(
                "expected JSON byte {} at {}",
                char::from(byte),
                self.offset
            ))
        }
    }
    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > 128 {
            return Err("JSON nesting limit exceeded".to_owned());
        }
        self.whitespace();
        match self.peek() {
            Some(b'"') => self.string().map(Value::String),
            Some(b'[') => {
                self.offset += 1;
                self.whitespace();
                let mut values = Vec::new();
                if self.consume(b']') {
                    return Ok(Value::Array(values));
                }
                loop {
                    values.push(self.value(depth + 1)?);
                    self.whitespace();
                    if self.consume(b']') {
                        break;
                    }
                    self.require(b',')?;
                }
                Ok(Value::Array(values))
            }
            Some(b'{') => {
                self.offset += 1;
                self.whitespace();
                let mut fields = BTreeMap::new();
                if self.consume(b'}') {
                    return Ok(Value::Object(fields));
                }
                loop {
                    self.whitespace();
                    let key = self.string()?;
                    self.whitespace();
                    self.require(b':')?;
                    if fields.insert(key.clone(), self.value(depth + 1)?).is_some() {
                        return Err(format!("duplicate JSON key {key}"));
                    }
                    self.whitespace();
                    if self.consume(b'}') {
                        break;
                    }
                    self.require(b',')?;
                }
                Ok(Value::Object(fields))
            }
            Some(b'n') => self.literal("null", Value::Null),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(format!("invalid JSON value at {}", self.offset)),
        }
    }
    fn literal(&mut self, literal: &str, value: Value) -> Result<Value, String> {
        if !self.text[self.offset..].starts_with(literal) {
            return Err("invalid JSON literal".to_owned());
        }
        self.offset += literal.len();
        Ok(value)
    }
    fn number(&mut self) -> Result<Value, String> {
        let start = self.offset;
        self.consume(b'-');
        if !self.consume(b'0') {
            self.digits()?;
        }
        if self.consume(b'.') {
            self.digits()?;
        }
        if self.consume(b'e') || self.consume(b'E') {
            if !self.consume(b'+') {
                self.consume(b'-');
            }
            self.digits()?;
        }
        Ok(Value::Number(self.text[start..self.offset].to_owned()))
    }
    fn digits(&mut self) -> Result<(), String> {
        let start = self.offset;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.offset += 1;
        }
        if start == self.offset {
            Err("expected JSON digits".to_owned())
        } else {
            Ok(())
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.require(b'"')?;
        let mut value = String::new();
        loop {
            match self.peek().ok_or("unterminated JSON string")? {
                b'"' => {
                    self.offset += 1;
                    return Ok(value);
                }
                b'\\' => {
                    self.offset += 1;
                    let byte = self.peek().ok_or("unterminated JSON escape")?;
                    self.offset += 1;
                    value.push(match byte {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let first = self.hex4()?;
                            let code = if (0xd800..=0xdbff).contains(&first) {
                                self.require(b'\\')?;
                                self.require(b'u')?;
                                let second = self.hex4()?;
                                if !(0xdc00..=0xdfff).contains(&second) {
                                    return Err("invalid JSON surrogate pair".to_owned());
                                }
                                0x10000 + ((first - 0xd800) << 10) + second - 0xdc00
                            } else {
                                first
                            };
                            char::from_u32(code).ok_or("invalid JSON Unicode escape")?
                        }
                        _ => return Err("invalid JSON escape".to_owned()),
                    });
                }
                0..=31 => return Err("unescaped JSON control character".to_owned()),
                _ => {
                    let character = self.text[self.offset..]
                        .chars()
                        .next()
                        .ok_or("missing JSON character")?;
                    value.push(character);
                    self.offset += character.len_utf8();
                }
            }
        }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        let mut result = 0;
        for _ in 0..4 {
            let digit = self
                .peek()
                .and_then(|byte| char::from(byte).to_digit(16))
                .ok_or("invalid JSON hex escape")?;
            self.offset += 1;
            result = result * 16 + digit;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_and_nested_metadata_are_parsed_without_silent_loss() {
        let value =
            parse(r#"{"path":"E:\\中文 空格\\\uD83D\uDE00","nodes":[null,true,12]}"#).unwrap();
        assert_eq!(
            value.field("path").unwrap().string().unwrap(),
            "E:\\中文 空格\\😀"
        );
        assert_eq!(
            value.field("nodes").unwrap().array().unwrap()[2]
                .unsigned()
                .unwrap(),
            12
        );
        assert_eq!(Value::Null.optional_string().unwrap(), None);
        for text in [
            "{} {}",
            r#"{"a":1,"a":2}"#,
            "[1,]",
            "01",
            "1.",
            "1e",
            "--1",
            r#""\uDC00""#,
            r#""\uD800x""#,
            "[null",
            "\"\n\"",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
        assert!(parse(&"[".repeat(130)).is_err());
    }
}
