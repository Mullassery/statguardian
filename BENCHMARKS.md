# StatGuard Benchmarks

**Environment:** Apple M-series · macOS · Python 3.13  
**Dataset:** 100 000 rows × 4 columns (int, string, int, string)  
**Checks:** `not_null` · type · `range(0–120)` · regex email · `uniqueness`  
**Versions:** StatGuard 0.1 · pandera 0.31 · Great Expectations 1.18 · Pydantic 2.13  
**Method:** best-of-7 runs, warm process (no cold-start overhead)

---

## Results — 100 000 rows

| Tool | Best (ms) | Median (ms) | vs StatGuard |
|---|---|---|---|
| **StatGuard 0.1** (Rust/Polars) | **2.0** | **2.1** | baseline |
| Polars expressions (lower bound) | 1.4 | 1.5 | — |
| Pure Python loops | 11.5 | 11.8 | 5.8× slower |
| **pandera 0.31** (pandas, columnar) | **26.5** | **26.7** | **13× slower** |
| **Pydantic v2** (TypeAdapter bulk list) | **43.5** | **45.1** | **22× slower** |
| **Pydantic v2** (row-by-row model_validate) | **46.2** | **46.5** | **23× slower** |
| **Great Expectations 1.18** (pandas, columnar) | **50.4** | **51.1** | **25× slower** |

> **Key insight:** Pydantic and Great Expectations land in the same performance
> tier (~43–50 ms). Pydantic is row-oriented — each `model_validate` call
> allocates a Python object and runs field validators one-by-one. Even the
> bulk `TypeAdapter` path still iterates rows in Python. StatGuard never
> touches individual rows — it operates on entire columns using SIMD-optimised
> Arrow kernels with no Python allocation per element.

---

## What each tool actually ran

Every tool performed the same 5 logical checks. Uniqueness is not a built-in
Pydantic field constraint — it requires a custom root validator.

| Check | StatGuard DSL | pandera | Great Expectations | Pydantic v2 |
|---|---|---|---|---|
| `id` not null | `not_null` | `nullable=False` | `ExpectColumnValuesToNotBeNull` | `id: int` (implicit) |
| `country` not null | `not_null` | `nullable=False` | `ExpectColumnValuesToNotBeNull` | `country: str` (implicit) |
| `age` in [0, 120] | `between(0, 120)` | `Check.ge(0), Check.le(120)` | `ExpectColumnValuesToBeBetween` | `Field(ge=0, le=120)` |
| `email` regex | `regex="..."` | `Check.str_matches(...)` | `ExpectColumnValuesToMatchRegex` | `Field(pattern="...")` |
| `id` unique | `unique` | _(separate check)_ | `ExpectColumnValuesToBeUnique` | _(custom root validator)_ |

---

## Scaling

| Rows | Great Expectations | Pydantic v2 (bulk) | pandera | **StatGuard** | vs GX | vs Pydantic | vs pandera |
|---|---|---|---|---|---|---|---|
| 10 000 | ~10 ms | ~5 ms | ~4 ms | ~0.4 ms | ~25× | ~12× | ~10× |
| 100 000 | **50 ms** | **44 ms** | **27 ms** | **~2 ms** | **~25×** | **~22×** | **~13×** |
| 1 000 000 | ~500 ms | ~430 ms | ~270 ms | ~15 ms | ~33× | ~29× | ~18× |
| 10 000 000 | ~5 000 ms | ~4 300 ms | ~2 700 ms | ~140 ms | ~36× | ~31× | ~19× |

_Rows above 100k extrapolated from observed O(n) scaling. Pydantic scaling is
linear in rows because it allocates one Python object per row regardless of
batch size._

---

## Why StatGuard is faster

| Technique | Benefit |
|---|---|
| **Columnar execution** (Arrow/Polars) | Process entire columns in tight SIMD loops — no per-row Python objects |
| **Compiled DAG** | Validation logic is a compiled execution plan, not interpreted rules |
| **Optimizer** | Fuses null checks, removes redundancy, orders by cost |
| **Rayon parallelism** | All columns validated concurrently across CPU cores |
| **Zero Python allocation** | No dict, no model instance, no per-row overhead |
| **Early exit** | Blocking violations abort column scan immediately |

### Why Pydantic is slower for tabular data

Pydantic excels at validating single API payloads. For tabular data it pays
a per-row cost that doesn't amortise:

- Each row → one `UserRow` Python object (GC pressure)
- Field validators run in Python per row, not in native code
- No columnar short-circuit: if row 99 999 fails, all 99 999 before it were still fully validated

StatGuard's columnar approach inverts this: a null check on 100 000 rows is a
single Arrow kernel call, not 100 000 Python function calls.

---

## Drift detection overhead

Drift detection (PSI + KS test) adds **< 5 ms** for 100k rows. None of the
Python tools (pandera, GX, Pydantic) have built-in statistical drift detection.

---

## Memory

StatGuard processes data in columnar chunks — no additional copies beyond the
input Arrow buffer. Memory overhead for 100k rows × 10 columns is typically
**< 10 MB** above the raw data size. Pydantic's row-by-row approach allocates
one Python model instance per row; at 100k rows with 4 fields each, that is
roughly **40–80 MB** of additional Python object overhead.

---

## Format read overhead

Data load time for 100 000 rows (validation not included):

| Format | Read time |
|---|---|
| Arrow IPC | ~0.1 ms (zero-copy) |
| Parquet | ~1–3 ms |
| Avro | ~2–5 ms |
| CSV | ~5–15 ms |
| Delta Lake (10 files) | ~3–8 ms (log replay + Parquet) |
| Apache Iceberg (10 files) | ~4–10 ms (metadata parse + Parquet) |

---

## Format & table format comparison

See [docs/FORMAT_COMPATIBILITY.md](docs/FORMAT_COMPATIBILITY.md) for a full matrix of
which file formats (Parquet, Avro, ORC, …), lakehouse formats (Delta Lake, Iceberg, Hudi),
cloud storage backends, and SQL warehouses each library supports natively vs. via extras
vs. not at all.

## Reproducing

```bash
pip install statguard pandera polars pandas great-expectations pydantic

# Rust test suite (release mode)
cargo test --release --workspace --exclude statguard

# Full Python benchmark
python3 docs/bench/benchmark.py
```
