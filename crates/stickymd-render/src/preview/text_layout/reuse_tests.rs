//! Reuse must preserve per-occurrence behavior and stay within one bounded layout.
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use super::*;
use crate::preview::{LinkKind, SourceRange};

#[derive(Clone, Copy)]
struct Settings {
    width: f32,
    metrics: Metrics,
    align: Align,
    wrap: Wrap,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            width: 120.0,
            metrics: Metrics::new(17.0, 26.35),
            align: Align::Left,
            wrap: Wrap::WordOrGlyph,
        }
    }
}

struct Fixture {
    font_system: FontSystem,
    fonts: FontSelection,
}

impl Fixture {
    fn new() -> Self {
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        Self { font_system, fonts }
    }

    fn layout(
        &mut self,
        spans: &[RenderSpan],
        settings: Settings,
        cache: &mut TextLayoutCache,
        prefix: &str,
    ) -> (TextLayout, String) {
        let mut copied = prefix.to_owned();
        let built = make_text_chunk(
            &mut self.font_system,
            &self.fonts,
            spans,
            0.0,
            0.0,
            settings.width,
            settings.metrics,
            settings.align,
            settings.wrap,
            &mut copied,
            cache,
        );
        let LayoutContent::Text(layout) = built.chunks.into_iter().next().unwrap().content else {
            panic!("text expected")
        };
        (layout, copied)
    }
}

fn span(text: &str, source_start: usize, destination: &str) -> RenderSpan {
    RenderSpan {
        text: Arc::from(text),
        copy_text: Arc::from(text),
        source_range: SourceRange::new(source_start, source_start + text.len()),
        style: RenderStyle {
            link: true,
            ..RenderStyle::default()
        },
        action: Some(SpanAction::OpenLink {
            destination: destination.to_owned(),
            kind: LinkKind::Https,
        }),
        math: None,
        image: None,
        hard_break: false,
    }
}

fn boxes(layout: &TextLayout) -> Vec<PreviewTextBox> {
    project_visible_text_boxes(layout, 0.0, 0.0, 0.0, f32::MAX)
}

#[test]
fn phase5_text_reuse_shares_geometry_without_sharing_source_actions_or_tooltips() {
    let mut fixture = Fixture::new();
    let mut cache = TextLayoutCache::default();
    let text = "中文 e\u{301} 👩‍💻 abc אבג\nwrapped second line";
    let first = span(text, 0, "https://first.example");
    fixture.layout(
        std::slice::from_ref(&first),
        Settings::default(),
        &mut cache,
        "",
    );
    assert!(
        cache.shapes.is_empty(),
        "first occurrence must only record the key"
    );
    let (second, _) = fixture.layout(&[first], Settings::default(), &mut cache, "");
    let third = span(text, 200, "https://third.example");
    let (mut cached, copied) = fixture.layout(
        std::slice::from_ref(&third),
        Settings::default(),
        &mut cache,
        "prefix ",
    );
    let (fresh, expected_copy) = fixture.layout(
        &[third],
        Settings::default(),
        &mut TextLayoutCache::default(),
        "prefix ",
    );
    assert!(Arc::ptr_eq(&second.shaped, &cached.shaped));
    assert_eq!(copied, expected_copy);
    assert_eq!(boxes(&cached), boxes(&fresh));
    assert!(
        boxes(&cached)
            .iter()
            .all(|item| item.source_range == SourceRange::new(200, 200 + text.len()))
    );

    cached.mark_atomic_with_tooltip(Arc::from("only this occurrence"));
    assert!(
        boxes(&cached)
            .iter()
            .all(|item| item.atomic && item.tooltip.as_deref() == Some("only this occurrence"))
    );
    assert!(
        boxes(&second)
            .iter()
            .all(|item| !item.atomic && item.tooltip.is_none())
    );

    let lifetime = Arc::downgrade(&cached.shaped);
    drop(cache);
    assert!(
        lifetime.upgrade().is_some(),
        "live chunks retain their geometry"
    );
    drop(second);
    drop(cached);
    assert!(
        lifetime.upgrade().is_none(),
        "no cache survives the layout's owners"
    );
}

#[test]
fn phase5_text_reuse_keeps_layout_keys_and_attribution_boundaries_distinct() {
    let mut fixture = Fixture::new();
    let mut cache = TextLayoutCache::default();
    let base = Settings::default();
    let original = span("same words 中文", 0, "https://example.com");
    let spans = [original.clone()];
    fixture.layout(&spans, base, &mut cache, "");
    let (shared, _) = fixture.layout(&spans, base, &mut cache, "");
    for settings in [
        Settings {
            width: 70.0,
            ..base
        },
        Settings {
            metrics: Metrics::new(20.0, 26.35),
            ..base
        },
        Settings {
            metrics: Metrics::new(17.0, 40.0),
            ..base
        },
        Settings {
            align: Align::Right,
            ..base
        },
        Settings {
            wrap: Wrap::None,
            ..base
        },
    ] {
        fixture.layout(&spans, settings, &mut cache, "");
        let (changed, _) = fixture.layout(&spans, settings, &mut cache, "");
        let (fresh, _) = fixture.layout(&spans, settings, &mut TextLayoutCache::default(), "");
        assert!(!Arc::ptr_eq(&shared.shaped, &changed.shaped));
        assert_eq!(boxes(&changed), boxes(&fresh));
    }
    let mut bold = original.clone();
    bold.style.strong = true;
    let split = [
        span("same words ", 100, "https://one.example"),
        span("中文", 120, "https://two.example"),
    ];
    for changed in [vec![bold], split.to_vec()] {
        fixture.layout(&changed, base, &mut cache, "");
        let (changed_layout, _) = fixture.layout(&changed, base, &mut cache, "");
        let (fresh, _) = fixture.layout(&changed, base, &mut TextLayoutCache::default(), "");
        assert!(!Arc::ptr_eq(&shared.shaped, &changed_layout.shaped));
        assert_eq!(boxes(&changed_layout), boxes(&fresh));
    }
}

#[test]
fn phase5_text_reuse_keeps_admission_bounded_and_does_not_reuse_another_layout() {
    let mut fixture = Fixture::new();
    let settings = Settings::default();
    let mut cache = TextLayoutCache::default();
    for index in 0..=MAX_TEXT_LAYOUT_CACHE_ENTRIES {
        fixture.layout(
            &[span(&format!("{index}"), 0, "https://example.com")],
            settings,
            &mut cache,
            "",
        );
    }
    assert_eq!(cache.seen_once.len(), MAX_TEXT_LAYOUT_CACHE_ENTRIES);
    assert!(cache.shapes.is_empty());
    let repeated = [span("0", 0, "https://example.com")];
    let (promoted, _) = fixture.layout(&repeated, settings, &mut cache, "");
    let (hit, _) = fixture.layout(&repeated, settings, &mut cache, "");
    assert!(Arc::ptr_eq(&promoted.shaped, &hit.shaped));
    assert_eq!(
        cache.shapes.len() + cache.seen_once.len(),
        MAX_TEXT_LAYOUT_CACHE_ENTRIES
    );
    let (next_layout, _) = fixture.layout(&repeated, settings, &mut TextLayoutCache::default(), "");
    assert!(!Arc::ptr_eq(&hit.shaped, &next_layout.shaped));

    let mut empty_cache = TextLayoutCache::default();
    let oversized = [span(&"界".repeat(342), 0, "https://example.com")];
    for _ in 0..3 {
        fixture.layout(&oversized, settings, &mut empty_cache, "");
    }
    assert!(empty_cache.shapes.is_empty() && empty_cache.seen_once.is_empty());
}
