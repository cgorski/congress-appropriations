# Environment Variables and API Keys

Complete reference for all environment variables used by `congress-approp`. No API keys are needed to query pre-extracted example data — keys are only required for downloading new bills, extracting provisions, or using semantic search.

## API Keys

| Variable | Used By | Required For | Cost | How to Get |
|----------|---------|-------------|------|------------|
| `CONGRESS_API_KEY` | `download`, `api test`, `api bill list`, `api bill get`, `api bill text` | Downloading bill XML from Congress.gov | **Free** | [api.congress.gov/sign-up](https://api.congress.gov/sign-up/) |
| `ANTHROPIC_API_KEY` | `extract` | Extracting provisions using Claude | Pay-per-use | [console.anthropic.com](https://console.anthropic.com/) |
| `OPENAI_API_KEY` | `embed`, `search --semantic` | Generating embeddings and embedding search queries | Pay-per-use | [platform.openai.com](https://platform.openai.com/) |

### Setting API Keys

Set keys in your shell before running commands:

```bash
export CONGRESS_API_KEY="your-congress-key"
export ANTHROPIC_API_KEY="your-anthropic-key"
export OPENAI_API_KEY="your-openai-key"
```

To persist across sessions, add the `export` lines to your shell profile (`~/.bashrc`, `~/.zshrc`, or equivalent).

### Testing API Keys

Verify that your Congress.gov and Anthropic keys are working:

```bash
congress-approp api test
```

There is no built-in test for the OpenAI key — the `embed` command will fail with a clear error message if the key is missing or invalid.

## Configuration Variables

| Variable | Used By | Description | Default |
|----------|---------|-------------|---------|
| `APPROP_MODEL` | `extract` | Override the default LLM model for extraction. The `--model` command-line flag takes precedence if both are set. | `claude-opus-4-6` |

### Setting the Model Override

```bash
# Use a different model for all extractions in this session
export APPROP_MODEL="claude-sonnet-4-20250514"
congress-approp extract --dir data/118/hr/9468

# Or override per-command with the flag (takes precedence over env var)
congress-approp extract --dir data/118/hr/9468 --model claude-sonnet-4-20250514
```

> **Quality note:** The system prompt and expected output format are specifically tuned for Claude Opus. Other models may produce lower-quality extractions. Always check `audit` output after extracting with a non-default model.

## Which Keys Do I Need?

### Querying pre-extracted data (no keys needed)

These commands work with the included `data/` data and any previously extracted bills — **no API keys required**:

```bash
congress-approp summary --dir data
congress-approp search --dir data --type appropriation
congress-approp search --dir data --keyword "Veterans"
congress-approp audit --dir data
congress-approp compare --base data/118-hr4366 --current data/118-hr9468
congress-approp upgrade --dir data --dry-run
```

### Semantic search (OPENAI_API_KEY only)

Semantic search requires one API call to embed your query text (~100ms, costs fractions of a cent):

```bash
export OPENAI_API_KEY="your-key"
congress-approp search --dir data --semantic "school lunch programs" --top 5
```

The `--similar` flag does **not** require an API key — it uses pre-computed vectors stored locally:

```bash
# No API key needed for --similar
congress-approp search --dir data --similar 118-hr9468:0 --top 5
```

### Downloading bills (CONGRESS_API_KEY only)

```bash
export CONGRESS_API_KEY="your-key"
congress-approp download --congress 118 --type hr --number 9468 --output-dir data
congress-approp api bill list --congress 118 --enacted-only
```

### Extracting provisions (ANTHROPIC_API_KEY only)

```bash
export ANTHROPIC_API_KEY="your-key"
congress-approp extract --dir data/118/hr/9468
```

### Generating embeddings (OPENAI_API_KEY only)

```bash
export OPENAI_API_KEY="your-key"
congress-approp embed --dir data/118/hr/9468
```

### Full pipeline (all three keys)

```bash
export CONGRESS_API_KEY="your-congress-key"
export ANTHROPIC_API_KEY="your-anthropic-key"
export OPENAI_API_KEY="your-openai-key"

congress-approp download --congress 118 --enacted-only --output-dir data
congress-approp extract --dir data --parallel 6
congress-approp embed --dir data
congress-approp summary --dir data
```

## Error Messages

| Error | Missing Variable | Fix |
|-------|-----------------|-----|
| `"CONGRESS_API_KEY environment variable not set"` | `CONGRESS_API_KEY` | `export CONGRESS_API_KEY="your-key"` |
| `"ANTHROPIC_API_KEY environment variable not set"` | `ANTHROPIC_API_KEY` | `export ANTHROPIC_API_KEY="your-key"` |
| `"OPENAI_API_KEY environment variable not set"` | `OPENAI_API_KEY` | `export OPENAI_API_KEY="your-key"` |
| `"API key invalid"` or 401 error | Key is set but incorrect | Double-check the key value; regenerate if necessary |
| `"Rate limited"` or 429 error | Key is valid but quota exceeded | Wait and retry; reduce `--parallel` for extraction |

## Security Best Practices

- **Never hardcode API keys** in scripts, configuration files checked into version control, or command-line arguments (which may be logged in shell history).
- **Use environment variables** as shown above, or source them from a file that is **not** checked into version control:

  ```bash
  # Create a file (add to .gitignore!)
  echo 'export CONGRESS_API_KEY="your-key"' > ~/.congress-approp-keys
  echo 'export ANTHROPIC_API_KEY="your-key"' >> ~/.congress-approp-keys
  echo 'export OPENAI_API_KEY="your-key"' >> ~/.congress-approp-keys

  # Source before use
  source ~/.congress-approp-keys
  congress-approp extract --dir data
  ```

- **Rotate keys** periodically, especially if they may have been exposed.
- **Use separate keys** for development and production if your organization supports it.

## Cost Estimates

The tool tracks token usage but never displays dollar costs. Here are approximate costs for reference:

### Extraction (Anthropic)

| Bill Type | Estimated Input Tokens | Estimated Output Tokens |
|-----------|----------------------|------------------------|
| Small supplemental (~10 KB XML) | ~1,200 | ~1,500 |
| Continuing resolution (~130 KB XML) | ~25,000 | ~15,000 |
| Omnibus (~1.8 MB XML) | ~315,000 | ~200,000 |

Token usage is recorded in `tokens.json` after extraction. Use `extract --dry-run` to preview token counts before committing.

### Embeddings (OpenAI)

| Bill Type | Provisions | Estimated Cost |
|-----------|-----------|---------------|
| Small supplemental | 7 | < $0.001 |
| Continuing resolution | 130 | < $0.01 |
| Omnibus | 2,364 | < $0.01 |

### Semantic Search (OpenAI)

Each `--semantic` query makes one API call to embed the query text: approximately $0.0001 per search.

The `--similar` flag uses stored vectors and makes **no API calls** — completely free after initial embedding.

## Summary

| Task | Keys Needed |
|------|-------------|
| Query pre-extracted data | **None** |
| `search --similar` (cross-bill matching) | **None** (uses stored vectors) |
| `search --semantic` (meaning-based search) | `OPENAI_API_KEY` |
| Download bills from Congress.gov | `CONGRESS_API_KEY` |
| Extract provisions from bill XML | `ANTHROPIC_API_KEY` |
| Generate embeddings | `OPENAI_API_KEY` |
| Full pipeline (download → extract → embed → query) | All three |

## Next Steps

- **[Installation](../getting-started/installation.md)** — getting started with the tool
- **[Extract Your Own Bill](../tutorials/extract-your-own-bill.md)** — the full pipeline tutorial
- **[CLI Command Reference](./cli.md)** — complete reference for all commands and flags