use num_traits::{CheckedAdd, CheckedMul, Zero};

use crate::{
    market::{Market, MarketExt},
    num::{MulDiv, Unsigned, UnsignedAbs},
    pool::Pool,
    utils, PoolExt,
};

/// A deposit.
#[must_use = "Action do nothing if not execute"]
pub struct Deposit<M: Market> {
    market: M,
    long_token_amount: M::Num,
    short_token_amount: M::Num,
    long_token_price: M::Num,
    short_token_price: M::Num,
}

struct PoolParams<T> {
    long_token_usd_value: T,
    short_token_usd_value: T,
    delta_long_token_usd_value: T,
    delta_short_token_usd_value: T,
    next_long_token_usd_value: T,
    next_short_token_usd_value: T,
}

impl<T> PoolParams<T>
where
    T: MulDiv + Clone + Ord,
{
    #[inline]
    fn initial_diff_usd(&self) -> T {
        self.long_token_usd_value
            .clone()
            .diff(self.short_token_usd_value.clone())
    }

    #[inline]
    fn next_diff_usd(&self) -> T {
        self.next_long_token_usd_value
            .clone()
            .diff(self.next_short_token_usd_value.clone())
    }

    #[inline]
    fn is_same_side_rebalance(&self) -> bool {
        (self.long_token_usd_value <= self.short_token_usd_value)
            == (self.next_long_token_usd_value <= self.next_short_token_usd_value)
    }

    fn price_impact(&self) -> T::Signed {
        if self.is_same_side_rebalance() {
            self.price_impact_for_same_side_rebalance()
        } else {
            self.price_impact_for_cross_over_rebalance()
        }
    }

    #[inline]
    fn price_impact_for_same_side_rebalance(&self) -> T::Signed {
        let initial = self.initial_diff_usd();
        let next = self.next_diff_usd();
        todo!()
    }

    #[inline]
    fn price_impact_for_cross_over_rebalance(&self) -> T::Signed {
        let initial = self.initial_diff_usd();
        let next = self.next_diff_usd();
        todo!()
    }
}

impl<M: Market> Deposit<M> {
    /// Create a new deposit to the given market.
    pub fn try_new(
        market: M,
        long_token_amount: M::Num,
        short_token_amount: M::Num,
        long_token_price: M::Num,
        short_token_price: M::Num,
    ) -> Result<Self, crate::Error> {
        if long_token_amount.is_zero() && short_token_amount.is_zero() {
            return Err(crate::Error::EmptyDeposit);
        }
        Ok(Self {
            market,
            long_token_amount,
            short_token_amount,
            long_token_price,
            short_token_price,
        })
    }

    fn pool_params(&self) -> Option<PoolParams<M::Num>> {
        let long_token_usd_value = self
            .market
            .pool()
            .long_token_usd_value(&self.long_token_price)?;
        let short_token_usd_value = self
            .market
            .pool()
            .short_token_usd_value(&self.short_token_price)?;
        let delta_long_token_usd_value =
            self.long_token_amount.checked_mul(&self.long_token_price)?;
        let delta_short_token_usd_value = self
            .short_token_amount
            .checked_mul(&self.short_token_price)?;
        Some(PoolParams {
            next_long_token_usd_value: long_token_usd_value
                .checked_add(&delta_long_token_usd_value)?,
            next_short_token_usd_value: short_token_usd_value
                .checked_add(&delta_short_token_usd_value)?,
            long_token_usd_value,
            short_token_usd_value,
            delta_long_token_usd_value,
            delta_short_token_usd_value,
        })
    }

    /// Get the price impact USD value.
    fn price_impact(&self) -> Option<(M::Signed, M::Num, M::Num)> {
        let params = self.pool_params()?;
        let price_impact = params.price_impact();
        Some((
            price_impact,
            params.delta_long_token_usd_value,
            params.delta_short_token_usd_value,
        ))
    }

    fn deposit(
        &mut self,
        is_long_token: bool,
        pool_value: M::Num,
        _price_impact: M::Signed,
    ) -> Result<M::Num, crate::Error> {
        let mut mint_amount: M::Num = Zero::zero();
        let supply = self.market.total_supply();
        if pool_value.is_zero() && !supply.is_zero() {
            return Err(crate::Error::InvalidPoolValueForDeposit);
        }
        let (amount, price) = if is_long_token {
            (&self.long_token_amount, &self.long_token_price)
        } else {
            (&self.short_token_amount, &self.short_token_price)
        };
        // TODO: handle fees.
        // TODO: apply price impact.
        mint_amount = mint_amount
            .checked_add(
                &utils::usd_to_market_token_amount(
                    amount.checked_mul(price).ok_or(crate::Error::Computation)?,
                    pool_value,
                    supply.clone(),
                    self.market.usd_to_amount_divisor(),
                )
                .ok_or(crate::Error::Computation)?,
            )
            .ok_or(crate::Error::Computation)?;
        if is_long_token {
            self.market.pool_mut().apply_delta_to_long_token_amount(
                amount
                    .clone()
                    .try_into()
                    .map_err(|_| crate::Error::Convert)?,
            )?;
        } else {
            self.market.pool_mut().apply_delta_to_short_token_amount(
                amount
                    .clone()
                    .try_into()
                    .map_err(|_| crate::Error::Convert)?,
            )?;
        }
        Ok(mint_amount)
    }

    /// Execute.
    pub fn execute(mut self) -> Result<(), crate::Error> {
        debug_assert!(
            !self.long_token_amount.is_zero() || !self.short_token_amount.is_zero(),
            "shouldn't be empty deposit"
        );
        let (price_impact, long_token_usd_value, short_token_usd_value) =
            self.price_impact().ok_or(crate::Error::Computation)?;
        let mut market_token_to_mint: M::Num = Zero::zero();
        let pool_value = self
            .market
            .pool_value(&self.long_token_price, &self.short_token_price)
            .ok_or(crate::Error::Computation)?;
        if !self.long_token_amount.is_zero() {
            let price_impact = long_token_usd_value
                .clone()
                .checked_mul_div_with_signed_numberator(
                    price_impact.clone(),
                    long_token_usd_value
                        .checked_add(&short_token_usd_value)
                        .ok_or(crate::Error::Computation)?,
                )
                .ok_or(crate::Error::Computation)?;
            market_token_to_mint = market_token_to_mint
                .checked_add(&self.deposit(true, pool_value.clone(), price_impact)?)
                .ok_or(crate::Error::Computation)?;
        }
        if !self.short_token_amount.is_zero() {
            let price_impact = short_token_usd_value
                .clone()
                .checked_mul_div_with_signed_numberator(
                    price_impact,
                    long_token_usd_value
                        .checked_add(&short_token_usd_value)
                        .ok_or(crate::Error::Computation)?,
                )
                .ok_or(crate::Error::Computation)?;
            market_token_to_mint = market_token_to_mint
                .checked_add(&self.deposit(false, pool_value, price_impact)?)
                .ok_or(crate::Error::Computation)?;
        }
        self.market.mint(&market_token_to_mint)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{market::MarketExt, test::TestMarket};

    #[test]
    fn basic() -> Result<(), crate::Error> {
        let mut market = TestMarket::<u64>::default();
        market.deposit(1000, 0, 120, 1)?.execute()?;
        market.deposit(0, 2000, 120, 1)?.execute()?;
        market.deposit(100, 0, 100, 1)?.execute()?;
        println!("{market:?}, {}", market.pool_value(&200, &1).unwrap());
        market.deposit(100, 0, 200, 1)?.execute()?;
        println!("{market:?}, {}", market.pool_value(&200, &1).unwrap());
        market.deposit(100, 0, 200, 1)?.execute()?;
        Ok(())
    }

    #[cfg(feature = "u128")]
    #[test]
    fn basic_u128() -> Result<(), crate::Error> {
        let mut market = TestMarket::<u128>::default();
        market.deposit(1000, 0, 120, 1)?.execute()?;
        market.deposit(0, 2000, 120, 1)?.execute()?;
        market.deposit(100, 0, 100, 1)?.execute()?;
        println!("{market:?}, {}", market.pool_value(&200, &1).unwrap());
        market.deposit(100, 0, 200, 1)?.execute()?;
        println!("{market:?}, {}", market.pool_value(&200, &1).unwrap());
        market.deposit(100, 0, 200, 1)?.execute()?;
        Ok(())
    }
}
