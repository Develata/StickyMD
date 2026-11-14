//! Finds the glyph whose position overflows `SubpixelBin::new` in the two failing render tests.
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, Wrap};

fn scan_cost_and_outline_census() {
    let mut samples = Vec::new();
    for _ in 0..5 {
        let start = std::time::Instant::now();
        let fonts = FontSystem::new();
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(fonts);
    }
    samples.sort_by(f64::total_cmp);
    println!("FontSystem::new median {:.1} ms max {:.1} ms", samples[2], samples[4]);
    let fonts = FontSystem::new();
    let ids: Vec<_> = fonts.db().faces().map(|f| (f.id, f.post_script_name.clone())).collect();
    let start = std::time::Instant::now();
    let mut without_head = Vec::new();
    for (id, name) in &ids {
        let has_head = fonts
            .db()
            .with_face_data(*id, |data, index| {
                let be32 = |at: usize| u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
                let start = if &data[..4] == b"ttcf" { be32(12 + 4 * index as usize) } else { 0 };
                let tables = u16::from_be_bytes(data[start + 4..start + 6].try_into().unwrap()) as usize;
                (0..tables).any(|table| &data[start + 12 + 16 * table..start + 16 + 16 * table] == b"head")
            })
            .unwrap_or(false);
        if !has_head {
            without_head.push(name.clone());
        }
    }
    println!(
        "faces={} without head table={without_head:?}, census took {:.1} ms",
        ids.len(),
        start.elapsed().as_secs_f64() * 1000.0
    );
}

fn main() {
    scan_cost_and_outline_census();
    let mut fonts = FontSystem::new();
    let text = format!(
        "{} a{} 中🙂👩‍💻 עברית العربية italic {}",
        "mixed ".repeat(15),
        "\u{301}".repeat(60),
        "end ".repeat(20)
    );
    for (name, attrs) in [
        ("monospace italic", Attrs::new().family(Family::Monospace).style(Style::Italic)),
        ("monospace", Attrs::new().family(Family::Monospace)),
    ] {
        let mut buffer = Buffer::new_empty(Metrics::new(17.0, 26.0));
        buffer.set_wrap(Wrap::None);
        buffer.set_text(&text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut fonts, false);
        let mut bad = 0;
        let mut total = 0;
        let mut used = std::collections::BTreeSet::new();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                total += 1;
                let family = fonts
                    .db()
                    .face(glyph.font_id)
                    .map(|f| f.post_script_name.clone())
                    .unwrap_or_default();
                used.insert(family.clone());
                if !(glyph.x.is_finite() && glyph.w.is_finite() && glyph.x_offset.is_finite() && glyph.x.abs() < 1e7) {
                    bad += 1;
                    if bad <= 8 {
                        println!(
                            "BAD[{name}] start={} ch={:?} id={} font={family} x={} w={} x_off={} y_off={} size={} ",
                            glyph.start,
                            &text[glyph.start..glyph.end],
                            glyph.glyph_id, glyph.x, glyph.w, glyph.x_offset, glyph.y_offset, glyph.font_size
                        );
                    }
                }
            }
        }
        println!("[{name}] glyphs={total} bad={bad} fonts={used:?}");
    }
    // Which face is it, and does the layout stay finite without it?
    let bitmap: Vec<_> = fonts
        .db()
        .faces()
        .filter(|f| f.post_script_name.contains("GB18030"))
        .map(|f| (f.id, f.families.clone(), f.monospaced))
        .collect();
    println!("bitmap faces: {bitmap:?}");
    let mut db = fonts.db().clone();
    for (id, ..) in &bitmap {
        db.remove_face(*id);
    }
    let mut fonts = FontSystem::new_with_locale_and_db("zh-CN".into(), db);
    let mut buffer = Buffer::new_empty(Metrics::new(17.0, 26.0));
    buffer.set_wrap(Wrap::None);
    buffer.set_text(&text, &Attrs::new().family(Family::Monospace), Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut fonts, false);
    let glyphs: Vec<_> = buffer.layout_runs().flat_map(|r| r.glyphs.iter().cloned()).collect();
    let bad = glyphs
        .iter()
        .filter(|g| !(g.x.is_finite() && g.w.is_finite() && g.x_offset.is_finite()))
        .count();
    let han = glyphs.iter().find(|g| &text[g.start..g.end] == "中").unwrap();
    println!(
        "without bitmap faces: glyphs={} bad={bad}; 中 -> {} w={}",
        glyphs.len(),
        fonts.db().face(han.font_id).unwrap().post_script_name,
        han.w
    );
    // Candidate default fonts: what this machine's font database really offers.
    let wanted = [
        "Times New Roman", "Georgia", "Songti SC", "STSong", "STFangsong", "FangSong", "仿宋",
        "仿宋_GB2312", "Hiragino Sans GB", "PingFang SC", "Heiti SC", "Kaiti SC", "Menlo",
        "SF Mono", "Monaco", "Courier New", "Consolas",
    ];
    for name in wanted {
        let hit = fonts.db().faces().filter(|f| f.families.iter().any(|(n, _)| n == name)).count();
        println!("family {name:?}: {hit} faces");
    }
    println!("total faces: {}", fonts.db().faces().count());
}
