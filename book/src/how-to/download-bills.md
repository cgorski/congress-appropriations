# Download Bills from Congress.gov

> **You will need:** `congress-approp` installed, `CONGRESS_API_KEY` environment variable set.
>
> **You will learn:** How to discover available appropriations bills, download their enrolled XML, and set up a data directory for extraction.

This guide covers every option for downloading bill XML from Congress.gov. If you just want the quick path, skip to [Quick Reference](#quick-reference) at the end.

## Set Up Your API Key

The Congress.gov API requires a free API key. Sign up at [api.congress.gov/sign-up](https://api.congress.gov/sign-up/) — approval is usually instant.

Set the key in your environment:

```bash
export CONGRESS_API_KEY="your-key-here"
```

You can verify connectivity with:

```bash
congress-approp api test
```

## Discover Available Bills

Before downloading, you'll usually want to see what's available. The `api bill list` command queries Congress.gov for appropriations bills:

### List all appropriations bills for a congress

```bash
congress-approp api bill list --congress 118
```

This returns every bill in the 118th Congress (2023–2024) that Congress.gov classifies as an appropriations bill — introduced, passed, vetoed, or enacted.

### List only enacted bills

Most of the time you only want bills that became law:

```bash
congress-approp api bill list --congress 118 --enacted-only
```

The `--enacted-only` flag filters to bills signed by the President (or with a veto override). These are the authoritative spending laws.

### Congress numbers

Each Congress spans two years. Here are the recent ones:

| Congress | Years | Fiscal Years Typically Covered |
|----------|-------|-------------------------------|
| 116th | 2019–2020 | FY2020, FY2021 |
| 117th | 2021–2022 | FY2022, FY2023 |
| 118th | 2023–2024 | FY2024, FY2025 |
| 119th | 2025–2026 | FY2026, FY2027 |

Note that fiscal years don't align perfectly with congresses — a bill enacted in the 118th Congress might fund FY2024 (which started October 1, 2023) or FY2025.

### Get metadata for a specific bill

If you know which bill you want, you can inspect its metadata before downloading:

```bash
congress-approp api bill get --congress 118 --type hr --number 4366
```

### Check available text versions

Bills have multiple text versions (introduced, engrossed, enrolled, etc.). To see what's available:

```bash
congress-approp api bill text --congress 118 --type hr --number 4366
```

This lists every text version with its format (XML, PDF, HTML) and download URL. For extraction, you want the **enrolled** (enr) version — the final text signed into law.

## Bill Type Codes

When specifying a bill, you need the type code:

| Code | Meaning | Example |
|------|---------|---------|
| `hr` | House bill | H.R. 4366 |
| `s` | Senate bill | S. 1234 |
| `hjres` | House joint resolution | H.J.Res. 100 |
| `sjres` | Senate joint resolution | S.J.Res. 50 |

Most enacted appropriations bills originate in the House (`hr`), since the Constitution requires spending bills to originate there. Joint resolutions (`hjres`, `sjres`) are sometimes used for continuing resolutions.

## Download a Single Bill

To download one specific bill's enrolled XML:

```bash
congress-approp download --congress 118 --type hr --number 9468 --output-dir data
```

This creates the directory structure and saves the XML:

```text
data/
└── 118/
    └── hr/
        └── 9468/
            └── BILLS-118hr9468enr.xml
```

The file name follows the Government Publishing Office convention: `BILLS-{congress}{type}{number}enr.xml`.

### Only the enrolled version is downloaded

By default, the tool downloads **only the enrolled version** (the final text signed into law). This is the version you need for extraction and analysis — one XML file per bill, no clutter.

If you need other text versions (for example, to compare the House-passed version to the final enrolled version), you can request specific versions or all versions:

```bash
# Download only the introduced version
congress-approp download --congress 118 --type hr --number 4366 --output-dir data --version ih

# Download all available text versions (introduced, engrossed, enrolled, etc.)
congress-approp download --congress 118 --type hr --number 4366 --output-dir data --all-versions
```

Available version codes for `--version`:

| Code | Version | Description |
|------|---------|-------------|
| `enr` | Enrolled | Final version, signed into law (**downloaded by default**) |
| `ih` | Introduced in House | As originally introduced |
| `is` | Introduced in Senate | As originally introduced |
| `eh` | Engrossed in House | As passed by the House |
| `es` | Engrossed in Senate | As passed by the Senate |

> **Tip:** For extraction and analysis, always use the enrolled version (the default). Non-enrolled versions may have different XML structures that the parser doesn't support. The `--all-versions` flag is for advanced workflows like tracking how a bill changed during the legislative process.

### Download multiple formats

You can download both XML (for extraction) and PDF (for reading) at once:

```bash
congress-approp download --congress 118 --type hr --number 4366 --output-dir data --format xml,pdf
```

## Download All Enacted Bills for a Congress

To batch-download every enacted appropriations bill:

```bash
congress-approp download --congress 118 --enacted-only --output-dir data
```

This scans Congress.gov for all enacted appropriations bills in the specified congress, then downloads the enrolled XML for each one. The process may take a minute or two depending on how many bills exist and the API's response time.

Each bill gets its own directory:

```text
data/
└── 118/
    └── hr/
        ├── 4366/
        │   └── BILLS-118hr4366enr.xml
        ├── 5860/
        │   └── BILLS-118hr5860enr.xml
        └── 9468/
            └── BILLS-118hr9468enr.xml
```

## Preview Without Downloading

Use `--dry-run` to see what would be downloaded without actually fetching anything:

```bash
congress-approp download --congress 118 --enacted-only --output-dir data --dry-run
```

This queries the API and lists each bill that would be downloaded, along with the file size and output path. Useful for estimating how much data you're about to pull down.

## Choosing an Output Directory

The `--output-dir` flag controls where bills are saved. The default is `./data`. You can use any directory structure you like:

```bash
# Default location
congress-approp download --congress 118 --type hr --number 4366

# Custom location
congress-approp download --congress 118 --type hr --number 4366 --output-dir ~/appropriations-data

# Organized by fiscal year (your choice of structure)
congress-approp download --congress 118 --type hr --number 4366 --output-dir data/fy2024
```

The tool creates intermediate directories as needed. Later, when you run `extract`, `search`, `summary`, and other commands, you point `--dir` at whatever directory contains your bills — the loader walks recursively to find all `extraction.json` files.

## Handling Rate Limits and Errors

The Congress.gov API has rate limits (typically 5,000 requests per hour for registered users). If you're downloading many bills in quick succession, you may encounter rate limiting.

**Symptoms:** HTTP 429 (Too Many Requests) errors, or slow responses.

**Solutions:**
- Wait a few minutes and retry
- Download bills one at a time rather than in batch
- The tool handles most retries automatically, but persistent rate limiting may require reducing your request frequency

**Other common issues:**

| Error | Cause | Solution |
|-------|-------|----------|
| "API key not set" | `CONGRESS_API_KEY` not in environment | `export CONGRESS_API_KEY="your-key"` |
| "Bill not found" (404) | Wrong congress number, bill type, or number | Double-check using `api bill list` |
| "No enrolled text available" | Bill hasn't been enrolled yet, or text not yet published | Check `api bill text` for available versions; some bills take days to appear after signing |
| "Connection refused" | Network issue or Congress.gov maintenance | Check your internet connection; try again later |

## After Downloading

Once you have the XML, the next step is extraction:

```bash
# Preview extraction (no API calls)
congress-approp extract --dir data/118/hr/9468 --dry-run

# Run extraction
congress-approp extract --dir data/118/hr/9468
```

See [Extract Provisions from a Bill](./extract-provisions.md) for the full extraction guide, or [Extract Your Own Bill](../tutorials/extract-your-own-bill.md) for the end-to-end tutorial.

## Quick Reference

```bash
# Set your API key
export CONGRESS_API_KEY="your-key"

# Test connectivity
congress-approp api test

# List enacted bills for a congress
congress-approp api bill list --congress 118 --enacted-only

# Download a single bill
congress-approp download --congress 118 --type hr --number 4366 --output-dir data

# Download all enacted bills for a congress
congress-approp download --congress 118 --enacted-only --output-dir data

# Preview without downloading
congress-approp download --congress 118 --enacted-only --output-dir data --dry-run

# Check available text versions for a bill
congress-approp api bill text --congress 118 --type hr --number 4366
```

## Full Command Reference

```text
congress-approp download [OPTIONS] --congress <CONGRESS>

Options:
    --congress <CONGRESS>      Congress number (e.g., 118 for 2023-2024)
    --type <TYPE>              Bill type: hr, s, hjres, sjres
    --number <NUMBER>          Bill number (used with --type for single-bill download)
    --output-dir <OUTPUT_DIR>  Output directory [default: ./data]
    --enacted-only             Only download enacted (signed into law) bills
    --format <FORMAT>          Download format: xml, pdf [comma-separated] [default: xml]
    --version <VERSION>        Text version filter: enr, ih, eh, es, is
    --all-versions             Download all text versions instead of just enrolled
    --dry-run                  Show what would be downloaded without fetching
```

## Next Steps

- **[Extract Provisions from a Bill](./extract-provisions.md)** — turn downloaded XML into structured data
- **[Extract Your Own Bill](../tutorials/extract-your-own-bill.md)** — the full end-to-end tutorial
- **[Environment Variables and API Keys](../reference/environment-variables.md)** — all API key configuration options