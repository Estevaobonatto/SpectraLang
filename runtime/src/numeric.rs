//! Exact-width numeric helpers shared by the stdlib ABI and future native lowering.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericError {
    Overflow,
    NonFinite,
    OutOfRange,
}

pub fn signed_bounds(bits: u32) -> (i128, i128) {
    let max = (1_i128 << (bits - 1)) - 1;
    (-1_i128 << (bits - 1), max)
}

pub fn unsigned_max(bits: u32) -> u128 {
    if bits == 128 { u128::MAX } else { (1_u128 << bits) - 1 }
}

pub fn checked_int_to_int(value: i128, signed: bool, bits: u32) -> Result<i128, NumericError> {
    if signed {
        let (min, max) = signed_bounds(bits);
        if value < min || value > max { return Err(NumericError::OutOfRange); }
    } else if value < 0 || value as u128 > unsigned_max(bits) {
        return Err(NumericError::OutOfRange);
    }
    Ok(value)
}

pub fn checked_float_to_int(value: f64, signed: bool, bits: u32) -> Result<i128, NumericError> {
    if !value.is_finite() || value.fract() != 0.0 { return Err(NumericError::NonFinite); }
    let value = value as i128;
    checked_int_to_int(value, signed, bits)
}

pub fn checked_f64_to_f32(value: f64) -> Result<f32, NumericError> {
    if !value.is_finite() { return Err(NumericError::NonFinite); }
    let narrowed = value as f32;
    if !narrowed.is_finite() || narrowed as f64 != value { return Err(NumericError::OutOfRange); }
    Ok(narrowed)
}

pub fn wrapping_signed(value: i64, bits: u32) -> i64 {
    let mask = (1_i128 << bits) - 1;
    let raw = (value as i128) & mask;
    let sign = 1_i128 << (bits - 1);
    let signed = if raw & sign != 0 { raw - (1_i128 << bits) } else { raw };
    signed as i64
}

pub fn wrapping_unsigned(value: i64, bits: u32) -> i64 {
    if bits == 64 { value } else { ((value as u64) & ((1_u64 << bits) - 1)) as i64 }
}

pub fn wrapping_add_signed(a: i64, b: i64, bits: u32) -> i64 { wrapping_signed(a.wrapping_add(b), bits) }
pub fn wrapping_sub_signed(a: i64, b: i64, bits: u32) -> i64 { wrapping_signed(a.wrapping_sub(b), bits) }
pub fn wrapping_mul_signed(a: i64, b: i64, bits: u32) -> i64 { wrapping_signed(a.wrapping_mul(b), bits) }
pub fn wrapping_add_unsigned(a: i64, b: i64, bits: u32) -> i64 { wrapping_unsigned(a.wrapping_add(b), bits) }
pub fn wrapping_sub_unsigned(a: i64, b: i64, bits: u32) -> i64 { wrapping_unsigned(a.wrapping_sub(b), bits) }
pub fn wrapping_mul_unsigned(a: i64, b: i64, bits: u32) -> i64 { wrapping_unsigned(a.wrapping_mul(b), bits) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn signed_wraps() { assert_eq!(wrapping_add_signed(127, 1, 8), -128); }
    #[test] fn unsigned_wraps() { assert_eq!(wrapping_add_unsigned(255, 1, 8), 0); }
    #[test] fn checked_rejects() { assert!(checked_int_to_int(256, false, 8).is_err()); }
}
