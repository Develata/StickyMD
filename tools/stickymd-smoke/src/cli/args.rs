//! Named `--key=value` argument helpers shared by the command parsers.

pub(super) fn named_value<'a>(arguments: &'a [String], prefix: &str) -> Result<&'a str, String> {
    arguments
        .iter()
        .find_map(|argument| argument.strip_prefix(prefix))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing `{prefix}<value>`"))
}

/// The values of `<prefix><value>` arguments in `prefixes` order, each given at most once;
/// any other argument, a duplicate or an empty value is an error.
pub(super) fn exact_named_values<'a, const N: usize>(
    arguments: &'a [String],
    prefixes: [&str; N],
) -> Result<[Option<&'a str>; N], String> {
    let mut values = [None; N];
    for argument in arguments {
        let (index, value) = prefixes
            .iter()
            .enumerate()
            .find_map(|(index, prefix)| argument.strip_prefix(prefix).map(|value| (index, value)))
            .ok_or_else(|| format!("unexpected argument `{argument}`"))?;
        if value.is_empty() {
            return Err(format!("empty `{}<value>`", prefixes[index]));
        }
        if values[index].replace(value).is_some() {
            return Err(format!("duplicate `{}<value>`", prefixes[index]));
        }
    }
    Ok(values)
}

pub(super) fn optional_named_value<'a>(
    arguments: &'a [String],
    prefix: &str,
) -> Result<Option<&'a str>, String> {
    let mut values = arguments
        .iter()
        .filter_map(|argument| argument.strip_prefix(prefix));
    let value = values.next();
    if values.next().is_some() {
        return Err(format!("duplicate `{prefix}<value>`"));
    }
    match value {
        Some("") => Err(format!("empty `{prefix}<value>`")),
        value => Ok(value),
    }
}

pub(super) fn named_u64(arguments: &[String], prefix: &str) -> Result<u64, String> {
    let value = named_value(arguments, prefix)?;
    value
        .parse::<u64>()
        .map_err(|error| format!("invalid `{prefix}{value}`: {error}"))
}

pub(super) fn named_bounded_usize(
    arguments: &[String],
    prefix: &str,
    minimum: usize,
    maximum: usize,
) -> Result<usize, String> {
    let value = named_value(arguments, prefix)?;
    let parsed = value
        .parse::<usize>()
        .map_err(|error| format!("invalid `{prefix}{value}`: {error}"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(format!(
            "`{prefix}{value}` is outside {minimum}..={maximum}"
        ));
    }
    Ok(parsed)
}
