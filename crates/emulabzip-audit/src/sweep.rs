//! Trying one screen in several languages and text sizes, and saying what gets worse.
//!
//! The device is changed for the run and put back at the end, whatever happens in between.

use std::collections::BTreeMap;
use std::time::Duration;

use emulabzip_core::a11y::{self, Finding};
use emulabzip_core::i18n;
use emulabzip_core::layout::LayoutSnapshot;
use emulabzip_core::{Control, Lab, Platform, Result, Target};
use serde::Serialize;

/// The part of a device a sweep needs, so it can be tried without one.
pub trait Screen {
    fn platform(&self) -> Platform;
    fn control(&self, control: &Control) -> impl std::future::Future<Output = Result<()>>;
    fn layout(&self) -> impl std::future::Future<Output = Result<LayoutSnapshot>>;
}

/// A device reached through [`Lab`].
pub struct LabScreen<'a> {
    pub lab: &'a Lab,
    pub target: &'a Target,
}

impl Screen for LabScreen<'_> {
    fn platform(&self) -> Platform {
        self.target.platform
    }

    async fn control(&self, control: &Control) -> Result<()> {
        self.lab.control(self.target, control).await
    }

    async fn layout(&self) -> Result<LayoutSnapshot> {
        self.lab.layout(self.target).await
    }
}

#[derive(Debug, Clone)]
pub struct Plan {
    /// Language tags to try, besides the device's own.
    pub locales: Vec<String>,
    /// Text sizes to try, in percent of normal.
    pub scales: Vec<u32>,
    /// The app whose language is changed (Android changes one app at a time).
    pub app: Option<String>,
    /// How long to wait after a change for the screen to settle.
    pub settle: Duration,
}

/// The rules a sweep looks at: the text checks, and what text size does to touch targets and focus order.
const A11Y_RULES: &[&str] = &["touch-target", "focus-order"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// `None` is the device as it is now.
    pub locale: Option<String>,
    pub percent: u32,
    pub rtl: bool,
    pub findings: Vec<Finding>,
    pub by_rule: BTreeMap<String, usize>,
    /// Rules with more findings than the first row has, and by how many.
    pub worse: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sweep {
    pub rows: Vec<Row>,
    pub notes: Vec<String>,
}

fn row(snapshot: &LayoutSnapshot, locale: Option<&str>, percent: u32) -> Row {
    let rtl = locale.is_some_and(i18n::is_rtl);
    let mut findings = i18n::check(snapshot, i18n::Options { rtl }).findings;
    findings.extend(a11y::audit(snapshot).findings.into_iter().filter(|f| A11Y_RULES.contains(&f.rule.as_str())));
    let mut by_rule: BTreeMap<String, usize> = BTreeMap::new();
    for f in &findings {
        *by_rule.entry(f.rule.clone()).or_default() += 1;
    }
    Row { locale: locale.map(String::from), percent, rtl, findings, by_rule, worse: Vec::new() }
}

pub async fn run<S: Screen>(screen: &S, plan: &Plan) -> Result<Sweep> {
    let mut sweep = Sweep::default();
    let ios = screen.platform() == Platform::Ios;
    let mut changed_locale = false;
    let mut changed_scale = false;

    let result: Result<()> = async {
        sweep.rows.push(row(&screen.layout().await?, None, 100));
        let mut locales: Vec<Option<&str>> = vec![None];
        for l in &plan.locales {
            if ios {
                continue;
            }
            locales.push(Some(l.as_str()));
        }
        if ios && !plan.locales.is_empty() {
            sweep.notes.push(
                "The iOS Simulator picks up a language when an app starts again, so languages are not swept there: only text sizes are."
                    .into(),
            );
        }
        for locale in locales {
            if let Some(tag) = locale {
                screen.control(&Control::Locale { locale: tag.into(), app: plan.app.clone() }).await?;
                changed_locale = true;
            }
            for &percent in &plan.scales {
                if locale.is_none() && percent == 100 {
                    continue;
                }
                screen.control(&Control::FontScale { percent }).await?;
                changed_scale = true;
                tokio::time::sleep(plan.settle).await;
                sweep.rows.push(row(&screen.layout().await?, locale, percent));
            }
            // A language with no size to try is looked at once, at the size the device is at.
            if locale.is_some() && plan.scales.iter().all(|&p| p == 100) {
                tokio::time::sleep(plan.settle).await;
                sweep.rows.push(row(&screen.layout().await?, locale, 100));
            }
            if changed_scale {
                screen.control(&Control::FontScale { percent: 100 }).await?;
                changed_scale = false;
            }
            if locale.is_some() {
                screen.control(&Control::ResetLocale { app: plan.app.clone() }).await?;
                changed_locale = false;
            }
        }
        Ok(())
    }
    .await;

    // Put the device back, even when a step failed.
    if changed_scale && let Err(e) = screen.control(&Control::FontScale { percent: 100 }).await {
        sweep.notes.push(format!("Could not put the text size back to 100%: {e}"));
    }
    if changed_locale && let Err(e) = screen.control(&Control::ResetLocale { app: plan.app.clone() }).await {
        sweep.notes.push(format!("Could not put the language back: {e}"));
    }
    result?;

    if let Some(first) = sweep.rows.first().map(|r| r.by_rule.clone()) {
        for r in sweep.rows.iter_mut().skip(1) {
            r.worse = r
                .by_rule
                .iter()
                .filter_map(|(rule, n)| {
                    let before = first.get(rule).copied().unwrap_or(0);
                    (*n > before).then(|| (rule.clone(), n - before))
                })
                .collect();
        }
    }
    Ok(sweep)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use emulabzip_core::layout::{Bounds, LayoutSnapshot, Screen as ScreenSize, UiNode};
    use emulabzip_core::{Error, Platform};

    use super::*;

    fn node(class: &str, b: (i32, i32, i32, i32)) -> UiNode {
        UiNode { class: class.into(), bounds: Bounds::new(b.0, b.1, b.2, b.3), enabled: true, ..Default::default() }
    }

    /// A screen whose label spills past the right edge at large text and in `ar-XB`, and that remembers what it was told.
    struct Fake {
        platform: Platform,
        said: RefCell<Vec<Control>>,
        scale: RefCell<u32>,
        locale: RefCell<Option<String>>,
        fail_layout_at: Option<u32>,
    }

    fn fake(platform: Platform) -> Fake {
        Fake {
            platform,
            said: RefCell::new(Vec::new()),
            scale: RefCell::new(100),
            locale: RefCell::new(None),
            fail_layout_at: None,
        }
    }

    impl Screen for Fake {
        fn platform(&self) -> Platform {
            self.platform
        }

        async fn control(&self, control: &Control) -> Result<()> {
            self.said.borrow_mut().push(control.clone());
            match control {
                Control::FontScale { percent } => *self.scale.borrow_mut() = *percent,
                Control::Locale { locale, .. } => *self.locale.borrow_mut() = Some(locale.clone()),
                Control::ResetLocale { .. } => *self.locale.borrow_mut() = None,
                _ => {}
            }
            Ok(())
        }

        async fn layout(&self) -> Result<LayoutSnapshot> {
            let scale = *self.scale.borrow();
            if self.fail_layout_at == Some(scale) {
                return Err(Error::failed("the screen went away"));
            }
            let width = 120 * scale as i32 / 100 + if self.locale.borrow().is_some() { 40 } else { 0 };
            let mut root: UiNode = node("FrameLayout", (0, 0, 400, 400));
            let mut label = node("TextView", (240, 10, width, 30));
            label.text = Some("Order summary".into());
            root.children.push(label);
            Ok(LayoutSnapshot::new(
                self.platform,
                root,
                ScreenSize { width: 400, height: 400, density: 1.0 },
                Vec::new(),
            ))
        }
    }

    fn plan(locales: &[&str], scales: &[u32]) -> Plan {
        Plan {
            locales: locales.iter().map(|s| s.to_string()).collect(),
            scales: scales.to_vec(),
            app: Some("com.example".into()),
            settle: Duration::ZERO,
        }
    }

    fn block<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(f)
    }

    #[test]
    fn a_sweep_tries_each_setting_says_what_got_worse_and_puts_the_device_back() {
        let screen = fake(Platform::Android);
        let sweep = block(run(&screen, &plan(&["ar-XB"], &[100, 130, 200]))).unwrap();
        let names: Vec<_> = sweep.rows.iter().map(|r| (r.locale.as_deref(), r.percent)).collect();
        assert_eq!(
            names,
            [(None, 100), (None, 130), (None, 200), (Some("ar-XB"), 100), (Some("ar-XB"), 130), (Some("ar-XB"), 200)]
        );
        assert!(sweep.rows[0].findings.is_empty(), "the label fits at first");
        assert!(sweep.rows[1].worse.is_empty(), "and at 130%: it ends at 396 of 400");
        // At 200% the label is 240 wide from x = 240: past the 400 edge.
        assert_eq!(sweep.rows[2].worse, [("text-off-screen".to_string(), 1)]);
        // In ar-XB the label is 40 wider: it still fits at 100%, and not at 130%.
        assert!(sweep.rows[3].rtl && sweep.rows[3].worse.is_empty(), "{:?}", sweep.rows[3]);
        assert_eq!(sweep.rows[4].worse, [("text-off-screen".to_string(), 1)]);
        // Back to normal.
        assert_eq!(*screen.scale.borrow(), 100);
        assert_eq!(*screen.locale.borrow(), None);
    }

    #[test]
    fn a_failure_in_the_middle_still_puts_the_device_back() {
        let mut screen = fake(Platform::Android);
        screen.fail_layout_at = Some(200);
        let err = block(run(&screen, &plan(&[], &[130, 200]))).unwrap_err().to_string();
        assert!(err.contains("went away"));
        assert_eq!(*screen.scale.borrow(), 100, "the text size was put back");
        let mut screen = fake(Platform::Android);
        screen.fail_layout_at = Some(150);
        let _ = block(run(&screen, &plan(&["de"], &[150])));
        assert_eq!(*screen.locale.borrow(), None, "the language was put back");
    }

    #[test]
    fn ios_sweeps_text_sizes_only_and_says_why() {
        let screen = fake(Platform::Ios);
        let sweep = block(run(&screen, &plan(&["de", "ar-XB"], &[130]))).unwrap();
        assert!(sweep.rows.iter().all(|r| r.locale.is_none()));
        assert_eq!(sweep.rows.len(), 2);
        assert!(sweep.notes[0].contains("starts again"));
        assert!(screen.said.borrow().iter().all(|c| !matches!(c, Control::Locale { .. })));
    }
}
