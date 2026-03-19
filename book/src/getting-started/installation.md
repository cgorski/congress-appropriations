# Installation

> **You will need:** A computer running macOS or Linux, and an internet connection.
>
> **You will learn:** How to install `congress-approp` and verify it's working.

## Install Rust

`congress-approp` is written in Rust and requires **Rust 1.93 or later**. If you don't have Rust installed, the easiest way is via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

If you already have Rust, make sure it's up to date:

```bash
rustup update
```

Verify your version:

```bash
rustc --version
# Should show 1.93.0 or later
```

## Install from Source (Recommended)

Cloning the repository gives you the full example data — thirteen pre-extracted appropriations bills (FY2024–FY2026) with pre-computed embeddings, ready to query with no API keys.

```bash
git clone https://github.com/cgorski/congress-appropriations.git
cd congress-appropriations
cargo install --path .
```

This compiles the project and places the `congress-approp` binary on your `PATH`. The first build takes a few minutes; subsequent builds are much faster.

## Install from crates.io

If you just want the binary without cloning the full repository:

```bash
cargo install congress-appropriations
```

> **Note:** The crates.io package does not include the pre-computed embedding vectors (`vectors.bin`) for the example data because they exceed the crates.io 10 MB upload limit. The example bills and extracted provisions are still included. If you want to use semantic search on the example data, run `congress-approp embed --dir examples` after installing (requires `OPENAI_API_KEY`).

## Verify the Installation

Run the summary command against the included example data:

```bash
congress-approp summary --dir examples
```

You should see:

```text
┌───────────┬───────────────────────┬────────────┬─────────────────┬─────────────────┬─────────────────┐
│ Bill      ┆ Classification        ┆ Provisions ┆ Budget Auth ($) ┆ Rescissions ($) ┆      Net BA ($) │
╞═══════════╪═══════════════════════╪════════════╪═════════════════╪═════════════════╪═════════════════╡
│ H.R. 4366 ┆ Omnibus               ┆       2364 ┆ 846,137,099,554 ┆  24,659,349,709 ┆ 821,477,749,845 │
│ H.R. 5860 ┆ Continuing Resolution ┆        130 ┆  16,000,000,000 ┆               0 ┆  16,000,000,000 │
│ H.R. 9468 ┆ Supplemental          ┆          7 ┆   2,882,482,000 ┆               0 ┆   2,882,482,000 │
│ TOTAL     ┆                       ┆       2501 ┆ 865,019,581,554 ┆  24,659,349,709 ┆ 840,360,231,845 │
└───────────┴───────────────────────┴────────────┴─────────────────┴─────────────────┴─────────────────┘

0 dollar amounts unverified across all bills. Run `congress-approp audit` for detailed verification.
```

If you see this table with thirteen bills and 8,554 total provisions, everything is working. You're ready to start querying.

> **Tip:** If you're running from the cloned repo directory, `examples` is a relative path that points to the included example data. If you installed via `cargo install` and are running from a different directory, provide the full path to the examples directory inside your clone.

## API Keys (Optional)

No API keys are needed to query pre-extracted example data. Keys are only required if you want to download new bills, extract provisions from them, or use semantic search:

| Environment Variable | Required For | How to Get It |
|---|---|---|
| `CONGRESS_API_KEY` | Downloading bill XML (`download` command) | Free — [sign up at api.congress.gov](https://api.congress.gov/sign-up/) |
| `ANTHROPIC_API_KEY` | Extracting provisions (`extract` command) | [Sign up at console.anthropic.com](https://console.anthropic.com/) |
| `OPENAI_API_KEY` | Generating embeddings (`embed` command) and semantic search (`search --semantic`) | [Sign up at platform.openai.com](https://platform.openai.com/) |

Set them in your shell when needed:

```bash
export CONGRESS_API_KEY="your-key-here"
export ANTHROPIC_API_KEY="your-key-here"
export OPENAI_API_KEY="your-key-here"
```

See [Environment Variables and API Keys](../reference/environment-variables.md) for details.

## Rebuilding After Source Changes

If you modify the source code (or pull updates), rebuild and reinstall with:

```bash
cargo install --path .
```

For development iteration without reinstalling:

```bash
cargo build --release
./target/release/congress-approp summary --dir examples
```

## Next Steps

You're installed. Head to [Your First Query](./first-query.md) to start exploring the data.