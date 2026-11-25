# Contributing

Thanks for your interest in s3-inventory-bill.

## Building and testing

```sh
cargo build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

These checks must pass before a change is merged, also on Rust 1.85 (the minimum supported version): `cargo +1.85 clippy --all-targets -- -D warnings` and `cargo +1.85 test`. The README examples are tests: a console block that no longer matches the binary fails `cargo test`, so update the document and the code together.

Test fixtures must be synthetic. Never add a real inventory, a real bucket name, key or account, or a real token. Money is exact or an error: do not add a silent rounding.

## Commit messages

Commit messages are short English sentences that say what changed, for example `Escape controls in values shown on the report`.

## Reporting problems

Bugs and questions go to GitHub Issues. Security problems go through the private route in [SECURITY.md](SECURITY.md), not a public issue.

## Contact

PEPAI, pep_ai@icloud.com
