# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Commands

```bash
cargo build              # debug build
cargo build --release    # optimized binary → target/release/broom
cargo run -- ~/projects  # run against a path
cargo install --path .   # install to ~/.cargo/bin/broom
```

## Architecture

Single-file Rust CLI: `src/main.rs`.

**Flow:** scan → sort by size → interactive checkbox → confirm → move to Trash.

Two tunable constants at the top of `main.rs`:
- `TARGETS` — folder names to match and measure (e.g. `node_modules`, `target`, `.next`)
- `SKIP` — folder names to never walk into (e.g. `.git`, `.vscode`)

When a folder name matches `TARGETS`, `walkdir` skips descending into it (`skip_current_dir()`), so nested matches don't double-count. Size is computed with a second `WalkDir` pass over matched folders.

**Dependencies:** `walkdir` for traversal, `dialoguer` for the checkbox UI.
