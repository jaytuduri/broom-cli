use dialoguer::{theme::ColorfulTheme, MultiSelect};
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

const TARGETS: &[&str] = &[
    "target",       // Rust / Cargo / Tauri
    "node_modules", // Node
    "dist",
    "build",
    "out",
    ".next",       // Next.js
    ".nuxt",       // Nuxt
    ".svelte-kit", // SvelteKit
    ".turbo",
    ".vite",
    ".parcel-cache",
    ".gradle",
    "__pycache__",
    ".venv",
    "venv",
    "DerivedData", // Xcode
];

// Generic names only matched when a project marker exists in the parent directory.
const AMBIGUOUS: &[&str] = &["dist", "build", "out"];
const MARKERS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "Makefile",
];

// Don't walk into these — saves time and avoids noise.
const SKIP: &[&str] = &[".git", ".svn", ".hg", ".idea", ".vscode"];

struct Found {
    path: PathBuf,
    size: u64,
}

fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

fn has_marker(dir: &Path) -> bool {
    let parent = match dir.parent() {
        Some(p) => p,
        None => return false,
    };
    MARKERS.iter().any(|m| parent.join(m).exists())
}

fn human(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{:>6.1} {:<2}", size, UNITS[unit])
}

fn find_targets(root: &Path, spinner: &ProgressBar) -> Vec<Found> {
    let mut found = Vec::new();
    let mut walker = WalkDir::new(root).into_iter();

    loop {
        let entry = match walker.next() {
            Some(Ok(e)) => e,
            Some(Err(_)) => continue,
            None => break,
        };

        if !entry.file_type().is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        if SKIP.contains(&name.as_str()) {
            walker.skip_current_dir();
            continue;
        }

        if TARGETS.contains(&name.as_str()) {
            let path = entry.path();

            if AMBIGUOUS.contains(&name.as_str()) && !has_marker(path) {
                walker.skip_current_dir();
                continue;
            }

            let size = dir_size(path);
            found.push(Found {
                path: path.to_path_buf(),
                size,
            });
            spinner.set_message(format!("{} found", found.len()));
            walker.skip_current_dir();
        }
    }

    found
}

fn move_to_trash(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("osascript")
            .arg("-e")
            .arg("on run argv")
            .arg("-e")
            .arg("set itemPath to item 1 of argv")
            .arg("-e")
            .arg("tell application \"Finder\"")
            .arg("-e")
            .arg("delete (POSIX file itemPath as alias)")
            .arg("-e")
            .arg("end tell")
            .arg("-e")
            .arg("end run")
            .arg(path.as_os_str())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let message = stderr.trim();
            Err(io::Error::new(
                io::ErrorKind::Other,
                if message.is_empty() {
                    "Finder could not move the item to Trash".to_string()
                } else {
                    message.to_string()
                },
            ))
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err(io::Error::new(
            io::ErrorKind::Other,
            "moving items to Trash through Finder is only available on macOS",
        ))
    }
}

fn update_self() -> Result<(), Box<dyn std::error::Error>> {
    println!("Fetching and installing the latest broom release...");

    let status = Command::new("cargo")
        .args(["install", "broom-cli", "--force"])
        .status();

    match status {
        Ok(status) if status.success() => {
            println!("broom updated successfully.");
            Ok(())
        }
        Ok(status) => Err(format!("cargo install failed with status {status}").into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(
            "cargo was not found in PATH; install Rust from https://rustup.rs and try again".into(),
        ),
        Err(error) => Err(error.into()),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.get(1).is_some_and(|arg| arg == "update") {
        return update_self();
    }

    let root = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap());
    let root = root.canonicalize().unwrap_or(root);

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} Scanning… {msg}")
            .unwrap(),
    );
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let mut found = find_targets(&root, &spinner);
    spinner.finish_and_clear();

    found.sort_by(|a, b| b.size.cmp(&a.size));

    if found.is_empty() {
        println!("Nothing to clean.");
        return Ok(());
    }

    let total: u64 = found.iter().map(|f| f.size).sum();
    println!(
        "Found {} folder(s), {} total.\n",
        found.len(),
        human(total).trim()
    );

    let items: Vec<String> = found
        .iter()
        .map(|f| {
            let rel = f.path.strip_prefix(&root).unwrap_or(&f.path);
            format!("{}  {}", human(f.size), rel.display())
        })
        .collect();

    let selected = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Space to toggle, Enter to confirm")
        .items(&items)
        .interact()?;

    if selected.is_empty() {
        println!("Nothing selected.");
        return Ok(());
    }

    let freed: u64 = selected.iter().map(|&i| found[i].size).sum();
    print!(
        "\nMove {} folder(s) to Trash, freeing {}? [y/N] ",
        selected.len(),
        human(freed).trim()
    );
    io::stdout().flush()?;
    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm)?;
    if !confirm.trim().eq_ignore_ascii_case("y") {
        println!("Aborted.");
        return Ok(());
    }

    for &i in &selected {
        let path = &found[i].path;
        match move_to_trash(path) {
            Ok(_) => println!("✓ {}", path.display()),
            Err(e) => eprintln!("✗ {}: {}", path.display(), e),
        }
    }

    println!("\nDone. Moved ~{} to Trash.", human(freed).trim());
    Ok(())
}
