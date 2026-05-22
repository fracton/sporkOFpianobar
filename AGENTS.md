# Repository Guidelines

## Project Structure & Module Organization

This repository contains the original C pianobar client plus an in-progress Rust workspace. C application entry points and UI/player code live in `src/`; the embedded Pandora protocol library is in `src/libpiano/`. Rust crates are under `rust/`: `pianobar-core` contains reusable API/config/storage logic, and `pianobar-cli` contains the CLI binary. Contributor scripts, examples, and the man page live in `contrib/`. Configuration examples are in `config` and `contrib/config-example`.

## Build, Test, and Development Commands

- `make`: builds the C `pianobar` binary using system libraries discovered through `pkg-config`.
- `make V=1`: builds verbosely, useful when debugging compiler or linker flags.
- `make clean`: removes generated C objects, dependency files, binaries, and local `libpiano` artifacts.
- `cargo build`: builds the Rust workspace from the root `Cargo.toml`.
- `cargo test`: runs Rust unit/doc tests for all workspace crates.
- `cargo fmt --check`: verifies Rust formatting before review.

The C build expects GNU make, pthreads, libao, libcurl, libgcrypt, json-c, and ffmpeg/libav components.

## Coding Style & Naming Conventions

Keep C code in the existing C99 style: tabs for indentation, same-line opening braces, and prefixes such as `Bar*` for app/UI code and `Piano*` for libpiano APIs. Place public C declarations in matching headers beside implementations. Rust code should follow `rustfmt` defaults, `snake_case` for functions/modules, `PascalCase` for types, and explicit error enums where practical.

## Testing Guidelines

There is no standalone C test suite in this tree; for C changes, at minimum run `make` and manually exercise the affected command path when possible. For Rust changes, add focused unit tests near the changed module or integration tests under the relevant crate when behavior crosses module boundaries. Run `cargo test` before submitting Rust changes.

## Commit & Pull Request Guidelines

Recent local history uses short, informal subjects, but prefer actionable imperative summaries such as `Fix station rename response parsing` or `Add config path fallback`. Keep each commit focused on one logical change. Pull requests should describe user-visible behavior, list validation performed (`make`, `cargo test`, manual run), mention dependency or configuration impacts, and link related issues. Include terminal output only when it clarifies failures or compatibility concerns.

## Security & Configuration Tips

Do not commit real Pandora credentials, API tokens, or machine-specific paths. Use example files for documented settings and keep generated build outputs, downloaded media, and local credentials out of version control.
