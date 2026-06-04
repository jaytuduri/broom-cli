# broom

A fast CLI for finding common build and cache folders, sorting them by size, and moving the ones you choose to Trash.

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

Install from crates.io with [Rust](https://rustup.rs):

```bash
cargo install broom-cli
```

This installs the `broom` command.

Install from source:

```bash
cargo install --git https://github.com/jaytuduri/broom-cli
```

Or clone and install locally:

```bash
git clone https://github.com/jaytuduri/broom-cli
cd broom-cli
cargo install --path .
```

Pre-built binaries for macOS (Apple Silicon and Intel), Linux, and Windows are available on the [Releases](https://github.com/jaytuduri/broom-cli/releases) page.

## Usage

```bash
broom                  # scan current directory
broom ~/projects       # scan a specific path
broom update           # fetch and install the latest release
```

Use `Space` to toggle folders and `Enter` to confirm your selection. broom asks for one final confirmation before moving anything to Trash.

broom moves selected items to the normal Trash or Recycle Bin instead of deleting them permanently. On Linux, it uses `gio trash` or `trash-put` when available. On macOS, it asks Finder. On Windows, it uses PowerShell to send items to the Recycle Bin. If moving an item to Trash fails, broom reports the failure and leaves the item in place.

## What it finds

| Folder | Stack |
|--------|-------|
| `node_modules` | Node.js |
| `target` | Rust / Cargo |
| `.next` | Next.js |
| `.nuxt` | Nuxt |
| `.svelte-kit` | SvelteKit |
| `dist`, `build`, `out` | Various, only when a project marker exists nearby |
| `.turbo`, `.vite`, `.parcel-cache` | Build tools |
| `.gradle` | Java / Kotlin |
| `__pycache__`, `.venv`, `venv` | Python |
| `DerivedData` | Xcode |

To avoid false positives, generic names like `dist`, `build`, and `out` are only matched when a project marker file (`package.json`, `Cargo.toml`, `go.mod`, etc.) exists in the same directory.

## Customization

Adjust these constants at the top of `src/main.rs`:

- **`TARGETS`** — folder names to find and measure
- **`SKIP`** — folder names to never walk into (e.g. `.git`)

## License

MIT
