"""
Referential integrity validation for StatGuard.

Checks that foreign-key values in one DataFrame exist in the
primary-key column of another — catching orphaned records before
they break downstream joins or aggregations.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Optional


@dataclass
class IntegrityViolation:
    """One referential integrity finding."""
    foreign_table: str
    foreign_column: str
    primary_table: str
    primary_column: str
    orphaned_count: int
    orphaned_values: list = field(default_factory=list)
    severity: str = "error"

    def __str__(self) -> str:
        sample = self.orphaned_values[:5]
        suffix = f" …+{self.orphaned_count - len(sample)}" if self.orphaned_count > len(sample) else ""
        return (
            f"[{self.severity.upper()}] "
            f"{self.foreign_table}.{self.foreign_column} → "
            f"{self.primary_table}.{self.primary_column}: "
            f"{self.orphaned_count} orphaned value(s): {sample}{suffix}"
        )


def check_referential_integrity(
    df,
    reference_df,
    foreign_key: str,
    primary_key: str,
    *,
    foreign_table: str = "current",
    primary_table: str = "reference",
    allow_nulls: bool = True,
    severity: str = "error",
) -> List[IntegrityViolation]:
    """
    Check that every value in ``df[foreign_key]`` exists in
    ``reference_df[primary_key]``.

    Args:
        df:             DataFrame containing the foreign key column.
        reference_df:   DataFrame containing the authoritative primary key set.
        foreign_key:    Column name in ``df`` to validate.
        primary_key:    Column name in ``reference_df`` that defines the valid set.
        foreign_table:  Label for ``df`` used in violation messages.
        primary_table:  Label for ``reference_df`` used in violation messages.
        allow_nulls:    If True (default), null values in the foreign key are
                        not flagged as orphaned.
        severity:       Severity of reported violations (``"error"`` by default).

    Returns:
        List of ``IntegrityViolation`` objects — empty if all foreign key
        values are present in the primary key set.

    Example::

        import polars as pl
        import statguard

        orders    = pl.read_parquet("orders.parquet")
        customers = pl.read_parquet("customers.parquet")

        violations = statguard.check_referential_integrity(
            orders, customers,
            foreign_key="customer_id",
            primary_key="id",
            foreign_table="orders",
            primary_table="customers",
        )
        print(statguard.integrity_report(violations))
    """
    if foreign_key not in df.columns:
        raise ValueError(f"Foreign key column '{foreign_key}' not found in DataFrame")
    if primary_key not in reference_df.columns:
        raise ValueError(f"Primary key column '{primary_key}' not found in reference DataFrame")

    fk_series = df[foreign_key]
    pk_series = reference_df[primary_key]

    if allow_nulls:
        fk_series = fk_series.drop_nulls()

    pk_set = set(pk_series.drop_nulls().to_list())
    fk_values = fk_series.to_list()

    orphaned_all = [v for v in fk_values if v not in pk_set]
    if not orphaned_all:
        return []

    # Deduplicated sample, preserving first-occurrence order
    seen: set = set()
    orphaned_unique = []
    for v in orphaned_all:
        if v not in seen:
            seen.add(v)
            orphaned_unique.append(v)

    return [IntegrityViolation(
        foreign_table=foreign_table,
        foreign_column=foreign_key,
        primary_table=primary_table,
        primary_column=primary_key,
        orphaned_count=len(orphaned_all),
        orphaned_values=orphaned_unique[:20],
        severity=severity,
    )]


def check_all_foreign_keys(
    df,
    reference_df,
    key_pairs: List[tuple],
    **kwargs,
) -> List[IntegrityViolation]:
    """
    Check multiple foreign-key → primary-key pairs in one call.

    Args:
        df:           DataFrame containing foreign key columns.
        reference_df: DataFrame containing primary key columns.
        key_pairs:    List of ``(foreign_key, primary_key)`` tuples.
        **kwargs:     Passed through to ``check_referential_integrity``.

    Example::

        violations = statguard.check_all_foreign_keys(
            orders, dimension_table,
            key_pairs=[
                ("customer_id", "id"),
                ("product_id", "sku"),
            ],
        )
    """
    violations = []
    for fk, pk in key_pairs:
        violations.extend(
            check_referential_integrity(df, reference_df, fk, pk, **kwargs)
        )
    return violations


def integrity_report(violations: List[IntegrityViolation]) -> str:
    """Format a list of IntegrityViolations as a human-readable report."""
    if not violations:
        return "Referential integrity: OK"
    lines = [f"Referential integrity — {len(violations)} violation(s):", ""]
    for v in violations:
        lines.append(f"  {v}")
    return "\n".join(lines)
