use std::fmt::Write as _;

use serde_json::{json, Value};

use crate::bill::{Bill, Lifecycle};
use crate::error::{escape_controls, needs_escape};

/// The one line that says the amounts are list-price estimates.
pub const ESTIMATE_NOTE: &str =
    "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)";

/// Version of the JSON layout, the first key of every JSON document.
pub const SCHEMA_VERSION: u32 = 1;

/// Text table, or pretty JSON when `json` is set.
#[must_use]
pub fn render(bill: &Bill, json: bool) -> String {
    if json {
        render_json(bill)
    } else {
        render_text(bill)
    }
}

fn render_text(bill: &Bill) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "s3-inventory-bill");
    let _ = writeln!(out, "source: {}", escape_controls(&bill.source_url));
    let _ = writeln!(out, "as of: {}", bill.source_date);
    let _ = writeln!(
        out,
        "region: {} ({})",
        escape_controls(&bill.region),
        escape_controls(&bill.region_name)
    );
    let _ = writeln!(out, "prefix_depth: {}", bill.prefix_depth);
    let _ = writeln!(out, "{ESTIMATE_NOTE}");
    let _ = writeln!(out);
    let _ = writeln!(out, "storage by class");
    for class in &bill.classes {
        let usd = class.usd_month.as_deref().unwrap_or("unpriced");
        let _ = writeln!(
            out,
            "{} objects={} raw_bytes={} billable_bytes={} estimated_usd_month={usd}",
            escape_controls(&class.name),
            class.objects,
            class.raw_bytes,
            class.billable_bytes
        );
    }
    let _ = writeln!(
        out,
        "total objects={} raw_bytes={} estimated_usd_month={}",
        bill.total_objects, bill.total_raw_bytes, bill.total_usd
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "storage by prefix");
    for prefix in &bill.prefixes {
        let _ = writeln!(
            out,
            "{} objects={} raw_bytes={} estimated_usd_month={}",
            text_line_key(&prefix.prefix),
            prefix.objects,
            prefix.raw_bytes,
            prefix.usd_month
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "lifecycle");
    match &bill.lifecycle {
        Some(life) => write_lifecycle(&mut out, life),
        None => {
            let _ = writeln!(out, "omitted: pass --as-of YYYY-MM-DD");
        }
    }
    for warning in &bill.warnings {
        let _ = writeln!(out, "warning: {warning}");
    }
    out
}

/// The report as Markdown tables. Every amount is the same string as in the text
/// report and the JSON.
#[must_use]
pub fn render_markdown(bill: &Bill) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# s3-inventory-bill");
    let _ = writeln!(out);
    let _ = writeln!(out, "{ESTIMATE_NOTE}");
    let _ = writeln!(out);
    let _ = writeln!(out, "- Source: {}", md_text(&bill.source_url));
    let _ = writeln!(out, "- As of: {}", bill.source_date);
    let _ = writeln!(
        out,
        "- Region: {} ({})",
        md_text(&bill.region),
        md_text(&bill.region_name)
    );
    let _ = writeln!(out, "- Prefix depth: {}", bill.prefix_depth);
    let _ = writeln!(out);
    let _ = writeln!(out, "## Storage by class");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "| Class | Objects | Raw bytes | Billable bytes | Estimated USD per month |"
    );
    let _ = writeln!(out, "| --- | ---: | ---: | ---: | ---: |");
    for class in &bill.classes {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            md_text(&class.name),
            class.objects,
            class.raw_bytes,
            class.billable_bytes,
            class.usd_month.as_deref().unwrap_or("unpriced")
        );
    }
    let _ = writeln!(
        out,
        "| **Total** | {} | {} |  | {} |",
        bill.total_objects, bill.total_raw_bytes, bill.total_usd
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "## Storage by prefix");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "| Prefix | Objects | Raw bytes | Estimated USD per month |"
    );
    let _ = writeln!(out, "| --- | ---: | ---: | ---: |");
    for prefix in &bill.prefixes {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            md_text(&prefix.prefix),
            prefix.objects,
            prefix.raw_bytes,
            prefix.usd_month
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "## Lifecycle");
    let _ = writeln!(out);
    match &bill.lifecycle {
        None => {
            let _ = writeln!(out, "Omitted: pass --as-of YYYY-MM-DD");
        }
        Some(life) => {
            let _ = writeln!(out, "As of: {}", life.as_of);
            let _ = writeln!(out);
            let _ = writeln!(
                out,
                "| Target | Horizon days | Eligible objects | Monthly now USD | Monthly after USD | Transition request USD | Early delete USD | Savings over horizon USD | Status |"
            );
            let _ = writeln!(
                out,
                "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |"
            );
            for target in &life.targets {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} | {} | {} | {} | Not executed |",
                    md_text(&target.name),
                    target.horizon_days,
                    target.eligible_objects,
                    target.monthly_now_usd,
                    target.monthly_after_usd,
                    target.transition_request_usd,
                    target.early_delete_usd,
                    target.savings_over_horizon_usd
                );
            }
        }
    }
    if !bill.warnings.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "## Warnings");
        let _ = writeln!(out);
        for warning in &bill.warnings {
            let _ = writeln!(out, "- {}", md_text(warning));
        }
    }
    out
}

/// Text for a table cell or list item: control characters and line breaks are
/// escaped like in the text report, then every character that Markdown or a
/// renderer could act on gets a backslash, so a key from the bucket stays
/// plain text (no link, image, emphasis, code, HTML or autolink).
fn md_text(text: &str) -> String {
    let plain = escape_controls(text);
    let chars: Vec<char> = plain.chars().collect();
    let mut out = String::with_capacity(plain.len());
    for (i, &ch) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| chars[j]);
        let next = chars.get(i + 1).copied();
        let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
        let special = matches!(
            ch,
            '|' | '`' | '*' | '[' | ']' | '<' | '>' | '&' | '!' | '~' | '#' | '@'
        ) || (ch == '_' && !(word(prev) && word(next)))
            || (ch == '/' && prev == Some(':'))
            // `www.` would become a link on GitHub; a backslash before the dot stops it.
            || (ch == '.' && i >= 3 && chars[i - 3..i].iter().all(|c| c.eq_ignore_ascii_case(&'w')));
        if special {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// A key at the start of a text line: control characters are escaped, and a key
/// that starts with `total` or `warning` gets its first letter written as
/// `\u{..}`, so no key can make a second line that starts like one of the tool's
/// own lines. A backslash in a key is already doubled, so this cannot be forged.
fn text_line_key(key: &str) -> String {
    let escaped = escape_controls(key);
    for word in ["total", "warning"] {
        if let Some(rest) = escaped.strip_prefix(&word[..1]) {
            if rest.starts_with(&word[1..]) {
                return format!("\\u{{{:x}}}{rest}", u32::from(word.as_bytes()[0]));
            }
        }
    }
    escaped.into_owned()
}

fn write_lifecycle(out: &mut String, life: &Lifecycle) {
    let _ = writeln!(out, "as-of: {}", life.as_of);
    for target in &life.targets {
        let _ = writeln!(
            out,
            "{} horizon_days={} eligible_objects={} monthly_now_usd={} monthly_after_usd={} transition_request_usd={} early_delete_usd={} savings_over_horizon_usd={} Not executed",
            target.name,
            target.horizon_days,
            target.eligible_objects,
            target.monthly_now_usd,
            target.monthly_after_usd,
            target.transition_request_usd,
            target.early_delete_usd,
            target.savings_over_horizon_usd
        );
    }
}

fn render_json(bill: &Bill) -> String {
    let classes: Vec<Value> = bill
        .classes
        .iter()
        .map(|line| {
            json!({
                "class": line.name,
                "objects": line.objects,
                "raw_bytes": line.raw_bytes.to_string(),
                "billable_bytes": line.billable_bytes.to_string(),
                "estimated_usd_month": line.usd_month,
            })
        })
        .collect();
    let prefixes: Vec<Value> = bill
        .prefixes
        .iter()
        .map(|line| {
            json!({
                "prefix": line.prefix,
                "objects": line.objects,
                "raw_bytes": line.raw_bytes.to_string(),
                "estimated_usd_month": line.usd_month,
            })
        })
        .collect();
    let lifecycle = match &bill.lifecycle {
        None => Value::Null,
        Some(life) => json!({
            "as_of": life.as_of,
            "targets": life.targets.iter().map(|target| json!({
                "target": target.name,
                "horizon_days": target.horizon_days,
                "eligible_objects": target.eligible_objects,
                "monthly_now_usd": target.monthly_now_usd,
                "monthly_after_usd": target.monthly_after_usd,
                "transition_request_usd": target.transition_request_usd,
                "early_delete_usd": target.early_delete_usd,
                "savings_over_horizon_usd": target.savings_over_horizon_usd,
                "not_executed": true,
            })).collect::<Vec<_>>(),
        }),
    };
    let value = json!({
        "schema_version": SCHEMA_VERSION,
        "estimate": true,
        "source_url": bill.source_url,
        "source_date": bill.source_date,
        "region": bill.region,
        "region_name": bill.region_name,
        "prefix_depth": bill.prefix_depth,
        "objects": bill.total_objects,
        "raw_bytes": bill.total_raw_bytes.to_string(),
        "estimated_usd_month": bill.total_usd,
        "classes": classes,
        "prefixes": prefixes,
        "lifecycle": lifecycle,
        "warnings": bill.warnings,
    });
    let mut text = serde_json::to_string_pretty(&value).expect("json");
    text.push('\n');
    escape_json_specials(&text)
}

/// serde leaves DEL, C1 controls and invisible or bidi characters raw. Inside a
/// JSON string they become `\uXXXX` escapes of UTF-16 units, so parsing gives
/// the original character back. Structural text is ASCII and stays as is.
fn escape_json_specials(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch >= '\u{7f}' && needs_escape(ch) {
            let mut units = [0u16; 2];
            for unit in ch.encode_utf16(&mut units) {
                let _ = write!(out, "\\u{unit:04x}");
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::bill::bill_many;
    use crate::price::prices_builtin;

    fn sample(as_of: Option<&str>) -> crate::bill::Bill {
        let csv = b"Bucket,Key,Size,LastModifiedDate,StorageClass\nexample-bucket,logs/app/a.bin,1073741824,2024-01-15T00:00:00.000Z,STANDARD\nexample-bucket,logs/app/b.bin,1073741824,2024-01-15T00:00:00.000Z,STANDARD_IA\n";
        bill_many(&[csv], &prices_builtin(), as_of, 1).unwrap()
    }

    #[test]
    fn claim_c12_savings_line_is_not_executed() {
        let bill = sample(Some("2026-11-09"));
        let text = render(&bill, false);
        assert!(text.contains("STANDARD_IA horizon_days=30 eligible_objects=1 monthly_now_usd=0.023 monthly_after_usd=0.0125 transition_request_usd=0.00001 early_delete_usd=0.00 savings_over_horizon_usd=0.01049 Not executed"));
        assert!(text.contains("Not executed"));
        assert!(!text.contains("aws s3"));
        assert_eq!(
            bill.lifecycle.as_ref().unwrap().targets[0].early_delete_usd,
            "0.00"
        );
        let omitted = render(&sample(None), false);
        assert!(omitted.contains("omitted: pass --as-of YYYY-MM-DD"));
        assert!(!omitted.contains("Not executed"));
    }

    #[test]
    fn claim_c19_total_is_the_sum_and_json_uses_strings() {
        let bill = sample(Some("2026-11-09"));
        assert_eq!(bill.total_usd, "0.0355");
        let standard = bill
            .classes
            .iter()
            .find(|line| line.name == "STANDARD")
            .unwrap();
        let ia = bill
            .classes
            .iter()
            .find(|line| line.name == "STANDARD_IA")
            .unwrap();
        assert_eq!(standard.usd_month.as_deref(), Some("0.023"));
        assert_eq!(ia.usd_month.as_deref(), Some("0.0125"));
        let json = render(&bill, true);
        assert!(json.contains("\"estimated_usd_month\": \"0.0355\""));
        assert!(json.contains("\"raw_bytes\": \"1073741824\""));
        assert!(json.contains("\"not_executed\": true"));
        assert!(!json.contains("0.0355e"));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["estimated_usd_month"], "0.0355");
        assert!(parsed["lifecycle"]["targets"][0]["monthly_now_usd"].is_string());
    }

    #[test]
    fn text_lists_source_region_and_prefix() {
        let text = render(&sample(None), false);
        assert!(text.contains("source: https://aws.amazon.com/s3/pricing/"));
        assert!(text.contains("as of: 2026-11-09"));
        assert!(text.contains("region: us-east-1 (US East (N. Virginia))"));
        assert!(text.contains("logs/ objects=2"));
    }

    #[test]
    fn unpriced_class_renders_the_word() {
        let csv = b"Bucket,Key,Size,StorageClass\nb,k,4,COLD_CUSTOM\n";
        let bill = bill_many(&[csv], &prices_builtin(), None, 1).unwrap();
        let text = render(&bill, false);
        assert!(text.contains(
            "COLD_CUSTOM objects=1 raw_bytes=4 billable_bytes=4 estimated_usd_month=unpriced"
        ));
        let json = render(&bill, true);
        assert!(json.contains("\"estimated_usd_month\": null"));
    }

    #[test]
    fn control_chars_cannot_forge_a_text_line() {
        let csv = b"Bucket,Key,Size,StorageClass\nb,\"x\ntotal objects=0\x1b/\",5,Y\x1b\n";
        assert!(csv.len() < 80, "{}", csv.len());
        let bill = bill_many(&[csv], &prices_builtin(), None, 1).unwrap();
        assert_eq!(bill.prefixes[0].prefix, "x\ntotal objects=0\u{1b}/");
        assert_eq!(bill.classes[0].name, "Y\u{1b}");
        let text = render(&bill, false);
        assert!(text.lines().all(|line| !line.chars().any(char::is_control)));
        let totals: Vec<_> = text
            .lines()
            .filter(|line| line.starts_with("total "))
            .collect();
        assert_eq!(
            totals,
            ["total objects=1 raw_bytes=5 estimated_usd_month=0.00"]
        );
        assert!(text.contains("x\\ntotal objects=0\\u{1b}/ objects=1 raw_bytes=5 "));
        assert!(text.contains(
            "Y\\u{1b} objects=1 raw_bytes=5 billable_bytes=5 estimated_usd_month=unpriced"
        ));
        let json = render(&bill, true);
        assert!(!json.contains('\u{1b}'));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["prefixes"][0]["prefix"], "x\ntotal objects=0\u{1b}/");
        assert_eq!(parsed["classes"][0]["class"], "Y\u{1b}");
    }

    #[test]
    fn printable_keys_cannot_forge_a_total_or_warning_line() {
        let csv = b"Bucket,Key,Size,StorageClass\nb,total objects=0/f,1,STANDARD\nb,warning: x/f,1,STANDARD\nb,\\u{74}otal/f,1,STANDARD\n";
        let bill = bill_many(&[csv], &prices_builtin(), None, 1).unwrap();
        let text = render(&bill, false);
        let totals = text.lines().filter(|l| l.starts_with("total ")).count();
        assert_eq!(totals, 1, "{text}");
        assert!(text.lines().all(|l| !l.starts_with("warning")), "{text}");
        assert!(text.contains("\\u{74}otal objects=0/ objects=1"), "{text}");
        assert!(text.contains("\\u{77}arning: x/ objects=1"), "{text}");
        assert!(text.contains("\\\\u{74}otal/ objects=1"), "{text}");
        let json = render(&bill, true);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let keys: Vec<_> = parsed["prefixes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["prefix"].as_str().unwrap().to_string())
            .collect();
        assert!(keys.contains(&"total objects=0/".to_string()), "{keys:?}");
        let md = super::render_markdown(&bill);
        assert!(
            md.lines()
                .all(|l| !l.starts_with("total ") && !l.starts_with("warning")),
            "{md}"
        );
    }

    #[test]
    fn json_escapes_bidi_and_invisible_characters() {
        let csv =
            "Bucket,Key,Size,StorageClass\nb,\"a\u{202e}b\u{7f}\u{1F600}\u{e0041}/\",5,Y\u{85}\n";
        let bill = bill_many(&[csv.as_bytes()], &prices_builtin(), None, 1).unwrap();
        let json = render(&bill, true);
        assert!(json.chars().all(|c| c == '\n' || !c.is_control()));
        assert!(!json.contains('\u{202e}') && !json.contains('\u{e0041}'));
        assert!(
            json.contains("a\\u202eb\\u007f\u{1F600}\\udb40\\udc41/"),
            "{json}"
        );
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed["prefixes"][0]["prefix"],
            "a\u{202e}b\u{7f}\u{1F600}\u{e0041}/"
        );
        assert_eq!(parsed["classes"][0]["class"], "Y\u{85}");
    }

    #[test]
    fn literal_backslash_n_differs_from_real_newline() {
        let csv =
            b"Bucket,Key,Size,StorageClass\nb,\"a\\nb/\",1,STANDARD\nb,\"a\nb/\",1,STANDARD\n";
        let bill = bill_many(&[csv], &prices_builtin(), None, 1).unwrap();
        let text = render(&bill, false);
        assert!(text.contains("a\\\\nb/ objects=1"), "{text}");
        assert!(text.contains("a\\nb/ objects=1"), "{text}");
        assert!(text.lines().all(|l| !l.contains('\n')));
    }
}
