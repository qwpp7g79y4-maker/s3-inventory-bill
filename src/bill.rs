use std::collections::BTreeMap;

use rust_decimal::Decimal;

use crate::csv_inv::{parse_size, prefix_of, trim_ascii, visit_csv};
use crate::decode::prepare_bytes;
use crate::error::Error;
use crate::money::{
    add_dec, charge, checked_add, format_usd, horizon_savings, mul_dec, share_of, sub_dec,
    sum_charges, ymd_days, Terms,
};
use crate::price::Prices;

const GIB: u64 = 1_073_741_824;
const TB: u64 = 1024 * GIB;

/// One storage class in the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassLine {
    pub name: String,
    pub objects: u64,
    pub raw_bytes: u64,
    pub billable_bytes: u64,
    pub usd_month: Option<String>,
}

/// One key prefix in the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixLine {
    pub prefix: String,
    pub objects: u64,
    pub raw_bytes: u64,
    pub usd_month: String,
}

/// One lifecycle target. The transition is not executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLine {
    pub name: String,
    pub horizon_days: u32,
    pub eligible_objects: u64,
    pub monthly_now_usd: String,
    pub monthly_after_usd: String,
    pub transition_request_usd: String,
    pub early_delete_usd: String,
    pub savings_over_horizon_usd: String,
}

/// Lifecycle block. Absent when `--as-of` was not set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lifecycle {
    pub as_of: String,
    pub targets: Vec<TargetLine>,
}

/// Priced inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bill {
    pub source_url: String,
    pub source_date: String,
    pub region: String,
    pub region_name: String,
    pub prefix_depth: u8,
    pub classes: Vec<ClassLine>,
    pub prefixes: Vec<PrefixLine>,
    pub total_objects: u64,
    pub total_raw_bytes: u64,
    pub total_usd: String,
    pub lifecycle: Option<Lifecycle>,
    pub warnings: Vec<String>,
}

#[derive(Default)]
struct Pool {
    objects: u64,
    raw: u64,
    standard_objects: u64,
    standard: u64,
    ia_objects: u64,
    ia_raw: u64,
    ia_billable: u64,
    oz_objects: u64,
    oz_raw: u64,
    oz_billable: u64,
    gir_objects: u64,
    gir_raw: u64,
    gir_billable: u64,
    gl_objects: u64,
    gl_raw: u64,
    da_objects: u64,
    da_raw: u64,
    ex_objects: u64,
    ex_raw: u64,
    it_objects: u64,
    it_raw: u64,
    it_frequent: u64,
    it_infrequent: u64,
    it_air: u64,
    it_aa_objects: u64,
    it_aa_bytes: u64,
    it_da_objects: u64,
    it_da_bytes: u64,
    it_monitored: u64,
    other: BTreeMap<String, (u64, u64)>,
}

#[derive(Default)]
struct Eligible {
    objects: u64,
    raw: u64,
    ia_billable: u64,
    gir_billable: u64,
}

#[derive(Default)]
struct Accum {
    all: Pool,
    prefixes: BTreeMap<String, Pool>,
    eligible: Eligible,
    skipped_savings: u64,
    unknown_tier: u64,
    saw_modified: bool,
}

struct Cols {
    bucket: usize,
    key: usize,
    size: usize,
    class: usize,
    modified: Option<usize>,
    tier: Option<usize>,
    width: usize,
}

enum Kind {
    Standard,
    Ia,
    OneZone,
    It,
    Gir,
    Glacier,
    Deep,
    Express,
    Other(String),
}

#[derive(Clone, Copy)]
enum ItPlace {
    Frequent,
    Infrequent,
    Air,
    Archive,
    Deep,
    FrequentUnknown,
}

struct Priced {
    standard: Decimal,
    ia: Decimal,
    onezone: Decimal,
    gir: Decimal,
    glacier: Decimal,
    glacier_billable: u64,
    deep: Decimal,
    deep_billable: u64,
    express: Decimal,
    it: Decimal,
    it_billable: u64,
    it_frequent: Decimal,
    meta: Decimal,
    total: Decimal,
}

/// Price one or more inventory payloads. Gzip, Parquet, and ORC are detected on each payload.
pub fn bill_many(
    files: &[&[u8]],
    prices: &Prices,
    as_of: Option<&str>,
    prefix_depth: u8,
) -> Result<Bill, Error> {
    if files.is_empty() {
        return Err(Error::new("no inventory files"));
    }
    let mut plains = Vec::with_capacity(files.len());
    for file in files {
        plains.push(prepare_bytes(file, crate::MAX_FILE_BYTES)?);
    }
    bill_plain(&plains, prices, as_of, prefix_depth)
}

/// Price payloads that `prepare_bytes` has already turned into plain CSV.
pub(crate) fn bill_plain(
    plains: &[Vec<u8>],
    prices: &Prices,
    as_of: Option<&str>,
    prefix_depth: u8,
) -> Result<Bill, Error> {
    bill_plain_named(plains, &[], prices, as_of, prefix_depth)
}

/// Like `bill_plain`; `names[i]` (when present) is put in front of errors from payload `i`.
pub(crate) fn bill_plain_named(
    plains: &[Vec<u8>],
    names: &[String],
    prices: &Prices,
    as_of: Option<&str>,
    prefix_depth: u8,
) -> Result<Bill, Error> {
    if plains.is_empty() {
        return Err(Error::new("no inventory files"));
    }
    if !(1..=8).contains(&prefix_depth) {
        return Err(Error::new("prefix depth must be 1..=8"));
    }
    let as_of_days = match as_of {
        Some(text) => Some(ymd_days(text).ok_or_else(|| Error::new("invalid --as-of"))?),
        None => None,
    };
    let mut accum = Accum::default();
    for (index, plain) in plains.iter().enumerate() {
        accum
            .ingest(plain, prices, prefix_depth, as_of_days)
            .map_err(|err| match names.get(index) {
                Some(name) => Error::new(format!("{name}: {err}")),
                None => err,
            })?;
    }
    accum.finish(prices, as_of, prefix_depth)
}

impl Accum {
    fn ingest(
        &mut self,
        data: &[u8],
        prices: &Prices,
        depth: u8,
        as_of_days: Option<i64>,
    ) -> Result<(), Error> {
        let mut cols: Option<Cols> = None;
        let mut first = true;
        visit_csv(data, |row_no, fields| {
            if first {
                first = false;
                let header = parse_header(fields)?;
                self.saw_modified = self.saw_modified || header.modified.is_some();
                cols = Some(header);
                return Ok(());
            }
            let header = cols.as_ref().expect("header row");
            self.ingest_row(header, fields, row_no, prices, depth, as_of_days)
        })?;
        if cols.is_none() {
            return Err(Error::new("empty inventory"));
        }
        Ok(())
    }

    fn ingest_row(
        &mut self,
        cols: &Cols,
        fields: &[&[u8]],
        row_no: u64,
        prices: &Prices,
        depth: u8,
        as_of_days: Option<i64>,
    ) -> Result<(), Error> {
        if fields.len() != cols.width {
            return Err(Error::new(format!(
                "row {row_no}: column count does not match the header"
            )));
        }
        let bucket = trim_ascii(fields[cols.bucket]);
        if bucket.is_empty() {
            return Err(Error::new(format!("row {row_no}: empty Bucket")));
        }
        if std::str::from_utf8(bucket).is_err() {
            return Err(Error::new(format!("row {row_no}: Bucket is not UTF-8")));
        }
        let key = std::str::from_utf8(fields[cols.key])
            .map_err(|_| Error::new(format!("row {row_no}: key is not UTF-8")))?;
        let size = parse_size(fields[cols.size])
            .map_err(|()| Error::new(format!("row {row_no}: invalid Size")))?;
        let kind = class_kind(fields[cols.class])
            .map_err(|err| Error::new(format!("row {row_no}: {err}")))?;
        let prefix = prefix_of(key, depth);
        let floor = prices.min_billable_bytes;
        match kind {
            Kind::Standard => {
                self.all.add_standard(size)?;
                self.prefixes
                    .entry(prefix)
                    .or_default()
                    .add_standard(size)?;
                if let Some(as_of) = as_of_days {
                    let days = cols.modified.and_then(|index| modified_days(fields[index]));
                    if matches!(days, Some(day) if day <= as_of) {
                        self.eligible.add(size, floor)?;
                    } else {
                        self.skipped_savings = self
                            .skipped_savings
                            .checked_add(1)
                            .ok_or_else(|| Error::new("object total does not fit"))?;
                    }
                }
            }
            Kind::Ia => self.add_both(&prefix, |pool| pool.add_ia(size, size.max(floor)))?,
            Kind::OneZone => {
                self.add_both(&prefix, |pool| pool.add_onezone(size, size.max(floor)))?
            }
            Kind::Gir => self.add_both(&prefix, |pool| pool.add_gir(size, size.max(floor)))?,
            Kind::Glacier => self.add_both(&prefix, |pool| pool.add_glacier(size))?,
            Kind::Deep => self.add_both(&prefix, |pool| pool.add_deep(size))?,
            Kind::Express => self.add_both(&prefix, |pool| pool.add_express(size))?,
            Kind::It => {
                let tier = cols.tier.map(|index| trim_ascii(fields[index]));
                let place = it_place(size, tier, prices);
                let monitor = size >= prices.it_monitor_min_bytes;
                if matches!(place, ItPlace::FrequentUnknown) {
                    self.unknown_tier = self
                        .unknown_tier
                        .checked_add(1)
                        .ok_or_else(|| Error::new("object total does not fit"))?;
                }
                self.add_both(&prefix, |pool| pool.add_it(size, place, monitor))?;
            }
            Kind::Other(name) => self.add_both(&prefix, |pool| pool.add_other(&name, size))?,
        }
        Ok(())
    }

    fn add_both(
        &mut self,
        prefix: &str,
        mut apply: impl FnMut(&mut Pool) -> Result<(), Error>,
    ) -> Result<(), Error> {
        apply(&mut self.all)?;
        apply(self.prefixes.entry(prefix.to_string()).or_default())?;
        Ok(())
    }

    fn finish(self, prices: &Prices, as_of: Option<&str>, prefix_depth: u8) -> Result<Bill, Error> {
        let priced = price_pool(&self.all, prices)?;
        let mut classes = Vec::new();
        push_priced(
            &mut classes,
            "STANDARD",
            self.all.standard_objects,
            self.all.standard,
            self.all.standard,
            priced.standard,
        )?;
        push_priced(
            &mut classes,
            "INTELLIGENT_TIERING",
            self.all.it_objects,
            self.all.it_raw,
            priced.it_billable,
            priced.it,
        )?;
        push_priced(
            &mut classes,
            "STANDARD_IA",
            self.all.ia_objects,
            self.all.ia_raw,
            self.all.ia_billable,
            priced.ia,
        )?;
        push_priced(
            &mut classes,
            "ONEZONE_IA",
            self.all.oz_objects,
            self.all.oz_raw,
            self.all.oz_billable,
            priced.onezone,
        )?;
        push_priced(
            &mut classes,
            "GLACIER_IR",
            self.all.gir_objects,
            self.all.gir_raw,
            self.all.gir_billable,
            priced.gir,
        )?;
        push_priced(
            &mut classes,
            "GLACIER",
            self.all.gl_objects,
            self.all.gl_raw,
            priced.glacier_billable,
            priced.glacier,
        )?;
        push_priced(
            &mut classes,
            "DEEP_ARCHIVE",
            self.all.da_objects,
            self.all.da_raw,
            priced.deep_billable,
            priced.deep,
        )?;
        push_priced(
            &mut classes,
            "EXPRESS_ONEZONE",
            self.all.ex_objects,
            self.all.ex_raw,
            self.all.ex_raw,
            priced.express,
        )?;
        let mut unpriced_objects = 0u64;
        for (name, (objects, raw)) in &self.all.other {
            unpriced_objects = unpriced_objects
                .checked_add(*objects)
                .ok_or_else(|| Error::new("object total does not fit"))?;
            classes.push(ClassLine {
                name: name.clone(),
                objects: *objects,
                raw_bytes: *raw,
                billable_bytes: *raw,
                usd_month: None,
            });
        }
        let mut amounts = Vec::with_capacity(self.prefixes.len());
        for pool in self.prefixes.values() {
            amounts.push(prefix_usd(pool, &self.all, &priced, prices)?);
        }
        reconcile(&mut amounts, priced.total)?;
        let mut prefixes = Vec::new();
        for ((name, pool), amount) in self.prefixes.iter().zip(amounts) {
            prefixes.push(PrefixLine {
                prefix: name.clone(),
                objects: pool.objects,
                raw_bytes: pool.raw,
                usd_month: format_usd(amount),
            });
        }
        let mut warnings = Vec::new();
        if unpriced_objects > 0 {
            warnings.push(format!(
                "objects without a built-in rate: {unpriced_objects}"
            ));
        }
        if as_of.is_some() && !self.saw_modified && self.all.standard_objects > 0 {
            warnings.push("LastModifiedDate column is absent".to_string());
        }
        if self.skipped_savings > 0 {
            warnings.push(format!(
                "standard objects skipped for lifecycle savings: {}",
                self.skipped_savings
            ));
        }
        if self.unknown_tier > 0 {
            warnings.push(format!(
                "intelligent-tiering objects priced as Frequent Access because the access tier was not recognized: {}",
                self.unknown_tier
            ));
        }
        Ok(Bill {
            source_url: prices.source_url.clone(),
            source_date: prices.source_date.clone(),
            region: prices.region.clone(),
            region_name: prices.region_name.clone(),
            prefix_depth,
            classes,
            prefixes,
            total_objects: self.all.objects,
            total_raw_bytes: self.all.raw,
            total_usd: format_usd(priced.total),
            lifecycle: as_of
                .map(|text| lifecycle_of(text, &self.eligible, prices))
                .transpose()?,
            warnings,
        })
    }
}

impl Pool {
    fn bump(&mut self, raw: u64) -> Result<(), Error> {
        self.objects = self
            .objects
            .checked_add(1)
            .ok_or_else(|| Error::new("object total does not fit"))?;
        checked_add(&mut self.raw, raw)
    }

    fn inc(slot: &mut u64) -> Result<(), Error> {
        *slot = slot
            .checked_add(1)
            .ok_or_else(|| Error::new("object total does not fit"))?;
        Ok(())
    }

    fn add_standard(&mut self, raw: u64) -> Result<(), Error> {
        self.bump(raw)?;
        Self::inc(&mut self.standard_objects)?;
        checked_add(&mut self.standard, raw)
    }

    fn add_ia(&mut self, raw: u64, billable: u64) -> Result<(), Error> {
        self.bump(raw)?;
        Self::inc(&mut self.ia_objects)?;
        checked_add(&mut self.ia_raw, raw)?;
        checked_add(&mut self.ia_billable, billable)
    }

    fn add_onezone(&mut self, raw: u64, billable: u64) -> Result<(), Error> {
        self.bump(raw)?;
        Self::inc(&mut self.oz_objects)?;
        checked_add(&mut self.oz_raw, raw)?;
        checked_add(&mut self.oz_billable, billable)
    }

    fn add_gir(&mut self, raw: u64, billable: u64) -> Result<(), Error> {
        self.bump(raw)?;
        Self::inc(&mut self.gir_objects)?;
        checked_add(&mut self.gir_raw, raw)?;
        checked_add(&mut self.gir_billable, billable)
    }

    fn add_glacier(&mut self, raw: u64) -> Result<(), Error> {
        self.bump(raw)?;
        Self::inc(&mut self.gl_objects)?;
        checked_add(&mut self.gl_raw, raw)
    }

    fn add_deep(&mut self, raw: u64) -> Result<(), Error> {
        self.bump(raw)?;
        Self::inc(&mut self.da_objects)?;
        checked_add(&mut self.da_raw, raw)
    }

    fn add_express(&mut self, raw: u64) -> Result<(), Error> {
        self.bump(raw)?;
        Self::inc(&mut self.ex_objects)?;
        checked_add(&mut self.ex_raw, raw)
    }

    fn add_it(&mut self, raw: u64, place: ItPlace, monitor: bool) -> Result<(), Error> {
        self.bump(raw)?;
        Self::inc(&mut self.it_objects)?;
        checked_add(&mut self.it_raw, raw)?;
        if monitor {
            Self::inc(&mut self.it_monitored)?;
        }
        match place {
            ItPlace::Frequent | ItPlace::FrequentUnknown => checked_add(&mut self.it_frequent, raw),
            ItPlace::Infrequent => checked_add(&mut self.it_infrequent, raw),
            ItPlace::Air => checked_add(&mut self.it_air, raw),
            ItPlace::Archive => {
                Self::inc(&mut self.it_aa_objects)?;
                checked_add(&mut self.it_aa_bytes, raw)
            }
            ItPlace::Deep => {
                Self::inc(&mut self.it_da_objects)?;
                checked_add(&mut self.it_da_bytes, raw)
            }
        }
    }

    fn add_other(&mut self, name: &str, raw: u64) -> Result<(), Error> {
        self.bump(raw)?;
        let entry = self.other.entry(name.to_string()).or_insert((0, 0));
        Self::inc(&mut entry.0)?;
        checked_add(&mut entry.1, raw)
    }
}

impl Eligible {
    fn add(&mut self, raw: u64, floor: u64) -> Result<(), Error> {
        self.objects = self
            .objects
            .checked_add(1)
            .ok_or_else(|| Error::new("object total does not fit"))?;
        checked_add(&mut self.raw, raw)?;
        checked_add(&mut self.ia_billable, raw.max(floor))?;
        checked_add(&mut self.gir_billable, raw.max(floor))
    }
}

fn price_pool(pool: &Pool, prices: &Prices) -> Result<Priced, Error> {
    let meta_objects = meta_object_count(pool)?;
    let meta_bytes = mul_bytes(meta_objects, prices.meta_standard_bytes)?;
    let meta_end = pool
        .standard
        .checked_add(meta_bytes)
        .ok_or_else(|| Error::new("byte total does not fit"))?;
    let meta_parts = if meta_bytes == 0 {
        Vec::new()
    } else {
        tier_parts(pool.standard, meta_end, prices.standard)
    };
    let meta = tiered_after(pool.standard, meta_bytes, prices.standard)?;
    let overhead = prices
        .meta_standard_bytes
        .checked_add(prices.meta_class_bytes)
        .ok_or_else(|| Error::new("byte total does not fit"))?;
    let it_meta_objects = pool
        .it_aa_objects
        .checked_add(pool.it_da_objects)
        .ok_or_else(|| Error::new("object total does not fit"))?;
    let it_frequent = tiered(pool.it_frequent, prices.it_frequent)?;

    // Every amount is the exact sum of its parts, rounded once. The total adds
    // the parts of all classes, not the rounded class amounts.
    let standard = Terms {
        charges: tier_parts(0, pool.standard, prices.standard),
        ..Terms::default()
    };
    let single = |bytes: u64, rate: Decimal| Terms {
        charges: vec![(bytes, rate)],
        ..Terms::default()
    };
    let mut it = Terms {
        charges: tier_parts(0, pool.it_frequent, prices.it_frequent),
        ..Terms::default()
    };
    it.charges.extend([
        (pool.it_infrequent, prices.it_infrequent),
        (pool.it_air, prices.it_archive_instant),
        (pool.it_aa_bytes, prices.it_archive),
        (
            mul_bytes(pool.it_aa_objects, prices.meta_class_bytes)?,
            prices.it_archive,
        ),
        (pool.it_da_bytes, prices.it_deep_archive),
        (
            mul_bytes(pool.it_da_objects, prices.meta_class_bytes)?,
            prices.it_deep_archive,
        ),
    ]);
    it.units = vec![(pool.it_monitored, prices.it_monitoring)];
    it.share = Some((it_meta_objects, meta_objects, meta_parts.clone()));
    let mut glacier = Terms {
        charges: vec![
            (pool.gl_raw, prices.glacier),
            (
                mul_bytes(pool.gl_objects, prices.meta_class_bytes)?,
                prices.glacier,
            ),
        ],
        ..Terms::default()
    };
    glacier.share = Some((pool.gl_objects, meta_objects, meta_parts.clone()));
    let mut deep = Terms {
        charges: vec![
            (pool.da_raw, prices.deep_archive),
            (
                mul_bytes(pool.da_objects, prices.meta_class_bytes)?,
                prices.deep_archive,
            ),
        ],
        ..Terms::default()
    };
    deep.share = Some((pool.da_objects, meta_objects, meta_parts.clone()));
    let flat = [
        single(pool.ia_billable, prices.standard_ia),
        single(pool.oz_billable, prices.onezone_ia),
        single(pool.gir_billable, prices.glacier_ir),
        single(pool.ex_raw, prices.express_onezone),
    ];
    let mut all = Terms::default();
    for part in [&standard, &it, &glacier, &deep].into_iter().chain(&flat) {
        all.charges.extend(part.charges.iter().copied());
        all.units.extend(part.units.iter().copied());
    }
    all.charges.extend(meta_parts);
    let exact = |terms: &Terms| fit(terms.amount());
    Ok(Priced {
        standard: exact(&standard)?,
        ia: exact(&flat[0])?,
        onezone: exact(&flat[1])?,
        gir: exact(&flat[2])?,
        glacier: exact(&glacier)?,
        glacier_billable: pool
            .gl_raw
            .checked_add(mul_bytes(pool.gl_objects, overhead)?)
            .ok_or_else(|| Error::new("byte total does not fit"))?,
        deep: exact(&deep)?,
        deep_billable: pool
            .da_raw
            .checked_add(mul_bytes(pool.da_objects, overhead)?)
            .ok_or_else(|| Error::new("byte total does not fit"))?,
        express: exact(&flat[3])?,
        it: exact(&it)?,
        it_billable: pool
            .it_raw
            .checked_add(mul_bytes(it_meta_objects, overhead)?)
            .ok_or_else(|| Error::new("byte total does not fit"))?,
        it_frequent,
        meta,
        total: exact(&all)?,
    })
}

fn prefix_usd(pool: &Pool, all: &Pool, priced: &Priced, prices: &Prices) -> Result<Decimal, Error> {
    sum_amounts(&[
        share(pool.standard, all.standard, priced.standard)?,
        charge(pool.ia_billable, prices.standard_ia)?,
        charge(pool.oz_billable, prices.onezone_ia)?,
        charge(pool.gir_billable, prices.glacier_ir)?,
        charge(pool.gl_raw, prices.glacier)?,
        charge(
            mul_bytes(pool.gl_objects, prices.meta_class_bytes)?,
            prices.glacier,
        )?,
        charge(pool.da_raw, prices.deep_archive)?,
        charge(
            mul_bytes(pool.da_objects, prices.meta_class_bytes)?,
            prices.deep_archive,
        )?,
        charge(pool.ex_raw, prices.express_onezone)?,
        share(pool.it_frequent, all.it_frequent, priced.it_frequent)?,
        charge(pool.it_infrequent, prices.it_infrequent)?,
        charge(pool.it_air, prices.it_archive_instant)?,
        charge(pool.it_aa_bytes, prices.it_archive)?,
        charge(
            mul_bytes(pool.it_aa_objects, prices.meta_class_bytes)?,
            prices.it_archive,
        )?,
        charge(pool.it_da_bytes, prices.it_deep_archive)?,
        charge(
            mul_bytes(pool.it_da_objects, prices.meta_class_bytes)?,
            prices.it_deep_archive,
        )?,
        share(
            meta_object_count(pool)?,
            meta_object_count(all)?,
            priced.meta,
        )?,
        mul_amount(Decimal::from(pool.it_monitored), prices.it_monitoring)?,
    ])
}

fn lifecycle_of(as_of: &str, eligible: &Eligible, prices: &Prices) -> Result<Lifecycle, Error> {
    let now = tier_parts(0, eligible.raw, prices.standard);
    let requests = Decimal::from(eligible.objects);
    let class_meta = mul_bytes(eligible.objects, prices.meta_class_bytes)?;
    let standard_meta = mul_bytes(eligible.objects, prices.meta_standard_bytes)?;
    let archive_after = |rate: Decimal| {
        let mut parts = vec![(eligible.raw, rate), (class_meta, rate)];
        parts.extend(tier_parts(0, standard_meta, prices.standard));
        parts
    };
    let targets = vec![
        one_target(
            "STANDARD_IA",
            prices.ia_min_days,
            &now,
            &[(eligible.ia_billable, prices.standard_ia)],
            mul_amount(requests, prices.life_standard_ia)?,
            eligible.objects,
        )?,
        one_target(
            "GLACIER_IR",
            prices.glacier_ir_min_days,
            &now,
            &[(eligible.gir_billable, prices.glacier_ir)],
            mul_amount(requests, prices.life_glacier_ir)?,
            eligible.objects,
        )?,
        one_target(
            "GLACIER",
            prices.glacier_min_days,
            &now,
            &archive_after(prices.glacier),
            mul_amount(requests, prices.life_glacier)?,
            eligible.objects,
        )?,
        one_target(
            "DEEP_ARCHIVE",
            prices.deep_archive_min_days,
            &now,
            &archive_after(prices.deep_archive),
            mul_amount(requests, prices.life_deep_archive)?,
            eligible.objects,
        )?,
    ];
    Ok(Lifecycle {
        as_of: as_of.to_string(),
        targets,
    })
}

/// Every amount is rounded once from the exact sum of its parts.
fn one_target(
    name: &str,
    days: u32,
    now: &[(u64, Decimal)],
    after: &[(u64, Decimal)],
    request: Decimal,
    objects: u64,
) -> Result<TargetLine, Error> {
    let savings = fit(horizon_savings(now, after, days, request))?;
    Ok(TargetLine {
        name: name.to_string(),
        horizon_days: days,
        eligible_objects: objects,
        monthly_now_usd: format_usd(fit(sum_charges(now))?),
        monthly_after_usd: format_usd(fit(sum_charges(after))?),
        transition_request_usd: format_usd(request),
        early_delete_usd: format_usd(Decimal::ZERO),
        savings_over_horizon_usd: format_usd(savings),
    })
}

fn push_priced(
    lines: &mut Vec<ClassLine>,
    name: &str,
    objects: u64,
    raw: u64,
    billable: u64,
    usd: Decimal,
) -> Result<(), Error> {
    if objects == 0 {
        return Ok(());
    }
    lines.push(ClassLine {
        name: name.to_string(),
        objects,
        raw_bytes: raw,
        billable_bytes: billable,
        usd_month: Some(format_usd(usd)),
    });
    Ok(())
}

fn tiered(bytes: u64, rates: [Decimal; 3]) -> Result<Decimal, Error> {
    marginal(0, bytes, rates)
}

fn tiered_after(already: u64, extra: u64, rates: [Decimal; 3]) -> Result<Decimal, Error> {
    if extra == 0 {
        return Ok(Decimal::ZERO);
    }
    let end = already
        .checked_add(extra)
        .ok_or_else(|| Error::new("byte total does not fit"))?;
    // The extra bytes only. A difference of two rounded totals drops a small
    // stack when the total is already huge.
    marginal(already, end, rates)
}

/// Bytes in `[start, end)` priced on the tier they sit in, rounded once.
fn marginal(start: u64, end: u64, rates: [Decimal; 3]) -> Result<Decimal, Error> {
    fit(sum_charges(&tier_parts(start, end, rates)))
}

/// The bytes of `[start, end)` that sit in each tier, with that tier's rate.
fn tier_parts(start: u64, end: u64, rates: [Decimal; 3]) -> Vec<(u64, Decimal)> {
    const EDGES: [u64; 4] = [0, 50 * TB, 500 * TB, u64::MAX];
    let mut parts = Vec::with_capacity(3);
    for i in 0..3 {
        let from = start.max(EDGES[i]);
        let to = end.min(EDGES[i + 1]);
        if to > from {
            parts.push((to - from, rates[i]));
        }
    }
    parts
}

/// Per-prefix division can drift from the total by a rounding error in the
/// last decimal. Such a drift goes onto the largest prefix. A larger gap means
/// the prefixes and the total disagree, which is an error, not a correction.
fn reconcile(amounts: &mut [Decimal], total: Decimal) -> Result<(), Error> {
    let drift = sub_amount(total, sum_amounts(amounts)?)?;
    if drift.is_zero() {
        return Ok(());
    }
    let unit = Decimal::new(1, 20);
    let relative = mul_dec(total.abs(), Decimal::new(1, 26)).unwrap_or(Decimal::ZERO);
    let each = if relative > unit { relative } else { unit };
    let allowed = mul_amount(each, Decimal::from(amounts.len().max(1) as u64))?;
    if drift.abs() > allowed {
        return Err(Error::new("prefix amounts do not add up to the total"));
    }
    let mut largest = 0;
    for (i, amount) in amounts.iter().enumerate() {
        if *amount > amounts[largest] {
            largest = i;
        }
    }
    if let Some(slot) = amounts.get_mut(largest) {
        *slot = add_amount(*slot, drift)?;
    }
    Ok(())
}

fn share(part: u64, whole: u64, money: Decimal) -> Result<Decimal, Error> {
    if part == whole && part != 0 && !money.is_zero() {
        return Ok(money);
    }
    fit(share_of(part, whole, money))
}

fn fit(value: Option<Decimal>) -> Result<Decimal, Error> {
    value.ok_or_else(|| Error::new("amount does not fit"))
}

fn add_amount(left: Decimal, right: Decimal) -> Result<Decimal, Error> {
    fit(add_dec(left, right))
}

fn sub_amount(left: Decimal, right: Decimal) -> Result<Decimal, Error> {
    fit(sub_dec(left, right))
}

fn mul_amount(left: Decimal, right: Decimal) -> Result<Decimal, Error> {
    fit(mul_dec(left, right))
}

fn sum_amounts(parts: &[Decimal]) -> Result<Decimal, Error> {
    let mut total = Decimal::ZERO;
    for part in parts {
        total = add_amount(total, *part)?;
    }
    Ok(total)
}

fn mul_bytes(count: u64, each: u64) -> Result<u64, Error> {
    count
        .checked_mul(each)
        .ok_or_else(|| Error::new("byte total does not fit"))
}

fn meta_object_count(pool: &Pool) -> Result<u64, Error> {
    pool.gl_objects
        .checked_add(pool.da_objects)
        .and_then(|count| count.checked_add(pool.it_aa_objects))
        .and_then(|count| count.checked_add(pool.it_da_objects))
        .ok_or_else(|| Error::new("object total does not fit"))
}

const CR_HINT: &str = "this row holds a carriage return, so the file may use only CR line ends, which are not read (use LF or CRLF)";

fn parse_header(fields: &[&[u8]]) -> Result<Cols, Error> {
    let cols = Cols {
        bucket: require_col(fields, "Bucket")?,
        key: require_col(fields, "Key")?,
        size: require_col(fields, "Size")?,
        class: require_col(fields, "StorageClass")?,
        modified: find_col(fields, "LastModifiedDate")?,
        tier: find_col(fields, "IntelligentTieringAccessTier")?,
        width: fields.len(),
    };
    // A header field never holds a carriage return: the line end of a CRLF file
    // is cut off before the fields are split. One inside means CR-only line ends
    // and the whole file is this one row, even when the last column is optional
    // and every required one was found.
    if fields.iter().any(|field| field.contains(&b'\r')) {
        return Err(Error::new(format!(
            "the first row is not a header line: {CR_HINT}"
        )));
    }
    Ok(cols)
}

fn require_col(fields: &[&[u8]], name: &str) -> Result<usize, Error> {
    find_col(fields, name)?.ok_or_else(|| {
        // A file whose only line ends are CR reads as one long first row.
        let hint = if fields.iter().any(|field| field.contains(&b'\r')) {
            format!("; {CR_HINT}")
        } else {
            String::new()
        };
        Error::new(format!(
            "missing column {name}: the first row must be a header line naming the columns Bucket,Key,Size,StorageClass (any order; LastModifiedDate and IntelligentTieringAccessTier are optional){hint}"
        ))
    })
}

fn find_col(fields: &[&[u8]], name: &str) -> Result<Option<usize>, Error> {
    let mut found = None;
    for (index, field) in fields.iter().enumerate() {
        if trim_ascii(field) == name.as_bytes() {
            if found.is_some() {
                return Err(Error::new("duplicate column"));
            }
            found = Some(index);
        }
    }
    Ok(found)
}

fn class_kind(raw: &[u8]) -> Result<Kind, Error> {
    let raw = trim_ascii(raw);
    if raw.is_empty() {
        return Err(Error::new("empty StorageClass"));
    }
    if eq(raw, b"STANDARD") {
        Ok(Kind::Standard)
    } else if eq(raw, b"STANDARD_IA") {
        Ok(Kind::Ia)
    } else if eq(raw, b"ONEZONE_IA") {
        Ok(Kind::OneZone)
    } else if eq(raw, b"INTELLIGENT_TIERING") {
        Ok(Kind::It)
    } else if eq(raw, b"GLACIER_IR") {
        Ok(Kind::Gir)
    } else if eq(raw, b"GLACIER") {
        Ok(Kind::Glacier)
    } else if eq(raw, b"DEEP_ARCHIVE") {
        Ok(Kind::Deep)
    } else if eq(raw, b"EXPRESS_ONEZONE") {
        Ok(Kind::Express)
    } else {
        std::str::from_utf8(raw)
            .map(|text| Kind::Other(text.to_ascii_uppercase()))
            .map_err(|_| Error::new("StorageClass is not UTF-8"))
    }
}

fn it_place(size: u64, tier: Option<&[u8]>, prices: &Prices) -> ItPlace {
    if size < prices.it_monitor_min_bytes {
        return ItPlace::Frequent;
    }
    let Some(tier) = tier.filter(|value| !value.is_empty()) else {
        return ItPlace::Frequent;
    };
    if eq(tier, b"FREQUENT") {
        ItPlace::Frequent
    } else if eq(tier, b"INFREQUENT") {
        ItPlace::Infrequent
    } else if eq(tier, b"ARCHIVE_INSTANT_ACCESS") {
        ItPlace::Air
    } else if eq(tier, b"ARCHIVE_ACCESS") || eq(tier, b"ARCHIVE") {
        ItPlace::Archive
    } else if eq(tier, b"DEEP_ARCHIVE_ACCESS") || eq(tier, b"DEEP_ARCHIVE") {
        ItPlace::Deep
    } else {
        ItPlace::FrequentUnknown
    }
}

fn eq(field: &[u8], expected: &[u8]) -> bool {
    field.len() == expected.len()
        && field
            .iter()
            .zip(expected.iter())
            .all(|(left, right)| left.to_ascii_uppercase() == *right)
}

fn modified_days(field: &[u8]) -> Option<i64> {
    let field = trim_ascii(field);
    if field.len() < 10 {
        return None;
    }
    let boundary = field.len() == 10 || field[10] == b'T' || field[10] == b' ';
    if !boundary {
        return None;
    }
    let date = std::str::from_utf8(&field[..10]).ok()?;
    ymd_days(date)
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;
    use crate::money::format_usd;
    use crate::price::{load_prices_str, prices_builtin};

    const HEADER: &str =
        "Bucket,Key,Size,LastModifiedDate,StorageClass,IntelligentTieringAccessTier\n";

    fn bill_of(body: &str, as_of: Option<&str>) -> Bill {
        let mut text = String::from(HEADER);
        text.push_str(body);
        bill_many(&[text.as_bytes()], &prices_builtin(), as_of, 1).unwrap()
    }

    fn class<'a>(bill: &'a Bill, name: &str) -> &'a ClassLine {
        bill.classes
            .iter()
            .find(|line| line.name == name)
            .expect(name)
    }

    fn target<'a>(bill: &'a Bill, name: &str) -> &'a TargetLine {
        bill.lifecycle
            .as_ref()
            .expect("lifecycle")
            .targets
            .iter()
            .find(|line| line.name == name)
            .expect(name)
    }

    fn charge(bytes: u64, rate: Decimal) -> Decimal {
        super::charge(bytes, rate).expect("amount fits")
    }

    fn usd(bytes: u64, rate: &str) -> String {
        format_usd(charge(bytes, rate.parse().unwrap()))
    }

    #[test]
    fn claim_c1_one_row_is_one_object() {
        let bill = bill_of("b,logs/a,10,2024-01-15T00:00:00.000Z,STANDARD,\n", None);
        assert_eq!(bill.total_objects, 1);
        assert_eq!(bill.total_raw_bytes, 10);
        assert_eq!(class(&bill, "STANDARD").objects, 1);
        let two = bill_of(
            "b,logs/a,10,2024-01-15T00:00:00.000Z,STANDARD,\nb,logs/b,3,2024-01-15T00:00:00.000Z,STANDARD,\n",
            None,
        );
        assert_eq!(two.total_objects, 2);
        assert_eq!(two.total_raw_bytes, 13);
    }

    #[test]
    fn claim_c4_standard_ia_floor_and_thirty_days() {
        let small = bill_of(
            "b,k,1000,2024-01-15T00:00:00.000Z,STANDARD_IA,\n",
            Some("2026-11-09"),
        );
        let line = class(&small, "STANDARD_IA");
        assert_eq!(line.raw_bytes, 1000);
        assert_eq!(line.billable_bytes, 131_072);
        assert_eq!(
            line.usd_month.as_deref(),
            Some(usd(131_072, "0.0125").as_str())
        );
        let moved = bill_of(
            "b,k,1073741824,2024-01-15T00:00:00.000Z,STANDARD,\n",
            Some("2026-11-09"),
        );
        assert_eq!(target(&moved, "STANDARD_IA").horizon_days, 30);
    }

    #[test]
    fn claim_c5_onezone_ia_floor() {
        let bill = bill_of("b,k,1000,2024-01-15T00:00:00.000Z,ONEZONE_IA,\n", None);
        let line = class(&bill, "ONEZONE_IA");
        assert_eq!(line.billable_bytes, 131_072);
        assert_eq!(
            line.usd_month.as_deref(),
            Some(usd(131_072, "0.01").as_str())
        );
        let big = bill_of(
            "b,k,1073741824,2024-01-15T00:00:00.000Z,ONEZONE_IA,\n",
            None,
        );
        assert_eq!(class(&big, "ONEZONE_IA").usd_month.as_deref(), Some("0.01"));
    }

    #[test]
    fn claim_c6_glacier_ir_floor_and_ninety_days() {
        let bill = bill_of("b,k,1000,2024-01-15T00:00:00.000Z,GLACIER_IR,\n", None);
        assert_eq!(class(&bill, "GLACIER_IR").billable_bytes, 131_072);
        assert_eq!(
            class(&bill, "GLACIER_IR").usd_month.as_deref(),
            Some(usd(131_072, "0.004").as_str())
        );
        let moved = bill_of(
            "b,k,1073741824,2024-01-15T00:00:00.000Z,STANDARD,\n",
            Some("2026-11-09"),
        );
        let goal = target(&moved, "GLACIER_IR");
        assert_eq!(goal.horizon_days, 90);
        assert_eq!(goal.monthly_after_usd, "0.004");
        assert_eq!(goal.transition_request_usd, "0.00002");
        assert_eq!(goal.savings_over_horizon_usd, "0.05698");
    }

    #[test]
    fn claim_c7_glacier_metadata_and_ninety_day_horizon() {
        let stored = bill_of("b,k,1073741824,2024-01-15T00:00:00.000Z,GLACIER,\n", None);
        let line = class(&stored, "GLACIER");
        assert_eq!(line.billable_bytes, GIB + 40_960);
        let expected = charge(GIB, "0.0036".parse().unwrap())
            + charge(32_768, "0.0036".parse().unwrap())
            + charge(8_192, "0.023".parse().unwrap());
        assert_eq!(
            line.usd_month.as_deref(),
            Some(format_usd(expected).as_str())
        );
        let moved = bill_of(
            "b,k,1073741824,2024-01-15T00:00:00.000Z,STANDARD,\n",
            Some("2026-11-09"),
        );
        let goal = target(&moved, "GLACIER");
        assert_eq!(goal.horizon_days, 90);
        assert_eq!(goal.early_delete_usd, "0.00");
        assert_eq!(goal.transition_request_usd, "0.00003");
        assert_eq!(goal.monthly_after_usd, format_usd(expected));
    }

    #[test]
    fn claim_c8_deep_archive_metadata_and_one_hundred_eighty_days() {
        let stored = bill_of(
            "b,k,1073741824,2024-01-15T00:00:00.000Z,DEEP_ARCHIVE,\n",
            None,
        );
        let expected = charge(GIB, "0.00099".parse().unwrap())
            + charge(32_768, "0.00099".parse().unwrap())
            + charge(8_192, "0.023".parse().unwrap());
        assert_eq!(
            class(&stored, "DEEP_ARCHIVE").usd_month.as_deref(),
            Some(format_usd(expected).as_str())
        );
        assert_eq!(class(&stored, "DEEP_ARCHIVE").billable_bytes, GIB + 40_960);
        let moved = bill_of(
            "b,k,1073741824,2024-01-15T00:00:00.000Z,STANDARD,\n",
            Some("2026-11-09"),
        );
        let goal = target(&moved, "DEEP_ARCHIVE");
        assert_eq!(goal.horizon_days, 180);
        assert_eq!(goal.transition_request_usd, "0.00005");
        let now = "0.023".parse::<Decimal>().unwrap();
        let savings = (now - expected) * Decimal::from(6) - "0.00005".parse::<Decimal>().unwrap();
        assert_eq!(goal.savings_over_horizon_usd, format_usd(savings));
    }

    #[test]
    fn claim_c9_intelligent_tiering_small_objects_and_monitoring() {
        let small = bill_of(
            "b,k,1000,2024-01-15T00:00:00.000Z,INTELLIGENT_TIERING,SECRET_TIER\n",
            None,
        );
        let line = class(&small, "INTELLIGENT_TIERING");
        assert_eq!(line.billable_bytes, 1000);
        assert_eq!(line.usd_month.as_deref(), Some(usd(1000, "0.023").as_str()));
        assert!(small.warnings.is_empty());
        let big = bill_of(
            "b,k,131072,2024-01-15T00:00:00.000Z,INTELLIGENT_TIERING,\n",
            None,
        );
        let monitor = "0.0000025".parse::<Decimal>().unwrap();
        let expected = charge(131_072, "0.023".parse().unwrap()) + monitor;
        assert_eq!(
            class(&big, "INTELLIGENT_TIERING").usd_month.as_deref(),
            Some(format_usd(expected).as_str())
        );
        let archive = bill_of(
            "b,k,200000,2024-01-15T00:00:00.000Z,INTELLIGENT_TIERING,ARCHIVE_ACCESS\n",
            None,
        );
        let archive_usd = charge(200_000, "0.0036".parse().unwrap())
            + charge(32_768, "0.0036".parse().unwrap())
            + charge(8_192, "0.023".parse().unwrap())
            + monitor;
        assert_eq!(
            class(&archive, "INTELLIGENT_TIERING").usd_month.as_deref(),
            Some(format_usd(archive_usd).as_str())
        );
        let unknown = bill_of(
            "b,k,131072,2024-01-15T00:00:00.000Z,INTELLIGENT_TIERING,SECRET_TIER\n",
            None,
        );
        assert_eq!(unknown.classes[0].usd_month, big.classes[0].usd_month);
        assert_eq!(unknown.warnings.len(), 1);
        assert!(!unknown.warnings[0].contains("SECRET_TIER"));
    }

    #[test]
    fn it_inventory_tier_names_without_access_suffix_match_the_access_names() {
        for (short, long) in [
            ("ARCHIVE", "ARCHIVE_ACCESS"),
            ("DEEP_ARCHIVE", "DEEP_ARCHIVE_ACCESS"),
        ] {
            let a = bill_of(
                &format!("b,k,200000,2024-01-15T00:00:00.000Z,INTELLIGENT_TIERING,{short}\n"),
                None,
            );
            let b = bill_of(
                &format!("b,k,200000,2024-01-15T00:00:00.000Z,INTELLIGENT_TIERING,{long}\n"),
                None,
            );
            assert_eq!(a.classes[0].usd_month, b.classes[0].usd_month);
            assert!(a.warnings.is_empty());
        }
    }

    #[test]
    fn claim_c10_express_has_no_floor() {
        let bill = bill_of("b,k,1000,2024-01-15T00:00:00.000Z,EXPRESS_ONEZONE,\n", None);
        let line = class(&bill, "EXPRESS_ONEZONE");
        assert_eq!(line.billable_bytes, 1000);
        assert_eq!(line.usd_month.as_deref(), Some(usd(1000, "0.11").as_str()));
        let gib = bill_of(
            "b,k,1073741824,2024-01-15T00:00:00.000Z,EXPRESS_ONEZONE,\n",
            None,
        );
        assert_eq!(
            class(&gib, "EXPRESS_ONEZONE").usd_month.as_deref(),
            Some("0.11")
        );
    }

    #[test]
    fn claim_c13_savings_omitted_without_as_of() {
        let bill = bill_of("b,k,1073741824,2024-01-15T00:00:00.000Z,STANDARD,\n", None);
        assert!(bill.lifecycle.is_none());
        assert_eq!(bill.total_usd, "0.023");
    }

    #[test]
    fn claim_c14_future_and_bad_dates_are_skipped_without_the_value() {
        let bill = bill_of(
            "b,ok,1073741824,2024-01-15T00:00:00.000Z,STANDARD,\nb,secret-key,5,2099-01-01T00:00:00.000Z,STANDARD,\nb,other-secret,5,not-a-date,STANDARD,\n",
            Some("2026-11-09"),
        );
        assert_eq!(bill.total_objects, 3);
        assert_eq!(
            bill.lifecycle.as_ref().unwrap().targets[0].eligible_objects,
            1
        );
        assert_eq!(bill.warnings.len(), 1);
        assert!(bill.warnings[0].contains("skipped for lifecycle savings: 2"));
        assert!(!bill.warnings[0].contains("secret"));
        assert!(!bill.warnings[0].contains("not-a-date"));
        assert!(!bill.warnings[0].contains("2099"));
    }

    #[test]
    fn claim_c15_prefix_depth_and_standard_share() {
        let mut text = String::from("Bucket,Key,Size,StorageClass\n");
        text.push_str("b,logs/2024/a,1073741824,STANDARD\n");
        text.push_str("b,logs/2024/b,1073741824,STANDARD\n");
        text.push_str("b,pics/c,1073741824,STANDARD\n");
        let deep = bill_many(&[text.as_bytes()], &prices_builtin(), None, 2).unwrap();
        assert_eq!(deep.prefixes.len(), 2);
        assert_eq!(deep.prefixes[0].prefix, "logs/2024/");
        assert_eq!(deep.prefixes[0].objects, 2);
        assert_eq!(deep.prefixes[1].prefix, "pics/c");
        assert_eq!(deep.total_usd, "0.069");
        assert_eq!(deep.prefixes[0].usd_month, "0.046");
        assert_eq!(deep.prefixes[1].usd_month, "0.023");
    }

    #[test]
    fn claim_c16_unknown_class_is_unpriced() {
        let bill = bill_of("b,k,10,2024-01-15T00:00:00.000Z,REDUCED_REDUNDANCY,\nb,k2,1073741824,2024-01-15T00:00:00.000Z,STANDARD,\n", None);
        let cold = class(&bill, "REDUCED_REDUNDANCY");
        assert_eq!(cold.usd_month, None);
        assert_eq!(bill.total_usd, "0.023");
        assert_eq!(bill.total_objects, 2);
        assert!(bill.warnings[0].contains("objects without a built-in rate: 1"));
    }

    #[test]
    fn unknown_class_uppercases_ascii_and_keeps_other_characters() {
        let bill = bill_of(
            "b,a,1073741824,2024-01-15T00:00:00.000Z,standard,\n\
             b,b,4,2024-01-15T00:00:00.000Z,caf\u{e9},\n\
             b,c,4,2024-01-15T00:00:00.000Z,Caf\u{e9},\n\
             b,d,1,2024-01-15T00:00:00.000Z,\u{e9},\n",
            None,
        );
        let standard = class(&bill, "STANDARD");
        assert_eq!(standard.name, "STANDARD");
        assert_eq!(standard.objects, 1);
        assert_eq!(standard.usd_month.as_deref(), Some("0.023"));
        let cafe = class(&bill, "CAF\u{e9}");
        assert_eq!(cafe.objects, 2);
        assert_eq!(cafe.raw_bytes, 8);
        assert_eq!(cafe.name.chars().count(), 4);
        assert_eq!(cafe.usd_month, None);
        let single = class(&bill, "\u{e9}");
        assert_eq!(single.name, "\u{e9}");
        assert_eq!(single.name.chars().count(), 1);
        let bad = b"Bucket,Key,Size,StorageClass\nb,k,1,\xff\n";
        let err = bill_many(&[bad], &prices_builtin(), None, 1).unwrap_err();
        assert_eq!(err.message(), "row 2: StorageClass is not UTF-8");
    }

    #[test]
    fn row_number_is_the_file_line_when_blank_lines_precede() {
        let bad = b"Bucket,Key,Size,StorageClass\n\nb,k,1,STANDARD\n\nb,k,x,STANDARD\n";
        let err = bill_many(&[bad], &prices_builtin(), None, 1).unwrap_err();
        assert_eq!(err.message(), "row 5: invalid Size");
    }

    #[test]
    fn composite_class_and_total_are_the_exact_sum_rounded_once() {
        let prices = prices_builtin();
        let rate = prices.glacier;
        let meta_rate = prices.standard[0];
        for size in 1u64..60 {
            let text =
                format!("Bucket,Key,Size,StorageClass\nb,k,{size},GLACIER\nb,j,{size},STANDARD\n");
            let bill = bill_many(&[text.as_bytes()], &prices, None, 1).unwrap();
            let meta_bytes = prices.meta_standard_bytes;
            let glacier = sum_charges(&[
                (size, rate),
                (prices.meta_class_bytes, rate),
                (meta_bytes, meta_rate),
            ])
            .unwrap();
            // Both classes are priced from the same Standard tier.
            let standard = sum_charges(&[(size, meta_rate)]).unwrap();
            let total = sum_charges(&[
                (size, rate),
                (prices.meta_class_bytes, rate),
                (meta_bytes, meta_rate),
                (size, meta_rate),
            ])
            .unwrap();
            assert_eq!(
                class(&bill, "GLACIER").usd_month,
                Some(format_usd(glacier)),
                "size {size}"
            );
            assert_eq!(
                class(&bill, "STANDARD").usd_month,
                Some(format_usd(standard))
            );
            assert_eq!(bill.total_usd, format_usd(total), "size {size}");
        }
    }

    #[test]
    fn claim_c18_prices_override_changes_the_bill() {
        let mut text = include_str!("../prices/s3-storage.json").to_string();
        text = text.replacen(
            "\"standard_first_50_tb\": \"0.023\"",
            "\"standard_first_50_tb\": \"0.5\"",
            1,
        );
        text = text.replacen(
            "\"source_url\": \"https://aws.amazon.com/s3/pricing/\"",
            "\"source_url\": \"https://example.test/quote\"",
            1,
        );
        let prices = load_prices_str(&text).unwrap();
        let csv = b"Bucket,Key,Size,StorageClass\nb,k,1073741824,STANDARD\n";
        let bill = bill_many(&[csv], &prices, None, 1).unwrap();
        assert_eq!(bill.total_usd, "0.50");
        assert_eq!(bill.source_url, "https://example.test/quote");
    }

    fn bill_with_standard_rate(rate: &str, rows: usize) -> Result<Bill, Error> {
        let text = include_str!("../prices/s3-storage.json").replacen(
            "\"standard_first_50_tb\": \"0.023\"",
            &format!("\"standard_first_50_tb\": \"{rate}\""),
            1,
        );
        let prices = load_prices_str(&text).unwrap();
        let mut csv = String::from("Bucket,Key,Size,StorageClass\n");
        for i in 0..rows {
            csv.push_str(&format!("b,k{i},1073741824,STANDARD\n"));
        }
        bill_many(&[csv.as_bytes()], &prices, None, 1)
    }

    #[test]
    fn share_survives_intermediate_overflow_when_total_fits() {
        let two = bill_with_standard_rate("100000000000000000000", 2).unwrap();
        assert_eq!(two.total_usd, "200000000000000000000.00");
        let one = bill_with_standard_rate("79228162514264337593543950335", 1).unwrap();
        assert_eq!(one.total_usd, "79228162514264337593543950335.00");
        let err = bill_with_standard_rate("79228162514264337593543950335", 2).unwrap_err();
        assert_eq!(err.message(), "amount does not fit");
    }

    #[test]
    fn wide_share_rounds_once_and_keeps_the_sum() {
        let money: Decimal = "11367196327.415037670449633151".parse().unwrap();
        let whole = 10_316_483_025_289_457_083u64;
        let (a, b) = (2_688_123_546_345_455_679u64, 7_628_359_478_944_001_404u64);
        let (left, right) = (
            share(a, whole, money).unwrap(),
            share(b, whole, money).unwrap(),
        );
        let gap = (left + right - money).abs();
        assert!(gap <= Decimal::new(2, 18), "{gap}");
        // Exact value 2961903589.503420583477500162213..., kept to 19 decimals.
        let expected: Decimal = "2961903589.5034205834775001622".parse().unwrap();
        assert!((left - expected).abs() <= Decimal::new(1, 19), "{left}");
        let negative = share(a, whole, -money).unwrap();
        assert_eq!(negative, -left);
        // The widest money and the widest ratio: rounded once, to the unit.
        let widest = Decimal::from_i128_with_scale((1i128 << 96) - 1, 0);
        let near = share(u64::MAX - 1, u64::MAX, widest).unwrap();
        assert_eq!(near.to_string(), "79228162514264337589248983039");
    }

    #[test]
    fn prefix_amounts_sum_to_the_total() {
        let text = include_str!("../prices/s3-storage.json").replacen(
            "\"standard_first_50_tb\": \"0.023\"",
            "\"standard_first_50_tb\": \"0.0233333333333333333333333333\"",
            1,
        );
        let prices = load_prices_str(&text).unwrap();
        let mut csv = String::from("Bucket,Key,Size,StorageClass\n");
        for key in ["a/1", "b/1", "c/1", "d/1"] {
            csv.push_str(&format!("b,{key},1073741827,STANDARD\n"));
        }
        let bill = bill_many(&[csv.as_bytes()], &prices, None, 1).unwrap();
        let sum = bill
            .prefixes
            .iter()
            .map(|p| p.usd_month.parse::<Decimal>().unwrap())
            .sum::<Decimal>();
        assert_eq!(sum, bill.total_usd.parse::<Decimal>().unwrap());
    }

    #[test]
    fn reconcile_fixes_rounding_but_rejects_a_large_gap() {
        let d = |s: &str| s.parse::<Decimal>().unwrap();
        let mut small = [d("1.00"), d("2.00")];
        reconcile(&mut small, d("3.000000000000000000001")).unwrap();
        assert_eq!(small[1], d("2.000000000000000000001"));
        let mut big = [d("1.00"), d("2.00")];
        let err = reconcile(&mut big, d("3.01")).unwrap_err();
        assert_eq!(err.message(), "prefix amounts do not add up to the total");
        assert_eq!(big, [d("1.00"), d("2.00")]);
    }

    #[test]
    fn claim_c20_errors_hide_file_contents() {
        let csv = b"Bucket,Key,Size,StorageClass\nsecret-bucket,super-secret-key,12abc,STANDARD\n";
        let err = bill_many(&[csv], &prices_builtin(), None, 1).unwrap_err();
        assert!(err.message().contains("invalid Size"));
        assert!(!err.message().contains("super-secret"));
        assert!(!err.message().contains("secret-bucket"));
        assert!(!err.message().contains("12abc"));
    }

    #[test]
    fn claim_c21_metadata_crosses_the_standard_tier() {
        let boundary = 50 * TB - 4096;
        let body = format!("b,std,{boundary},2024-01-15T00:00:00.000Z,STANDARD,\nb,gl,0,2024-01-15T00:00:00.000Z,GLACIER,\n");
        let bill = bill_of(&body, None);
        let right = charge(4096, "0.023".parse().unwrap()) + charge(4096, "0.022".parse().unwrap());
        let wrong = charge(8192, "0.023".parse().unwrap());
        assert_ne!(format_usd(right), format_usd(wrong));
        let glacier = charge(32_768, "0.0036".parse().unwrap()) + right;
        assert_eq!(
            class(&bill, "GLACIER").usd_month.as_deref(),
            Some(format_usd(glacier).as_str())
        );
    }

    #[test]
    fn claim_c22_small_standard_object_saves_a_negative_amount() {
        let bill = bill_of(
            "b,k,1000,2024-01-15T00:00:00.000Z,STANDARD,\n",
            Some("2026-11-09"),
        );
        let goal = target(&bill, "STANDARD_IA");
        assert!(goal.savings_over_horizon_usd.starts_with('-'));
        assert_eq!(goal.monthly_after_usd, usd(131_072, "0.0125"));
        assert_eq!(goal.transition_request_usd, "0.00001");
        assert_eq!(goal.early_delete_usd, "0.00");
    }

    #[test]
    fn claim_c23_retrieval_is_not_added() {
        let bill = bill_of(
            "b,k,1073741824,2024-01-15T00:00:00.000Z,STANDARD_IA,\n",
            None,
        );
        assert_eq!(
            class(&bill, "STANDARD_IA").usd_month.as_deref(),
            Some("0.0125")
        );
        let glacier = bill_of(
            "b,k,1073741824,2024-01-15T00:00:00.000Z,GLACIER_IR,\n",
            None,
        );
        assert_eq!(
            class(&glacier, "GLACIER_IR").usd_month.as_deref(),
            Some("0.004")
        );
    }

    #[test]
    fn claim_c24_files_share_one_standard_tier_schedule() {
        let first_size = 50 * TB - GIB;
        let file1 = format!("Bucket,Key,Size,StorageClass\nb,a,{first_size},STANDARD\n");
        let file2 = format!("Bucket,Key,Size,StorageClass\nb,b,{},STANDARD\n", 2 * GIB);
        let bill = bill_many(
            &[file1.as_bytes(), file2.as_bytes()],
            &prices_builtin(),
            None,
            1,
        )
        .unwrap();
        assert_eq!(bill.total_objects, 2);
        assert_eq!(bill.total_usd, "1177.622");
        assert_eq!(
            bill.prefixes.iter().map(|line| line.objects).sum::<u64>(),
            2
        );
    }

    #[test]
    fn one_gibibyte_standard_savings_over_thirty_days() {
        let bill = bill_of(
            "b,logs/app/a.bin,1073741824,2024-01-15T00:00:00.000Z,STANDARD,\n",
            Some("2026-11-09"),
        );
        let goal = target(&bill, "STANDARD_IA");
        assert_eq!(goal.monthly_now_usd, "0.023");
        assert_eq!(goal.monthly_after_usd, "0.0125");
        assert_eq!(goal.transition_request_usd, "0.00001");
        assert_eq!(goal.early_delete_usd, "0.00");
        assert_eq!(goal.savings_over_horizon_usd, "0.01049");
        assert_eq!(bill.prefixes[0].prefix, "logs/");
        assert_eq!(bill.total_usd, "0.023");
    }

    #[test]
    fn header_only_is_a_zero_bill() {
        let text = b"Bucket,Key,Size,StorageClass\n";
        let bill = bill_many(&[text], &prices_builtin(), None, 1).unwrap();
        assert_eq!(bill.total_objects, 0);
        assert_eq!(bill.total_usd, "0.00");
        assert!(bill.classes.is_empty());
    }

    #[test]
    fn empty_file_and_bad_depth_and_bad_as_of() {
        let err = bill_many(&[b""], &prices_builtin(), None, 1).unwrap_err();
        assert_eq!(err.message(), "empty inventory");
        let err = bill_many(
            &[b"Bucket,Key,Size,StorageClass\n"],
            &prices_builtin(),
            None,
            0,
        )
        .unwrap_err();
        assert_eq!(err.message(), "prefix depth must be 1..=8");
        let err = bill_many(
            &[b"Bucket,Key,Size,StorageClass\nb,k,1,STANDARD\n"],
            &prices_builtin(),
            Some("2026-02-31"),
            1,
        )
        .unwrap_err();
        assert_eq!(err.message(), "invalid --as-of");
        assert!(!err.message().contains("02-31"));
    }

    #[test]
    fn missing_column_and_short_row_and_duplicate_column() {
        let err = bill_many(&[b"Bucket,Key,Size\n"], &prices_builtin(), None, 1).unwrap_err();
        assert!(err.message().starts_with("missing column StorageClass: "));
        assert!(err.message().contains("Bucket,Key,Size,StorageClass"));
        let err = bill_many(
            &[b"Bucket,Key,Size,StorageClass\nb,k,1\n"],
            &prices_builtin(),
            None,
            1,
        )
        .unwrap_err();
        assert!(err.message().contains("column count"));
        let err = bill_many(
            &[b"Bucket,Key,Size,Size,StorageClass\n"],
            &prices_builtin(),
            None,
            1,
        )
        .unwrap_err();
        assert_eq!(err.message(), "duplicate column");
    }

    #[test]
    fn quoted_key_with_a_comma_is_one_object() {
        let csv = b"Bucket,Key,Size,StorageClass\nb,\"a,b/c\",4,STANDARD\n";
        let bill = bill_many(&[csv], &prices_builtin(), None, 1).unwrap();
        assert_eq!(bill.total_objects, 1);
        assert_eq!(bill.prefixes[0].prefix, "a,b/");
    }

    #[test]
    fn absent_date_column_warns_and_prices_storage() {
        let csv = b"Bucket,Key,Size,StorageClass\nb,k,1073741824,STANDARD\n";
        let bill = bill_many(&[csv], &prices_builtin(), Some("2026-11-09"), 1).unwrap();
        assert_eq!(bill.total_usd, "0.023");
        assert_eq!(
            bill.lifecycle.as_ref().unwrap().targets[0].eligible_objects,
            0
        );
        assert!(bill
            .warnings
            .iter()
            .any(|line| line.contains("LastModifiedDate column is absent")));
    }

    #[test]
    fn fifty_tebibytes_uses_the_second_standard_tier() {
        let size = 50 * TB + GIB;
        let body = format!("b,k,{size},2024-01-15T00:00:00.000Z,STANDARD,\n");
        let bill = bill_of(&body, None);
        assert_eq!(bill.total_usd, "1177.622");
    }

    #[test]
    fn zero_byte_glacier_still_bills_metadata() {
        let bill = bill_of("b,k,0,2024-01-15T00:00:00.000Z,GLACIER,\n", None);
        assert_eq!(class(&bill, "GLACIER").raw_bytes, 0);
        assert_eq!(class(&bill, "GLACIER").billable_bytes, 40_960);
    }

    #[test]
    fn case_insensitive_class_names_merge() {
        let bill = bill_of(
            "b,a,1,2024-01-15T00:00:00.000Z,standard,\nb,b,2,2024-01-15T00:00:00.000Z,Standard,\n",
            None,
        );
        assert_eq!(class(&bill, "STANDARD").objects, 2);
        assert_eq!(class(&bill, "STANDARD").raw_bytes, 3);
    }

    #[test]
    fn large_total_leaves_the_prefix_drift_on_the_largest_prefix() {
        let text = include_str!("../prices/s3-storage.json").replacen(
            "\"standard_first_50_tb\": \"0.023\"",
            "\"standard_first_50_tb\": \"777777777.7777777\"",
            1,
        );
        let prices = load_prices_str(&text).unwrap();
        let sizes = [1_073_741_827_u64, 3_221_225_473, 1_073_741_831];
        let mut csv = String::from("Bucket,Key,Size,StorageClass\n");
        for (i, size) in sizes.iter().enumerate() {
            csv.push_str(&format!("b,p{i}/x,{size},STANDARD\n"));
        }
        let bill = bill_many(&[csv.as_bytes()], &prices, None, 1).unwrap();
        let total = bill.total_usd.parse::<Decimal>().unwrap();
        let shown = bill
            .prefixes
            .iter()
            .map(|p| p.usd_month.parse::<Decimal>().unwrap())
            .collect::<Vec<_>>();
        // The sum rule stays: the total is the class sum. At this size the
        // prefix sum no longer matches it to the last digit.
        assert_ne!(shown.iter().copied().sum::<Decimal>(), total);
        // Uncorrected shares, from the same Standard schedule.
        let whole: u64 = sizes.iter().sum();
        let standard = tiered(whole, prices.standard).unwrap();
        let raw = sizes
            .iter()
            .map(|size| share(*size, whole, standard).unwrap())
            .collect::<Vec<_>>();
        let largest = (0..sizes.len()).max_by_key(|i| sizes[*i]).unwrap();
        assert_eq!(largest, 1);
        // The smaller prefixes carry no correction.
        for i in [0, 2] {
            assert_eq!(bill.prefixes[i].usd_month, format_usd(raw[i]));
        }
        // The drift is below half a unit of the largest prefix. Adding it
        // there changes nothing, and the gap between the prefix sum and the
        // total stays.
        assert_eq!(shown[largest], raw[largest]);
        let gap = (shown.iter().copied().sum::<Decimal>() - total).abs();
        assert!(!gap.is_zero() && gap < Decimal::new(1, 18), "{gap}");
    }

    fn row_error(row: &[u8]) -> String {
        let mut csv = b"Bucket,Key,Size,StorageClass\n".to_vec();
        csv.extend_from_slice(row);
        bill_many(&[csv.as_slice()], &prices_builtin(), None, 1)
            .unwrap_err()
            .message()
            .to_string()
    }

    #[test]
    fn row_texts_for_bucket_key_and_class_name_only_the_row() {
        assert_eq!(row_error(b" ,k,1,STANDARD\n"), "row 2: empty Bucket");
        assert_eq!(
            row_error(b"b\xff-secret,k,1,STANDARD\n"),
            "row 2: Bucket is not UTF-8"
        );
        assert_eq!(
            row_error(b"b,k\xff-secret,1,STANDARD\n"),
            "row 2: key is not UTF-8"
        );
        assert_eq!(row_error(b"b,k,1, \n"), "row 2: empty StorageClass");
    }

    #[test]
    fn no_inventory_files_text_from_bill_many_and_bill_plain() {
        let err = bill_many(&[], &prices_builtin(), None, 1).unwrap_err();
        assert_eq!(err.message(), "no inventory files");
        let err = bill_plain(&[], &prices_builtin(), None, 1).unwrap_err();
        assert_eq!(err.message(), "no inventory files");
    }

    #[test]
    fn object_total_does_not_fit_from_every_counter() {
        let full = || Pool {
            objects: u64::MAX,
            gl_objects: u64::MAX,
            da_objects: 1,
            ..Pool::default()
        };
        assert_eq!(
            full().bump(1).unwrap_err().message(),
            "object total does not fit"
        );
        let mut slot = u64::MAX;
        assert_eq!(
            Pool::inc(&mut slot).unwrap_err().message(),
            "object total does not fit"
        );
        let mut eligible = Eligible {
            objects: u64::MAX,
            ..Eligible::default()
        };
        assert_eq!(
            eligible.add(1, 0).unwrap_err().message(),
            "object total does not fit"
        );
        assert_eq!(
            meta_object_count(&full()).unwrap_err().message(),
            "object total does not fit"
        );
        let err = price_pool(
            &Pool {
                it_aa_objects: u64::MAX,
                it_da_objects: 1,
                ..Pool::default()
            },
            &prices_builtin(),
        )
        .err()
        .unwrap();
        assert_eq!(err.message(), "object total does not fit");
    }

    #[test]
    fn byte_total_does_not_fit_from_every_site() {
        assert_eq!(
            mul_bytes(u64::MAX, 2).unwrap_err().message(),
            "byte total does not fit"
        );
        let rates = prices_builtin().standard;
        assert_eq!(
            tiered_after(u64::MAX, 1, rates).unwrap_err().message(),
            "byte total does not fit"
        );
        let text = include_str!("../prices/s3-storage.json").replacen(
            "\"meta_class_bytes\": 32768",
            &format!("\"meta_class_bytes\": {}", u64::MAX),
            1,
        );
        let prices = load_prices_str(&text).unwrap();
        let csv = b"Bucket,Key,Size,StorageClass\nb,k,1,STANDARD\n";
        let err = bill_many(&[csv], &prices, None, 1).unwrap_err();
        assert_eq!(err.message(), "byte total does not fit");
        let pool = Pool {
            gl_raw: u64::MAX,
            gl_objects: 1,
            ..Pool::default()
        };
        let err = price_pool(&pool, &prices_builtin()).err().unwrap();
        assert_eq!(err.message(), "byte total does not fit");
    }

    fn bill_with_rates(body: &str, replacements: &[(&str, &str)]) -> Bill {
        let mut text = include_str!("../prices/s3-storage.json").to_string();
        for (old, new) in replacements {
            assert!(text.contains(old), "{old}");
            text = text.replacen(old, new, 1);
        }
        let prices = load_prices_str(&text).unwrap();
        let mut csv = String::from("Bucket,Key,Size,StorageClass\n");
        csv.push_str(body);
        bill_many(&[csv.as_bytes()], &prices, None, 1).unwrap()
    }

    /// Fractions rounded once, half away from zero, at the finest scale that fits.
    /// `2 / 2^30` ends in ...03125, so the last kept digit moves from 2 to 3.
    /// `5e-29` is half of `1e-28`. The stacked Glacier line is `8167487/2560000`.
    #[test]
    fn half_away_charge_share_product_sum_and_stacked_metadata() {
        let half = bill_with_rates(
            "b,a,2,STANDARD\n",
            &[(
                "\"standard_first_50_tb\": \"0.023\"",
                "\"standard_first_50_tb\": \"1\"",
            )],
        );
        assert_eq!(half.total_usd, "0.0000000018626451492309570313");

        let tiny = bill_with_rates(
            "b,a,1,STANDARD\n",
            &[(
                "\"standard_first_50_tb\": \"0.023\"",
                "\"standard_first_50_tb\": \"0.0000000000000000000536870912\"",
            )],
        );
        assert_eq!(tiny.total_usd, "0.0000000000000000000000000001");

        let stacked = bill_with_rates(
            "b,s,100000000000000000,STANDARD\nb,g,0,GLACIER\n",
            &[(
                "\"standard_over_500_tb\": \"0.021\"",
                "\"standard_over_500_tb\": \"418175.32\"",
            )],
        );
        assert_eq!(
            class(&stacked, "GLACIER").usd_month.as_deref(),
            Some("3.190424609375")
        );

        let shared = share(1, 1_073_741_824, Decimal::from(2)).unwrap();
        assert_eq!(format_usd(shared), "0.0000000018626451492309570313");

        let product = mul_amount(
            "0.000000000000005".parse().unwrap(),
            "0.00000000000001".parse().unwrap(),
        )
        .unwrap();
        assert_eq!(format_usd(product), "0.0000000000000000000000000001");

        let even = Decimal::from_i128_with_scale((1i128 << 96) - 2, 0);
        let summed = add_amount(even, "0.5".parse().unwrap()).unwrap();
        assert_eq!(summed.to_string(), "79228162514264337593543950335");
    }
    #[test]
    fn lifecycle_amounts_round_once_from_exact_values() {
        // Expected values from Python `fractions`, rounded once half away from zero
        // at 28 digits. Summing rounded pieces gave ...594, ...1406 and so on one
        // to two units of the last digit lower.
        let bill = bill_of(
            "b,a/x,1073741831,2020-01-01T00:00:00.000Z,STANDARD,\n\
             b,a/y,300000,2020-01-01T00:00:00.000Z,STANDARD,\n\
             b,a/z,987654321,2020-01-01T00:00:00.000Z,STANDARD,\n",
            Some("2026-11-09"),
        );
        let expected = [
            (
                "STANDARD_IA",
                "0.0240013021044433116912841797",
                "0.0201310937677323818206787109",
            ),
            (
                "GLACIER_IR",
                "0.0076804166734218597412109375",
                "0.1093859375962615013122558594",
            ),
            (
                "GLACIER",
                "0.0069132310241460800170898438",
                "0.1116574945440888404846191406",
            ),
            (
                "DEEP_ARCHIVE",
                "0.001901520192101597785949707",
                "0.2534152540804445743560791016",
            ),
        ];
        for (name, after, savings) in expected {
            let line = target(&bill, name);
            assert_eq!(line.monthly_after_usd, after, "{name} after");
            assert_eq!(line.savings_over_horizon_usd, savings, "{name} savings");
        }
    }
}
