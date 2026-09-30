//! Ignore terminal colors in logs without interpreting cursor or other controls.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::borrow::Cow;

pub(super) fn strip_sgr(line: &str) -> Cow<'_, str> {
    let mut output = None;
    let mut copied = 0;
    let mut cursor = 0;
    while let Some(offset) = line[cursor..].find("\x1b[") {
        let start = cursor + offset;
        cursor = start + 2;
        while line
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_digit() || matches!(byte, b';' | b':'))
        {
            cursor += 1;
        }
        if line.as_bytes().get(cursor) != Some(&b'm') {
            continue;
        }
        let output = output.get_or_insert_with(|| String::with_capacity(line.len()));
        output.push_str(&line[copied..start]);
        cursor += 1;
        copied = cursor;
    }
    match output {
        Some(mut output) => {
            output.push_str(&line[copied..]);
            Cow::Owned(output)
        }
        None => Cow::Borrowed(line),
    }
}
