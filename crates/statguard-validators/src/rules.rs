use polars::prelude::*;
use statguard_core::ast::{
    ComparisonOp, CrossColumnRule, LiteralValue, MetricFn, QualityRule, Severity,
};
use crate::Violation;

pub struct RuleEngine;

impl RuleEngine {
    /// Evaluate aggregate quality metric rules (completeness, uniqueness, …).
    pub fn evaluate(df: &DataFrame, rules: &[QualityRule]) -> Vec<Violation> {
        rules.iter().flat_map(|r| evaluate_rule(df, r)).collect()
    }

    /// Evaluate cross-column conditional assertions:
    ///   assert <col> <op> <value> when <col> <op> <value>
    pub fn evaluate_cross_column(df: &DataFrame, rules: &[CrossColumnRule]) -> Vec<Violation> {
        rules.iter().flat_map(|r| evaluate_cross_column_rule(df, r)).collect()
    }
}

// ── Quality metric rules ──────────────────────────────────────────────────────

fn evaluate_rule(df: &DataFrame, rule: &QualityRule) -> Vec<Violation> {
    let series = match df.column(&rule.column).ok().and_then(|c| c.as_series().cloned()) {
        Some(s) => s,
        None => {
            return vec![Violation::new(
                &rule.column, "quality_check",
                format!("column '{}' not found", rule.column),
                Severity::Blocking,
            )];
        }
    };

    let observed = match compute_metric(&series, &rule.metric) {
        Some(v) => v,
        None => return vec![],
    };

    if !rule.op.evaluate(observed, rule.threshold) {
        vec![Violation::new(
            &rule.column,
            metric_name(&rule.metric),
            format!(
                "quality check failed: {}({}) = {:.4} {} {:.4}",
                metric_name(&rule.metric), rule.column, observed, rule.op, rule.threshold
            ),
            rule.severity.clone(),
        )
        .with_values(observed, rule.threshold)]
    } else {
        vec![]
    }
}

fn compute_metric(series: &Series, metric: &MetricFn) -> Option<f64> {
    let n = series.len() as f64;
    if n == 0.0 {
        return Some(0.0);
    }

    match metric {
        MetricFn::Completeness | MetricFn::NonNullRate => {
            Some(1.0 - series.null_count() as f64 / n)
        }
        MetricFn::Uniqueness => {
            let n_unique = series.n_unique().ok()? as f64;
            Some(n_unique / n)
        }
        MetricFn::Validity => Some((n - series.null_count() as f64) / n),
        MetricFn::Consistency => {
            let n_unique = series.n_unique().ok()? as f64;
            Some(n_unique / n)
        }
        MetricFn::Freshness => Some(1.0 - series.null_count() as f64 / n),
    }
}

fn metric_name(m: &MetricFn) -> &'static str {
    match m {
        MetricFn::Completeness => "completeness",
        MetricFn::Uniqueness   => "uniqueness",
        MetricFn::Validity     => "validity",
        MetricFn::Consistency  => "consistency",
        MetricFn::Freshness    => "freshness",
        MetricFn::NonNullRate  => "non_null_rate",
    }
}

// ── Cross-column conditional assertion rules ──────────────────────────────────

fn evaluate_cross_column_rule(df: &DataFrame, rule: &CrossColumnRule) -> Vec<Violation> {
    // Get both columns upfront
    let when_series = match df.column(&rule.when_column).ok().and_then(|c| c.as_series().cloned()) {
        Some(s) => s,
        None => return vec![Violation::new(
            &rule.assert_column, "cross_column_rule",
            format!("condition column '{}' not found", rule.when_column),
            rule.severity.clone(),
        )],
    };

    let assert_series = match df.column(&rule.assert_column).ok().and_then(|c| c.as_series().cloned()) {
        Some(s) => s,
        None => return vec![Violation::new(
            &rule.assert_column, "cross_column_rule",
            format!("assertion column '{}' not found", rule.assert_column),
            rule.severity.clone(),
        )],
    };

    let n = df.height();
    let mut failing_rows: Vec<usize> = Vec::new();

    for i in 0..n {
        if row_matches(&when_series, i, &rule.when_op, &rule.when_value) {
            if !row_matches(&assert_series, i, &rule.assert_op, &rule.assert_value) {
                failing_rows.push(i);
            }
        }
    }

    if failing_rows.is_empty() {
        return vec![];
    }

    vec![Violation::new(
        &rule.assert_column,
        "cross_column_rule",
        format!(
            "{} row(s) violate: assert {} {} {} when {} {} {}",
            failing_rows.len(),
            rule.assert_column, rule.assert_op, rule.assert_value,
            rule.when_column,   rule.when_op,   rule.when_value,
        ),
        rule.severity.clone(),
    )
    .with_rows(failing_rows)]
}

/// Returns true if the value at `idx` in `series` satisfies `op` against `value`.
/// Returns false for nulls.
fn row_matches(series: &Series, idx: usize, op: &ComparisonOp, value: &LiteralValue) -> bool {
    match value {
        LiteralValue::Number(n) => {
            if let Ok(cast) = series.cast(&DataType::Float64) {
                if let Ok(ca) = cast.f64() {
                    if let Some(v) = ca.get(idx) {
                        return op.evaluate(v, *n);
                    }
                }
            }
            false
        }
        LiteralValue::Str(s) => {
            if let Ok(ca) = series.str() {
                if let Some(v) = ca.get(idx) {
                    return op.evaluate_str(v, s.as_str());
                }
            }
            false
        }
        LiteralValue::Bool(b) => {
            if let Ok(ca) = series.bool() {
                if let Some(v) = ca.get(idx) {
                    return op.evaluate_bool(v, *b);
                }
            }
            false
        }
    }
}
