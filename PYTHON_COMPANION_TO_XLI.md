# Python Companion to XLI

## Goal

Use `xli` for fast, transactional workbook edits and use a slower Python script only for heavyweight checks where the Python ecosystem provides clear leverage.

The split is simple:

- `xli` is the fast path for inspect, read, write, format, sheet operations, lint, recalc, validate, and doctor.
- Python is the slow path for deep validation, rich analysis, cross-checks, artifact inspection, and final acceptance reporting.

This preserves XLI's core advantage: a cold-starting Rust CLI that can make small, safe workbook edits in milliseconds without forcing the agent to generate and debug custom Python every time.

## Why a Companion Instead of Replacing XLI

Python is strong when the task is:

- expensive enough that interpreter startup does not matter
- easier with mature ecosystem libraries than with bespoke Rust code
- naturally report-oriented rather than edit-oriented
- best run once at the end of a workflow, not after every cell change

That makes Python a good complement for final checkups, not a good default editing surface.

The operating principle should be:

1. Edit with `xli`.
2. Run `xli doctor` for the fast built-in quality pipeline.
3. Run the Python companion for deeper, slower checks.
4. If fixes are needed, feed them back into `xli` for atomic application.

Python should usually not write the workbook directly.

## Division of Labor

| Use case | Preferred tool | Reason |
|---|---|---|
| Single-cell or small-range edits | `xli` | Fast startup, atomic commit, no interpreter tax |
| Formatting and sheet operations | `xli` | Native addressability and consistent transaction model |
| Fast structural checks | `xli lint` / `xli validate` / `xli doctor` | Designed for repeated use during editing |
| Full-workbook acceptance report | Python companion | Heavy libraries and richer reporting are acceptable here |
| Cross-sheet data audits | Python companion | Easier with dataframe and validation libraries |
| OOXML artifact inspection beyond the fast path | Python companion | `zipfile`, `lxml`, and `openpyxl` are pragmatic |
| Auto-fix application | `xli` | Preserve fingerprint checks and atomic writes |

## Recommended Workflow

```bash
# 1. Fast editing loop
xli inspect model.xlsx
xli write model.xlsx "Summary!B5" --value 100
xli format model.xlsx "Summary!B5" --number-format '$#,##0'
xli sheet model.xlsx add Checks

# 2. Built-in fast quality pass
xli doctor model.xlsx > doctor.json

# 3. Slow deep validation
python python_companion.py \
  --workbook model.xlsx \
  --doctor doctor.json \
  --out companion-report.json

# 4. If the companion proposes deterministic repairs
xli batch model.xlsx fixes.ndjson --expect-fingerprint <sha256>
```

The important boundary is that Python returns findings and optional fix plans, but `xli` remains the tool that commits workbook changes.

## What the Python Companion Should Do

### 1. Final acceptance checks

The Python script should answer questions like:

- Does this workbook satisfy business rules that are too domain-specific for core `xli`?
- Do extracted tables match source data expectations?
- Are headers, totals, named ranges, and required sheets all present?
- Are charts, images, comments, validations, and other OOXML artifacts present where expected?
- Are outputs statistically plausible, not just syntactically valid?

### 2. Heavy data validation

Python is a good fit for:

- schema validation on extracted sheet data
- null-rate checks, uniqueness checks, and type coercion audits
- cross-sheet reconciliation
- source-vs-output comparisons
- tolerance-based numeric comparisons
- high-level business-rule evaluation

### 3. Rich reporting

The script can produce artifacts that are not worth building into the core CLI:

- detailed JSON findings
- markdown or HTML reports
- CSV summaries of broken rules
- charts for QA reviewers
- machine-readable fix proposals

### 4. Deep OOXML inspection

When workbook fidelity matters, Python can inspect:

- ZIP package contents
- relationships and drawing parts
- chart XML presence
- comments, validations, and conditional formatting
- style usage patterns
- workbook properties and defined names

This is useful as a final audit even if `xli` remains the primary editor.

## Recommended Output Contract

The companion should emit structured JSON, not prose. A practical result shape is:

```json
{
  "status": "ok",
  "workbook": "model.xlsx",
  "fingerprint": "sha256:...",
  "summary": {
    "checks_run": 18,
    "errors": 1,
    "warnings": 3
  },
  "findings": [
    {
      "code": "MISSING_REQUIRED_SHEET",
      "severity": "error",
      "message": "Sheet 'Checks' is missing."
    }
  ],
  "fix_plan": [
    {
      "op": "sheet.add",
      "name": "Checks"
    }
  ]
}
```

Two rules matter here:

- The companion should include the workbook fingerprint it validated.
- Any `fix_plan` should be expressed in a format that can be converted into `xli batch` ops or another `xli`-native command sequence.

## Recommended Python Libraries

| Problem | Libraries | Why |
|---|---|---|
| Workbook structure and metadata | `openpyxl` | Practical workbook introspection, styles, comments, names, tables |
| Raw OOXML inspection | `zipfile`, `lxml` | Direct access to parts that high-level libraries flatten or hide |
| Tabular validation | `pandas` or `polars` | Mature dataframe tooling for joins, diffs, and summaries |
| Data contracts | `pandera`, `pydantic` | Explicit schemas and typed validation results |
| SQL-style reconciliation | `duckdb` | Excellent for comparing extracted sheets and source datasets |
| Statistical and numeric checks | `numpy`, `scipy` | Robust tolerance checks and anomaly detection |
| Human-readable reports | `jinja2`, `markdown`, `rich` | Easy report generation without complicating `xli` |
| Snapshot and visual QA helpers | `matplotlib`, `seaborn`, `Pillow` | Useful for review artifacts, not for editing |

## Suggested Companion Responsibilities by Phase

### Phase 1: Read-only validator

Start with a read-only script that:

- consumes `xli inspect` and `xli doctor` output
- opens the workbook in Python for deeper inspection
- emits findings plus an optional fix plan

This is the safest starting point because it cannot corrupt the workbook.

### Phase 2: Fix-plan generator

Extend the script so it can produce:

- `xli batch` ndjson operations
- rule-specific repair suggestions
- acceptance reports for CI or human review

The Python script still does not write the workbook. It only proposes repairs.

### Phase 3: Specialized heavy analyzers

Add optional modules for:

- finance/model checks
- layout and artifact checks
- data quality suites
- regression comparisons against golden workbooks

These should remain optional because many workflows will only need the base validator.

## Validation Ideas That Fit Python Well

Good candidates for the companion include:

- compare workbook tables against canonical CSV or parquet inputs
- detect unexpected header drift using fuzzy matching
- verify that required charts exist and reference the expected ranges
- inspect named ranges for overlap, dead references, or policy violations
- detect suspicious formulas by pattern, not just parseability
- flag outliers, discontinuities, or broken monotonic trends in KPI sheets
- compare a newly generated workbook against a golden workbook and summarize meaningful differences
- verify template-specific conventions that should not live in core `xli`

## Things Python Should Not Own

To keep the architecture clean, Python should usually not own:

- routine workbook editing
- high-frequency agent loops
- the authoritative transaction boundary
- compare-and-swap safety
- the primary CLI interface for common operations

If Python finds a change is needed, it should ask `xli` to make that change.

## Practical Rule of Thumb

Use `xli` when the agent is still building or repairing the workbook.

Use Python when the workbook is mostly done and the remaining question is, "Is this artifact actually acceptable?"

That gives us the best of both worlds:

- Rust for fast, repeatable, low-latency workbook operations
- Python for slow, rich, ecosystem-heavy validation and final checkups

## Proposed Initial Scope

The first version of the companion should do only four things:

1. Read `xli doctor` output.
2. Open the workbook with `openpyxl` plus `zipfile`/`lxml`.
3. Run template-specific and data-specific final checks.
4. Emit JSON findings and an optional `xli` fix plan.

That scope is narrow enough to build quickly and useful enough to justify the extra startup cost.
