//! Minimal JSON helpers for the fixed release-receipt schemas.

pub(super) fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

pub(super) fn string_field(document: &str, key: &str) -> Result<String, String> {
    let marker = format!("\"{}\":", escape(key));
    let start = document
        .find(&marker)
        .ok_or_else(|| format!("JSON field `{key}` is missing"))?
        + marker.len();
    parse_string(document, start).map(|(value, _)| value)
}

pub(super) fn u64_field(document: &str, key: &str) -> Result<u64, String> {
    let marker = format!("\"{}\":", escape(key));
    let start = document
        .find(&marker)
        .ok_or_else(|| format!("JSON field `{key}` is missing"))?
        + marker.len();
    let tail = document[start..].trim_start();
    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return Err(format!("JSON field `{key}` is not an unsigned integer"));
    }
    digits
        .parse::<u64>()
        .map_err(|error| format!("JSON field `{key}` is invalid: {error}"))
}

pub(super) fn bool_field(document: &str, key: &str) -> Result<bool, String> {
    let marker = format!("\"{}\":", escape(key));
    let start = document
        .find(&marker)
        .ok_or_else(|| format!("JSON field `{key}` is missing"))?
        + marker.len();
    let tail = document[start..].trim_start();
    if tail.starts_with("true") {
        Ok(true)
    } else if tail.starts_with("false") {
        Ok(false)
    } else {
        Err(format!("JSON field `{key}` is not a boolean"))
    }
}

pub(super) fn status_values(document: &str) -> Result<Vec<String>, String> {
    let mut values = Vec::new();
    let marker = "\"status\":";
    let mut offset = 0usize;
    while let Some(relative) = document[offset..].find(marker) {
        let start = offset + relative + marker.len();
        let (value, end) = parse_string(document, start)?;
        values.push(value);
        offset = end;
    }
    Ok(values)
}

fn parse_string(document: &str, start: usize) -> Result<(String, usize), String> {
    let bytes = document.as_bytes();
    let mut index = start;
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    if bytes.get(index) != Some(&b'"') {
        return Err("expected JSON string".to_owned());
    }
    index += 1;
    let mut value = String::new();
    while let Some(&byte) = bytes.get(index) {
        match byte {
            b'"' => return Ok((value, index + 1)),
            b'\\' => {
                index += 1;
                let escaped = *bytes
                    .get(index)
                    .ok_or_else(|| "unterminated JSON escape".to_owned())?;
                match escaped {
                    b'"' => value.push('"'),
                    b'\\' => value.push('\\'),
                    b'n' => value.push('\n'),
                    b'r' => value.push('\r'),
                    b't' => value.push('\t'),
                    _ => return Err("unsupported JSON escape in receipt".to_owned()),
                }
            }
            byte if byte.is_ascii_control() => {
                return Err("control character in JSON string".to_owned());
            }
            byte if byte.is_ascii() => value.push(char::from(byte)),
            _ => {
                let character = document[index..]
                    .chars()
                    .next()
                    .ok_or_else(|| "invalid UTF-8 boundary in JSON string".to_owned())?;
                value.push(character);
                index += character.len_utf8() - 1;
            }
        }
        index += 1;
    }
    Err("unterminated JSON string".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{bool_field, escape, status_values, string_field, u64_field};

    #[test]
    fn fixed_receipt_fields_are_read_without_a_general_json_dependency() {
        let document =
            r#"{"schema_version":1,"name":"a\\b\"c","ready":false,"status":"NOT_READY"}"#;
        assert_eq!(u64_field(document, "schema_version"), Ok(1));
        assert_eq!(string_field(document, "name"), Ok("a\\b\"c".to_owned()));
        assert_eq!(bool_field(document, "ready"), Ok(false));
        assert_eq!(status_values(document), Ok(vec!["NOT_READY".to_owned()]));
        assert_eq!(escape("a\nb"), "a\\nb");
    }
}
