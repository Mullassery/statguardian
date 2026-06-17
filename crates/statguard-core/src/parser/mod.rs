use pest::Parser;
use pest_derive::Parser;
use indexmap::IndexMap;

use crate::ast::*;
use crate::error::{CoreError, CoreResult};

#[derive(Parser)]
#[grammar = "src/parser/grammar.pest"]
pub struct ContractParser;

pub fn parse(input: &str) -> CoreResult<Vec<DataContract>> {
    let pairs = ContractParser::parse(Rule::contract, input)
        .map_err(|e| CoreError::Parse(Box::new(e)))?;

    let mut contracts = Vec::new();

    for pair in pairs {
        for inner in pair.into_inner() {
            if inner.as_rule() == Rule::dataset_def {
                contracts.push(parse_dataset(inner)?);
            }
        }
    }

    Ok(contracts)
}

fn parse_dataset(pair: pest::iterators::Pair<Rule>) -> CoreResult<DataContract> {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let mut contract = DataContract::new(name);

    for section in inner {
        match section.as_rule() {
            Rule::section => {
                let inner = section.into_inner().next().unwrap();
                match inner.as_rule() {
                    Rule::schema_section => contract.schema = parse_schema(inner)?,
                    Rule::quality_section => {
                        let (qr, ccr) = parse_quality(inner)?;
                        contract.quality_rules = qr;
                        contract.cross_column_rules = ccr;
                    }
                    Rule::stats_section   => contract.stats_rules   = parse_stats(inner)?,
                    Rule::anomaly_section => contract.anomaly_rules  = parse_anomalies(inner)?,
                    Rule::stream_section  => contract.stream_config  = Some(parse_stream(inner)?),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(contract)
}

// ── Schema ────────────────────────────────────────────────────────────────────

fn parse_schema(pair: pest::iterators::Pair<Rule>) -> CoreResult<Vec<FieldDef>> {
    let mut fields = Vec::new();
    for field in pair.into_inner() {
        if field.as_rule() == Rule::field_def {
            fields.push(parse_field(field)?);
        }
    }
    Ok(fields)
}

fn parse_field(pair: pest::iterators::Pair<Rule>) -> CoreResult<FieldDef> {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let data_type = parse_data_type(inner.next().unwrap())?;
    let mut constraints = Vec::new();
    for c in inner {
        if c.as_rule() == Rule::constraint {
            constraints.push(parse_constraint(c)?);
        }
    }
    Ok(FieldDef { name, data_type, constraints })
}

fn parse_data_type(pair: pest::iterators::Pair<Rule>) -> CoreResult<DataType> {
    Ok(match pair.as_str() {
        "int"      => DataType::Int,
        "float"    => DataType::Float,
        "string"   => DataType::String,
        "bool"     => DataType::Bool,
        "date"     => DataType::Date,
        "datetime" => DataType::Datetime,
        "bytes"    => DataType::Bytes,
        other      => return Err(CoreError::Unsupported(format!("unknown type: {other}"))),
    })
}

fn parse_constraint(pair: pest::iterators::Pair<Rule>) -> CoreResult<Constraint> {
    let inner = pair.into_inner().next().unwrap();
    Ok(match inner.as_rule() {
        Rule::not_null          => Constraint::NotNull,
        Rule::unique            => Constraint::Unique,
        Rule::positive          => Constraint::Positive,
        Rule::negative          => Constraint::Negative,
        Rule::primary_key       => Constraint::PrimaryKey,
        Rule::coerce_constraint => Constraint::Coerce,
        Rule::regex_constraint  => {
            let s = inner.into_inner().next().unwrap().as_str();
            Constraint::Regex { pattern: s.to_string() }
        }
        Rule::between_constraint => {
            let mut nums = inner.into_inner();
            let min = nums.next().unwrap().as_str().parse::<f64>().unwrap();
            let max = nums.next().unwrap().as_str().parse::<f64>().unwrap();
            Constraint::Between { min, max }
        }
        Rule::min_constraint => {
            let v = inner.into_inner().next().unwrap().as_str().parse::<f64>().unwrap();
            Constraint::Min { value: v }
        }
        Rule::max_constraint => {
            let v = inner.into_inner().next().unwrap().as_str().parse::<f64>().unwrap();
            Constraint::Max { value: v }
        }
        Rule::len_constraint => {
            let mut nums = inner.into_inner();
            let min = nums.next().unwrap().as_str().parse::<f64>().unwrap() as usize;
            let max = nums.next().unwrap().as_str().parse::<f64>().unwrap() as usize;
            Constraint::Len { min, max }
        }
        Rule::enum_constraint => {
            let values = inner.into_inner().map(|p| p.as_str().to_string()).collect();
            Constraint::Enum { values }
        }
        Rule::foreign_key => {
            let mut parts = inner.into_inner();
            let table  = parts.next().unwrap().as_str().to_string();
            let column = parts.next().unwrap().as_str().to_string();
            Constraint::ForeignKey { table, column }
        }
        other => return Err(CoreError::Unsupported(format!("unknown constraint: {other:?}"))),
    })
}

// ── Quality ───────────────────────────────────────────────────────────────────

fn parse_quality(
    pair: pest::iterators::Pair<Rule>,
) -> CoreResult<(Vec<QualityRule>, Vec<CrossColumnRule>)> {
    let mut quality_rules = Vec::new();
    let mut cross_column_rules = Vec::new();
    for r in pair.into_inner() {
        match r.as_rule() {
            Rule::quality_rule      => quality_rules.push(parse_quality_rule(r)?),
            Rule::cross_column_rule => cross_column_rules.push(parse_cross_column_rule(r)?),
            _ => {}
        }
    }
    Ok((quality_rules, cross_column_rules))
}

fn parse_quality_rule(pair: pest::iterators::Pair<Rule>) -> CoreResult<QualityRule> {
    let mut inner = pair.into_inner();

    let first = inner.next().unwrap();
    let (severity, metric_pair) = if first.as_rule() == Rule::severity_prefix {
        let sev = parse_severity(first)?;
        (sev, inner.next().unwrap())
    } else {
        (Severity::default(), first)
    };

    let mut metric_inner = metric_pair.into_inner();
    let metric = parse_metric_fn(metric_inner.next().unwrap())?;
    let column = metric_inner.next().unwrap().as_str().to_string();

    let op = parse_comparison_op(inner.next().unwrap())?;
    let threshold = inner.next().unwrap().as_str().parse::<f64>().unwrap();

    Ok(QualityRule { metric, column, op, threshold, severity })
}

fn parse_cross_column_rule(pair: pest::iterators::Pair<Rule>) -> CoreResult<CrossColumnRule> {
    let mut inner = pair.into_inner();

    let first = inner.next().unwrap();
    let (severity, assert_col_pair) = if first.as_rule() == Rule::severity_prefix {
        (parse_severity(first)?, inner.next().unwrap())
    } else {
        (Severity::default(), first)
    };

    let assert_column = assert_col_pair.as_str().to_string();
    let assert_op     = parse_comparison_op(inner.next().unwrap())?;
    let assert_value  = parse_literal_value(inner.next().unwrap())?;
    let when_column   = inner.next().unwrap().as_str().to_string();
    let when_op       = parse_comparison_op(inner.next().unwrap())?;
    let when_value    = parse_literal_value(inner.next().unwrap())?;

    Ok(CrossColumnRule {
        assert_column,
        assert_op,
        assert_value,
        when_column,
        when_op,
        when_value,
        severity,
    })
}

fn parse_literal_value(pair: pest::iterators::Pair<Rule>) -> CoreResult<LiteralValue> {
    let inner = pair.into_inner().next().unwrap();
    Ok(match inner.as_rule() {
        Rule::number         => LiteralValue::Number(inner.as_str().parse::<f64>().unwrap()),
        Rule::string_literal => LiteralValue::Str(inner.as_str().to_string()),
        Rule::boolean        => LiteralValue::Bool(inner.as_str() == "true"),
        other                => return Err(CoreError::Unsupported(format!("unknown literal: {other:?}"))),
    })
}

fn parse_metric_fn(pair: pest::iterators::Pair<Rule>) -> CoreResult<MetricFn> {
    Ok(match pair.as_str() {
        "completeness"  => MetricFn::Completeness,
        "uniqueness"    => MetricFn::Uniqueness,
        "validity"      => MetricFn::Validity,
        "consistency"   => MetricFn::Consistency,
        "freshness"     => MetricFn::Freshness,
        "non_null_rate" => MetricFn::NonNullRate,
        other           => return Err(CoreError::Unsupported(format!("unknown metric: {other}"))),
    })
}

fn parse_comparison_op(pair: pest::iterators::Pair<Rule>) -> CoreResult<ComparisonOp> {
    Ok(match pair.as_str() {
        ">"  => ComparisonOp::Gt,
        "<"  => ComparisonOp::Lt,
        ">=" => ComparisonOp::Gte,
        "<=" => ComparisonOp::Lte,
        "==" => ComparisonOp::Eq,
        "!=" => ComparisonOp::Neq,
        other => return Err(CoreError::Unsupported(format!("unknown op: {other}"))),
    })
}

fn parse_severity(pair: pest::iterators::Pair<Rule>) -> CoreResult<Severity> {
    let level = pair.into_inner().next().unwrap().as_str();
    Ok(match level {
        "info"     => Severity::Info,
        "warning"  => Severity::Warning,
        "error"    => Severity::Error,
        "blocking" => Severity::Blocking,
        other      => return Err(CoreError::Unsupported(format!("unknown severity: {other}"))),
    })
}

// ── Stats ─────────────────────────────────────────────────────────────────────

fn parse_stats(pair: pest::iterators::Pair<Rule>) -> CoreResult<Vec<StatsRule>> {
    let mut rules = Vec::new();
    for r in pair.into_inner() {
        if r.as_rule() == Rule::stats_rule {
            rules.push(parse_stats_rule(r)?);
        }
    }
    Ok(rules)
}

fn parse_stats_rule(pair: pest::iterators::Pair<Rule>) -> CoreResult<StatsRule> {
    let mut inner = pair.into_inner();

    let first = inner.next().unwrap();
    let (severity, col_pair) = if first.as_rule() == Rule::severity_prefix {
        (parse_severity(first)?, inner.next().unwrap())
    } else {
        (Severity::default(), first)
    };

    let column    = col_pair.as_str().to_string();
    let stat      = parse_stat_fn(inner.next().unwrap())?;
    let op        = parse_comparison_op(inner.next().unwrap())?;
    let threshold = inner.next().unwrap().as_str().parse::<f64>().unwrap();

    Ok(StatsRule { column, stat, op, threshold, severity })
}

fn parse_stat_fn(pair: pest::iterators::Pair<Rule>) -> CoreResult<StatFn> {
    Ok(match pair.as_str() {
        "mean"   => StatFn::Mean,
        "std"    => StatFn::Std,
        "median" => StatFn::Median,
        "min"    => StatFn::Min,
        "max"    => StatFn::Max,
        "p05"    => StatFn::P05,
        "p95"    => StatFn::P95,
        "p99"    => StatFn::P99,
        "p999"   => StatFn::P999,
        other    => return Err(CoreError::Unsupported(format!("unknown stat fn: {other}"))),
    })
}

// ── Anomalies ─────────────────────────────────────────────────────────────────

fn parse_anomalies(pair: pest::iterators::Pair<Rule>) -> CoreResult<Vec<AnomalyRule>> {
    let mut rules = Vec::new();
    for r in pair.into_inner() {
        if r.as_rule() == Rule::anomaly_rule {
            rules.push(parse_anomaly_rule(r)?);
        }
    }
    Ok(rules)
}

fn parse_anomaly_rule(pair: pest::iterators::Pair<Rule>) -> CoreResult<AnomalyRule> {
    let mut inner = pair.into_inner();

    let first = inner.next().unwrap();
    let (severity, fn_pair) = if first.as_rule() == Rule::severity_prefix {
        (parse_severity(first)?, inner.next().unwrap())
    } else {
        (Severity::default(), first)
    };

    let function = parse_anomaly_fn(fn_pair)?;
    let column   = inner.next().unwrap().as_str().to_string();

    let mut args = IndexMap::new();
    for arg in inner {
        if arg.as_rule() == Rule::named_arg {
            let mut parts = arg.into_inner();
            let k = parts.next().unwrap().as_str().to_string();
            let v = parts.next().unwrap().as_str().to_string();
            args.insert(k, v);
        }
    }

    Ok(AnomalyRule { function, column, args, severity })
}

fn parse_anomaly_fn(pair: pest::iterators::Pair<Rule>) -> CoreResult<AnomalyFn> {
    Ok(match pair.as_str() {
        "detect_outliers"              => AnomalyFn::DetectOutliers,
        "detect_nulls"                 => AnomalyFn::DetectNulls,
        "detect_duplicates"            => AnomalyFn::DetectDuplicates,
        "detect_pattern_breaks"        => AnomalyFn::DetectPatternBreaks,
        "detect_cardinality_explosion" => AnomalyFn::DetectCardinalityExplosion,
        other => return Err(CoreError::Unsupported(format!("unknown anomaly fn: {other}"))),
    })
}

// ── Streaming ─────────────────────────────────────────────────────────────────

fn parse_stream(pair: pest::iterators::Pair<Rule>) -> CoreResult<StreamConfig> {
    let mut cfg = StreamConfig::default();
    for opt in pair.into_inner() {
        match opt.as_rule() {
            Rule::stream_window    => cfg.window    = Some(opt.into_inner().next().unwrap().as_str().to_string()),
            Rule::stream_watermark => cfg.watermark = Some(opt.into_inner().next().unwrap().as_str().to_string()),
            Rule::stream_emit      => cfg.emit      = Some(opt.into_inner().next().unwrap().as_str().to_string()),
            _ => {}
        }
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DSL: &str = r#"
dataset users {
    schema {
        id: int, not_null, unique, primary_key
        email: string, regex="^[^@]+@[^@]+\.[^@]+$"
        age: int, between(0, 120)
        country: string, not_null
        score: float, min=0.0, max=1.0
    }
    quality {
        completeness(id) > 0.99
        @warning: uniqueness(email) == 1.0
    }
    stats {
        age.mean drift < 0.1
        age.std drift < 0.2
    }
    anomalies {
        detect_outliers(age, method="iqr")
        @blocking: detect_duplicates(id)
    }
}
"#;

    const CROSS_COL_DSL: &str = r#"
dataset orders {
    schema {
        order_id: string, not_null
        amount:   float,  not_null
        status:   string, not_null, enum=["pending","paid","cancelled"]
        discount: float
    }
    quality {
        completeness(order_id) > 0.999
        @blocking: assert amount > 0.0 when status == "paid"
        @warning:  assert discount >= 0.0 when status == "paid"
        assert amount < 10000.0 when status != "cancelled"
    }
}
"#;

    #[test]
    fn test_parse_full_contract() {
        let contracts = parse(SAMPLE_DSL).expect("parse failed");
        assert_eq!(contracts.len(), 1);
        let c = &contracts[0];
        assert_eq!(c.name, "users");
        assert_eq!(c.schema.len(), 5);
        assert_eq!(c.quality_rules.len(), 2);
        assert_eq!(c.cross_column_rules.len(), 0);
        assert_eq!(c.stats_rules.len(), 2);
        assert_eq!(c.anomaly_rules.len(), 2);
    }

    #[test]
    fn test_parse_cross_column_rules() {
        let contracts = parse(CROSS_COL_DSL).expect("parse failed");
        let c = &contracts[0];
        assert_eq!(c.quality_rules.len(), 1);
        assert_eq!(c.cross_column_rules.len(), 3);

        let r0 = &c.cross_column_rules[0];
        assert_eq!(r0.assert_column, "amount");
        assert_eq!(r0.assert_op, ComparisonOp::Gt);
        assert_eq!(r0.assert_value, LiteralValue::Number(0.0));
        assert_eq!(r0.when_column, "status");
        assert_eq!(r0.when_op, ComparisonOp::Eq);
        assert_eq!(r0.when_value, LiteralValue::Str("paid".into()));
        assert_eq!(r0.severity, Severity::Blocking);

        let r2 = &c.cross_column_rules[2];
        assert_eq!(r2.when_op, ComparisonOp::Neq);
        assert_eq!(r2.severity, Severity::Error); // default
    }

    #[test]
    fn test_field_types_and_constraints() {
        let contracts = parse(SAMPLE_DSL).unwrap();
        let schema = &contracts[0].schema;
        assert_eq!(schema[0].name, "id");
        assert!(schema[0].constraints.contains(&Constraint::NotNull));
        assert!(schema[1].constraints.iter().any(|c| matches!(c, Constraint::Regex { .. })));
        assert!(schema[2].constraints.iter().any(|c| matches!(c, Constraint::Between { min, max } if *min == 0.0 && *max == 120.0)));
    }

    #[test]
    fn test_severity_prefix() {
        let contracts = parse(SAMPLE_DSL).unwrap();
        let quality = &contracts[0].quality_rules;
        assert_eq!(quality[0].severity, Severity::Error);
        assert_eq!(quality[1].severity, Severity::Warning);
    }
}
