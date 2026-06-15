"""
StatGuard — Python quickstart example
======================================

Prerequisites:
    pip install statguard polars

Usage:
    python examples/python_quickstart.py
"""

import polars as pl
import statguard

# ── 1. Define a data contract in DSL ─────────────────────────────────────────

contract = statguard.DataContract.from_dsl("""
dataset users {
    schema {
        id:      int,    not_null, unique, primary_key
        email:   string, regex="^[^@]+@[^@]+\\.[^@]+$"
        age:     int,    between(0, 120)
        country: string, not_null
        score:   float,  min=0.0, max=1.0
    }
    quality {
        completeness(id)         > 0.99
        @warning: uniqueness(email) == 1.0
    }
    stats {
        age.mean drift < 0.10
        age.std  drift < 0.20
    }
    anomalies {
        detect_outliers(age, method="iqr")
        @blocking: detect_duplicates(id)
    }
}
""")

print(f"Contract: {contract}")

# ── 2. Create sample data ─────────────────────────────────────────────────────

clean_df = pl.DataFrame({
    "id":      [1, 2, 3, 4, 5],
    "email":   ["alice@example.com", "bob@example.com", "carol@example.com",
                "dave@example.com", "eve@example.com"],
    "age":     [25, 32, 41, 28, 55],
    "country": ["US", "UK", "DE", "FR", "CA"],
    "score":   [0.91, 0.85, 0.73, 0.96, 0.62],
})

dirty_df = pl.DataFrame({
    "id":      [1, 2, 2, None, 5],           # duplicate + null
    "email":   ["alice@example.com", "not-an-email", "carol@example.com",
                "dave@example.com", "eve@example.com"],
    "age":     [25, 32, 300, 28, -1],         # out-of-range
    "country": ["US", None, "DE", "FR", "CA"],
    "score":   [0.91, 1.5, 0.73, -0.1, 0.62],
})

# ── 3. Validate clean data ────────────────────────────────────────────────────

print("\n--- Validating clean data ---")
report = statguard.execute(contract, clean_df)
print(report.summary())
print(f"  Health score : {report.health_score:.3f}")
print(f"  Grade        : {report.grade}")
print(f"  Violations   : {report.violation_count}")
print(f"  Passed       : {report.passed}")

# ── 4. Validate dirty data ────────────────────────────────────────────────────

print("\n--- Validating dirty data ---")
dirty_report = statguard.execute(contract, dirty_df)
print(dirty_report.summary())
print(f"  Passed: {dirty_report.passed}")
print("\n  Violations found:")
for v in dirty_report.violations():
    print(f"    [{v['severity']:8}] {v['column']:10} | {v['check']:20} | {v['message']}")

# ── 5. Drift detection ────────────────────────────────────────────────────────

reference_df = pl.DataFrame({
    "id":      [10, 20, 30, 40, 50],
    "email":   ["r1@example.com", "r2@example.com", "r3@example.com",
                "r4@example.com", "r5@example.com"],
    "age":     [22, 30, 44, 26, 52],       # similar distribution
    "country": ["US", "UK", "DE", "FR", "CA"],
    "score":   [0.88, 0.79, 0.71, 0.94, 0.60],
})

shifted_df = pl.DataFrame({
    "id":      [60, 70, 80, 90, 100],
    "email":   ["s1@example.com", "s2@example.com", "s3@example.com",
                "s4@example.com", "s5@example.com"],
    "age":     [65, 70, 80, 72, 69],       # older cohort → drift!
    "country": ["US", "UK", "DE", "FR", "CA"],
    "score":   [0.40, 0.35, 0.28, 0.42, 0.31],
})

print("\n--- Drift detection: no shift ---")
r_no_drift = statguard.execute(contract, reference_df, reference=reference_df)
for d in r_no_drift.drift_results():
    status = "✓" if d["passed"] else "✗"
    print(f"  {status} {d['column']}.{d['stat']}: drift={d['drift']:.4f} (PSI={d.get('psi', 0):.4f})")

print("\n--- Drift detection: age distribution shift ---")
r_drift = statguard.execute(contract, shifted_df, reference=reference_df)
for d in r_drift.drift_results():
    status = "✓" if d["passed"] else "✗ DRIFT DETECTED"
    print(f"  {status} {d['column']}.{d['stat']}: drift={d['drift']:.4f} (PSI={d.get('psi', 0):.4f})")

# ── 6. JSON and Prometheus output ─────────────────────────────────────────────

print("\n--- JSON report (excerpt) ---")
import json
j = json.loads(report.to_json())
print(json.dumps({"id": j["id"][:8]+"…", "passed": j["passed"], "health": j["health"]}, indent=2))

print("\n--- Prometheus metrics ---")
print(report.to_prometheus())

# ── 7. Column profiles ────────────────────────────────────────────────────────

print("\n--- Column profiles ---")
for p in report.column_profiles():
    if p["mean"] is not None:
        print(f"  {p['name']:10} | mean={p['mean']:.2f} | std={p['std']:.2f} | "
              f"null_rate={p['null_rate']:.1%} | distinct={p['distinct_count']}")
    else:
        print(f"  {p['name']:10} | null_rate={p['null_rate']:.1%} | distinct={p['distinct_count']}")
