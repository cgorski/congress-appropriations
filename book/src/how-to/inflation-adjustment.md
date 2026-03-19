# Adjust for Inflation

When comparing appropriations across fiscal years, nominal dollar changes can be misleading. A program that received $100M in FY2024 and $104M in FY2026 looks like it got a 4% increase — but if inflation over that period was 3.9%, the real increase is only 0.1%. The program's purchasing power barely changed.

The `--real` flag on `compare` adds inflation-adjusted context to every row, showing you which programs received real increases and which ones lost ground to inflation.

## Quick Start

```bash
# Compare THUD FY2024 → FY2026 with inflation adjustment
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir examples --real
```

The output adds two columns: **Real Δ %*** (the inflation-adjusted percentage change) and a directional indicator:

- **▲** — real increase (nominal change exceeded inflation)
- **▼** — real cut or inflation erosion (purchasing power decreased)
- **—** — unchanged in both nominal and real terms

The asterisk on "Real Δ %*" reminds you this is a computed value based on an external price index, not a number verified against bill text.

A summary line at the bottom counts how many programs beat inflation and how many fell behind.

## What It Shows

```text
Account                   Base ($)       Current ($)   Δ %    Real Δ %*  
TBRA                   28,386,831,000  34,438,557,000  +21.3%  +16.7%  ▲
Project-Based Rental   16,010,000,000  18,543,000,000  +15.8%  +11.4%  ▲
Operations (FAA)       12,729,627,000  13,710,000,000   +7.7%   +3.6%  ▲
Public Housing Fund     8,810,784,000   8,319,393,000   -5.6%   -9.1%  ▼
Capital Inv Grants      2,205,000,000   1,700,000,000  -22.9%  -25.8%  ▼
Payment to NRC            158,000,000     158,000,000    0.0%   -3.9%  ▼

45 beat inflation, 17 fell behind | CPI-U FY2024→FY2026: 3.9% (2 months of FY2026 data)
```

Key insight: "Payment to NRC" got the exact same dollar amount both years. Nominally that's "unchanged." But after adjusting for 3.9% inflation, it's effectively a 3.9% cut in purchasing power. The **▼** flag makes this visible at a glance.

## How It Works

The tool ships with a bundled CPI data file containing monthly Consumer Price Index values from the Bureau of Labor Statistics (CPI-U All Items, series CUUR0000SA0). When you pass `--real`:

1. The tool identifies the base and current fiscal years from the comparison
2. It computes fiscal-year-weighted CPI averages (October through September) from the monthly data
3. The inflation rate is the ratio: `current_fy_cpi / base_fy_cpi - 1`
4. For each row, the real percentage change is: `(current / (base × (1 + inflation))) - 1`
5. The inflation flag compares the nominal change to the inflation rate

The bundled CPI data is compiled into the binary — no network access is needed. It's updated with each tool release.

## Using Your Own Price Index

The default deflator is CPI-U (Consumer Price Index for All Urban Consumers), which is the standard measure used in journalism and public policy discussion. However, different analyses may call for different deflators:

- **GDP Deflator** — used by CBO for aggregate budget analysis; broader than CPI
- **PCE Price Index** — the Federal Reserve's preferred measure; typically 0.3–0.5% below CPI
- **Sector-specific deflators** — DoD procurement indices, medical care CPI, construction cost indices

To use a different deflator, provide your own data file:

```bash
congress-approp compare --base-fy 2024 --current-fy 2026 --subcommittee thud --dir examples \
  --real --cpi-file my_gdp_deflator.json
```

The file must follow this JSON schema:

```json
{
  "source": "GDP Deflator (BEA NIPA Table 1.1.4)",
  "retrieved": "2026-03-15",
  "note": "Quarterly values interpolated to monthly",
  "monthly": {
    "2023-10": 118.432,
    "2023-11": 118.576,
    "2023-12": 118.701,
    "2024-01": 118.823,
    "...": "..."
  }
}
```

The tool reads the `monthly` values and computes fiscal-year averages (Oct–Sep) from them. The `source` and `note` fields are displayed in the output footer, so the reader knows exactly which deflator was used.

If you provide calendar-year annual averages instead of monthly data, you can use:

```json
{
  "source": "My custom deflator",
  "retrieved": "2026-03-15",
  "annual_averages": {
    "2024": 118.9,
    "2025": 121.3,
    "2026": 123.1
  },
  "partial_years": {
    "2026": { "months": 2, "through": "2026-02" }
  }
}
```

The tool prefers `monthly` data for precise fiscal year computation, falling back to `annual_averages` (calendar year proxy) when monthly data is not available.

## Understanding the Output

### Nominal vs. Real

| Column | What It Means |
|--------|--------------|
| **Δ %** | The nominal percentage change — what Congress actually voted. Verifiable against bill text. |
| **Real Δ %*** | The inflation-adjusted percentage change — what the money can buy. Computed from an external price index. |

The nominal number answers: "What did Congress decide?" The real number answers: "Did the program's purchasing power go up or down?"

### Inflation Flags

| Flag | Meaning | Example |
|------|---------|---------|
| **▲** | Real increase — nominal growth exceeded inflation | +7.7% nominal with 3.9% inflation = real increase |
| **▼** | Real cut — program lost purchasing power | -5.6% nominal = real cut regardless of inflation |
| **▼** | Inflation erosion — nominal increase but below inflation | +2.0% nominal with 3.9% inflation = real cut |
| **—** | Unchanged — zero nominal change, zero real change | Only when both base and current are $0 |

The most important insight is **inflation erosion**: programs that received a nominal increase but still lost purchasing power. These are politically described as "increases" but economically function as cuts. The `--real` flag makes this visible.

### The Footer

Every inflation-adjusted output includes a footer showing:

- The deflator used (CPI-U by default, or whatever `--cpi-file` specifies)
- The base and current fiscal year CPI values
- The inflation rate between them
- How many months of data are available for partial years
- A count of programs that beat or fell behind inflation

This metadata ensures the analysis is reproducible and the methodology is transparent.

## CSV and JSON Output

### CSV

With `--real --format csv`, the CSV output adds three columns:

```text
account_name,agency,base_dollars,current_dollars,delta,delta_pct,status,real_delta_pct,inflation_flag
```

The `inflation_flag` values are: `real_increase`, `real_cut`, `inflation_erosion`, or `unchanged`. These are designed for filtering in spreadsheets — sort or filter on `inflation_flag` to find all programs that lost ground.

### JSON

With `--real --format json`, the output includes an `inflation` metadata object:

```json
{
  "inflation": {
    "source": "Bureau of Labor Statistics, CPI-U All Items (CUUR0000SA0)",
    "base_fy": 2024,
    "current_fy": 2026,
    "base_cpi": 311.6,
    "current_cpi": 325.1,
    "rate": 0.0434,
    "current_fy_months": 4,
    "note": "FY2026 based on 4 months of data (Oct 2025 – Jan 2026)"
  },
  "rows": [
    {
      "account_name": "Tenant-Based Rental Assistance",
      "base_dollars": 28386831000,
      "current_dollars": 34438557000,
      "delta": 6051726000,
      "delta_pct": 21.3,
      "real_delta_pct": 16.7,
      "inflation_flag": "real_increase",
      "status": "changed"
    }
  ],
  "summary": {
    "beat_inflation": 45,
    "fell_behind": 17,
    "inflation_rate_pct": 4.34
  }
}
```

## Important Caveats

### CPI-U is a consumer measure

CPI-U measures the cost of goods and services purchased by urban consumers — groceries, rent, gasoline, healthcare. Government spending has a different cost structure: federal employee salaries, military procurement, construction, transfer payments. CPI-U is the standard deflator for public-facing analysis but may not precisely reflect the cost pressures facing a specific government program.

For sector-specific analysis, consider using `--cpi-file` with a deflator appropriate to the spending category (medical care CPI for VA health, construction cost index for infrastructure, etc.).

### Partial-year data

For the most recent fiscal year, CPI data may be incomplete. The output always notes how many months of data are available. The inflation rate may shift as more months are published — typically by 0.1–0.3 percentage points.

### This is analysis, not extraction

Nominal dollar amounts in this tool are verified against the enrolled bill text — every number traces to a specific position in the source XML. Inflation-adjusted numbers are computed values that depend on an external data source (BLS) and methodology choices (CPI-U, fiscal year weighting). The asterisk on "Real Δ %*" marks this distinction. When citing inflation-adjusted figures, note the deflator used.

## Updating the Bundled CPI Data

The tool includes CPI-U data current as of its release date. To use more recent data:

1. Download fresh monthly CPI from the [BLS Public Data API](https://www.bls.gov/developers/home.htm) or [FRED](https://fred.stlouisfed.org/series/CPIAUCNS)
2. Format as the JSON schema shown above
3. Pass via `--cpi-file`

Alternatively, wait for the next tool release — each version bundles the latest available CPI data.

## Related

- [Compare Two Bills](../tutorials/compare-two-bills.md) — the base comparison workflow that `--real` extends
- [Budget Authority Calculation](../explanation/budget-authority.md) — how nominal budget authority is computed
- [Why the Numbers Might Not Match Headlines](../explanation/numbers-vs-headlines.md) — context for interpreting appropriations figures
- [CLI Command Reference](../reference/cli.md) — full flag reference for `compare`
