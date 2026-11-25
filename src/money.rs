use std::cmp::Ordering;

use rust_decimal::{Decimal, RoundingStrategy};

use crate::error::Error;

const GIB: u64 = 1_073_741_824;

/// `bytes / 1024^3`. The S3 pricing examples treat 1 TB as 1,024 GB.
/// Rounded once, half away from zero.
#[must_use]
pub fn gb_from_bytes(bytes: Decimal) -> Decimal {
    if bytes.is_zero() {
        return Decimal::ZERO;
    }
    let numer = Big::from_u128(bytes.mantissa().unsigned_abs());
    let denom = Big::from_u64(GIB).mul_pow10(bytes.scale());
    round_ratio(bytes.is_sign_negative(), &numer, &denom).expect("amount does not fit")
}

/// Days since 1970-01-01 in the civil calendar. Howard Hinnant's algorithm.
/// `year`, `month`, and `day` must be a real Gregorian date.
#[must_use]
pub fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let mut y = i64::from(year);
    let m = i64::from(month);
    let d = i64::from(day);
    if m <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Decimal text with no exponent. Trailing zeros that `normalize` drops stay dropped.
#[must_use]
pub fn format_dec(value: Decimal) -> String {
    let text = value.normalize().to_string();
    if text.contains(['e', 'E']) {
        return value
            .round_dp_with_strategy(12, RoundingStrategy::MidpointAwayFromZero)
            .normalize()
            .to_string();
    }
    text
}

/// Like [`format_dec`], with at least two digits after the decimal point.
#[must_use]
pub fn format_usd(value: Decimal) -> String {
    let negative = value.is_sign_negative() && !value.is_zero();
    let text = format_dec(value.abs());
    let body = match text.split_once('.') {
        Some((whole, frac)) if frac.len() >= 2 => format!("{whole}.{frac}"),
        Some((whole, frac)) => format!("{whole}.{frac}{}", "0".repeat(2 - frac.len())),
        None => format!("{text}.00"),
    };
    if negative {
        format!("-{body}")
    } else {
        body
    }
}

pub(crate) fn parse_money(raw: &str) -> Result<Decimal, ()> {
    let text = raw.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
    if text.is_empty() || text.len() > 40 {
        return Err(());
    }
    let bytes = text.as_bytes();
    let mut dot = false;
    for (i, byte) in bytes.iter().enumerate() {
        match byte {
            b'0'..=b'9' => {}
            b'.' if !dot && i > 0 && i + 1 < bytes.len() => dot = true,
            _ => return Err(()),
        }
    }
    if let Some((_, frac)) = text.split_once('.') {
        if frac.len() > 28 {
            return Err(());
        }
    }
    Decimal::from_str_exact(text).map_err(|_| ())
}

pub(crate) fn valid_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    if !(digits(0..4) && digits(5..7) && digits(8..10)) {
        return false;
    }
    let Ok(year) = text[0..4].parse::<i32>() else {
        return false;
    };
    let Ok(month) = text[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = text[8..10].parse::<u32>() else {
        return false;
    };
    if !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=max).contains(&day)
}

pub(crate) fn ymd_days(text: &str) -> Option<i64> {
    if !valid_date(text) {
        return None;
    }
    Some(days_from_civil(
        text[0..4].parse().ok()?,
        text[5..7].parse().ok()?,
        text[8..10].parse().ok()?,
    ))
}

/// `bytes / 1024^3 * rate`, rounded once, half away from zero.
/// Overflow is an error, not a panic.
pub(crate) fn charge(bytes: u64, rate: Decimal) -> Result<Decimal, Error> {
    if bytes == 0 || rate.is_zero() {
        return Ok(Decimal::ZERO);
    }
    let numer = Big::from_u128(rate.mantissa().unsigned_abs()).mul_u64(bytes);
    let denom = Big::from_u64(GIB).mul_pow10(rate.scale());
    round_ratio(rate.is_sign_negative(), &numer, &denom)
        .ok_or_else(|| Error::new("amount does not fit"))
}

/// Product of two amounts, rounded once, half away from zero.
pub(crate) fn mul_dec(left: Decimal, right: Decimal) -> Option<Decimal> {
    if left.is_zero() || right.is_zero() {
        return Some(Decimal::ZERO);
    }
    let neg = left.is_sign_negative() ^ right.is_sign_negative();
    let numer = Big::from_u128(left.mantissa().unsigned_abs())
        .mul(&Big::from_u128(right.mantissa().unsigned_abs()));
    let denom = Big::pow10(left.scale().saturating_add(right.scale()));
    round_ratio(neg, &numer, &denom)
}

pub(crate) fn add_dec(left: Decimal, right: Decimal) -> Option<Decimal> {
    combine(left, right, false)
}

pub(crate) fn sub_dec(left: Decimal, right: Decimal) -> Option<Decimal> {
    combine(left, right, true)
}

/// `money * part / whole`, rounded once, half away from zero.
pub(crate) fn share_of(part: u64, whole: u64, money: Decimal) -> Option<Decimal> {
    if part == 0 || whole == 0 || money.is_zero() {
        return Some(Decimal::ZERO);
    }
    if part == whole {
        return Some(money);
    }
    let numer = Big::from_u128(money.mantissa().unsigned_abs()).mul_u64(part);
    let denom = Big::pow10(money.scale()).mul_u64(whole);
    round_ratio(money.is_sign_negative(), &numer, &denom)
}

/// Sum of `bytes / 1024^3 * rate`, rounded once, half away from zero.
pub(crate) fn sum_charges(parts: &[(u64, Decimal)]) -> Option<Decimal> {
    let mut max_scale = 0u32;
    let mut any = false;
    for (bytes, rate) in parts {
        if *bytes == 0 || rate.is_zero() {
            continue;
        }
        any = true;
        max_scale = max_scale.max(rate.scale());
    }
    if !any {
        return Some(Decimal::ZERO);
    }
    let mut pos = Big::zero();
    let mut neg = Big::zero();
    for (bytes, rate) in parts {
        if *bytes == 0 || rate.is_zero() {
            continue;
        }
        let piece = Big::from_u128(rate.mantissa().unsigned_abs())
            .mul_u64(*bytes)
            .mul_pow10(max_scale - rate.scale());
        if rate.is_sign_negative() {
            neg = neg.add(&piece);
        } else {
            pos = pos.add(&piece);
        }
    }
    let denom = Big::from_u64(GIB).mul_pow10(max_scale);
    let (neg, numer) = if pos.cmp(&neg) == Ordering::Less {
        (true, neg.sub(&pos))
    } else {
        (false, pos.sub(&neg))
    };
    round_ratio(neg, &numer, &denom)
}

/// The exact parts of one amount: `bytes / 1024^3 * rate` charges, per-unit
/// charges (`count * rate`), and one share (`part / whole`) of a sum of charges.
/// [`Terms::amount`] adds them all exactly and rounds once.
#[derive(Debug, Default, Clone)]
pub(crate) struct Terms {
    pub charges: Vec<(u64, Decimal)>,
    pub units: Vec<(u64, Decimal)>,
    pub share: Option<Share>,
}

/// One share of a sum of charges: `(part, whole, charges)`.
pub(crate) type Share = (u64, u64, Vec<(u64, Decimal)>);

impl Terms {
    /// The exact sum of every part, rounded once, half away from zero.
    pub(crate) fn amount(&self) -> Option<Decimal> {
        let mut scale = 0u32;
        let rates = self
            .charges
            .iter()
            .chain(&self.units)
            .chain(self.share.iter().flat_map(|(_, _, parts)| parts));
        for (_, rate) in rates {
            scale = scale.max(rate.scale());
        }
        let whole = match &self.share {
            Some((_, whole, _)) if *whole > 0 => *whole,
            _ => 1,
        };
        let mut pos = Big::zero();
        let mut neg = Big::zero();
        let mut put = |piece: Big, negative: bool| {
            if negative {
                neg = neg.add(&piece);
            } else {
                pos = pos.add(&piece);
            }
        };
        let piece = |count: u64, rate: &Decimal| {
            Big::from_u128(rate.mantissa().unsigned_abs())
                .mul_u64(count)
                .mul_pow10(scale - rate.scale())
        };
        for (bytes, rate) in &self.charges {
            if *bytes != 0 && !rate.is_zero() {
                put(piece(*bytes, rate).mul_u64(whole), rate.is_sign_negative());
            }
        }
        for (count, rate) in &self.units {
            if *count != 0 && !rate.is_zero() {
                let each = piece(*count, rate).mul_u64(GIB).mul_u64(whole);
                put(each, rate.is_sign_negative());
            }
        }
        if let Some((part, wide, parts)) = &self.share {
            if *part != 0 && *wide != 0 {
                for (bytes, rate) in parts {
                    if *bytes != 0 && !rate.is_zero() {
                        put(piece(*bytes, rate).mul_u64(*part), rate.is_sign_negative());
                    }
                }
            }
        }
        let denom = Big::from_u64(GIB).mul_pow10(scale).mul_u64(whole);
        let (negative, numer) = mag_diff_signed(&pos, &neg);
        round_ratio(negative, &numer, &denom)
    }
}

/// `(now - after) * days / 30 - request`, where `now` and `after` are sums of
/// `bytes / 1024^3 * rate`. Rounded once, half away from zero.
pub(crate) fn horizon_savings(
    now: &[(u64, Decimal)],
    after: &[(u64, Decimal)],
    days: u32,
    request: Decimal,
) -> Option<Decimal> {
    let mut scale = request.scale();
    for (_, rate) in now.iter().chain(after) {
        scale = scale.max(rate.scale());
    }
    let mut pos = Big::zero();
    let mut neg = Big::zero();
    let mut put = |piece: Big, negative: bool| {
        if negative {
            neg = neg.add(&piece);
        } else {
            pos = pos.add(&piece);
        }
    };
    for (bytes, rate) in now {
        if *bytes != 0 && !rate.is_zero() {
            let piece = Big::from_u128(rate.mantissa().unsigned_abs())
                .mul_u64(*bytes)
                .mul_pow10(scale - rate.scale())
                .mul_u64(u64::from(days));
            put(piece, rate.is_sign_negative());
        }
    }
    for (bytes, rate) in after {
        if *bytes != 0 && !rate.is_zero() {
            let piece = Big::from_u128(rate.mantissa().unsigned_abs())
                .mul_u64(*bytes)
                .mul_pow10(scale - rate.scale())
                .mul_u64(u64::from(days));
            put(piece, !rate.is_sign_negative());
        }
    }
    if !request.is_zero() {
        let piece = Big::from_u128(request.mantissa().unsigned_abs())
            .mul_pow10(scale - request.scale())
            .mul_u64(GIB)
            .mul_u64(30);
        put(piece, !request.is_sign_negative());
    }
    let denom = Big::from_u64(GIB).mul_u64(30).mul_pow10(scale);
    let (negative, numer) = mag_diff_signed(&pos, &neg);
    round_ratio(negative, &numer, &denom)
}

fn mag_diff_signed(pos: &Big, neg: &Big) -> (bool, Big) {
    if pos.cmp(neg) == Ordering::Less {
        (true, neg.sub(pos))
    } else {
        (false, pos.sub(neg))
    }
}

/// The exact sum in 128-bit integers, when it fits in 96 bits. `None` means
/// the general path must decide.
fn combine_small(left: Decimal, right: Decimal, subtract: bool) -> Option<Decimal> {
    let scale = left.scale().max(right.scale());
    let lift = |value: Decimal| {
        let factor = 10i128.checked_pow(scale - value.scale())?;
        value.mantissa().checked_mul(factor)
    };
    let left_int = lift(left)?;
    let right_int = lift(right)?;
    let sum = if subtract {
        left_int.checked_sub(right_int)?
    } else {
        left_int.checked_add(right_int)?
    };
    if sum.unsigned_abs() >= 1u128 << 96 {
        return None;
    }
    Some(Decimal::from_i128_with_scale(sum, scale))
}

fn combine(left: Decimal, right: Decimal, subtract: bool) -> Option<Decimal> {
    if let Some(sum) = combine_small(left, right, subtract) {
        return Some(sum);
    }
    let (left_neg, left_mag, left_scale) = signed_parts(left);
    let (mut right_neg, right_mag, right_scale) = signed_parts(right);
    if subtract && !right_mag.is_zero() {
        right_neg = !right_neg;
    }
    let scale = left_scale.max(right_scale);
    let left_mag = left_mag.mul_pow10(scale - left_scale);
    let right_mag = right_mag.mul_pow10(scale - right_scale);
    let (neg, numer) = match (left_neg, right_neg) {
        (false, false) => (false, left_mag.add(&right_mag)),
        (true, true) => (true, left_mag.add(&right_mag)),
        (false, true) => mag_diff(&left_mag, &right_mag),
        (true, false) => mag_diff(&right_mag, &left_mag),
    };
    round_ratio(neg, &numer, &Big::pow10(scale))
}

fn signed_parts(value: Decimal) -> (bool, Big, u32) {
    if value.is_zero() {
        return (false, Big::zero(), 0);
    }
    (
        value.is_sign_negative(),
        Big::from_u128(value.mantissa().unsigned_abs()),
        value.scale(),
    )
}

/// `left - right` as a sign and a magnitude. `left` and `right` are magnitudes.
fn mag_diff(left: &Big, right: &Big) -> (bool, Big) {
    if left.cmp(right) == Ordering::Less {
        (true, right.sub(left))
    } else {
        (false, left.sub(right))
    }
}

/// Round `numer / denom` once, half away from zero, at the finest scale in
/// `0..=28` whose mantissa still fits in 96 bits.
fn round_ratio(negative: bool, numer: &Big, denom: &Big) -> Option<Decimal> {
    if numer.is_zero() {
        return Some(Decimal::ZERO);
    }
    if denom.is_zero() {
        return None;
    }
    // The value is at least 2^(bits(numer) - bits(denom) - 1). A scale that makes
    // that lower bound exceed 96 bits cannot fit, so start below it.
    let low_bits = i64::from(numer.bit_len()) - i64::from(denom.bit_len()) - 1;
    let room = 96 - low_bits.max(0);
    let mut scale = u32::try_from((room * 30103 / 100_000).clamp(0, 28)).unwrap_or(28);
    loop {
        let (mut quot, rem) = numer.mul_pow10(scale).divmod(denom);
        if !rem.is_zero() && rem.double().cmp(denom) != Ordering::Less {
            quot = quot.add_u64(1);
        }
        if quot.is_zero() {
            return Some(Decimal::ZERO);
        }
        if quot.fits_mantissa() {
            let mag = quot.to_u128();
            let signed = i128::try_from(mag).ok()?;
            let signed = if negative { -signed } else { signed };
            return Some(Decimal::from_i128_with_scale(signed, scale));
        }
        if scale == 0 {
            return None;
        }
        scale -= 1;
    }
}

/// Little-endian unsigned integer. Leading zero limbs are dropped.
#[derive(Clone)]
struct Big {
    v: Vec<u64>,
}

impl Big {
    fn zero() -> Self {
        Self { v: Vec::new() }
    }

    fn from_u64(n: u64) -> Self {
        if n == 0 {
            Self::zero()
        } else {
            Self { v: vec![n] }
        }
    }

    fn from_u128(n: u128) -> Self {
        let lo = n as u64;
        let hi = (n >> 64) as u64;
        if hi != 0 {
            Self { v: vec![lo, hi] }
        } else {
            Self::from_u64(lo)
        }
    }

    fn is_zero(&self) -> bool {
        self.v.is_empty()
    }

    fn cmp(&self, other: &Self) -> Ordering {
        match self.v.len().cmp(&other.v.len()) {
            Ordering::Equal => {
                for i in (0..self.v.len()).rev() {
                    match self.v[i].cmp(&other.v[i]) {
                        Ordering::Equal => {}
                        order => return order,
                    }
                }
                Ordering::Equal
            }
            order => order,
        }
    }

    fn add(&self, other: &Self) -> Self {
        let n = self.v.len().max(other.v.len());
        let mut out = Vec::with_capacity(n + 1);
        let mut carry = 0u128;
        for i in 0..n {
            carry += u128::from(*self.v.get(i).unwrap_or(&0));
            carry += u128::from(*other.v.get(i).unwrap_or(&0));
            out.push(carry as u64);
            carry >>= 64;
        }
        if carry != 0 {
            out.push(carry as u64);
        }
        Self { v: out }
    }

    fn add_u64(&self, n: u64) -> Self {
        self.add(&Self::from_u64(n))
    }

    fn sub(&self, other: &Self) -> Self {
        let mut out = Vec::with_capacity(self.v.len());
        let mut borrow = 0i128;
        for (i, limb) in self.v.iter().enumerate() {
            let mut diff = i128::from(*limb) - i128::from(*other.v.get(i).unwrap_or(&0)) - borrow;
            if diff < 0 {
                diff += 1_i128 << 64;
                borrow = 1;
            } else {
                borrow = 0;
            }
            out.push(diff as u64);
        }
        Self::trimmed(out)
    }

    fn mul_u64(&self, factor: u64) -> Self {
        if factor == 0 || self.is_zero() {
            return Self::zero();
        }
        let mut out = Vec::with_capacity(self.v.len() + 1);
        let mut carry = 0u128;
        for limb in &self.v {
            carry += u128::from(*limb) * u128::from(factor);
            out.push(carry as u64);
            carry >>= 64;
        }
        if carry != 0 {
            out.push(carry as u64);
        }
        Self { v: out }
    }

    fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        let mut out = vec![0u64; self.v.len() + other.v.len()];
        for (i, left) in self.v.iter().enumerate() {
            let mut carry = 0u128;
            for (j, right) in other.v.iter().enumerate() {
                carry += u128::from(out[i + j]) + u128::from(*left) * u128::from(*right);
                out[i + j] = carry as u64;
                carry >>= 64;
            }
            let mut k = i + other.v.len();
            while carry != 0 {
                if k == out.len() {
                    out.push(0);
                }
                carry += u128::from(out[k]);
                out[k] = carry as u64;
                carry >>= 64;
                k += 1;
            }
        }
        Self::trimmed(out)
    }

    fn pow10(n: u32) -> Self {
        Self::from_u64(1).mul_pow10(n)
    }

    fn mul_pow10(&self, n: u32) -> Self {
        if n == 0 || self.is_zero() {
            return self.clone();
        }
        let mut value = self.clone();
        let mut left = n;
        while left >= 18 {
            value = value.mul_u64(1_000_000_000_000_000_000);
            left -= 18;
        }
        if left > 0 {
            let mut factor = 1u64;
            for _ in 0..left {
                factor *= 10;
            }
            value = value.mul_u64(factor);
        }
        value
    }

    fn double(&self) -> Self {
        self.mul_u64(2)
    }

    /// Quotient and remainder, schoolbook division on 64-bit limbs (Knuth, algorithm D).
    /// `denom` must not be zero.
    fn divmod(&self, denom: &Self) -> (Self, Self) {
        if self.cmp(denom) == Ordering::Less {
            return (Self::zero(), self.clone());
        }
        if let [only] = denom.v.as_slice() {
            let divisor = u128::from(*only);
            let mut quot = vec![0u64; self.v.len()];
            let mut rem = 0u128;
            for i in (0..self.v.len()).rev() {
                let cur = (rem << 64) | u128::from(self.v[i]);
                quot[i] = (cur / divisor) as u64;
                rem = cur % divisor;
            }
            return (Self::trimmed(quot), Self::from_u64(rem as u64));
        }
        let shift = denom.v[denom.v.len() - 1].leading_zeros();
        let v = denom.shl_bits(shift).v;
        let mut u = self.shl_bits(shift).v;
        if u.len() == self.v.len() {
            u.push(0);
        }
        let n = v.len();
        let m = u.len() - n - 1;
        let mut quot = vec![0u64; m + 1];
        let base = 1u128 << 64;
        for j in (0..=m).rev() {
            let top = (u128::from(u[j + n]) << 64) | u128::from(u[j + n - 1]);
            let mut qhat = top / u128::from(v[n - 1]);
            let mut rhat = top % u128::from(v[n - 1]);
            while qhat >= base
                || qhat * u128::from(v[n - 2]) > ((rhat << 64) | u128::from(u[j + n - 2]))
            {
                qhat -= 1;
                rhat += u128::from(v[n - 1]);
                if rhat >= base {
                    break;
                }
            }
            let mut borrow = 0i128;
            let mut carry = 0u128;
            for i in 0..n {
                let product = qhat * u128::from(v[i]) + carry;
                carry = product >> 64;
                let diff = i128::from(u[i + j]) - borrow - i128::from(product as u64);
                u[i + j] = diff as u64;
                borrow = i128::from(diff < 0);
            }
            let diff = i128::from(u[j + n]) - borrow - carry as i128;
            u[j + n] = diff as u64;
            if diff < 0 {
                qhat -= 1;
                let mut carry = 0u128;
                for i in 0..n {
                    carry += u128::from(u[i + j]) + u128::from(v[i]);
                    u[i + j] = carry as u64;
                    carry >>= 64;
                }
                u[j + n] = u[j + n].wrapping_add(carry as u64);
            }
            quot[j] = qhat as u64;
        }
        u.truncate(n);
        (Self::trimmed(quot), Self::trimmed(u).shr_bits(shift))
    }

    fn shl_bits(&self, shift: u32) -> Self {
        if shift == 0 || self.is_zero() {
            return self.clone();
        }
        let mut out = Vec::with_capacity(self.v.len() + 1);
        let mut carry = 0u64;
        for limb in &self.v {
            out.push((limb << shift) | carry);
            carry = limb >> (64 - shift);
        }
        if carry != 0 {
            out.push(carry);
        }
        Self { v: out }
    }

    fn shr_bits(&self, shift: u32) -> Self {
        if shift == 0 || self.is_zero() {
            return self.clone();
        }
        let out: Vec<u64> = (0..self.v.len())
            .map(|i| {
                let high = self.v.get(i + 1).map_or(0, |next| next << (64 - shift));
                (self.v[i] >> shift) | high
            })
            .collect();
        Self::trimmed(out)
    }

    fn fits_mantissa(&self) -> bool {
        match self.v.as_slice() {
            [] | [_] => true,
            [_, high] => *high < 1 << 32,
            _ => false,
        }
    }

    fn to_u128(&self) -> u128 {
        match self.v.as_slice() {
            [] => 0,
            [lo] => u128::from(*lo),
            [lo, hi, ..] => u128::from(*lo) | (u128::from(*hi) << 64),
        }
    }

    fn bit_len(&self) -> u32 {
        let Some(last) = self.v.last() else {
            return 0;
        };
        (self.v.len() as u32 - 1) * 64 + (64 - last.leading_zeros())
    }

    #[cfg(test)]
    fn bit(&self, index: u32) -> bool {
        let limb = (index / 64) as usize;
        self.v
            .get(limb)
            .is_some_and(|word| word & (1u64 << (index % 64)) != 0)
    }

    fn trimmed(v: Vec<u64>) -> Self {
        let mut v = v;
        while v.last() == Some(&0) {
            v.pop();
        }
        Self { v }
    }
}

pub(crate) fn checked_add(slot: &mut u64, add: u64) -> Result<(), Error> {
    *slot = slot
        .checked_add(add)
        .ok_or_else(|| Error::new("byte total does not fit"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{
        days_from_civil, format_dec, format_usd, gb_from_bytes, parse_money, valid_date, ymd_days,
        Big,
    };

    #[test]
    fn parse_money_trims_only_space_tab_cr_lf() {
        for ok in [" 1", "\t1", "1 ", "1\t", "\r\n1\r\n", " 1.50 "] {
            assert!(parse_money(ok).is_ok(), "{ok:?}");
        }
        for bad in [
            "\u{a0}1",
            "1\u{a0}",
            "\u{2028}1",
            "1\u{2028}",
            "\u{3000}1",
            "1\u{3000}",
            " \u{a0}1",
        ] {
            assert!(parse_money(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn claim_c2_gibibyte_and_terabyte() {
        let gib = Decimal::from(1_073_741_824_u64);
        assert_eq!(gb_from_bytes(gib), Decimal::ONE);
        let one_tb = gib * Decimal::from(1024);
        assert_eq!(gb_from_bytes(one_tb), Decimal::from(1024));
        let rate = "0.023".parse::<Decimal>().unwrap();
        assert_eq!(format_usd(gb_from_bytes(one_tb) * rate), "23.552");
        assert_eq!(format_usd(gb_from_bytes(gib) * rate), "0.023");
    }

    #[test]
    fn unix_epoch_and_a_non_leap_span() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        let span = days_from_civil(2026, 11, 9) - days_from_civil(2025, 11, 9);
        assert_eq!(span, 365);
    }

    #[test]
    fn leap_day_span() {
        assert_eq!(
            days_from_civil(2024, 3, 1) - days_from_civil(2024, 2, 28),
            2
        );
        assert_eq!(
            days_from_civil(2025, 3, 1) - days_from_civil(2025, 2, 28),
            1
        );
    }

    #[test]
    fn money_rejects_exponent_sign_and_empty() {
        assert!(parse_money("0.023").is_ok());
        assert!(parse_money("0").is_ok());
        assert!(parse_money("1e-7").is_err());
        assert!(parse_money("1E-7").is_err());
        assert!(parse_money("-1").is_err());
        assert!(parse_money("+1").is_err());
        assert!(parse_money(".2").is_err());
        assert!(parse_money("1.").is_err());
        assert!(parse_money("").is_err());
        assert!(parse_money("0.0000000000000000000000000001").is_ok());
        assert!(parse_money("0.00000000000000000000000000001").is_err());
        assert!(parse_money("79228162514264337593543950335").is_ok());
        assert!(parse_money("10000000000000000000000000.12345678").is_err());
    }

    #[test]
    fn usd_keeps_two_digits_and_a_minus() {
        // 2 / 2^30 ends in the digit 5. Half away from zero keeps the next digit up.
        assert_eq!(
            format_dec(gb_from_bytes(Decimal::from(2))),
            "0.0000000018626451492309570313"
        );
        assert_eq!(format_usd(Decimal::ZERO), "0.00");
        assert_eq!(format_usd(Decimal::from(2)), "2.00");
        assert_eq!(format_usd("0.3".parse().unwrap()), "0.30");
        assert_eq!(format_dec("10.05".parse().unwrap()), "10.05");
        assert_eq!(format_usd("-0.01049".parse().unwrap()), "-0.01049");
        let tiny = Decimal::from(1) / Decimal::from(1_073_741_824_u64) * Decimal::new(23, 3);
        let text = format_usd(tiny);
        assert!(!text.contains(['e', 'E']), "{text}");
    }

    #[test]
    fn dates_follow_the_calendar() {
        assert!(valid_date("2026-11-09"));
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2025-02-29"));
        assert!(!valid_date("2026-13-01"));
        assert!(!valid_date("2026-00-01"));
        assert!(!valid_date("2026-+1-09"));
        assert!(!valid_date("2026-01-+9"));
        assert!(!valid_date("+026-01-09"));
        assert!(!valid_date("2026-04-31"));
        assert!(!valid_date("2026-11-9"));
        assert!(!valid_date("20261109"));
        assert!(valid_date("2000-02-29"));
        assert!(!valid_date("1900-02-29"));
        assert_eq!(ymd_days("1970-01-01"), Some(0));
        assert_eq!(ymd_days("1970-01-02"), Some(1));
    }

    /// Bit-by-bit long division, the slow reference for `Big::divmod`.
    fn slow_divmod(numer: &Big, denom: &Big) -> (Big, Big) {
        let mut quot = Big::zero();
        let mut rem = Big::zero();
        for i in (0..numer.bit_len()).rev() {
            quot = quot.double();
            rem = rem.double();
            if numer.bit(i) {
                rem = rem.add_u64(1);
            }
            if rem.cmp(denom) != std::cmp::Ordering::Less {
                rem = rem.sub(denom);
                quot = quot.add_u64(1);
            }
        }
        (quot, Big::trimmed(rem.v))
    }

    #[test]
    fn fast_division_equals_bit_by_bit_division() {
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let edge = [0, 1, 2, 1 << 63, u64::MAX, u64::MAX - 1, 1 << 32];
        for round in 0..3000 {
            let mut limbs = |count: usize| {
                let v: Vec<u64> = (0..count)
                    .map(|_| match next() % 4 {
                        0 => edge[(next() % 7) as usize],
                        _ => next(),
                    })
                    .collect();
                Big::trimmed(v)
            };
            let n_len = 1 + (round % 6);
            let d_len = 1 + ((round / 6) % 4);
            let numer = limbs(n_len);
            let denom = limbs(d_len);
            if denom.is_zero() {
                continue;
            }
            let (q, r) = numer.divmod(&denom);
            let (sq, sr) = slow_divmod(&numer, &denom);
            assert_eq!(q.v, sq.v, "quotient, round {round}");
            assert_eq!(r.v, sr.v, "remainder, round {round}");
        }
    }
}
