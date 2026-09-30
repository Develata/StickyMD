//! Shared resource scenarios, cohort coverage and per-invocation deduplication.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

mod coverage;
pub(crate) mod diagnostic;
mod observations;
pub(crate) mod progress;
#[cfg(test)]
pub(crate) mod tests;

use crate::cli::ResourceModule;
#[cfg(windows)]
pub(crate) use coverage::cohort_coverage;
#[cfg(test)]
use coverage::coverage_measurements;
pub(crate) use coverage::validate_receipt;
#[cfg(test)]
use observations::required_gates;

pub(crate) const REPETITIONS: usize = 5;
pub(crate) const WARMUP_SECONDS: u64 = 30;
pub(crate) const CPU_SECONDS: u64 = 60;
pub(crate) const ZOOM_WARMUP_SECONDS: u64 = 5;
pub(crate) const IDLE_CPU_PERCENT_LIMIT: f64 = 0.1;
pub(crate) const HIDDEN_PRIVATE_WORKING_SET_LIMIT: u64 = 36 * 1024 * 1024;
pub(crate) const ZOOM_PRIVATE_WORKING_SET_LIMIT: u64 = 64 * 1024 * 1024;
pub(crate) const ZOOM_PRIVATE_GROWTH_LIMIT: u64 = 8 * 1024 * 1024;
pub(crate) const GROUPS: [ResourceModule; 5] = [
    ResourceModule::SourcePreview,
    ResourceModule::Math,
    ResourceModule::Images,
    ResourceModule::Window,
    ResourceModule::Zoom,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImageResourceFixture {
    None,
    FourK,
    SaturatedCache,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Transition {
    Fresh,
    PreviewToSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResourceCase {
    pub(crate) label: &'static str,
    pub(crate) view_mode: &'static str,
    pub(crate) formula_count: usize,
    pub(crate) image_count: usize,
    pub(crate) image_fixture: ImageResourceFixture,
    pub(crate) measure_cpu: bool,
    pub(crate) transition: Transition,
}

const fn case(
    label: &'static str,
    view: &'static str,
    math: usize,
    images: usize,
    cpu: bool,
) -> ResourceCase {
    ResourceCase {
        label,
        view_mode: view,
        formula_count: math,
        image_count: images,
        image_fixture: ImageResourceFixture::None,
        measure_cpu: cpu,
        transition: Transition::Fresh,
    }
}

impl ResourceCase {
    const fn image(mut self, fixture: ImageResourceFixture) -> Self {
        self.image_fixture = fixture;
        self
    }
    const fn release(mut self) -> Self {
        self.transition = Transition::PreviewToSource;
        self
    }
    pub(crate) fn equivalent(self, other: Self) -> bool {
        Self {
            label: other.label,
            ..self
        } == other
    }
    pub(crate) fn minimum_wait_seconds(self) -> u64 {
        REPETITIONS as u64
            * (WARMUP_SECONDS
                + if self.measure_cpu { CPU_SECONDS } else { 0 }
                + if self.transition == Transition::PreviewToSource {
                    5
                } else {
                    0
                })
    }
}

const SOURCE: &[ResourceCase] = &[
    case("source", "source", 20, 0, true),
    case("preview", "preview", 20, 0, true),
    case("split", "split", 20, 0, true),
];
const MATH: &[ResourceCase] = &[
    case("source-20-math-lazy", "source", 20, 0, true),
    case("preview-no-math", "preview", 0, 0, false),
    case("preview-1-math", "preview", 1, 0, false),
    case("preview-20-math", "preview", 20, 0, true),
    case("split-20-math", "split", 20, 0, true),
    case("preview-200-unique", "preview", 200, 0, false),
];
const IMAGES: &[ResourceCase] = &[
    case("source-no-images", "source", 0, 0, false),
    case("source-12-images-lazy", "source", 0, 12, true),
    case("preview-no-images", "preview", 0, 0, false),
    case("preview-1-image", "preview", 0, 1, false),
    case("preview-12-images", "preview", 0, 12, true),
    case("split-12-images", "split", 0, 12, true),
    case("preview-4k-image", "preview", 0, 0, false).image(ImageResourceFixture::FourK),
    case("preview-image-cache-saturated", "preview", 0, 0, true)
        .image(ImageResourceFixture::SaturatedCache),
    case("split-image-cache-saturated", "split", 0, 0, true)
        .image(ImageResourceFixture::SaturatedCache),
    case("source-after-preview-cache-release", "preview", 0, 0, true)
        .image(ImageResourceFixture::SaturatedCache)
        .release(),
];

impl ResourceModule {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::SourcePreview => "source-preview",
            Self::Math => "math",
            Self::Images => "images",
            Self::Window => "window",
            Self::Zoom => "zoom",
        }
    }
    pub(crate) const fn ledger_id(self) -> &'static str {
        match self {
            Self::SourcePreview => "resources-source-preview",
            Self::Math => "resources-math",
            Self::Images => "resources-images",
            Self::Window => "resources-window",
            Self::Zoom => "resources-zoom",
        }
    }
    pub(crate) const fn receipt(self) -> &'static str {
        match self {
            Self::SourcePreview => "dist/evidence/resources/source-preview.json",
            Self::Math => "dist/evidence/resources/math.json",
            Self::Images => "dist/evidence/resources/images.json",
            Self::Window => "dist/evidence/resources/window.json",
            Self::Zoom => "dist/evidence/resources/zoom.json",
        }
    }
    pub(crate) const fn task_label(self) -> &'static str {
        match self {
            Self::SourcePreview => "copied Release Source/Preview/Split resource measurement",
            Self::Math => "copied Release Phase 6 math resource matrix",
            Self::Images => "copied Release Phase 7 image resource matrix",
            Self::Window => "copied Release Phase 8 hidden-window resource matrix",
            Self::Zoom => "copied Release Phase 10 zoom resource matrix",
        }
    }
    pub(crate) const fn cases(self) -> &'static [ResourceCase] {
        match self {
            Self::SourcePreview => SOURCE,
            Self::Math => MATH,
            Self::Images => IMAGES,
            _ => &[],
        }
    }
    pub(crate) fn cohorts(self) -> Vec<(&'static str, bool, u64)> {
        match self {
            Self::Window => ["visible-source", "docked-collapsed", "hidden-to-tray"]
                .into_iter()
                .map(|name| (name, true, WARMUP_SECONDS))
                .collect(),
            Self::Zoom => ["split-zoom-50", "split-zoom-100", "split-zoom-300"]
                .into_iter()
                .map(|name| (name, false, ZOOM_WARMUP_SECONDS))
                .collect(),
            _ => self
                .cases()
                .iter()
                .map(|case| (case.label, case.measure_cpu, WARMUP_SECONDS))
                .collect(),
        }
    }
}

/// One command, one candidate and one sampling protocol. Failed executions never enter this cache.
#[cfg(any(windows, test))]
pub(crate) struct ScenarioCache<T> {
    entries: Vec<(ResourceCase, T)>,
}
#[cfg(any(windows, test))]
impl<T> Default for ScenarioCache<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}
#[cfg(any(windows, test))]
impl<T: Clone> ScenarioCache<T> {
    pub(crate) fn measure(
        &mut self,
        case: ResourceCase,
        run: impl FnOnce() -> Result<T, String>,
    ) -> Result<(T, Option<&'static str>), String> {
        if let Some((origin, value)) = self
            .entries
            .iter()
            .find(|(origin, _)| origin.equivalent(case))
        {
            return Ok((value.clone(), Some(origin.label)));
        }
        let value = run()?;
        self.entries.push((case, value.clone()));
        Ok((value, None))
    }
}

pub(crate) fn minimum_wait_seconds(
    groups: &[ResourceModule],
    deduplicate: bool,
    filter: Option<&str>,
) -> u64 {
    let mut seen: Vec<ResourceCase> = Vec::new();
    let mut seconds = 0;
    for group in groups {
        if group.cases().is_empty() {
            seconds += group
                .cohorts()
                .iter()
                .map(|(_, cpu, warmup)| {
                    REPETITIONS as u64 * (warmup + if *cpu { CPU_SECONDS } else { 0 })
                })
                .sum::<u64>();
        }
        for &case in group.cases() {
            if filter.is_some_and(|filter| !filter.is_empty() && case.label != filter) {
                continue;
            }
            if !deduplicate || !seen.iter().any(|other| other.equivalent(case)) {
                seconds += case.minimum_wait_seconds();
                seen.push(case);
            }
        }
    }
    seconds
}
