use dialoguer::{theme::ColorfulTheme, Confirm, Input, MultiSelect, Select};
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::io;
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

#[derive(Clone, Copy)]
enum Category {
    Here,
    Downloads,
    Caches,
}

#[derive(Clone, Copy)]
enum Risk {
    ReviewOnly,
}

struct Candidate {
    path: PathBuf,
    size: u64,
    label: String,
    category: Category,
    risk: Risk,
    selected_by_default: bool,
}

impl Candidate {
    fn new(
        root: &Path,
        path: PathBuf,
        size: u64,
        category: Category,
        selected_by_default: bool,
    ) -> Self {
        Self {
            label: label_for(root, &path, size),
            path,
            size,
            category,
            risk: Risk::ReviewOnly,
            selected_by_default,
        }
    }
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

fn file_size(path: &Path) -> u64 {
    path.metadata().map(|m| m.len()).unwrap_or(0)
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

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn downloads_dir() -> Option<PathBuf> {
    home_dir().map(|home| home.join("Downloads"))
}

fn caches_dir() -> Option<PathBuf> {
    home_dir().map(|home| home.join("Library").join("Caches"))
}

fn expand_tilde(input: &str) -> PathBuf {
    if input == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(input));
    }

    if let Some(rest) = input.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }

    PathBuf::from(input)
}

fn label_for(root: &Path, path: &Path, size: u64) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    format!("{}  {}", human(size), rel.display())
}

fn scan_here(root: &Path, spinner: &ProgressBar) -> Vec<Candidate> {
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
            found.push(Candidate::new(
                root,
                path.to_path_buf(),
                size,
                Category::Here,
                true,
            ));
            spinner.set_message(format!("{} found", found.len()));
            walker.skip_current_dir();
        }
    }

    found
}

fn is_download_candidate(path: &Path) -> Option<bool> {
    let file_name = path.file_name()?.to_string_lossy().to_ascii_lowercase();

    if file_name.ends_with(".dmg") || file_name.ends_with(".pkg") || file_name.ends_with(".mpkg") {
        return Some(true);
    }

    if file_name.ends_with(".zip")
        || file_name.ends_with(".tar.gz")
        || file_name.ends_with(".tgz")
        || file_name.ends_with(".rar")
        || file_name.ends_with(".7z")
    {
        return Some(false);
    }

    None
}

fn scan_downloads(spinner: &ProgressBar) -> Vec<Candidate> {
    let Some(root) = downloads_dir() else {
        return Vec::new();
    };

    let mut found = Vec::new();
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(_) => return found,
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(selected_by_default) = is_download_candidate(&path) else {
            continue;
        };

        let size = file_size(&path);
        found.push(Candidate::new(
            &root,
            path,
            size,
            Category::Downloads,
            selected_by_default,
        ));
        spinner.set_message(format!("{} found", found.len()));
    }

    found
}

fn scan_caches(spinner: &ProgressBar) -> Vec<Candidate> {
    let Some(root) = caches_dir() else {
        return Vec::new();
    };

    let mut found = Vec::new();
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(_) => return found,
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let size = dir_size(&path);
        found.push(Candidate::new(&root, path, size, Category::Caches, false));
        spinner.set_message(format!("{} found", found.len()));
    }

    found
}

fn trash_command_error(program: &str, output: std::process::Output, fallback: &str) -> io::Error {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = stderr.trim();
    io::Error::other(if message.is_empty() {
        fallback.to_string()
    } else {
        format!("{program} failed: {message}")
    })
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
            Err(trash_command_error(
                "osascript",
                output,
                "Finder could not move the item to Trash",
            ))
        }
    }

    #[cfg(target_os = "linux")]
    {
        let mut failures = Vec::new();

        for (program, args) in [("gio", &["trash"][..]), ("trash-put", &[][..])] {
            let output = Command::new(program)
                .args(args)
                .arg(path.as_os_str())
                .output();

            match output {
                Ok(output) if output.status.success() => return Ok(()),
                Ok(output) => failures.push(
                    trash_command_error(
                        program,
                        output,
                        &format!("{program} could not move the item to Trash"),
                    )
                    .to_string(),
                ),
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => failures.push(format!("{program} failed: {error}")),
            }
        }

        if failures.is_empty() {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "could not find gio or trash-put; install GLib or trash-cli to move items to Trash",
            ))
        } else {
            Err(io::Error::other(format!(
                "could not move the item to Trash: {}",
                failures.join("; ")
            )))
        }
    }

    #[cfg(target_os = "windows")]
    {
        let command = if path.is_dir() {
            "Add-Type -AssemblyName Microsoft.VisualBasic; [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($args[0], 'OnlyErrorDialogs', 'SendToRecycleBin')"
        } else {
            "Add-Type -AssemblyName Microsoft.VisualBasic; [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($args[0], 'OnlyErrorDialogs', 'SendToRecycleBin')"
        };

        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", command])
            .arg(path.as_os_str())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            Err(trash_command_error(
                "powershell",
                output,
                "PowerShell could not move the item to Recycle Bin",
            ))
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = path;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "moving items to Trash is not supported on this platform",
        ))
    }
}

fn canonical_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn is_protected_path(path: &Path) -> bool {
    let path = canonical_or_original(path);

    if path == Path::new("/") {
        return true;
    }

    if path
        .file_name()
        .is_some_and(|name| [".git", ".ssh", ".gnupg"].contains(&name.to_string_lossy().as_ref()))
    {
        return true;
    }

    if let Some(home) = home_dir() {
        let home = canonical_or_original(&home);
        let protected = [
            home.clone(),
            home.join("Desktop"),
            home.join("Documents"),
            home.join("Downloads"),
            home.join("Library"),
            home.join("Library").join("Caches"),
            home.join(".ssh"),
            home.join(".gnupg"),
        ];

        if protected.iter().any(|p| path == canonical_or_original(p)) {
            return true;
        }
    }

    false
}

fn safe_to_trash(path: &Path, scope_root: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err("path no longer exists".to_string());
    }

    let path = path
        .canonicalize()
        .map_err(|error| format!("could not canonicalize path: {error}"))?;
    let scope_root = scope_root
        .canonicalize()
        .map_err(|error| format!("could not canonicalize scan root: {error}"))?;

    if path == scope_root || !path.starts_with(&scope_root) {
        return Err(format!(
            "path is outside scan root {}",
            scope_root.display()
        ));
    }

    if is_protected_path(&path) {
        return Err("protected path".to_string());
    }

    Ok(path)
}

fn review_and_clean(
    root: &Path,
    mut candidates: Vec<Candidate>,
) -> Result<(), Box<dyn std::error::Error>> {
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.size));

    if candidates.is_empty() {
        println!("Nothing to clean.");
        return Ok(());
    }

    let total: u64 = candidates.iter().map(|candidate| candidate.size).sum();
    println!(
        "Found {} item(s) in {}, {} total.\n",
        candidates.len(),
        root.display(),
        human(total).trim()
    );

    let items: Vec<String> = candidates
        .iter()
        .map(|candidate| candidate.label.clone())
        .collect();
    let defaults: Vec<bool> = candidates
        .iter()
        .map(|candidate| candidate.selected_by_default)
        .collect();

    let selected = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Space to toggle, Enter to confirm")
        .items(&items)
        .defaults(&defaults)
        .interact()?;

    if selected.is_empty() {
        println!("Nothing selected.");
        return Ok(());
    }

    let freed: u64 = selected.iter().map(|&i| candidates[i].size).sum();
    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "Move {} item(s) to Trash, freeing {}?",
            selected.len(),
            human(freed).trim()
        ))
        .default(false)
        .interact()?;

    if !confirmed {
        println!("Aborted.");
        return Ok(());
    }

    let mut trashed = 0;
    let mut failed = 0;
    let mut cleaned = 0;

    for &i in &selected {
        let item = &candidates[i];
        let _category = item.category;
        let _risk = item.risk;

        let path = match safe_to_trash(&item.path, root) {
            Ok(path) => path,
            Err(error) => {
                eprintln!("✗ {}: {}", item.path.display(), error);
                failed += 1;
                continue;
            }
        };

        match move_to_trash(&path) {
            Ok(_) => {
                println!("✓ trashed {}", path.display());
                trashed += 1;
                cleaned += item.size;
            }
            Err(error) => {
                eprintln!("✗ {}: {}", path.display(), error);
                failed += 1;
            }
        }
    }

    println!(
        "\nDone. Trashed {}, skipped {}. Cleaned ~{}.",
        trashed,
        failed,
        human(cleaned).trim()
    );
    Ok(())
}

fn spinner() -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} Scanning… {msg}")
            .unwrap(),
    );
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner
}

fn require_directory(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !root.exists() {
        return Err(format!("{} does not exist.", root.display()).into());
    }

    if !root.is_dir() {
        return Err(format!("{} is not a directory.", root.display()).into());
    }

    Ok(())
}

fn scan_with_spinner(scan: impl FnOnce(&ProgressBar) -> Vec<Candidate>) -> Vec<Candidate> {
    let spinner = spinner();
    let candidates = scan(&spinner);
    spinner.finish_and_clear();
    candidates
}

fn clean_here(root: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    require_directory(&root)?;

    let root = root.canonicalize()?;
    let candidates = scan_with_spinner(|spinner| scan_here(&root, spinner));

    review_and_clean(&root, candidates)
}

fn clean_downloads() -> Result<(), Box<dyn std::error::Error>> {
    let root = downloads_dir().ok_or("Could not find your home directory.")?;
    if !root.exists() {
        return Err(format!("{} does not exist.", root.display()).into());
    }

    let candidates = scan_with_spinner(scan_downloads);

    review_and_clean(&root.canonicalize()?, candidates)
}

fn clean_caches() -> Result<(), Box<dyn std::error::Error>> {
    let root = caches_dir().ok_or("Could not find your home directory.")?;
    if !root.exists() {
        return Err(format!("{} does not exist.", root.display()).into());
    }

    let candidates = scan_with_spinner(scan_caches);

    review_and_clean(&root.canonicalize()?, candidates)
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

fn run_menu() -> Result<(), Box<dyn std::error::Error>> {
    let items = [
        "This directory",
        "Choose another directory",
        "Downloads / installers",
        "Caches",
        "Update broom",
        "Quit",
    ];

    let choice = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("What do you want to clean?")
        .items(&items)
        .default(0)
        .interact()?;

    match choice {
        0 => clean_here(env::current_dir()?),
        1 => {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Directory to clean")
                .interact_text()?;
            clean_here(expand_tilde(input.trim()))
        }
        2 => clean_downloads(),
        3 => clean_caches(),
        4 => update_self(),
        _ => Ok(()),
    }
}

fn route_arg(arg: &str) -> Result<(), Box<dyn std::error::Error>> {
    match arg {
        "update" => update_self(),
        "here" | "." => clean_here(env::current_dir()?),
        "downloads" | "installers" => clean_downloads(),
        "caches" => clean_caches(),
        path => clean_here(expand_tilde(path)),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);

    match args.next() {
        Some(arg) => route_arg(&arg),
        None => run_menu(),
    }
}
