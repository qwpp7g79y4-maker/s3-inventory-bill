use rust_decimal::Decimal;
use serde_json::{Map, Value};

use crate::error::Error;
use crate::json_scan::{reject_duplicate_keys, KeyFault};
use crate::money::{parse_money, valid_date};

/// Amazon S3 pricing page the built-in US East (N. Virginia) rates were read from.
///
/// The page's storage and request tables are filled from the public pricing map
/// published 2026-11-09T15:43:10Z. Glacier Deep Archive storage on that page is
/// the map published 2026-09-11T12:45:07Z, which resolves to 0.00099 USD per GB-month.
pub const PRICE_PAGE_URL: &str = "https://aws.amazon.com/s3/pricing/";

/// Calendar date the pricing page and its map were read.
pub const PRICE_PAGE_DATE: &str = "2026-11-09";

/// One regional quote. Unknown JSON fields are ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prices {
    pub source_url: String,
    pub source_date: String,
    pub region: String,
    pub region_name: String,
    pub(crate) standard: [Decimal; 3],
    pub(crate) standard_ia: Decimal,
    pub(crate) onezone_ia: Decimal,
    pub(crate) glacier_ir: Decimal,
    pub(crate) glacier: Decimal,
    pub(crate) deep_archive: Decimal,
    pub(crate) it_frequent: [Decimal; 3],
    pub(crate) it_infrequent: Decimal,
    pub(crate) it_archive_instant: Decimal,
    pub(crate) it_archive: Decimal,
    pub(crate) it_deep_archive: Decimal,
    pub(crate) express_onezone: Decimal,
    pub(crate) it_monitoring: Decimal,
    pub(crate) life_standard_ia: Decimal,
    pub(crate) life_onezone_ia: Decimal,
    pub(crate) life_intelligent_tiering: Decimal,
    pub(crate) life_glacier_ir: Decimal,
    pub(crate) life_glacier: Decimal,
    pub(crate) life_deep_archive: Decimal,
    pub(crate) min_billable_bytes: u64,
    pub(crate) ia_min_days: u32,
    pub(crate) glacier_ir_min_days: u32,
    pub(crate) glacier_min_days: u32,
    pub(crate) deep_archive_min_days: u32,
    pub(crate) meta_standard_bytes: u64,
    pub(crate) meta_class_bytes: u64,
    pub(crate) it_monitor_min_bytes: u64,
}

const BUILTIN_JSON: &str = include_str!("../prices/s3-storage.json");

/// Rates from <https://aws.amazon.com/s3/pricing/>, read 2026-11-09, US East (N. Virginia).
#[must_use]
pub fn prices_builtin() -> Prices {
    load_prices_str(BUILTIN_JSON).expect("built-in prices")
}

pub fn load_prices_str(text: &str) -> Result<Prices, Error> {
    let text = strip_bom(text);
    match reject_duplicate_keys(text) {
        Ok(()) => {}
        Err(KeyFault::Duplicate) => return Err(Error::new("duplicate key")),
        Err(KeyFault::Bad) => return Err(Error::new("not JSON")),
    }
    let value: Value = serde_json::from_str(text).map_err(|_| Error::new("not JSON"))?;
    let Some(obj) = value.as_object() else {
        return Err(Error::new("expected an object"));
    };
    let gb = req_object(obj, "usd_per_gb_month")?;
    let per_object = req_object(obj, "usd_per_object")?;
    let per_request = req_object(obj, "usd_per_request")?;
    Ok(Prices {
        source_url: req_source(obj)?,
        source_date: req_date(obj)?,
        region: req_label(obj, "region")?,
        region_name: req_label(obj, "region_name")?,
        standard: [
            req_decimal(gb, "standard_first_50_tb")?,
            req_decimal(gb, "standard_next_450_tb")?,
            req_decimal(gb, "standard_over_500_tb")?,
        ],
        standard_ia: req_decimal(gb, "standard_ia")?,
        onezone_ia: req_decimal(gb, "onezone_ia")?,
        glacier_ir: req_decimal(gb, "glacier_ir")?,
        glacier: req_decimal(gb, "glacier")?,
        deep_archive: req_decimal(gb, "deep_archive")?,
        it_frequent: [
            req_decimal(gb, "it_frequent_first_50_tb")?,
            req_decimal(gb, "it_frequent_next_450_tb")?,
            req_decimal(gb, "it_frequent_over_500_tb")?,
        ],
        it_infrequent: req_decimal(gb, "it_infrequent")?,
        it_archive_instant: req_decimal(gb, "it_archive_instant")?,
        it_archive: req_decimal(gb, "it_archive")?,
        it_deep_archive: req_decimal(gb, "it_deep_archive")?,
        express_onezone: req_decimal(gb, "express_onezone")?,
        it_monitoring: req_decimal(per_object, "it_monitoring")?,
        life_standard_ia: req_decimal(per_request, "lifecycle_standard_ia")?,
        life_onezone_ia: req_decimal(per_request, "lifecycle_onezone_ia")?,
        life_intelligent_tiering: req_decimal(per_request, "lifecycle_intelligent_tiering")?,
        life_glacier_ir: req_decimal(per_request, "lifecycle_glacier_ir")?,
        life_glacier: req_decimal(per_request, "lifecycle_glacier")?,
        life_deep_archive: req_decimal(per_request, "lifecycle_deep_archive")?,
        min_billable_bytes: req_u64(obj, "min_billable_bytes")?,
        ia_min_days: req_days(obj, "ia_min_days")?,
        glacier_ir_min_days: req_days(obj, "glacier_ir_min_days")?,
        glacier_min_days: req_days(obj, "glacier_min_days")?,
        deep_archive_min_days: req_days(obj, "deep_archive_min_days")?,
        meta_standard_bytes: req_u64(obj, "meta_standard_bytes")?,
        meta_class_bytes: req_u64(obj, "meta_class_bytes")?,
        it_monitor_min_bytes: req_u64(obj, "it_monitor_min_bytes")?,
    })
}

fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn req_object<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<&'a Map<String, Value>, Error> {
    match obj.get(key) {
        Some(Value::Object(inner)) => Ok(inner),
        Some(_) => Err(Error::new(format!("bad {key}"))),
        None => Err(Error::new(format!("price file is missing {key}"))),
    }
}

fn req_source(obj: &Map<String, Value>) -> Result<String, Error> {
    let text = req_string(obj, "source_url")?;
    if text.starts_with("https://")
        && (12..=500).contains(&text.chars().count())
        && !text.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        Ok(text)
    } else {
        Err(Error::new("bad source_url"))
    }
}

fn req_date(obj: &Map<String, Value>) -> Result<String, Error> {
    let text = req_string(obj, "source_date")?;
    if valid_date(&text) {
        Ok(text)
    } else {
        Err(Error::new("bad source_date"))
    }
}

fn req_label(obj: &Map<String, Value>, key: &str) -> Result<String, Error> {
    let text = req_string(obj, key)?;
    if (1..=80).contains(&text.chars().count()) && !text.chars().any(|c| c.is_control()) {
        Ok(text)
    } else {
        Err(Error::new(format!("bad {key}")))
    }
}

fn req_string(obj: &Map<String, Value>, key: &str) -> Result<String, Error> {
    match obj.get(key) {
        Some(Value::String(text)) => Ok(text.clone()),
        Some(_) => Err(Error::new(format!("bad {key}"))),
        None => Err(Error::new(format!("price file is missing {key}"))),
    }
}

fn req_decimal(obj: &Map<String, Value>, key: &str) -> Result<Decimal, Error> {
    let text = req_string(obj, key)?;
    parse_money(&text).map_err(|()| Error::new(format!("bad {key}")))
}

fn req_u64(obj: &Map<String, Value>, key: &str) -> Result<u64, Error> {
    match obj.get(key) {
        Some(Value::Number(number)) => number
            .as_u64()
            .ok_or_else(|| Error::new(format!("bad {key}"))),
        Some(_) => Err(Error::new(format!("bad {key}"))),
        None => Err(Error::new(format!("price file is missing {key}"))),
    }
}

fn req_days(obj: &Map<String, Value>, key: &str) -> Result<u32, Error> {
    let value = req_u64(obj, key)?;
    if (1..=36_500).contains(&value) {
        Ok(u32::try_from(value).expect("range fits u32"))
    } else {
        Err(Error::new(format!("bad {key}")))
    }
}

#[cfg(test)]
mod tests {
    use super::{load_prices_str, prices_builtin, PRICE_PAGE_DATE, PRICE_PAGE_URL};
    use crate::money::format_usd;

    /// The built-in file with LF line ends, also in a CRLF checkout.
    fn builtin_text() -> String {
        include_str!("../prices/s3-storage.json").replace("\r\n", "\n")
    }

    #[test]
    fn claim_c3_standard_rate_source_date_region() {
        let text = builtin_text();
        assert!(text.contains(PRICE_PAGE_URL));
        assert!(text.contains(PRICE_PAGE_DATE));
        assert!(text.contains("2026-11-09T15:43:10Z"));
        assert!(text.contains("2026-09-11T12:45:07Z"));
        let prices = prices_builtin();
        assert_eq!(prices.source_url, PRICE_PAGE_URL);
        assert_eq!(prices.source_date, PRICE_PAGE_DATE);
        assert_eq!(prices.region, "us-east-1");
        assert_eq!(prices.region_name, "US East (N. Virginia)");
        assert_eq!(format_usd(prices.standard[0]), "0.023");
        assert_eq!(format_usd(prices.standard[1]), "0.022");
        assert_eq!(format_usd(prices.standard[2]), "0.021");
        assert_eq!(prices.min_billable_bytes, 131_072);
    }

    #[test]
    fn claim_c11_lifecycle_request_prices() {
        let prices = prices_builtin();
        assert_eq!(format_usd(prices.life_standard_ia), "0.00001");
        assert_eq!(format_usd(prices.life_onezone_ia), "0.00001");
        assert_eq!(format_usd(prices.life_intelligent_tiering), "0.00001");
        assert_eq!(format_usd(prices.life_glacier_ir), "0.00002");
        assert_eq!(format_usd(prices.life_glacier), "0.00003");
        assert_eq!(format_usd(prices.life_deep_archive), "0.00005");
        assert_eq!(
            format_usd(prices.it_monitoring * rust_decimal::Decimal::from(1000)),
            "0.0025"
        );
    }

    #[test]
    fn claim_c18_unknown_field_ignored_and_duplicate_hidden() {
        let mut text = builtin_text().to_string();
        text = text.replacen(
            "{\n  \"note\":",
            "{\n  \"extra\": \"not-a-rate\",\n  \"note\":",
            1,
        );
        let loaded = load_prices_str(&text).unwrap();
        assert_eq!(format_usd(loaded.standard_ia), "0.0125");
        let dup = "{\"source_url\":\"https://aws.amazon.com/s3/pricing/\",\"source_url\":\"https://aws.amazon.com/s3/pricing/\"}";
        let err = load_prices_str(dup).unwrap_err();
        assert_eq!(err.message(), "duplicate key");
        assert!(!err.message().contains("s3/pricing"));
        let nested = builtin_text().replacen(
            "\"standard_first_50_tb\": \"0.023\"",
            "\"standard_first_50_tb\": \"0.023\", \"standard_first_50_tb\": \"9\"",
            1,
        );
        let err = load_prices_str(&nested).unwrap_err();
        assert_eq!(err.message(), "duplicate key");
        assert!(!err.message().contains('9'));
    }

    #[test]
    fn surrogate_pair_in_a_key_or_label_is_ordinary_json() {
        let text = builtin_text().replacen(
            "\"region_name\": \"US East (N. Virginia)\"",
            "\"region_name\": \"\\uD83D\\uDE00\", \"\\uD83D\\uDE00\": \"tag\"",
            1,
        );
        let prices = load_prices_str(&text).unwrap();
        assert_eq!(prices.region_name, "\u{1F600}");
        assert_eq!(prices.region, "us-east-1");
        assert_eq!(
            load_prices_str(r#"{"region_name":"\uD800"}"#)
                .unwrap_err()
                .message(),
            "not JSON"
        );
        assert_eq!(
            load_prices_str(r#"{"tag":"\uDE00"}"#)
                .unwrap_err()
                .message(),
            "not JSON"
        );
        assert_eq!(
            load_prices_str("{\"\u{1F600}\":1,\"\\uD83D\\uDE00\":2}")
                .unwrap_err()
                .message(),
            "duplicate key"
        );
        assert_eq!(
            load_prices_str(r#"{"a":1,"\u0061":2}"#)
                .unwrap_err()
                .message(),
            "duplicate key"
        );
        assert_eq!(
            load_prices_str(r#"{"a":1,"a":2}"#).unwrap_err().message(),
            "duplicate key"
        );
    }

    #[test]
    fn bad_money_does_not_echo_the_value() {
        let text = builtin_text().replacen(
            "\"standard_ia\": \"0.0125\"",
            "\"standard_ia\": \"1e-9-secret\"",
            1,
        );
        let err = load_prices_str(&text).unwrap_err();
        assert_eq!(err.message(), "bad standard_ia");
        assert!(!err.message().contains("secret"));
        assert!(!err.message().contains("1e-9"));
    }

    #[test]
    fn missing_field_names_only_the_field() {
        let text = builtin_text().replacen("\"deep_archive\": \"0.00099\",\n", "", 1);
        let err = load_prices_str(&text).unwrap_err();
        assert_eq!(err.message(), "price file is missing deep_archive");
    }

    #[test]
    fn csv_text_is_not_a_duplicate_key_error() {
        let err = load_prices_str("Bucket,Key\n").unwrap_err();
        assert_eq!(err.message(), "not JSON");
    }

    #[test]
    fn bom_and_page_rates_for_the_other_classes() {
        let text = format!("\u{feff}{}", builtin_text());
        let prices = load_prices_str(&text).unwrap();
        assert_eq!(format_usd(prices.standard_ia), "0.0125");
        assert_eq!(format_usd(prices.onezone_ia), "0.01");
        assert_eq!(format_usd(prices.glacier_ir), "0.004");
        assert_eq!(format_usd(prices.glacier), "0.0036");
        assert_eq!(format_usd(prices.deep_archive), "0.00099");
        assert_eq!(format_usd(prices.express_onezone), "0.11");
        assert_eq!(format_usd(prices.it_frequent[0]), "0.023");
        assert_eq!(format_usd(prices.it_infrequent), "0.0125");
        assert_eq!(format_usd(prices.it_archive_instant), "0.004");
        assert_eq!(format_usd(prices.it_archive), "0.0036");
        assert_eq!(format_usd(prices.it_deep_archive), "0.00099");
        assert_eq!(prices.ia_min_days, 30);
        assert_eq!(prices.glacier_ir_min_days, 90);
        assert_eq!(prices.glacier_min_days, 90);
        assert_eq!(prices.deep_archive_min_days, 180);
        assert_eq!(prices.meta_standard_bytes, 8192);
        assert_eq!(prices.meta_class_bytes, 32768);
        assert_eq!(prices.it_monitor_min_bytes, 131_072);
    }

    #[test]
    fn fractional_rule_is_rejected_without_the_number() {
        let text = builtin_text().replacen(
            "\"min_billable_bytes\": 131072",
            "\"min_billable_bytes\": 131072.5",
            1,
        );
        let err = load_prices_str(&text).unwrap_err();
        assert_eq!(err.message(), "bad min_billable_bytes");
        assert!(!err.message().contains("131072"));
    }

    fn price_error(from: &str, to: &str) -> String {
        assert!(builtin_text().contains(from), "{from}");
        load_prices_str(&builtin_text().replacen(from, to, 1))
            .unwrap_err()
            .message()
            .to_string()
    }

    #[test]
    fn every_bad_field_kind_names_only_the_field() {
        assert_eq!(
            load_prices_str("[1]").unwrap_err().message(),
            "expected an object"
        );
        assert_eq!(
            load_prices_str("3").unwrap_err().message(),
            "expected an object"
        );
        assert_eq!(
            price_error(
                "\"source_url\": \"https://aws.amazon.com/s3/pricing/\"",
                "\"source_url\": \"http://x.test/secret-page\""
            ),
            "bad source_url"
        );
        assert_eq!(
            price_error(
                "\"source_date\": \"2026-11-09\"",
                "\"source_date\": \"2026-02-31\""
            ),
            "bad source_date"
        );
        assert_eq!(
            price_error("\"region\": \"us-east-1\"", "\"region\": \"\""),
            "bad region"
        );
        assert_eq!(
            price_error("\"region\": \"us-east-1\"", "\"region\": 7"),
            "bad region"
        );
        assert_eq!(
            price_error(
                "\"usd_per_gb_month\": {",
                "\"usd_per_gb_month\": [], \"x\": {"
            ),
            "bad usd_per_gb_month"
        );
        assert_eq!(
            price_error("\"ia_min_days\": 30", "\"ia_min_days\": 0"),
            "bad ia_min_days"
        );
        assert_eq!(
            price_error("\"ia_min_days\": 30", "\"ia_min_days\": \"30\""),
            "bad ia_min_days"
        );
        assert_eq!(
            price_error("\"ia_min_days\": 30", "\"ia_min_days\": -3"),
            "bad ia_min_days"
        );
        assert_eq!(
            price_error("\"meta_class_bytes\": 32768,", ""),
            "price file is missing meta_class_bytes"
        );
        assert_eq!(
            price_error("\"standard_ia\": \"0.0125\"", "\"standard_ia\": 0.0125"),
            "bad standard_ia"
        );
    }
}
