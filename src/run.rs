use std::fs::{self, File};
use std::path::{Path, PathBuf};

use crate::bill::bill_plain_named;
use crate::decode::{exceeds_file_cap, prepare_owned, read_capped, rejected_extension, shown_path};
use crate::error::{escape_controls, Error};
use crate::money::ymd_days;
use crate::price::{prices_builtin, Prices};
use crate::report::{render, render_markdown};
use crate::MAX_FILE_BYTES;

/// What to price, and how to print it.
#[derive(Debug, Clone)]
pub struct Options {
    pub prices: Prices,
    pub as_of: Option<String>,
    pub prefix_depth: u8,
    pub json: bool,
    /// Print Markdown tables instead of the text report. Ignored when `json` is set.
    pub markdown: bool,
    /// Today as days since 1970-01-01, for the price-age warning when `as_of` is
    /// absent. `None` skips that check, so the library never reads the clock.
    pub today_days: Option<i64>,
    /// Called with one progress line (no prefix, no newline) at each stage.
    pub progress: Option<fn(&str)>,
}

/// Price data older than this many days, measured to `--as-of` or today, is warned about.
pub const PRICE_MAX_AGE_DAYS: i64 = 180;

impl Default for Options {
    fn default() -> Self {
        Self {
            prices: prices_builtin(),
            as_of: None,
            prefix_depth: 1,
            json: false,
            markdown: false,
            today_days: None,
            progress: None,
        }
    }
}

/// Rendered report. `warnings` is also included in `stdout` for text output.
#[derive(Debug)]
pub struct Output {
    pub stdout: String,
    pub warnings: Vec<String>,
    /// The price-age warning, if any: standard error shows it in every output mode.
    pub price_age: Option<String>,
}

pub fn execute_paths(paths: &[PathBuf], options: &Options) -> Result<Output, Error> {
    if paths.is_empty() {
        return Err(Error::new("no inventory files"));
    }
    let mut files = Vec::with_capacity(paths.len());
    let mut names = Vec::with_capacity(paths.len());
    let say = |line: &str| {
        if let Some(progress) = options.progress {
            progress(line);
        }
    };
    for path in paths {
        say(&format!("reading {}", escape_controls(&shown_path(path))));
        let raw = read_one(path)?;
        let plain = prepare_owned(raw, MAX_FILE_BYTES)
            .map_err(|err| Error::new(format!("{}: {err}", shown_path(path))))?;
        files.push(plain);
        names.push(shown_path(path));
    }
    say(&format!(
        "pricing {} file{}",
        files.len(),
        if files.len() == 1 { "" } else { "s" }
    ));
    let mut bill = bill_plain_named(
        &files,
        &names,
        &options.prices,
        options.as_of.as_deref(),
        options.prefix_depth,
    )?;
    let reference = options
        .as_of
        .as_deref()
        .and_then(ymd_days)
        .map(|days| (days, "--as-of"))
        .or_else(|| options.today_days.map(|days| (days, "today")));
    let mut price_age = None;
    if let Some((days, name)) = reference {
        if let Some(warning) = price_age_warning(&bill.source_date, days, name) {
            bill.warnings.push(warning.clone());
            price_age = Some(warning);
        }
    }
    let warnings = bill.warnings.clone();
    let stdout = if options.markdown && !options.json {
        render_markdown(&bill)
    } else {
        render(&bill, options.json)
    };
    say("done");
    Ok(Output {
        stdout,
        warnings,
        price_age,
    })
}

/// A warning when the price date is more than 180 days before `reference_days`.
fn price_age_warning(source_date: &str, reference_days: i64, name: &str) -> Option<String> {
    let priced = ymd_days(source_date)?;
    (reference_days - priced > PRICE_MAX_AGE_DAYS).then(|| {
        format!("price data dated {source_date} is more than {PRICE_MAX_AGE_DAYS} days older than {name}")
    })
}

fn read_one(path: &Path) -> Result<Vec<u8>, Error> {
    let shown = shown_path(path);
    if let Some(message) = rejected_extension(path) {
        return Err(Error::new(format!("{}: {message}", shown)));
    }
    let meta = fs::metadata(path).map_err(|err| Error::new(format!("{}: {err}", shown)))?;
    if meta.is_dir() {
        return Err(Error::new(format!("{}: is a directory", shown)));
    }
    if !meta.is_file() {
        return Err(Error::new(format!("{}: not a file", shown)));
    }
    if exceeds_file_cap(meta.len()) {
        return Err(Error::new(format!(
            "{}: file is larger than {MAX_FILE_BYTES} bytes",
            shown
        )));
    }
    let mut file = File::open(path).map_err(|err| Error::new(format!("{}: {err}", shown)))?;
    read_capped(&mut file, MAX_FILE_BYTES).map_err(|err| Error::new(format!("{}: {err}", shown)))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::write::GzEncoder;
    use flate2::Compression;

    use super::execute_paths;
    use crate::bill::bill_many;
    use crate::decode::{exceeds_file_cap, prepare_bytes, read_capped};
    use crate::price::prices_builtin;
    use crate::Options;
    use crate::MAX_FILE_BYTES;

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn claim_c17_parquet_orc_gzip_and_no_sdk() {
        let csv = b"Bucket,Key,Size,StorageClass\nb,k,1073741824,STANDARD\n";
        let bill = bill_many(&[gzip(csv).as_slice()], &prices_builtin(), None, 1).unwrap();
        assert_eq!(bill.total_usd, "0.023");
        let err = bill_many(&[b"PAR1not-csv"], &prices_builtin(), None, 1).unwrap_err();
        assert_eq!(
            err.message(),
            "parquet inventories are not read; only CSV and gzip CSV are"
        );
        let err = bill_many(&[b"ORCnot-csv"], &prices_builtin(), None, 1).unwrap_err();
        assert_eq!(
            err.message(),
            "orc inventories are not read; only CSV and gzip CSV are"
        );
        let err = bill_many(
            &[gzip(b"PAR1inside").as_slice()],
            &prices_builtin(),
            None,
            1,
        )
        .unwrap_err();
        assert_eq!(
            err.message(),
            "parquet inventories are not read; only CSV and gzip CSV are"
        );
        let src = include_str!("lib.rs");
        assert!(!src.contains("aws-sdk"));
        assert!(!src.contains("reqwest"));
        assert!(!src.contains("Command::new"));
        let cargo = include_str!("../Cargo.toml");
        assert!(!cargo.contains("aws-sdk"));
        assert!(!cargo.contains("parquet"));
        assert!(!cargo.contains("reqwest"));
        assert!(exceeds_file_cap(MAX_FILE_BYTES + 1));
        assert!(!exceeds_file_cap(MAX_FILE_BYTES));
        assert_eq!(MAX_FILE_BYTES, 1_073_741_824);
    }

    #[test]
    fn file_larger_than_1073741824_bytes() {
        assert!(exceeds_file_cap(1_073_741_825));
        let err = read_capped(&b"abcdef"[..], 4).unwrap_err();
        assert_eq!(err.message(), "file is larger than 4 bytes");
        assert!(!err.message().contains("abcdef"));
        let err = prepare_bytes(b"PAR1", 4).unwrap_err();
        assert!(err.message().contains("parquet"));
    }

    #[test]
    fn directory_is_rejected_and_symlink_is_followed() {
        let root = std::env::temp_dir().join(format!(
            "s3-inventory-bill-{}-{}",
            std::process::id(),
            "sym"
        ));
        let _ = fs_remove(&root);
        std::fs::create_dir_all(&root).unwrap();
        let options = Options::default();
        #[cfg(unix)]
        {
            let csv = root.join("inv.csv");
            std::fs::write(
                &csv,
                b"Bucket,Key,Size,StorageClass\nb,k,1073741824,STANDARD\n",
            )
            .unwrap();
            let link = root.join("link.csv");
            std::os::unix::fs::symlink(&csv, &link).unwrap();
            let out = execute_paths(std::slice::from_ref(&link), &options).unwrap();
            assert!(out.stdout.contains("estimated_usd_month=0.023"));
        }
        let dir = root.join("nested");
        std::fs::create_dir(&dir).unwrap();
        let err = execute_paths(&[dir], &options).unwrap_err();
        assert!(err.message().contains("is a directory"));
        let orc = root.join("report.orc");
        std::fs::write(&orc, b"Bucket,Key,Size,StorageClass\nb,k,1,STANDARD\n").unwrap();
        let err = execute_paths(&[orc], &options).unwrap_err();
        assert!(err
            .message()
            .contains("orc inventories are not read; only CSV and gzip CSV are"));
        let _ = fs_remove(&root);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn non_utf8_names_follow_the_extension_rule() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let root =
            std::env::temp_dir().join(format!("s3-inventory-bill-{}-nonutf8", std::process::id()));
        let _ = fs_remove(&root);
        std::fs::create_dir_all(&root).unwrap();
        let csv = b"Bucket,Key,Size,StorageClass\nb,k,1073741824,STANDARD\n";
        let options = Options::default();
        let bad = root.join(OsStr::from_bytes(b"x\xff.parquet"));
        std::fs::write(&bad, csv).unwrap();
        let err = execute_paths(&[bad], &options).unwrap_err();
        assert!(err
            .message()
            .contains("parquet inventories are not read; only CSV and gzip CSV are"));
        assert!(err.message().contains(r"x\\xff.parquet"));
        assert!(!err.message().contains('\u{fffd}'));
        let good = root.join(OsStr::from_bytes(b"x\xff.csv"));
        std::fs::write(&good, csv).unwrap();
        let out = execute_paths(&[good], &options).unwrap();
        assert!(out.stdout.contains("estimated_usd_month=0.023"));
        let _ = fs_remove(&root);
    }

    fn fs_remove(path: &std::path::Path) -> std::io::Result<()> {
        if path.exists() {
            std::fs::remove_dir_all(path)?;
        }
        Ok(())
    }

    #[test]
    fn nul_byte_is_not_csv() {
        let err = prepare_bytes(b"Bucket,\0Key", 100).unwrap_err();
        assert_eq!(err.message(), "not a CSV inventory");
    }

    #[test]
    fn two_paths_share_the_tier_schedule() {
        let root =
            std::env::temp_dir().join(format!("s3-inventory-bill-{}-two", std::process::id()));
        let _ = fs_remove(&root);
        std::fs::create_dir_all(&root).unwrap();
        let a = root.join("a.csv");
        let b = root.join("b.csv");
        std::fs::write(
            &a,
            b"Bucket,Key,Size,StorageClass\nb,a,1073741824,STANDARD\n",
        )
        .unwrap();
        std::fs::write(
            &b,
            b"Bucket,Key,Size,StorageClass\nb,b,1073741824,STANDARD\n",
        )
        .unwrap();
        let out = execute_paths(&[a, b], &Options::default()).unwrap();
        assert!(out
            .stdout
            .contains("total objects=2 raw_bytes=2147483648 estimated_usd_month=0.046"));
        assert!(out.warnings.is_empty());
        let _ = fs_remove(&root);
    }

    #[test]
    fn gzip_reads_every_member() {
        let first = b"Bucket,Key,Size,StorageClass\nb,a,10,STANDARD\n";
        let second = b"b,c,20,STANDARD\n";
        let mut bytes = gzip(first);
        bytes.extend(gzip(second));
        assert!(bytes.len() < 8 * 1024, "{}", bytes.len());
        let bill = bill_many(&[bytes.as_slice()], &prices_builtin(), None, 1).unwrap();
        assert_eq!(bill.total_objects, 2);
        assert_eq!(bill.total_raw_bytes, 30);
        bytes.extend(b"junk");
        let err = bill_many(&[bytes.as_slice()], &prices_builtin(), None, 1).unwrap_err();
        assert_eq!(err.message(), "not a gzip inventory");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn path_controls_stay_on_one_line() {
        let path = std::env::temp_dir().join("nofile\nfake\u{1b}.csv");
        let err = execute_paths(std::slice::from_ref(&path), &Options::default()).unwrap_err();
        let message = err.message();
        assert!(!message.chars().any(char::is_control), "{message:?}");
        assert!(message.contains("\\n"), "{message}");
        assert!(message.contains("\\u{1b}"), "{message}");
    }
}
