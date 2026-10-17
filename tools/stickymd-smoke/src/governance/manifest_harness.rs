//! Finds `harness` keys in a Cargo manifest with TOML's lexical rules (plan 11: unit-test
//! owners rely on libtest's default harness, so only a `[[bench]]` target may set one).
//!
//! A scanner, not a TOML parser: it follows strings (basic, literal and their multi-line
//! forms, with the TOML 1.1 escapes Cargo accepts), comments, table headers and inline
//! tables and arrays far enough to see every key, bare or quoted, with its full path. A path
//! is a shape whose parts are keys and array elements, so `[[bench]]` then `harness` is
//! `bench[].harness` while `meta.harness` under it is `bench[].meta.harness`. A `harness`
//! key is allowed only as a direct field of a root-level bench target: `bench[].harness`, or
//! `bench.harness` for a bench table. Anything the scanner cannot follow fails closed.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

/// One step of a key path: a key, or an element of an array.
#[derive(Clone, Debug, PartialEq)]
enum Part {
    Key(String),
    Element,
}

/// Whether a key at `shape` may be called `harness`.
fn harness_allowed(shape: &[Part]) -> bool {
    let bench = Part::Key("bench".to_owned());
    let harness = Part::Key("harness".to_owned());
    shape == [bench.clone(), Part::Element, harness.clone()] || shape == [bench, harness]
}

/// Whether any key named `harness` along `shape` sits outside a bench target: a dotted key
/// or a header names every table on its path, so `harness.enabled` names `harness` too.
fn misplaces_harness(shape: &[Part]) -> bool {
    shape.iter().enumerate().any(|(index, part)| {
        *part == Part::Key("harness".to_owned()) && !harness_allowed(&shape[..=index])
    })
}

/// `prefix` followed by the keys of `path`.
fn extend(prefix: &[Part], path: &[String]) -> Vec<Part> {
    prefix
        .iter()
        .cloned()
        .chain(path.iter().cloned().map(Part::Key))
        .collect()
}

/// The 1-based line of the first `harness` key outside a bench target; an unterminated
/// string, header or bracket is reported at the line where scanning stopped.
pub(super) fn harness_outside_benches(manifest: &str) -> Option<usize> {
    let mut scanner = Scanner {
        // Cargo's TOML lexer skips one UTF-8 byte-order mark at the start of the file.
        chars: manifest
            .strip_prefix('\u{feff}')
            .unwrap_or(manifest)
            .chars()
            .filter(|&c| c != '\r')
            .collect(),
        index: 0,
        line: 1,
    };
    // The shape of the current table, and per open bracket the shape of what it contains.
    let mut table: Vec<Part> = Vec::new();
    let mut open: Vec<(char, Vec<Part>)> = Vec::new();
    let mut expect_key = true;
    loop {
        scanner.skip_blank();
        let Some(c) = scanner.peek() else {
            return (!open.is_empty()).then_some(scanner.line);
        };
        match c {
            '#' => scanner.skip_comment(),
            '\n' => {
                scanner.index += 1;
                scanner.line += 1;
                if open.is_empty() {
                    expect_key = true;
                }
            }
            '[' if expect_key && open.is_empty() => match scanner.header() {
                Some((path, array)) => {
                    // A header names each key on its path: `[a.harness.b]` is a harness key.
                    table = extend(&[], &path);
                    if misplaces_harness(&table) {
                        return Some(scanner.line);
                    }
                    if array {
                        table.push(Part::Element);
                    }
                    expect_key = false;
                }
                None => return Some(scanner.line),
            },
            // A bracket without a key is an array element; anywhere else it is invalid.
            '{' | '[' => {
                scanner.index += 1;
                let Some((parent, prefix)) = open.last() else {
                    return Some(scanner.line);
                };
                if *parent != '[' {
                    return Some(scanner.line);
                }
                let mut shape = prefix.clone();
                if c == '[' {
                    shape.push(Part::Element);
                }
                open.push((c, shape));
                expect_key = c == '{';
            }
            '}' | ']' => {
                scanner.index += 1;
                let expected = if c == '}' { '{' } else { '[' };
                if open.pop().map(|(kind, _)| kind) != Some(expected) {
                    return Some(scanner.line);
                }
                expect_key = false;
            }
            // A comma separates array elements or inline-table pairs; at the top level it is
            // invalid TOML, which fails closed.
            ',' => {
                scanner.index += 1;
                match open.last() {
                    Some((kind, _)) => expect_key = *kind == '{',
                    None => return Some(scanner.line),
                }
            }
            _ if expect_key => {
                let Some(path) = scanner.key() else {
                    return Some(scanner.line);
                };
                // Keys sit in the current table, or in the inline table that is open.
                let context = open.last().map_or(&table, |(_, prefix)| prefix);
                let shape = extend(context, &path);
                if misplaces_harness(&shape) {
                    return Some(scanner.line);
                }
                // An inline value belongs to this key: a table continues its shape, an
                // array adds an element level.
                scanner.skip_blank();
                match scanner.peek() {
                    Some('{') => {
                        scanner.index += 1;
                        open.push(('{', shape));
                        expect_key = true;
                    }
                    Some('[') => {
                        scanner.index += 1;
                        let mut elements = shape;
                        elements.push(Part::Element);
                        open.push(('[', elements));
                        expect_key = false;
                    }
                    _ => expect_key = false,
                }
            }
            '"' | '\'' => {
                if scanner.string().is_none() {
                    return Some(scanner.line);
                }
            }
            _ => scanner.skip_value(),
        }
    }
}

struct Scanner {
    chars: Vec<char>,
    index: usize,
    line: usize,
}

impl Scanner {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn starts_with(&self, text: &str) -> bool {
        text.chars()
            .enumerate()
            .all(|(offset, c)| self.chars.get(self.index + offset) == Some(&c))
    }

    fn skip_blank(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.index += 1;
        }
    }

    fn skip_comment(&mut self) {
        while self.peek().is_some_and(|c| c != '\n') {
            self.index += 1;
        }
    }

    /// A number, boolean or date: everything up to the next delimiter.
    fn skip_value(&mut self) {
        while self
            .peek()
            .is_some_and(|c| !matches!(c, ',' | ']' | '}' | '#' | '\n' | ' ' | '\t' | '"' | '\''))
        {
            self.index += 1;
        }
    }

    /// A table header `[a.b]` or `[[a.b]]`: its key path and whether it names an array of
    /// tables. The header must end its line.
    fn header(&mut self) -> Option<(Vec<String>, bool)> {
        let array = self.starts_with("[[");
        self.index += if array { 2 } else { 1 };
        let mut path = Vec::new();
        loop {
            self.skip_blank();
            path.push(self.key_part()?);
            self.skip_blank();
            match self.peek()? {
                '.' => self.index += 1,
                ']' => break,
                _ => return None,
            }
        }
        let close = if array { "]]" } else { "]" };
        if !self.starts_with(close) {
            return None;
        }
        self.index += close.len();
        Some((path, array))
    }

    /// A dotted key followed by `=`, consumed through the `=`.
    fn key(&mut self) -> Option<Vec<String>> {
        let mut path = Vec::new();
        loop {
            self.skip_blank();
            path.push(self.key_part()?);
            self.skip_blank();
            match self.peek()? {
                '.' => self.index += 1,
                '=' => {
                    self.index += 1;
                    return Some(path);
                }
                _ => return None,
            }
        }
    }

    /// One bare or quoted key part, unquoted and unescaped.
    fn key_part(&mut self) -> Option<String> {
        match self.peek()? {
            '"' | '\'' => self.string(),
            _ => {
                let start = self.index;
                while self
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    self.index += 1;
                }
                (self.index > start).then(|| self.chars[start..self.index].iter().collect())
            }
        }
    }

    /// A basic or literal string, single- or multi-line; its value with escapes applied.
    fn string(&mut self) -> Option<String> {
        let quote = self.peek()?;
        let basic = quote == '"';
        let multi = if basic { "\"\"\"" } else { "'''" };
        let mut value = String::new();
        if self.starts_with(multi) {
            self.index += 3;
            loop {
                if self.starts_with(multi) {
                    // The first run of three or more quotes closes the string; up to two of
                    // them may be content just before the delimiter (TOML 1.0, strings).
                    let mut run = 0;
                    while self.chars.get(self.index + run) == Some(&quote) {
                        run += 1;
                    }
                    if run > 5 {
                        return None;
                    }
                    value.extend(std::iter::repeat_n(quote, run - 3));
                    self.index += run;
                    return Some(value);
                }
                let c = self.peek()?;
                if c == '\\' && basic {
                    if let Some(escaped) = self.escape(true)? {
                        value.push(escaped);
                    }
                } else {
                    if c == '\n' {
                        self.line += 1;
                    }
                    value.push(c);
                    self.index += 1;
                }
            }
        }
        self.index += 1;
        loop {
            match self.peek()? {
                '\n' => return None,
                c if c == quote => {
                    self.index += 1;
                    return Some(value);
                }
                '\\' if basic => value.push(self.escape(false)??),
                c => {
                    value.push(c);
                    self.index += 1;
                }
            }
        }
    }

    /// One escape of a basic string, the backslash included: the character it stands for,
    /// or nothing for a line-ending backslash (multi-line strings only: optional spaces or
    /// tabs, a newline, then every following space, tab and newline are trimmed). `None` is
    /// an invalid escape.
    fn escape(&mut self, multiline: bool) -> Option<Option<char>> {
        self.index += 1;
        if multiline {
            let mut ahead = self.index;
            while matches!(self.chars.get(ahead), Some(' ' | '\t')) {
                ahead += 1;
            }
            if self.chars.get(ahead) == Some(&'\n') {
                self.index = ahead;
                while let Some(c @ (' ' | '\t' | '\n')) = self.peek() {
                    if c == '\n' {
                        self.line += 1;
                    }
                    self.index += 1;
                }
                return Some(None);
            }
        }
        let c = self.peek()?;
        self.index += 1;
        let hex = |scanner: &mut Self, digits: usize| {
            let text = scanner.chars.get(scanner.index..scanner.index + digits)?;
            scanner.index += digits;
            char::from_u32(u32::from_str_radix(&text.iter().collect::<String>(), 16).ok()?)
        };
        let escaped = match c {
            // TOML 1.1, which Cargo accepts since 1.94.
            'x' => hex(self, 2)?,
            'u' => hex(self, 4)?,
            'U' => hex(self, 8)?,
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            'b' => '\u{8}',
            'f' => '\u{c}',
            'e' => '\u{1b}',
            '"' | '\\' => c,
            _ => return None,
        };
        Some(Some(escaped))
    }
}

#[cfg(test)]
mod tests {
    use super::harness_outside_benches;

    #[test]
    fn only_bench_targets_may_set_a_harness() {
        for (manifest, line) in [
            ("[[test]]\nname = \"t\"\nharness = false\n", Some(3)),
            (
                "[lib]\nharness = false\n[[bench]]\nharness = false\n",
                Some(2),
            ),
            (
                "[[ bench ]]\nharness = false\n[[bin]]\nname = \"x\"\nharness = false\n",
                Some(5),
            ),
            (
                "[package]\ntest = [{ name = \"t\", harness = false }]\n",
                Some(2),
            ),
            (
                "test = [\n  { name = \"t\",\n    harness = false },\n]\n",
                Some(3),
            ),
            (
                "[package]\nbench = [{ name = \"b\", harness = false }]\n",
                Some(2),
            ),
            ("[[test]]\nfoo.harness = 1\n", Some(2)),
            // Nested inside a bench target is not the target's own field.
            ("[[bench]]\nname = \"b\"\nmeta.harness = false\n", Some(3)),
            (
                "[[bench]]\nname = \"b\"\n[bench.meta]\nharness = false\n",
                Some(4),
            ),
            (
                "bench = [{ name = \"b\", meta = { harness = false } }]\n",
                Some(1),
            ),
            ("bench = [[{ harness = false }]]\n", Some(1)),
            ("[harness]\nx = 1\n", Some(1)),
            ("[[test.harness]]\nx = 1\n", Some(1)),
            ("[[test]]\n\"\\x68arness\" = false\n", Some(2)),
            // A parent named harness is a harness key too, however it is written.
            ("[package.metadata]\nharness.enabled = false\n", Some(2)),
            ("[package.metadata.harness.settings]\nx = 1\n", Some(1)),
            ("[[package.metadata.harness]]\nx = 1\n", Some(1)),
            (
                "[package.metadata]\nh = { harness.enabled = false }\n",
                Some(2),
            ),
            (
                "[package.metadata]\nharness = { enabled = false }\n",
                Some(2),
            ),
            // Allowed: a bench table and the root-level bench array.
            ("bench.harness = false\n", None),
            ("[bench]\nharness = false\n", None),
            (
                "[package]\nname = \"a\"\n\n[[bench]]\nname = \"b\"\nharness = false\n",
                None,
            ),
            ("bench = [{ name = \"b\", harness = false }]\n", None),
            ("[[\"bench\"]]\n'harness' = false\n", None),
        ] {
            assert_eq!(harness_outside_benches(manifest), line, "{manifest}");
        }
    }

    #[test]
    fn quoted_keys_count_and_strings_or_comments_do_not() {
        for manifest in [
            "[[test]]\n\"harness\" = false\n",
            "[[test]]\n'harness' = false\n",
            "[[test]]\n\"har\\u006Eess\" = false\n",
            "[[test]]\nname = \"t\" , harness = false\n",
        ] {
            assert!(harness_outside_benches(manifest).is_some(), "{manifest}");
        }
        for manifest in [
            "[package]\ndescription = \"uses harness = false\"\n",
            // TOML 1.1 `\xHH`, which Cargo accepts.
            "[package]\ndescription = \"\\x53tickyMD\"\n",
            "[package]\ndescription = \"\"\"\\x53tickyMD\n\"\"\"\n",
            "[package]\ndescription = 'harness = false'\n",
            "[package]\ndescription = \"\"\"\nharness = false\n\"\"\"\n",
            "[package]\n# harness = false\nname = \"harnesses\"\nmy_harness = 1\nharness_extra = 2\n",
            "[[bench]]\nharness = false\n[[test]]\nname = \"x\" # harness = false\n",
            "[package]\nkeywords = [\"harness = false\", 'x']\n",
        ] {
            assert_eq!(harness_outside_benches(manifest), None, "{manifest}");
        }
    }

    /// Multi-line strings end at the first run of three or more quotes, up to two of which
    /// are content; a line-ending backslash may be followed by spaces before the newline.
    #[test]
    fn multi_line_strings_end_where_toml_ends_them() {
        let hidden = concat!(
            "[package]\nname = \"demo\"\n",
            "description = \"\"\"hello\"\"\"\" # \"\"\"\"\n",
            "[[bin]]\nname = \"demo\"\nharness = false\n# \"\"\"\n",
        );
        assert_eq!(harness_outside_benches(hidden), Some(6));
        let literal = concat!(
            "[package]\ndescription = '''hello'''' # ''''\n",
            "[[bin]]\nharness = false\n# '''\n",
        );
        assert_eq!(harness_outside_benches(literal), Some(4));
        for manifest in [
            "[package]\ndescription = \"\"\"hello\"\"\"\"\n",
            "[package]\ndescription = \"\"\"hello\"\"\"\"\"\n",
            "[package]\ndescription = '''hello''''\n",
            "[package]\ndescription = \"\"\"\"\"\"\n",
            "[package]\ndescription = \"\"\"hello\\  \n  world\"\"\"\n",
            "[package]\ndescription = \"\"\"hello\\\n\n\t world\"\"\"\n",
        ] {
            assert_eq!(harness_outside_benches(manifest), None, "{manifest}");
        }
        for manifest in [
            // Six quotes close nothing valid; a single-line string cannot continue a line;
            // a backslash and spaces not followed by a newline is no escape.
            "[package]\ndescription = \"\"\"hello\"\"\"\"\"\"\n",
            "[package]\ndescription = \"hello\\\nworld\"\n",
            "[package]\ndescription = \"\"\"hello\\  world\"\"\"\n",
        ] {
            assert!(harness_outside_benches(manifest).is_some(), "{manifest}");
        }
    }

    #[test]
    fn a_leading_byte_order_mark_is_skipped_like_cargo_does() {
        assert_eq!(
            harness_outside_benches("\u{feff}[workspace]\nmembers = []\n"),
            None
        );
        assert_eq!(
            harness_outside_benches("\u{feff}[[test]]\nharness = false\n"),
            Some(2)
        );
        // Only one mark, and only at the start: elsewhere it is ordinary content.
        assert!(harness_outside_benches("\u{feff}\u{feff}[workspace]\n").is_some());
        assert_eq!(
            harness_outside_benches("[package]\ndescription = \"\u{feff}\"\n"),
            None
        );
    }

    #[test]
    fn what_the_scanner_cannot_follow_fails_closed() {
        for manifest in [
            "[package]\nname = \"unterminated\n",
            "[package]\ndescription = \"\"\"\nopen\n",
            "test = [{ name = \"t\"\n",
            "[package\nname = \"a\"\n",
            "[package]\n= 1\n",
        ] {
            assert!(harness_outside_benches(manifest).is_some(), "{manifest}");
        }
        // The workspace's real manifests read cleanly.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for relative in [
            "Cargo.toml",
            "crates/stickymd-core/Cargo.toml",
            "crates/stickymd-render/Cargo.toml",
            "apps/stickymd-win/Cargo.toml",
            "tools/stickymd-smoke/Cargo.toml",
        ] {
            let manifest = std::fs::read_to_string(root.join(relative)).unwrap();
            assert_eq!(harness_outside_benches(&manifest), None, "{relative}");
        }
    }
}
