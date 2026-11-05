//! Comparing a run with an earlier one, to see what got slower.

use serde::Serialize;

use crate::budget::{label, metrics, number};
use crate::summary::Summary;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub metric: String,
    pub unit: String,
    pub before: f64,
    pub after: f64,
    /// How much it moved, in percent of the baseline. `None` when the baseline was zero.
    pub percent: Option<f64>,
    /// It got worse by more than the tolerance (every number here is one where less is better).
    pub regression: bool,
}

/// Lines up the numbers both runs have, and says which got worse by more than `tolerance_percent`.
/// A number that was zero in the baseline and is not now is always a regression: there is no percentage
/// to be tolerant about.
pub fn compare(baseline: &Summary, run: &Summary, tolerance_percent: f64) -> Vec<Change> {
    let new = metrics(run);
    metrics(baseline)
        .into_iter()
        .filter_map(|(name, before, unit)| {
            let before = before?;
            let after = new.iter().find(|m| m.0 == name)?.1?;
            let percent = (before > 0.0).then(|| (after - before) / before * 100.0);
            let regression = match percent {
                Some(p) => p > tolerance_percent,
                None => after > 0.0,
            };
            Some(Change { metric: name.into(), unit: unit.into(), before, after, percent, regression })
        })
        .collect()
}

pub fn regressions(changes: &[Change]) -> Vec<&Change> {
    changes.iter().filter(|c| c.regression).collect()
}

fn arrow(c: &Change) -> &'static str {
    if c.regression {
        "❌ worse"
    } else if c.after < c.before {
        "✅ better"
    } else {
        "same"
    }
}

/// A Markdown table of every number both runs have.
pub fn markdown(changes: &[Change], tolerance_percent: f64) -> String {
    if changes.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "\n### Compared with the baseline\n\nA number counts as worse when it grew by more than {tolerance_percent:.0}%.\n\n| | Baseline | Now | Change | |\n|---|---|---|---|---|\n"
    );
    for c in changes {
        let change = c
            .percent
            .map(|p| format!("{p:+.0}%"))
            .unwrap_or_else(|| if c.after > 0.0 { "new".into() } else { "—".into() });
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            label(&c.metric),
            number(c.before, &c.unit),
            number(c.after, &c.unit),
            change,
            arrow(c)
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::tests::summary;
    use emulabzip_core::{StartKind, StartupReport, StartupRun};

    fn with_start(ms: u64) -> Summary {
        let mut s = summary();
        s.startup = StartupReport::new(
            "com.example",
            StartKind::Cold,
            vec![StartupRun { total_ms: ms, wait_ms: None, launch_state: None }],
        );
        s
    }

    #[test]
    fn a_number_that_grew_past_the_tolerance_is_a_regression() {
        let changes = compare(&with_start(700), &with_start(800), 10.0);
        let start = changes.iter().find(|c| c.metric == "start_time_ms").unwrap();
        assert!(start.regression);
        assert!((start.percent.unwrap() - 14.2857).abs() < 0.01);
        // 5 percent worse is within a tolerance of 10.
        assert!(!compare(&with_start(700), &with_start(735), 10.0).iter().any(|c| c.regression));
    }

    #[test]
    fn a_number_that_shrank_is_better() {
        let changes = compare(&with_start(700), &with_start(500), 10.0);
        let start = changes.iter().find(|c| c.metric == "start_time_ms").unwrap();
        assert!(!start.regression && start.percent.unwrap() < 0.0);
        assert!(markdown(&changes, 10.0).contains("✅ better"));
    }

    #[test]
    fn something_that_was_zero_and_is_not_is_a_regression() {
        let mut worse = summary();
        worse.crashes.push(crate::summary::CrashLine {
            id: "c".into(),
            kind: "Crash".into(),
            title: "boom".into(),
            app: None,
        });
        let changes = compare(&summary(), &worse, 10.0);
        let crashes = changes.iter().find(|c| c.metric == "crashes").unwrap();
        assert!(crashes.regression && crashes.percent.is_none());
        assert!(markdown(&changes, 10.0).contains("new"));
    }

    #[test]
    fn only_numbers_both_runs_have_are_compared() {
        let mut no_start = summary();
        no_start.startup = None;
        let changes = compare(&summary(), &no_start, 10.0);
        assert!(!changes.iter().any(|c| c.metric == "start_time_ms"));
        assert!(changes.iter().any(|c| c.metric == "log_errors"));
        assert_eq!(regressions(&changes).len(), 0);
    }
}
