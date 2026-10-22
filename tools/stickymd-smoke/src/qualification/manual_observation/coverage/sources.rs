//! The documents the mapping decomposes, bound by digests: the whole Phase 8 matrix document
//! (its `Manual` rows are the sources) and the plan 09 sections holding the Esc, manual
//! collapse, scrollbar, opacity and quit clauses.
//!
//! Both documents are digested whole, so an edit anywhere in them (a changed lead sentence, an
//! extra clause, an anchor followed by new text, a reformatted row, an HTML comment hiding a
//! section) fails until the decomposition is reviewed; each bound plan 09 section is also
//! digested as raw text up to the next heading, to say which one changed. The extractors
//! reject content they do not recognize instead of skipping it, and per-clause digests name
//! which source changed. Line endings are normalized, so a CRLF checkout digests the same.
//! Pure functions over the text, so the counterexamples start from edited Markdown.

use super::tests::{digest, read};

const PHASE08: &str = "docs/acceptance-cases/phase-08.md";
const PLAN09: &str = "docs/plan/09_windows_shell.md";

/// The cells of a Markdown table line without the optional outer pipes (GFM allows rows with no
/// leading or trailing pipe).
fn cells(line: &str) -> Vec<&str> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);
    line.split('|').map(str::trim).collect()
}

/// Every `Manual` row of the Phase 8 matrix as `(id, row text)`, in document order. A matrix
/// row (a table line whose first cell is a P08 ID) must have five cells and the mode
/// `Automated` or `Manual` exactly; anything else is an error, so no Manual row escapes by its
/// formatting.
fn phase08_manual_rows(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut rows = Vec::new();
    for line in text.lines() {
        if !line.contains('|') {
            continue;
        }
        let cells = cells(line);
        if !cells[0].starts_with("P08-") {
            continue;
        }
        match (cells.len(), cells.get(2).copied()) {
            (5, Some("Manual")) => rows.push((cells[0].to_owned(), line.trim_end().to_owned())),
            (5, Some("Automated")) => {}
            _ => return Err(format!("unrecognized Phase 8 matrix row `{line}`")),
        }
    }
    Ok(rows)
}

/// Whether `line` ends a `###` section: a heading of level 1–3. Anchors and deeper headings
/// stay inside the section, where the bullet extractor decides what they may be.
fn ends_section(line: &str) -> bool {
    ["# ", "## ", "### "]
        .iter()
        .any(|level| line.starts_with(level))
}

/// The raw lines under `heading`, which must occur exactly once.
fn raw_section<'a>(text: &'a str, heading: &str) -> Result<Vec<&'a str>, String> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut starts = (0..lines.len()).filter(|&index| lines[index].trim_end() == heading);
    let start = starts
        .next()
        .ok_or_else(|| format!("no section `{heading}`"))?;
    if starts.next().is_some() {
        return Err(format!("section `{heading}` repeats"));
    }
    Ok(lines[start + 1..]
        .iter()
        .copied()
        .take_while(|line| !ends_section(line))
        .collect())
}

/// The top-level `- ` bullets with their indented continuation lines. The only other content
/// allowed is a trailing anchor for the next section; any other non-blank line (a paragraph, a
/// deeper heading, an orphan continuation, text after an anchor) is an error: it would sit
/// outside every clause the mapping decomposes.
fn bullets(heading: &str, lines: &[&str]) -> Result<Vec<String>, String> {
    let mut items: Vec<String> = Vec::new();
    let mut anchored = false;
    for line in lines {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        if anchored {
            return Err(format!("`{heading}` has content after an anchor `{line}`"));
        }
        if line.starts_with("<a id=\"") && line.ends_with("\"></a>") {
            anchored = true;
        } else if line.starts_with("- ") {
            items.push(line.to_owned());
        } else if line.starts_with("  ")
            && let Some(last) = items.last_mut()
        {
            last.push('\n');
            last.push_str(line);
        } else {
            return Err(format!("`{heading}` has unrecognized content `{line}`"));
        }
    }
    Ok(items)
}

/// The numbered items of the Esc rule: after its lead line, every line starts an item (`  1. `)
/// or continues the current one.
fn numbered(bullet: &str) -> Result<Vec<String>, String> {
    let mut items: Vec<String> = Vec::new();
    for line in bullet.lines().skip(1) {
        let starts_item = line
            .trim_start()
            .split_once(". ")
            .is_some_and(|(number, _)| {
                !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
            });
        if starts_item {
            items.push(line.to_owned());
        } else if let Some(last) = items.last_mut() {
            last.push('\n');
            last.push_str(line);
        } else {
            return Err(format!(
                "the Esc rule has text before its first item `{line}`"
            ));
        }
    }
    Ok(items)
}

/// The one bullet starting with `prefix`.
fn unique<'a>(bullets: &'a [String], prefix: &str) -> Result<&'a String, String> {
    let mut found = bullets.iter().filter(|bullet| bullet.starts_with(prefix));
    match (found.next(), found.next()) {
        (Some(bullet), None) => Ok(bullet),
        (None, _) => Err(format!("no clause `{prefix}`")),
        (Some(_), Some(_)) => Err(format!("clause `{prefix}` repeats")),
    }
}

/// What the documents define: every source as `(source id, clause text)`, and each bound
/// section as `(id, clause count, digest of its raw text)`.
pub(super) struct Sources {
    pub(super) clauses: Vec<(String, String)>,
    sections: Vec<(&'static str, usize, String)>,
}

pub(super) fn decompose(phase08: &str, plan09: &str) -> Result<Sources, String> {
    let rows = phase08_manual_rows(phase08)?;
    let whole = phase08.lines().collect::<Vec<_>>().join("\n");
    let mut sections = vec![("phase08:document", rows.len(), digest(&whole))];
    let mut bound = |id: &'static str, heading: &str| -> Result<Vec<String>, String> {
        let raw = raw_section(plan09, heading)?;
        let items = bullets(heading, &raw)?;
        sections.push((id, items.len(), digest(&raw.join("\n"))));
        Ok(items)
    };
    let dock = bound("plan09:dock", "### Dock 与自动隐藏")?;
    let scroll = bound("plan09:scrollbars", "### 垂直滚动条")?;
    let opacity = bound("plan09:opacity", "### 透明度")?;
    let tray = bound("plan09:tray", "### 托盘生命周期")?;
    let bound_clauses = dock.len() + scroll.len() + opacity.len() + tray.len();
    let whole = plan09.lines().collect::<Vec<_>>().join("\n");
    sections.push(("plan09:document", bound_clauses, digest(&whole)));
    let esc = numbered(unique(&dock, "- Esc 按下列顺序判定")?)?;
    let collapse = unique(&dock, "- 手动收起按钮")?.clone();
    let quit = [
        "- 退出保存失败",
        "- 退出因 canonical note",
        "- 窗口标题由唯一函数",
    ]
    .iter()
    .map(|prefix| unique(&tray, prefix).cloned())
    .collect::<Result<Vec<_>, _>>()?;
    let numbered_ids = |prefix: &str, clauses: &[String]| {
        clauses
            .iter()
            .enumerate()
            .map(|(index, clause)| (format!("{prefix}-{}", index + 1), clause.clone()))
            .collect::<Vec<_>>()
    };
    let mut clauses = rows;
    clauses.extend(numbered_ids("plan09:esc", &esc));
    clauses.push(("plan09:manual-collapse".to_owned(), collapse));
    clauses.extend(numbered_ids("plan09:scroll", &scroll));
    clauses.extend(numbered_ids("plan09:opacity", &opacity));
    clauses.extend(numbered_ids("plan09:quit", &quit));
    Ok(Sources { clauses, sections })
}

pub(super) fn documents() -> (String, String) {
    (read(PHASE08), read(PLAN09))
}

/// Section digests (raw text) and clause counts the decomposition was reviewed against.
const SECTIONS: &[(&str, usize, &str)] = &[
    (
        "phase08:document",
        62,
        "a8faf7fe16d10e775dd7de11a128e2fcdfbc575566240c5beab9ad2a48825892",
    ),
    (
        "plan09:dock",
        15,
        "61eaa593a1f9ec7b8fd1e78eeb71933de2c53aa4b5e6e357c5972d46c6098c7e",
    ),
    (
        "plan09:scrollbars",
        7,
        "3bbe303f08dab13c1be10f7ca0352e0bac91502ae59df304f2c175a153a1d90d",
    ),
    (
        "plan09:opacity",
        3,
        "b73c13b169f1a525f4a16a207afc2cb297ad7d6d0f4c73c5ff7eb801c664b3a3",
    ),
    (
        "plan09:tray",
        7,
        "12a552a2eaea96e0e4199442bb4510a9edb35573183722917861b38fc81e7709",
    ),
    (
        "plan09:document",
        32,
        "d059bf18b84cfcbe30ff8efea2fe9e23a553f676bb500ca37da9642c086fadfa",
    ),
];

/// The first 16 hex digits of each clause's digest: enough to name which source changed.
const CLAUSES: &[(&str, &str)] = &[
    ("P08-M01", "672baba44c61d75b"),
    ("P08-M02", "1ae3c3621ce57e8a"),
    ("P08-M03", "d2e68e72a0c73822"),
    ("P08-M04", "d0916db02d57b91f"),
    ("P08-M05", "aa12a332835ccbb7"),
    ("P08-M06", "aab16b1eb1995d2b"),
    ("P08-M07", "35b6839ced79c46f"),
    ("P08-M08", "8872c6bbbc015e60"),
    ("P08-M09", "a1717a48b17c603c"),
    ("P08-M10", "343347f7769cbf6c"),
    ("P08-M11", "d7d6b669d22b3c3f"),
    ("P08-M12", "d7d5bd56ed9c2523"),
    ("P08-M13", "d503c915b26437e3"),
    ("P08-M14", "9ff197ad81f56792"),
    ("P08-M15", "6b6814e72d06eedd"),
    ("P08-D004", "eacdc051071b6b81"),
    ("P08-D005", "2160408f20480842"),
    ("P08-D006", "a2dd4644912d350c"),
    ("P08-D007", "8dd6cda23d41c029"),
    ("P08-D008", "4aa67b8009a38259"),
    ("P08-D010", "bc908e90635b99fd"),
    ("P08-D012", "2a2cd70933229867"),
    ("P08-D013", "74b712a0efcb75fd"),
    ("P08-D016", "a7d3865f7cda6999"),
    ("P08-D020", "d5afaefc3f688ccf"),
    ("P08-D021", "8dfc9ccfd828805e"),
    ("P08-D022", "b4f8d6d9ead6615e"),
    ("P08-D027", "720ef4537e843d74"),
    ("P08-D028", "ebd901c91c2fbec6"),
    ("P08-D031", "c7b1ca3c8f753079"),
    ("P08-D039", "4093605f13759192"),
    ("P08-D040", "314c48e97c509fb6"),
    ("P08-D041", "7cd8f1e8c5ac8bb7"),
    ("P08-D051", "e79509b630d36ec2"),
    ("P08-D055", "a157b6667895e7a5"),
    ("P08-D068", "82ea4eca9401a3db"),
    ("P08-D070", "e908962d45d99b1e"),
    ("P08-D084", "93f59dceaf080f5a"),
    ("P08-D089", "cb70055818341764"),
    ("P08-D117", "91204334fdb33e13"),
    ("P08-D118", "647fff96ba4c8b14"),
    ("P08-D119", "33d32afb9a7a2961"),
    ("P08-D120", "2787335a7f090ddf"),
    ("P08-D121", "e641be716aeef612"),
    ("P08-D122", "870487392c365628"),
    ("P08-D123", "4a73bb72932d1b81"),
    ("P08-D124", "fbae05a7117de2d4"),
    ("P08-D125", "01b96a302853d0e0"),
    ("P08-D126", "532cf5fa2df78d8e"),
    ("P08-D127", "876c52d089c0952c"),
    ("P08-D128", "59a4a2e682a610ff"),
    ("P08-D129", "fda7f600652f2fc2"),
    ("P08-D130", "5a37e6c1562767b1"),
    ("P08-D131", "707119b6b340a92c"),
    ("P08-D132", "28d99f39044843d7"),
    ("P08-D133", "a938b86d563e6606"),
    ("P08-D134", "899ef03ce5987131"),
    ("P08-D135", "b9bb33ff03da696b"),
    ("P08-D136", "dfc476daff222c47"),
    ("P08-D137", "ea3612fdc55920d5"),
    ("P08-D138", "999b982347f35f5b"),
    ("P08-D139", "c36afc02ab26808b"),
    ("plan09:esc-1", "ac84db188dfd4821"),
    ("plan09:esc-2", "6d0ae8703514693a"),
    ("plan09:esc-3", "b8d347d2ebee4204"),
    ("plan09:esc-4", "b9e1fe5754923f78"),
    ("plan09:esc-5", "bba3b7b45136eaad"),
    ("plan09:esc-6", "602f57fc59ebf960"),
    ("plan09:manual-collapse", "da195cb87430e41c"),
    ("plan09:scroll-1", "5cf38711add9eba1"),
    ("plan09:scroll-2", "eae25bd9390174ca"),
    ("plan09:scroll-3", "9c8a6cb0d2f7d709"),
    ("plan09:scroll-4", "3fedde1d3c460be9"),
    ("plan09:scroll-5", "eb917ee6f5b45456"),
    ("plan09:scroll-6", "2fac9349e573b234"),
    ("plan09:scroll-7", "d11282587cc14247"),
    ("plan09:opacity-1", "a6a6078483506833"),
    ("plan09:opacity-2", "eb300362cab34743"),
    ("plan09:opacity-3", "af424a602e1d8223"),
    ("plan09:quit-1", "437b4ce76fd3b1b1"),
    ("plan09:quit-2", "7ade9c382b14821d"),
    ("plan09:quit-3", "039815986c36c8b6"),
];

/// Compares a decomposition with the reviewed one; the error lists every current value.
fn reviewed(sources: &Sources) -> Result<(), String> {
    let sections = sources
        .sections
        .iter()
        .map(|(id, count, digest)| (*id, *count, digest.as_str()))
        .collect::<Vec<_>>();
    let clauses = sources
        .clauses
        .iter()
        .map(|(id, text)| (id.as_str(), digest(text)[..16].to_owned()))
        .collect::<Vec<_>>();
    let expected = CLAUSES
        .iter()
        .map(|(id, prefix)| (*id, (*prefix).to_owned()))
        .collect::<Vec<_>>();
    if sections == SECTIONS && clauses == expected {
        Ok(())
    } else {
        Err(format!(
            "sources changed; review the decomposition, then update:\nSECTIONS {sections:#?}\nCLAUSES {clauses:#?}"
        ))
    }
}

/// A changed source fails here until its decomposition is reviewed and the digests updated.
#[test]
fn the_decomposed_sources_are_unchanged() {
    let (phase08, plan09) = documents();
    decompose(&phase08, &plan09)
        .and_then(|sources| reviewed(&sources))
        .unwrap();
}

/// `text` with `inserted` as a new line after the first line starting with `prefix`.
fn insert_after(text: &str, prefix: &str, inserted: &str) -> String {
    let mut lines = text.lines().collect::<Vec<_>>();
    let index = lines
        .iter()
        .position(|line| line.starts_with(prefix))
        .unwrap_or_else(|| panic!("no line starts with {prefix}"));
    lines.insert(index + 1, inserted);
    lines.join("\n")
}

/// The documents with an anchor and a new clause before `### Dock 与自动隐藏`: the text lands at
/// the end of the opacity section.
fn plan_with_anchor_clause(phase08: &str, plan09: &str) -> (String, String) {
    let mut lines = plan09.lines().collect::<Vec<_>>();
    let dock = lines
        .iter()
        .position(|line| *line == "### Dock 与自动隐藏")
        .unwrap();
    for line in ["- 新的透明度条款。", "", "<a id=\"inserted\"></a>"] {
        lines.insert(dock, line);
    }
    (phase08.to_owned(), lines.join("\n"))
}

/// Edits that leave every extracted clause intact still fail: the raw sections are bound, and
/// content the extractors do not recognize is an error.
#[test]
fn edits_to_the_raw_markdown_are_detected() {
    let (phase08, plan09) = documents();
    // The unedited documents pass, so each failure below is caused by its edit.
    decompose(&phase08, &plan09)
        .and_then(|sources| reviewed(&sources))
        .unwrap();
    let edits = [
        (
            "Esc lead sentence",
            phase08.clone(),
            plan09.replacen("第一个适用", "最后一个适用", 1),
        ),
        (
            "a second quit-failure clause",
            phase08.clone(),
            insert_after(&plan09, "- 退出保存失败", "- 退出保存失败：另一条要求。"),
        ),
        (
            "a new tray clause",
            phase08.clone(),
            insert_after(&plan09, "  因果。", "- 退出后另有要求。"),
        ),
        (
            "a scrollbar sub-heading",
            phase08.clone(),
            insert_after(&plan09, "  DPI/主题下的实际可抓取性", "#### 补充"),
        ),
        (
            "an opacity paragraph",
            phase08.clone(),
            insert_after(&plan09, "- 拖动实时预览", "正文要求：另有条件。"),
        ),
        (
            "a Manual row without outer pipes",
            insert_after(
                &phase08,
                "| P08-M15 |",
                "P08-M16 | new | Manual | receipt | NOT TESTED",
            ),
            plan09.clone(),
        ),
        (
            "a malformed Manual row",
            insert_after(&phase08, "| P08-M15 |", "| P08-M16 | new | Manual |"),
            plan09.clone(),
        ),
        (
            "an emphasized Manual mode",
            insert_after(
                &phase08,
                "| P08-M15 |",
                "P08-M16 | new | **Manual** | receipt | NOT TESTED",
            ),
            plan09.clone(),
        ),
        (
            "an emphasized row ID",
            insert_after(
                &phase08,
                "| P08-M15 |",
                "| **P08-M16** | new | Manual | receipt | NOT TESTED |",
            ),
            plan09.clone(),
        ),
        (
            "the bound sections hidden in an HTML comment",
            phase08.clone(),
            insert_after(
                &insert_after(&plan09, "### 主窗口身份与可达性", "-->"),
                "<a id=\"vertical-scrollbars\"></a>",
                "<!--",
            ),
        ),
        (
            "an anchor followed by a new clause",
            plan_with_anchor_clause(&phase08, &plan09).0,
            plan_with_anchor_clause(&phase08, &plan09).1,
        ),
    ];
    for (name, edited08, edited09) in &edits {
        assert!(
            *edited08 != phase08 || *edited09 != plan09,
            "{name} changed nothing"
        );
        assert!(
            decompose(edited08, edited09)
                .and_then(|sources| reviewed(&sources))
                .is_err(),
            "{name} was not detected"
        );
    }
}

#[test]
fn table_cells_ignore_optional_outer_pipes() {
    assert_eq!(cells("| a | b |"), ["a", "b"]);
    assert_eq!(cells("a | b"), ["a", "b"]);
    assert_eq!(cells("| a | b"), ["a", "b"]);
}
