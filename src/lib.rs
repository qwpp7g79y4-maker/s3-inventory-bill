//! Offline estimate of S3 storage cost from an Inventory CSV.
//!
//! The library reads bytes you already have. It does not open a socket, it does
//! not link an AWS SDK, and it does not run a lifecycle transition.

#![forbid(unsafe_code)]

mod bill;
mod csv_inv;
mod decode;
mod error;
mod json_scan;
mod money;
mod price;
mod report;
mod run;

pub use bill::{bill_many, Bill, ClassLine, Lifecycle, PrefixLine, TargetLine};
pub use decode::shown_path;
pub use error::{escape_controls, Error};
pub use money::{days_from_civil, format_dec, format_usd, gb_from_bytes};
pub use price::{load_prices_str, prices_builtin, Prices, PRICE_PAGE_DATE, PRICE_PAGE_URL};
pub use report::{render, render_markdown};
pub use run::{execute_paths, Options, Output};

/// Largest input accepted per file, on disk and after gzip, in bytes (1 GiB).
pub const MAX_FILE_BYTES: u64 = 1_073_741_824;

/// Largest `--prices` file. The built-in quote is about 2 KB; the parsed form of
/// a file costs about fifty times its size in memory.
pub const MAX_PRICE_BYTES: u64 = 1_048_576;

/// Page the built-in rates were read from.
pub const PRICE_SOURCE_LINE: &str = "https://aws.amazon.com/s3/pricing/ as of 2026-11-09";
