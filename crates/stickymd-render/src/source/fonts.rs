//! Script-level font selection for the source projection.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#font-runs

use std::ops::Range;

use cosmic_text::FontSystem;
use unicode_script::{Script, UnicodeScript};

const CJK_CANDIDATES: &[&str] = &[
    "仿宋_GB2312",
    "FangSong_GB2312",
    "仿宋",
    "FangSong",
    "Microsoft YaHei",
];
const LATIN_CANDIDATES: &[&str] = &["Times New Roman", "Georgia"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptClass {
    Cjk,
    Latin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptRun {
    pub range: Range<usize>,
    pub class: ScriptClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontSelection {
    pub cjk_family: &'static str,
    pub cjk_found: bool,
    pub latin_family: &'static str,
    pub latin_found: bool,
}

impl FontSelection {
    pub fn resolve(font_system: &mut FontSystem) -> Self {
        let (cjk_family, cjk_found) = first_present(font_system, CJK_CANDIDATES);
        let (latin_family, latin_found) = first_present(font_system, LATIN_CANDIDATES);
        Self {
            cjk_family,
            cjk_found,
            latin_family,
            latin_found,
        }
    }

    pub const fn family_for(&self, class: ScriptClass) -> &'static str {
        match class {
            ScriptClass::Cjk => self.cjk_family,
            ScriptClass::Latin => self.latin_family,
        }
    }
}

fn first_present(
    font_system: &mut FontSystem,
    candidates: &'static [&'static str],
) -> (&'static str, bool) {
    use cosmic_text::fontdb::{Family, Query, Stretch, Style, Weight};

    for &candidate in candidates {
        let query = Query {
            families: &[Family::Name(candidate)],
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            style: Style::Normal,
        };
        if font_system.db_mut().query(&query).is_some() {
            return (candidate, true);
        }
    }
    (candidates[0], false)
}

pub fn segment_script_runs(text: &str) -> Vec<ScriptRun> {
    if text.is_empty() {
        return Vec::new();
    }

    // Resolve the leading neutral run once, then carry the last explicit script
    // forward. This stays O(n), including punctuation-only input; looking ahead
    // from every leading neutral character would be O(n^2).
    let leading_class = text
        .chars()
        .find_map(classify)
        .unwrap_or(ScriptClass::Latin);
    let mut runs = Vec::new();
    let mut start = 0;
    let mut class = leading_class;
    for (byte, character) in text.char_indices() {
        let resolved = classify(character).unwrap_or(class);
        if resolved != class {
            runs.push(ScriptRun {
                range: start..byte,
                class,
            });
            start = byte;
            class = resolved;
        }
    }
    runs.push(ScriptRun {
        range: start..text.len(),
        class,
    });
    runs
}

fn classify(character: char) -> Option<ScriptClass> {
    match character.script() {
        Script::Han | Script::Hangul | Script::Hiragana | Script::Katakana | Script::Bopomofo => {
            Some(ScriptClass::Cjk)
        }
        Script::Latin => Some(ScriptClass::Latin),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_script_runs_keep_punctuation_with_adjacent_text() {
        let text = "这是 Rust 的 trait 示例。";
        let runs = segment_script_runs(text);
        let actual: Vec<(&str, ScriptClass)> = runs
            .iter()
            .map(|run| (&text[run.range.clone()], run.class))
            .collect();
        assert_eq!(
            actual,
            vec![
                ("这是 ", ScriptClass::Cjk),
                ("Rust ", ScriptClass::Latin),
                ("的 ", ScriptClass::Cjk),
                ("trait ", ScriptClass::Latin),
                ("示例。", ScriptClass::Cjk),
            ]
        );
    }

    #[test]
    fn leading_neutral_uses_the_first_explicit_script() {
        let text = "(中文)";
        assert_eq!(
            segment_script_runs(text),
            vec![ScriptRun {
                range: 0..text.len(),
                class: ScriptClass::Cjk,
            }]
        );
    }

    #[test]
    fn neutral_only_input_is_one_linear_run() {
        let text = ".".repeat(64 * 1024);
        assert_eq!(
            segment_script_runs(&text),
            vec![ScriptRun {
                range: 0..text.len(),
                class: ScriptClass::Latin,
            }]
        );
    }
}
