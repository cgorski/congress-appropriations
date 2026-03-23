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

Cloning the repository gives you the full dataset — 32 enacted appropriations bills (FY2019–FY2026) with pre-computed embeddings, ready to query with no API keys.

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

> **Note:** The crates.io package does not include the `data/` directory or pre-computed embedding vectors because they exceed the crates.io upload limit. If you install via crates.io, clone the repository separately to get the dataset, or download and extract your own bills.

## Verify the Installation

Run the summary command against the included data:

```bash
congress-approp summary --dir data
```

You should see a table listing all 32 bills with their provision counts, budget authority, and rescissions. The last line confirms data integrity:

```text
0 dollar amounts unverified across all bills. Run `congress-approp audit` for detailed verification.
```

If you see 32 bills and 34,568 total provisions across FY2019–FY2026, everything is working. You're ready to start querying.

> **Tip:** If you're running from the cloned repo directory, `data` is a relative path that points to the included dataset. If you installed via `cargo install` and are running from a different directory, provide the full path to the `data/` directory inside your clone.

## API Keys (Optional)

No API keys are needed to query the pre-extracted dataset. Keys are only required if you want to download new bills, extract provisions from them, or use semantic search:

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
./target/release/congress-approp summary --dir data
```

## Next Steps

Next: [Your First Query](./first-query.md).