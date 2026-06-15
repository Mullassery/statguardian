#!/usr/bin/env python3
"""
StatGuard benchmark — compare against popular Python data quality libraries.

Usage:
    pip install statguard pandera polars pandas
    python docs/bench/benchmark.py
"""

import time, random, sys

# ── Generate data ─────────────────────────────────────────────────────────────
N = 100_000
random.seed(42)
ids      = list(range(N))
emails   = [f"user{i}@example.com" for i in range(N)]
ages     = [random.randint(0, 120) for _ in range(N)]
countries = random.choices(["US","UK","DE","FR","CA","AU","JP"], k=N)

results = {}

# ── StatGuard ─────────────────────────────────────────────────────────────────
try:
    import polars as pl
    import statguard

    contract = statguard.DataContract.from_dsl("""
dataset bench {
    schema {
        id:      int,    not_null, unique
        email:   string, regex="^[^@]+@[^@]+\\.[^@]+$"
        age:     int,    between(0, 120)
        country: string, not_null
    }
    quality {
        completeness(id)    > 0.99
        uniqueness(email)   == 1.0
    }
}
""")
    df = pl.DataFrame({"id": ids, "email": emails, "age": ages, "country": countries})

    times = []
    for _ in range(10):
        t0 = time.perf_counter()
        statguard.execute(contract, df)
        times.append((time.perf_counter() - t0) * 1000)
    results["StatGuard (Rust/Polars)"] = min(times)
except ImportError:
    results["StatGuard (Rust/Polars)"] = None
    print("statguard not installed — run: maturin develop --release", file=sys.stderr)

# ── pandera ───────────────────────────────────────────────────────────────────
try:
    import pandera.pandas as pa
    import pandas as pd

    df_pd = pd.DataFrame({"id": ids, "email": emails, "age": ages, "country": countries})
    schema = pa.DataFrameSchema({
        "id":      pa.Column(int, nullable=False),
        "email":   pa.Column(str, pa.Check.str_matches(r"^[^@]+@[^@]+\.[^@]+$")),
        "age":     pa.Column(int, [pa.Check.ge(0), pa.Check.le(120)]),
        "country": pa.Column(str, nullable=False),
    })
    times = []
    for _ in range(5):
        t0 = time.perf_counter()
        schema.validate(df_pd)
        times.append((time.perf_counter() - t0) * 1000)
    results["pandera 0.31 (pandas)"] = min(times)
except ImportError:
    results["pandera 0.31 (pandas)"] = None

# ── Polars manual expressions ─────────────────────────────────────────────────
try:
    import polars as pl
    df = pl.DataFrame({"id": ids, "email": emails, "age": ages, "country": countries})
    times = []
    for _ in range(10):
        t0 = time.perf_counter()
        df.select([
            pl.col("id").is_null().sum(),
            pl.col("email").str.contains(r"^[^@]+@[^@]+\.[^@]+$").not_().sum(),
            ((pl.col("age") < 0) | (pl.col("age") > 120)).sum(),
            pl.col("country").is_null().sum(),
        ])
        times.append((time.perf_counter() - t0) * 1000)
    results["Polars manual expressions"] = min(times)
except ImportError:
    results["Polars manual expressions"] = None

# ── Pure Python ───────────────────────────────────────────────────────────────
import re
pattern = re.compile(r"^[^@]+@[^@]+\.[^@]+$")
times = []
for _ in range(5):
    t0 = time.perf_counter()
    [0 <= a <= 120 for a in ages]
    [pattern.match(e) for e in emails]
    sum(1 for i in ids if i is not None)
    times.append((time.perf_counter() - t0) * 1000)
results["Pure Python loops"] = min(times)

# ── Report ────────────────────────────────────────────────────────────────────
print(f"\n{'='*62}")
print(f"  StatGuard Benchmark  ·  {N:,} rows × 4 columns")
print(f"  Checks: null · type · range(0-120) · regex email · uniqueness")
print(f"{'='*62}")

baseline = results.get("pandera 0.31 (pandas)")
for name, ms in sorted(results.items(), key=lambda x: x[1] or float("inf")):
    if ms is None:
        print(f"  {name:<35}  NOT INSTALLED")
        continue
    if baseline and name != "pandera 0.31 (pandas)":
        speedup = f"  ({baseline/ms:.1f}× faster than pandera)"
    else:
        speedup = ""
    print(f"  {name:<35}  {ms:6.1f} ms{speedup}")

print(f"{'='*62}")
print(f"\nAll times are best-of-N (ms). Lower is better.")
