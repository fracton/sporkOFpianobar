# Repository Guidelines

## Project Structure & Module Organization

This repository is moving from the original C pianobar client to a Rust replacement. C app/UI/player code lives in `src/`; the embedded Pandora protocol library is in `src/libpiano/`. Treat C as legacy/reference code scheduled for removal. Rust crates are under `rust/`: `pianobar-core` contains API/config/storage logic, and `pianobar-cli` contains the replacement CLI. Scripts, examples, and the man page live in `contrib/`. Configuration examples are in `config` and `contrib/config-example`.

## Build, Test, and Development Commands

- `make`: builds the legacy C `pianobar` binary using system libraries discovered through `pkg-config`.
- `make V=1`: builds verbosely for compiler or linker debugging.
- `make clean`: removes generated C objects, dependency files, binaries, and `libpiano` artifacts.
- `cargo build`: builds the Rust workspace from the root `Cargo.toml`.
- `cargo test`: runs Rust unit/doc tests for all workspace crates.
- `cargo fmt --check`: verifies Rust formatting before review.

The C build expects GNU make, pthreads, libao, libcurl, libgcrypt, json-c, and ffmpeg/libav components.

## Development Direction

Prefer Rust for new features and fixes. Use C to confirm existing behavior, protocol details, and compatibility expectations, but avoid expanding C surface area unless needed to preserve the current build.

## Coding Style & Naming Conventions

Keep C code in the existing C99 style: tabs, same-line opening braces, and prefixes such as `Bar*` for app/UI code and `Piano*` for libpiano APIs. Place public C declarations in matching headers. Rust code should follow `rustfmt` defaults, `snake_case` for functions/modules, `PascalCase` for types, and explicit error enums where practical.

## Testing Guidelines

There is no standalone C test suite; for C changes, run `make` and manually exercise the affected path when possible. For Rust changes, add focused unit tests near the changed module, or integration tests when behavior crosses module boundaries. Run `cargo test` before submitting.

## Commit & Pull Request Guidelines

Recent local history uses short, informal subjects, but prefer actionable imperative summaries such as `Fix station rename response parsing`. Keep each commit focused. Pull requests should describe user-visible behavior, list validation performed (`make`, `cargo test`, manual run), mention dependency or configuration impacts, and link related issues. Include terminal output only when it clarifies failures or compatibility concerns.

## Security & Configuration Tips

Do not commit real Pandora credentials, API tokens, or machine-specific paths. Use example files for documented settings and keep generated build outputs, downloaded media, and local credentials out of version control.
