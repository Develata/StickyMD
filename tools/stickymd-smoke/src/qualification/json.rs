//! JSON for the fixed release-receipt schemas: writers escape strings, readers parse
//! the whole receipt strictly and read each field only from the object that owns it.

pub(super) use crate::release::json::Value;

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

/// Parse a whole receipt strictly (`crate::release::json`: duplicate keys, truncation and
/// trailing input are errors). It must be one JSON object.
pub(super) fn parse_object(text: &str) -> Result<Value, String> {
    match crate::release::json::parse(text)? {
        object @ Value::Object(_) => Ok(object),
        _ => Err("receipt is not a JSON object".to_owned()),
    }
}

/// A field of `object` itself; nested objects are never searched.
fn field<'a>(object: &'a Value, key: &str) -> Result<&'a Value, String> {
    let Value::Object(fields) = object else {
        return Err(format!("JSON field `{key}` is read from a non-object"));
    };
    fields
        .get(key)
        .ok_or_else(|| format!("JSON field `{key}` is missing"))
}

pub(super) fn has_field(object: &Value, key: &str) -> bool {
    matches!(object, Value::Object(fields) if fields.contains_key(key))
}

pub(super) fn string_field(object: &Value, key: &str) -> Result<String, String> {
    match field(object, key)? {
        Value::String(value) => Ok(value.clone()),
        _ => Err(format!("JSON field `{key}` is not a string")),
    }
}

pub(super) fn u64_field(object: &Value, key: &str) -> Result<u64, String> {
    match field(object, key)? {
        Value::Number(number) => number
            .parse::<u64>()
            .map_err(|_| format!("JSON field `{key}` is not an unsigned integer")),
        _ => Err(format!("JSON field `{key}` is not an unsigned integer")),
    }
}

pub(super) fn f64_field(object: &Value, key: &str) -> Result<f64, String> {
    match field(object, key)? {
        Value::Number(number) => number
            .parse::<f64>()
            .map_err(|_| format!("JSON field `{key}` is not a number")),
        _ => Err(format!("JSON field `{key}` is not a number")),
    }
}

pub(super) fn bool_field(object: &Value, key: &str) -> Result<bool, String> {
    match field(object, key)? {
        Value::Bool(value) => Ok(*value),
        _ => Err(format!("JSON field `{key}` is not a boolean")),
    }
}

/// The object stored in `key`.
pub(super) fn object_field<'a>(object: &'a Value, key: &str) -> Result<&'a Value, String> {
    match field(object, key)? {
        nested @ Value::Object(_) => Ok(nested),
        _ => Err(format!("JSON field `{key}` is not an object")),
    }
}

/// The array of objects stored in `key`.
pub(super) fn objects<'a>(object: &'a Value, key: &str) -> Result<&'a [Value], String> {
    match field(object, key)? {
        Value::Array(items) if items.iter().all(|item| matches!(item, Value::Object(_))) => {
            Ok(items)
        }
        _ => Err(format!("JSON field `{key}` is not an array of objects")),
    }
}

#[cfg(test)]
mod tests {
    use super::{bool_field, escape, has_field, objects, parse_object, string_field, u64_field};

    #[test]
    fn receipt_fields_come_only_from_their_own_object() {
        let document = parse_object(
            r#"{"schema_version":1,"name":"a\\b\"c","ready":false,"qualification_environment":{"status":"VALID"},"results":[{"status":"PASSED"}]}"#,
        )
        .unwrap();
        assert_eq!(u64_field(&document, "schema_version"), Ok(1));
        assert_eq!(string_field(&document, "name"), Ok("a\\b\"c".to_owned()));
        assert_eq!(bool_field(&document, "ready"), Ok(false));
        // Nested values are not top-level fields.
        assert!(string_field(&document, "status").is_err());
        assert!(!has_field(&document, "status"));
        assert_eq!(objects(&document, "results").unwrap().len(), 1);
        assert_eq!(escape("a\nb"), "a\\nb");

        // Re-spacing is legal JSON and reads the owning object's value, not a decoy.
        let spaced =
            parse_object(r#"{"status" : "FAILED", "extra": {"status":"PASSED"}}"#).unwrap();
        assert_eq!(string_field(&spaced, "status"), Ok("FAILED".to_owned()));
        for broken in [
            r#"{"status":"PASSED","status":"FAILED"}"#,
            r#"{"status":"PASSED""#,
            r#"{"status":"PASSED"} {}"#,
            r#"["status","PASSED"]"#,
        ] {
            assert!(parse_object(broken).is_err(), "{broken}");
        }
        let numbers = parse_object(r#"{"a":-1,"b":1.5,"c":"7"}"#).unwrap();
        for key in ["a", "b", "c"] {
            assert!(u64_field(&numbers, key).is_err(), "{key}");
        }
    }
}
