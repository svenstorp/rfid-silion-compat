# Copilot Instructions

These instructions apply to the whole repository.

## Rust and Cargo Best Practices

- Keep public APIs small and strongly typed. Prefer enums and structs over untyped integers and strings when protocol options are known.
- Favor explicit error propagation with meaningful error types over panics in library code.
- Keep feature-gated code isolated and clear:
  - `serial` for native serial support.
  - `web-serial` for wasm/browser bindings.
- Make feature interactions testable. When changing shared code paths, verify native and wasm builds.
- Preserve backward compatibility for public APIs unless a breaking change is explicitly intended and documented.
- Add or update docs and examples alongside API changes.
- Prefer small, focused modules and functions with descriptive names.
- Run formatting and checks before proposing changes:
  - `cargo fmt --all`
  - `cargo check`
  - `cargo test --lib`
  - `cargo check --features serial --examples`
  - `cargo check --target wasm32-unknown-unknown --features web-serial`

## TypeScript and pnpm Best Practices

- Use `pnpm` for all Node workflows in this repository.
- Keep TypeScript strict and type-first. Avoid `any` unless unavoidable.
- Prefer typed input/output shapes that match wasm bindings.
- Ensure browser-only APIs (for example Web Serial) are used behind runtime checks and user gesture flow where required.
- Keep wasm initialization explicit and awaited before using exported APIs.
- Keep package scripts reproducible and CI-friendly:
  - `pnpm install --frozen-lockfile`
  - `pnpm run check`
  - `pnpm run build`
- When changing web example or npm-facing APIs, update docs in `npm/README.md` and relevant examples.

## Staging a New Version

When preparing a release, perform all items below in one coherent change set.

### Versioning Policy (SemVer)

- Use Semantic Versioning: `MAJOR.MINOR.PATCH`.
- Bump `PATCH` for backward-compatible fixes, documentation-only updates, and non-breaking CI/build improvements.
- Bump `MINOR` for backward-compatible new features, new APIs, and additive protocol support.
- Bump `MAJOR` for any breaking change in public Rust APIs, wasm/TypeScript APIs, behavior contracts, or required runtime assumptions.
- If a change is potentially breaking, treat it as breaking unless migration impact is clearly documented and validated as non-breaking.
- For pre-1.0 releases, still follow the same intent and call out any breaking changes explicitly in `CHANGELOG.md`.

### 1. Bump Cargo Version

- Update `version` in `Cargo.toml`.
- Ensure npm package version produced from Cargo remains aligned with the crate version.

### 2. Update Changelog

- Add a new top section in `CHANGELOG.md` for the release version.
- Summarize notable changes by category (for example Added, Changed, Fixed, Docs, CI).
- Keep entries user-focused and concrete.
- Mention breaking changes clearly and include migration notes when relevant.

### 3. Validate Before Tagging

- Run Rust checks/tests for default, `serial`, and `web-serial` paths.
- Run web example checks/build with pnpm.
- Confirm CI workflow and branch protection required checks still match actual job names.

### 4. Release Hygiene

- Keep version bump and changelog update in the same PR.
- Use a clear PR title like `release: vX.Y.Z`.
- After merge, tag the release version in git and publish artifacts in the expected order.
