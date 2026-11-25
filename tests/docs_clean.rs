//! The published documents, the code, the tests and the CI files must not carry
//! local paths or internal names.
//! Every word is written in pieces so that this file does not contain them
//! either.

use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN: [&str; 20] = [
    concat!("/me", "dia/"),
    concat!("/ho", "me/"),
    concat!("~", "/"),
    concat!("--max", "-gb"),
    concat!("--na", "am"),
    concat!("task", ".md"),
    concat!("blueprint", ".md"),
    concat!("geheugen", "wachter"),
    concat!("bouw", "-tmp"),
    concat!("brug", "-prompts"),
    concat!("open", "bot"),
    concat!("co-auth", "ored"),
    concat!("cla", "ude"),
    concat!("gr", "ok"),
    concat!("anthro", "pic"),
    concat!("son", "net"),
    concat!("op", "us"),
    concat!("fa", "ble"),
    concat!("bit", "boi"),
    concat!("pepi", "jn"),
];

/// Single files that are scanned.
const FILES: [&str; 14] = [
    "README.md",
    "README.nl.md",
    "CONTRIBUTING.md",
    "build.rs",
    "about.hbs",
    "about.toml",
    "QUALITY.md",
    "QUALITY.nl.md",
    "CHANGELOG.md",
    "SECURITY.md",
    "deny.toml",
    ".gitattributes",
    ".github/dependabot.yml",
    "Cargo.lock",
];

/// The manifest is scanned too, without the array that excludes the local
/// work files from the package.
const MANIFEST: &str = "Cargo.toml";

/// Directories that are scanned, with every file below them.
/// A packaged tree built outside a checkout drops names that start with `.`.
/// Those paths are scanned only when they are present.
const DIRS: [&str; 7] = [
    "scripts",
    "man",
    "src",
    "tests",
    "examples",
    "prices",
    ".github/workflows",
];

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("{}: {err}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn scanned_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for name in FILES {
        let path = root.join(name);
        if path.is_file() {
            files.push(path);
        }
    }
    for dir in DIRS {
        let path = root.join(dir);
        if path.is_dir() {
            collect(&path, &mut files);
        }
    }
    files
}

/// Each forbidden word found in `text`, with its 1-based line number.
/// Case does not matter.
fn hits(text: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let lower = line.to_lowercase();
        for word in FORBIDDEN {
            if lower.contains(word) {
                found.push((index + 1, word));
            }
        }
    }
    found
}

#[test]
fn documents_code_tests_and_ci_hold_no_local_paths_or_work_file_names() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let files = scanned_files(&root);
    assert!(
        files.len() >= 20,
        "documents, man page, sources, tests and workflows expected, found {}",
        files.len()
    );
    let mut problems = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).unwrap_or_else(|err| {
            panic!("{}: {err}", file.display());
        });
        assert!(!text.is_empty(), "{} is empty", file.display());
        for (line, word) in hits(&text) {
            problems.push(format!("{}:{line}: {word}", file.display()));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Splits a manifest into the text without its `exclude` array and the quoted
/// entries of that array. The array is everything from `exclude = [` up to the
/// first `]`, on one line (as in the repository) or on several lines (as cargo
/// writes it into the package).
fn split_exclude(text: &str) -> (String, Vec<String>) {
    let mut rest = Vec::new();
    let mut entries = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if !inside && line.trim_start().starts_with("exclude") {
            inside = true;
        }
        if !inside {
            rest.push(line);
            continue;
        }
        let mut quoted = line.split('"');
        quoted.next();
        while let (Some(entry), Some(_)) = (quoted.next(), quoted.next()) {
            entries.push(entry.to_string());
        }
        if line.contains(']') {
            inside = false;
        }
    }
    (rest.join("\n"), entries)
}

#[test]
fn the_exclude_array_is_read_on_one_line_and_on_several() {
    let one = "[package]\nname = \"x\"\nexclude = [\"a.md\", \"b.md\"]\nbuild = false\n";
    let many =
        "[package]\nname = \"x\"\nexclude = [\n    \"a.md\",\n    \"b.md\",\n]\nbuild = false\n";
    let expected = (
        "[package]\nname = \"x\"\nbuild = false".to_string(),
        vec!["a.md".to_string(), "b.md".to_string()],
    );
    assert_eq!(split_exclude(one), expected);
    assert_eq!(split_exclude(many), expected);
    assert_eq!(split_exclude("[package]\n").1, Vec::<String>::new());
}

#[test]
fn the_manifest_holds_no_internal_name_and_excludes_exactly_the_work_files() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let text = fs::read_to_string(root.join(MANIFEST)).unwrap();
    let (rest, entries) = split_exclude(&text);
    assert!(rest.contains("[package]"), "{rest}");
    assert!(hits(&rest).is_empty(), "{:?}", hits(&rest));
    let want = vec![concat!("task", ".md"), concat!("blueprint", ".md")];
    assert_eq!(entries, want);
}

#[test]
fn the_scan_reaches_the_ci_file_and_the_sources() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let files = scanned_files(&root);
    for needed in [
        "src/main.rs",
        "src/bill.rs",
        "tests/cli.rs",
        "man/s3-inventory-bill.1",
    ] {
        assert!(
            files.iter().any(|file| file.ends_with(needed)),
            "{needed} is not scanned"
        );
    }
    let ci = ".github/workflows/ci.yml";
    if root.join(ci).is_file() {
        assert!(
            files.iter().any(|file| file.ends_with(ci)),
            "{ci} is not scanned"
        );
    }
}

#[test]
fn the_check_finds_each_pattern() {
    for word in FORBIDDEN {
        assert_eq!(hits(&format!("see {word} here")), vec![(1, word)]);
    }
    assert!(hits("plain text, logs/app/a.bin and --prefix-depth").is_empty());
}

/// The quoted entries of the `accepted` or `allow` array of a license file.
fn license_list(text: &str, key: &str) -> Vec<String> {
    let start = text.find(&format!("{key} = [")).expect("license array");
    let rest = &text[start..];
    let end = rest.find(']').expect("end of the license array");
    rest[..end]
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let inner = line.strip_prefix('"')?.split('"').next()?;
            Some(inner.to_string())
        })
        .collect()
}

#[test]
fn the_third_party_license_listing_accepts_what_cargo_deny_allows() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let about = fs::read_to_string(root.join("about.toml")).expect("about.toml");
    let deny = fs::read_to_string(root.join("deny.toml")).expect("deny.toml");
    let accepted = license_list(&about, "accepted");
    assert!(accepted.len() >= 7, "{accepted:?}");
    assert_eq!(accepted, license_list(&deny, "allow"));
    let template = fs::read_to_string(root.join("about.hbs")).expect("about.hbs");
    assert!(template.contains("{{#each licenses}}"));
    assert!(template.contains("{{text}}"));
}

#[test]
fn the_release_workflow_builds_from_a_tag_after_ci_and_lists_the_licenses() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // A package built outside a checkout drops names that start with `.`.
    if !root.join(".github/workflows/ci.yml").is_file() {
        return;
    }
    let text = fs::read_to_string(root.join(".github/workflows/release.yml"))
        .expect("the release workflow sits next to ci.yml");
    assert!(text.contains("tags: [\"v*\"]"));
    assert!(text.contains("uses: ./.github/workflows/ci.yml"));
    assert!(text.contains("about.hbs"));
    assert!(text.contains("THIRD-PARTY-LICENSES.html"));
    assert!(text.contains("BIN: s3-inventory-bill"));
    assert!(text.contains("--version"));
    // Only the last job may write; the default is read-only.
    assert!(text.contains("permissions:\n  contents: read"));
    assert_eq!(text.matches("contents: write").count(), 1);
    let ci = fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("workflow_call"));
}
