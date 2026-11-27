# Security policy

## Supported versions

| Version | Supported |
| --- | --- |
| 1.0.x | Yes |

Security fixes go into the latest release only. The crate has no git tag.

## Reporting a vulnerability

Please report security problems privately, not in a public issue: open the **Security** tab of this repository and choose **Report a vulnerability**. If that is not possible, write to pep_ai@icloud.com. Examples are a CSV cell, file name, or tag that can add a fake line or a terminal escape to the report or to an error, an error that echoes a cell it should hide, a way to make s3-inventory-bill write, run, or connect to something, a file that makes it read without the stated size limit or hang, and input that changes an amount without an error.

Memory use with many distinct prefixes or storage class names is a documented limit (see the README, "Memory and time"), not a vulnerability by itself.

Please include:

- the s3-inventory-bill version (the `version` field in `Cargo.toml`, or the commit you built) and your operating system;
- the exact command;
- a minimal synthetic CSV or `.csv.gz`, and a price file if you use one, that reproduces the problem, never a real inventory;
- what s3-inventory-bill printed and what you expected.

We aim to respond within 7 days (best effort). Once a fix is released, the report can be credited in the changelog if you wish.

## What the tool does and does not do

- It reads the files named on the command line and, with `--prices`, one regular price file. It writes to standard output and standard error. Only with `--output <path>` does it also write a file: the report goes to a temporary file in the same directory (named `.<name>.<process id>.<n>.tmp`), which is then renamed onto `<path>`. An existing file at `<path>` is replaced; a symbolic link at `<path>` is replaced and not followed, so its target is left unchanged. A directory or another non-regular file is refused. The temporary file is removed if the write fails or the run is interrupted. Nothing else is written.
- It makes no network call. The normal dependencies are `clap`, `flate2`, `rust_decimal` and `serde_json`; none is an HTTP, DNS or AWS client.
- The built tool starts no subprocess and links no AWS SDK. Only the build script may run local `git` during `cargo build`; if `git` is missing or fails, the release binary prints just the crate version.
- A file larger than 1073741824 bytes is refused, on disk and after gzip inflation. A gzip file with more than 100000 members is refused, and so is a `--prices` file larger than 1048576 bytes. A record has at most 64 columns, checked before the fields are copied.
- Error messages do not repeat a cell, key or price value. Control and bidirectional characters in file names and labels are escaped in text and JSON output.
- The binary has two modules with `unsafe` code, `stdout_probe` and `interrupt`; the library has none and forbids `unsafe`. `stdout_probe` makes one read-only `fcntl` call that notices a closed standard output; `interrupt` installs the SIGINT handler.
