# Contributing

Thanks for contributing to rfid-silion-compat.

## Development Setup

1. Install Rust (stable toolchain).
2. Optional for wasm/web work: install `wasm-pack`, Node.js, and pnpm.

3. Clone the repository and run checks:

```bash
cargo check
cargo test
cargo check --features serial --examples
cargo check --target wasm32-unknown-unknown --features web-serial
```

4. **Lint your code**: Run `cargo fmt --all -- --check` and ensure there are no formatting issues before submitting changes. To auto-format, run `cargo fmt --all`.

## Coding Guidelines

- Keep changes focused and minimal.
- Add or update tests when behavior changes.
- Keep public APIs documented.
- Prefer preserving existing style and naming conventions.

## Commit and Pull Request Guidelines

- Use clear commit messages that explain intent.
- In PRs, include:
  - What changed
  - Why it changed
  - How it was tested
  - Any breaking changes

## Reporting Issues

Please include:

- Expected behavior
- Actual behavior
- Steps to reproduce
- Relevant logs/error output
- Platform details (OS, Rust version, browser version for web-serial)

## License

By contributing, you agree that your contributions are licensed under:

- MIT
- Apache-2.0
