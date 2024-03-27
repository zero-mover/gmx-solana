use std::cmp::Ordering;

use crate::{
    fixed::{Fixed, FixedPointOps},
    num::{MulDiv, Num},
};

use num_traits::{CheckedMul, One, Zero};

/// Usd value to market token amount.
///
/// Returns `None` if the computation cannot be done.
pub fn usd_to_market_token_amount<T>(
    usd_value: T,
    pool_value: T,
    supply: T,
    usd_to_amount_divisor: T,
) -> Option<T>
where
    T: MulDiv + Num,
{
    if usd_to_amount_divisor.is_zero() {
        return None;
    }
    if supply.is_zero() && pool_value.is_zero() {
        Some(usd_value / usd_to_amount_divisor)
    } else if supply.is_zero() && !pool_value.is_zero() {
        Some((pool_value.checked_add(&usd_value)?) / usd_to_amount_divisor)
    } else {
        supply.checked_mul_div(&usd_value, &pool_value)
    }
}

/// Apply factors using this formula: `A * x^E`.
///
/// Assuming that all values are "float"s with the same decimals.
pub fn apply_factors<T, const DECIMALS: u8>(value: T, factor: T, exponent_factor: T) -> Option<T>
where
    T: FixedPointOps<DECIMALS>,
{
    Some(
        apply_exponent_factor_wrapped(value, exponent_factor)?
            .checked_mul(&Fixed::from_inner(factor))?
            .into_inner(),
    )
}

fn apply_exponent_factor_wrapped<T, const DECIMALS: u8>(
    value: T,
    exponent_factor: T,
) -> Option<Fixed<T, DECIMALS>>
where
    T: FixedPointOps<DECIMALS>,
{
    let unit = Fixed::ONE;
    let value = Fixed::from_inner(value);
    let exponent = Fixed::from_inner(exponent_factor);

    let ans = match value.cmp(&unit) {
        Ordering::Less => Fixed::zero(),
        Ordering::Equal => unit,
        Ordering::Greater => {
            if exponent.is_zero() {
                unit
            } else if exponent.is_one() {
                value
            } else {
                value.checked_pow(&exponent)?
            }
        }
    };
    Some(ans)
}

/// Apply exponent factor using this formula: `x^E`.
///
/// Assuming that all values are "float"s with the same decimals.
#[inline]
pub fn apply_exponent_factor<T, const DECIMALS: u8>(value: T, exponent_factor: T) -> Option<T>
where
    T: FixedPointOps<DECIMALS>,
{
    Some(apply_exponent_factor_wrapped(value, exponent_factor)?.into_inner())
}

/// Apply factor using this formula: `A * x`.
///
/// Assuming that `value` and `factor` are a fixed-point decimals,
/// but they do not need to be of the same decimals.
/// The const type `DECIMALS` is the decimals of `factor`.
#[inline]
pub fn apply_factor<T, const DECIMALS: u8>(value: &T, factor: &T) -> Option<T>
where
    T: FixedPointOps<DECIMALS>,
{
    value.checked_mul_div(factor, &FixedPointOps::UNIT)
}
