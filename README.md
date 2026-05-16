# devclean

A fast CLI tool that scans a directory tree for common build and cache folders, shows them sorted by size, and lets you pick which ones to delete.

```
⠸ Scanning… 12 found

Found 31 folder(s), 40.2 GB total.

  Space to toggle, Enter to confirm
> [ ] 15.3 GB  projects/my-app/node_modules
  [ ]  8.1 GB  projects/old-api/target
  [ ]  4.2 GB  projects/site/.next
  [ ]  ...
```

## Install

**From source (requires [Rust](https://rustup.rs)):**

```bash
cargo install --git https://github.com/jaytuduri/devclean
```

**Or clone and install locally:**

```bash
git clone https://github.com/jaytuduri/devclean
cd devclean
cargo install --path .
```

**Pre-built binaries** are available on the [Releases](https://github.com/jaytuduri/devclean/releases) page for macOS (Apple Silicon + Intel), Linux, and Windows.

## Usage

```bash
devclean                  # scan current directory
devclean ~/projects       # scan a specific path
```

Use `Space` to toggle folders, `Enter` to confirm the selection. You'll get one final confirmation before anything is deleted.

## What it finds

| Folder | Stack |
|--------|-------|
| `node_modules` | Node.js |
| `target` | Rust / Cargo |
| `.next` | Next.js |
| `.nuxt` | Nuxt |
| `.svelte-kit` | SvelteKit |
| `dist`, `build`, `out` | Various (only when a project marker exists nearby) |
| `.turbo`, `.vite`, `.parcel-cache` | Build tools |
| `.gradle` | Java / Kotlin |
| `__pycache__`, `.venv`, `venv` | Python |
| `DerivedData` | Xcode |

Generic names like `dist`, `build`, and `out` are only matched when a project marker file (`package.json`, `Cargo.toml`, `go.mod`, etc.) exists in the same directory, to avoid false positives.

## Customize

Two constants at the top of `src/main.rs`:

- **`TARGETS`** — folder names to find and measure
- **`SKIP`** — folder names to never walk into (e.g. `.git`)

## License

MIT
