# Contributing

## Code Quality

- Fix clippy warnings at the root cause, not with `#[allow]` or `_` prefixes on variables
- Run `cargo fmt` before committing
- Run `cargo test` and ensure all tests pass
- Run `cargo clippy` and ensure no warnings

## Pull Requests

- Keep commits focused and well-described
- Include test coverage for new functionality
- Update documentation when changing user-facing behavior