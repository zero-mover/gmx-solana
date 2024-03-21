use std::fmt;

use crate::{
    market::Market,
    num::{MulDiv, Num, UnsignedAbs},
    pool::Pool,
};
use num_traits::{CheckedSub, One, Signed};

/// Test Pool.
#[derive(Debug, Default)]
pub struct TestPool<T> {
    long_token_amount: T,
    short_token_amount: T,
}

impl<T> Pool for TestPool<T>
where
    T: MulDiv + Num + CheckedSub,
{
    type Num = T;

    type Signed = T::Signed;

    fn long_token_amount(&self) -> Self::Num {
        self.long_token_amount.clone()
    }

    fn short_token_amount(&self) -> Self::Num {
        self.short_token_amount.clone()
    }

    fn apply_delta_to_long_token_amount(
        &mut self,
        delta: Self::Signed,
    ) -> Result<(), crate::Error> {
        if delta.is_positive() {
            self.long_token_amount = self
                .long_token_amount
                .checked_add(&delta.unsigned_abs())
                .ok_or(crate::Error::Overflow)?;
        } else {
            self.long_token_amount = self
                .long_token_amount
                .checked_sub(&delta.unsigned_abs())
                .ok_or(crate::Error::Underflow)?;
        }
        Ok(())
    }

    fn apply_delta_to_short_token_amount(
        &mut self,
        delta: Self::Signed,
    ) -> Result<(), crate::Error> {
        if delta.is_positive() {
            self.short_token_amount = self
                .short_token_amount
                .checked_add(&delta.unsigned_abs())
                .ok_or(crate::Error::Overflow)?;
        } else {
            self.short_token_amount = self
                .short_token_amount
                .checked_sub(&delta.unsigned_abs())
                .ok_or(crate::Error::Underflow)?;
        }
        Ok(())
    }
}

/// Test Market.
#[derive(Debug, Default)]
pub struct TestMarket<T> {
    primary: TestPool<T>,
    price_impact: TestPool<T>,
    total_supply: T,
}

impl<T> Market for TestMarket<T>
where
    T: MulDiv + Num + CheckedSub + fmt::Display,
    T::Signed: Num,
{
    type Num = T;

    type Signed = T::Signed;

    type Pool = TestPool<T>;

    fn pool(&self) -> &Self::Pool {
        &self.primary
    }

    fn pool_mut(&mut self) -> &mut Self::Pool {
        &mut self.primary
    }

    fn price_impact_pool(&self) -> &Self::Pool {
        &self.price_impact
    }

    fn price_impact_pool_mut(&mut self) -> &mut Self::Pool {
        &mut self.price_impact
    }

    fn total_supply(&self) -> &Self::Num {
        &self.total_supply
    }

    fn mint(&mut self, amount: &Self::Num) -> Result<(), crate::Error> {
        self.total_supply = self
            .total_supply
            .checked_add(amount)
            .ok_or(crate::Error::Overflow)?;
        println!("minted: {amount}");
        Ok(())
    }

    fn usd_to_amount_divisor(&self) -> Self::Num {
        One::one()
    }
}
