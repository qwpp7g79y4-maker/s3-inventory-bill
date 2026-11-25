use std::io::{stdout, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

#[allow(unsafe_code)]
mod interrupt;
#[allow(unsafe_code)]
mod stdout_probe;

use s3_inventory_bill::{
    escape_controls, execute_paths, load_prices_str, shown_path, Error, Options, MAX_PRICE_BYTES,
};

/// `-V` and `--version` print this. A debug build adds the commit that the build
/// script found; a release build prints the crate version only.
#[cfg(all(debug_assertions, has_git_commit))]
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_COMMIT"), ")");
#[cfg(not(all(debug_assertions, has_git_commit)))]
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Estimate S3 storage cost from an inventory CSV. Reads files only, no network.
#[derive(Parser)]
#[command(
    name = "s3-inventory-bill",
    version = VERSION,
    about = "Estimate S3 storage cost from an inventory CSV. Reads files only, no network.",
    after_help = "Exit codes: 0 ok, 2 input or usage error, 130 interrupted (SIGINT).",
    term_width = 0
)]
struct Cli {
    /// S3 Inventory CSV files, or gzip-compressed CSV.
    #[arg(required = true, value_name = "FILE")]
    files: Vec<PathBuf>,
    /// Price file. The default is the built-in US East (N. Virginia) list.
    #[arg(long, value_name = "FILE")]
    prices: Option<PathBuf>,
    /// Day that ages are measured to, `YYYY-MM-DD`. An object last modified on or before that day is eligible; the time of day is not read. Without it, savings are omitted.
    #[arg(long, value_name = "YYYY-MM-DD")]
    as_of: Option<String>,
    /// Path segments in the prefix, 1 to 8. `logs/2024/a` at depth 1 is `logs/`.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=8))]
    prefix_depth: u8,
    /// Print JSON instead of the text table.
    #[arg(long)]
    json: bool,
    /// Print Markdown tables instead of the text table.
    #[arg(long, conflicts_with = "json")]
    md: bool,
    /// Write the report to this file instead of standard output. The file is written whole and renamed into place; an existing file is replaced, a symbolic link is replaced and not followed.
    #[arg(long, value_name = "FILE")]
    output: Option<PathBuf>,
    /// Print only the report and errors: no warning copies on standard error.
    #[arg(long, conflicts_with = "verbose")]
    quiet: bool,
    /// Also print progress lines on standard error.
    #[arg(long, conflicts_with = "quiet")]
    verbose: bool,
}

fn main() -> ExitCode {
    // Rust replaces a closed fd 1 with /dev/null (read-write) before main;
    // the probe records before that whether fd 1 was closed.
    if stdout_probe::stdout_was_closed_at_start() {
        let _ = writeln!(
            std::io::stderr(),
            "s3-inventory-bill: could not write output"
        );
        return ExitCode::from(2);
    }
    interrupt::install();
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let to_stderr = err.use_stderr();
            let printed = if to_stderr {
                print_usage_error(&err)
            } else {
                err.print().is_ok()
            };
            if !to_stderr && !printed {
                let _ = writeln!(
                    std::io::stderr(),
                    "s3-inventory-bill: could not write output"
                );
                return ExitCode::from(2);
            }
            return ExitCode::from(if to_stderr { 2 } else { 0 });
        }
    };
    match run(&cli) {
        Ok(output) => {
            if !cli.quiet {
                if cli.json {
                    for warning in &output.warnings {
                        let _ = writeln!(std::io::stderr(), "warning: {warning}");
                    }
                } else if let Some(warning) = &output.price_age {
                    let _ = writeln!(std::io::stderr(), "warning: {warning}");
                }
            }
            if let Some(path) = &cli.output {
                return match write_report_file(path, &output.stdout) {
                    Ok(()) => {
                        if cli.verbose {
                            print_progress(&format!(
                                "wrote {}",
                                escape_controls(&shown_path(path))
                            ));
                        }
                        ExitCode::SUCCESS
                    }
                    Err(err) => {
                        let _ = writeln!(std::io::stderr(), "s3-inventory-bill: {err}");
                        ExitCode::from(2)
                    }
                };
            }
            let written = interrupt::guarded(|| {
                let mut out = stdout().lock();
                out.write_all(output.stdout.as_bytes()).is_ok() && out.flush().is_ok()
            });
            if !written {
                let _ = writeln!(
                    std::io::stderr(),
                    "s3-inventory-bill: could not write output"
                );
                return ExitCode::from(2);
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            let _ = writeln!(std::io::stderr(), "s3-inventory-bill: {err}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: &Cli) -> Result<s3_inventory_bill::Output, Error> {
    let prices = if let Some(path) = &cli.prices {
        let shown = shown_path(path);
        let meta =
            std::fs::metadata(path).map_err(|err| Error::new(format!("{}: {err}", shown)))?;
        if !meta.is_file() {
            return Err(Error::new(format!("{}: not a file", shown)));
        }
        if meta.len() > MAX_PRICE_BYTES {
            return Err(Error::new(format!(
                "{}: file is larger than {MAX_PRICE_BYTES} bytes",
                shown
            )));
        }
        let text = read_prices_text(path, meta.len())
            .map_err(|err| Error::new(format!("{}: {err}", shown)))?;
        load_prices_str(&text).map_err(|err| Error::new(format!("price file {}: {err}", shown)))?
    } else {
        s3_inventory_bill::prices_builtin()
    };
    let options = Options {
        prices,
        as_of: cli.as_of.clone(),
        prefix_depth: cli.prefix_depth,
        json: cli.json,
        markdown: cli.md,
        today_days: today_days(),
        progress: cli.verbose.then_some(print_progress as fn(&str)),
    };
    execute_paths(&cli.files, &options)
}

/// Write `text` to a temporary file next to `path` and rename it over `path`,
/// so the file is the old report or the whole new one. A symbolic link at
/// `path` is replaced, not followed; a directory or a special file is refused.
/// A SIGINT meanwhile waits until the step is done, or, when it came before the
/// rename, leaves `path` as it was.
fn write_report_file(path: &std::path::Path, text: &str) -> Result<(), Error> {
    let fail = |reason: &str| {
        Error::new(format!(
            "could not write report to {}: {reason}",
            shown_path(path)
        ))
    };
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => return Err(fail("is a directory")),
        Ok(meta) if !meta.is_file() && !meta.file_type().is_symlink() => {
            return Err(fail("not a regular file"))
        }
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(fail(&err.to_string())),
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let name = path
        .file_name()
        .map_or_else(|| "report".into(), std::ffi::OsStr::to_os_string);
    interrupt::guarded(|| {
        let mut created = None;
        for attempt in 0..64u32 {
            let mut tmp_name = std::ffi::OsString::from(".");
            tmp_name.push(&name);
            tmp_name.push(format!(".{}.{attempt}.tmp", std::process::id()));
            let tmp = parent.join(tmp_name);
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)
            {
                Ok(file) => {
                    created = Some((tmp, file));
                    break;
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(fail(&err.to_string())),
            }
        }
        let Some((tmp, mut file)) = created else {
            return Err(fail("no free temporary file name"));
        };
        let written = file
            .write_all(text.as_bytes())
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all());
        drop(file);
        if let Err(err) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(fail(&err.to_string()));
        }
        if interrupt::pending() {
            let _ = std::fs::remove_file(&tmp);
            return Ok(());
        }
        std::fs::rename(&tmp, path).map_err(|err| {
            let _ = std::fs::remove_file(&tmp);
            fail(&err.to_string())
        })
    })
}

fn print_progress(line: &str) {
    let _ = writeln!(std::io::stderr(), "s3-inventory-bill: {line}");
}

/// Today in whole days since 1970-01-01 (UTC). `SOURCE_DATE_EPOCH`, when it is a
/// number of seconds, stands in for the clock so a run can be repeated.
fn today_days() -> Option<i64> {
    let seconds = match std::env::var("SOURCE_DATE_EPOCH") {
        Ok(text) => text.trim().parse::<i64>().ok()?,
        Err(_) => i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs(),
        )
        .ok()?,
    };
    Some(seconds.div_euclid(86_400))
}

/// Reads at most `len` bytes, the length the metadata reported.
fn read_prices_text(path: &std::path::Path, len: u64) -> std::io::Result<String> {
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(path)?
        .take(len)
        .read_to_string(&mut text)?;
    Ok(text)
}

/// Clap echoes a bad flag or value as typed. Parse again with the arguments
/// escaped so the message shows `\u{..}` forms and no raw control character.
fn print_usage_error(err: &clap::Error) -> bool {
    let escaped_args = std::env::args_os().map(|arg| {
        let text = arg.to_string_lossy();
        std::ffi::OsString::from(escape_controls(&text).into_owned())
    });
    let text = match Cli::try_parse_from(escaped_args) {
        Err(again) => again.render().to_string(),
        Ok(_) => escape_controls(&err.render().to_string()).into_owned(),
    };
    write!(std::io::stderr(), "{text}").is_ok()
}
