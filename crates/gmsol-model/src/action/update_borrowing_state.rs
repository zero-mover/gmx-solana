use num_traits::{CheckedAdd, Zero};

use crate::{
    market::{BaseMarket, BaseMarketExt, PerpMarketExt},
    num::Unsigned,
    Balance, PerpMarketMut, PoolExt,
};

use super::Prices;

/// Update Borrowing State Action.
#[must_use]
pub struct UpdateBorrowingState<M: BaseMarket<DECIMALS>, const DECIMALS: u8> {
    market: M,
    prices: Prices<M::Num>,
}

impl<M: PerpMarketMut<DECIMALS>, const DECIMALS: u8> UpdateBorrowingState<M, DECIMALS> {
    /// Create a new [`UpdateBorrowingState`] action.
    pub fn try_new(market: M, prices: &Prices<M::Num>) -> crate::Result<Self> {
        prices.validate()?;
        Ok(Self {
            market,
            prices: prices.clone(),
        })
    }

    /// Get borrowing factor per second.
    fn calc_borrowing_factor_per_second(
        &self,
        is_long: bool,
        prices: &Prices<M::Num>,
    ) -> crate::Result<M::Num> {
        use crate::utils;

        let reserved_value = self
            .market
            .reserved_value(&prices.index_token_price, is_long)?;

        if reserved_value.is_zero() {
            return Ok(Zero::zero());
        }

        let params = self.market.borrowing_fee_params()?;

        if params.skip_borrowing_fee_for_smaller_side() {
            let open_interest = self.market.open_interest()?;
            let long_interest = open_interest.long_amount()?;
            let short_interest = open_interest.short_amount()?;
            if (is_long && long_interest < short_interest)
                || (!is_long && short_interest < long_interest)
            {
                return Ok(Zero::zero());
            }
        }

        let pool_value = self
            .market
            .pool_value(&prices.long_token_price, &prices.short_token_price)?;

        if pool_value.is_zero() {
            return Err(crate::Error::UnableToGetBorrowingFactorEmptyPoolValue);
        }

        // TODO: apply optimal usage factor.

        let reserved_value_after_exponent =
            utils::apply_exponent_factor(reserved_value, params.exponent(is_long).clone()).ok_or(
                crate::Error::Computation("calculating reserved value after exponent"),
            )?;
        let reversed_value_to_pool_factor =
            utils::div_to_factor(&reserved_value_after_exponent, &pool_value, false).ok_or(
                crate::Error::Computation("calculating reserved value to pool factor"),
            )?;
        utils::apply_factor(&reversed_value_to_pool_factor, params.factor(is_long)).ok_or(
            crate::Error::Computation("calculating borrowing factort per second"),
        )
    }

    /// Get next cumulative borrowing factor of the given side.
    fn calc_next_cumulative_borrowing_factor(
        &self,
        is_long: bool,
        prices: &Prices<M::Num>,
        duration_in_second: u64,
    ) -> crate::Result<(M::Num, M::Num)> {
        use num_traits::{CheckedMul, FromPrimitive};

        let borrowing_factor_per_second = self.calc_borrowing_factor_per_second(is_long, prices)?;
        let current_factor = self.market.borrowing_factor(is_long)?;

        let duration_value = M::Num::from_u64(duration_in_second).ok_or(crate::Error::Convert)?;
        let delta = borrowing_factor_per_second
            .checked_mul(&duration_value)
            .ok_or(crate::Error::Computation(
                "calculating borrowing factor delta",
            ))?;
        let next_cumulative_borrowing_factor =
            current_factor
                .checked_add(&delta)
                .ok_or(crate::Error::Computation(
                    "calculating next borrowing factor",
                ))?;
        Ok((next_cumulative_borrowing_factor, delta))
    }

    fn execute_one_side(
        &mut self,
        is_long: bool,
        duration_in_seconds: u64,
    ) -> crate::Result<M::Num> {
        let (next_cumulative_borrowing_factor, delta) =
            self.calc_next_cumulative_borrowing_factor(is_long, &self.prices, duration_in_seconds)?;
        self.market
            .borrowing_factor_pool_mut()?
            .apply_delta_amount(is_long, &delta.to_signed()?)?;
        Ok(next_cumulative_borrowing_factor)
    }

    /// Execute.
    pub fn execute(mut self) -> crate::Result<UpdateBorrowingReport<M::Num>> {
        let duration_in_seconds = self.market.just_passed_in_seconds_for_borrowing()?;
        let next_cumulative_borrowing_factor_for_long =
            self.execute_one_side(true, duration_in_seconds)?;
        let next_cumulative_borrowing_factor_for_short =
            self.execute_one_side(false, duration_in_seconds)?;
        Ok(UpdateBorrowingReport {
            duration_in_seconds,
            next_cumulative_borrowing_factor_for_long,
            next_cumulative_borrowing_factor_for_short,
        })
    }
}

/// Update Borrowing Report.
#[derive(Debug)]
pub struct UpdateBorrowingReport<T> {
    duration_in_seconds: u64,
    next_cumulative_borrowing_factor_for_long: T,
    next_cumulative_borrowing_factor_for_short: T,
}

impl<T> UpdateBorrowingReport<T> {
    /// Get considered duration in seconds.
    pub fn duration_in_seconds(&self) -> u64 {
        self.duration_in_seconds
    }

    /// Get next cumulative borrowing factor
    pub fn next_cumulative_borrowing_factor(&self, is_long: bool) -> &T {
        if is_long {
            &self.next_cumulative_borrowing_factor_for_long
        } else {
            &self.next_cumulative_borrowing_factor_for_short
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{thread::sleep, time::Duration};

    use crate::{
        market::LiquidityMarketExt,
        test::{TestMarket, TestPosition},
        PositionMutExt,
    };

    use super::*;

    #[test]
    fn test_update_borrowing_state() -> crate::Result<()> {
        let mut market = TestMarket::<u64, 9>::default();
        let prices = Prices {
            index_token_price: 120,
            long_token_price: 120,
            short_token_price: 1,
        };
        market
            .deposit(1_000_000_000_000, 100_000_000_000_000, prices)?
            .execute()?;
        println!("{market:#?}");
        let mut position = TestPosition::long(true);
        let prices = Prices {
            index_token_price: 123,
            long_token_price: 123,
            short_token_price: 1,
        };
        let report = position
            .ops(&mut market)
            .increase(prices, 1_000_000_000_000, 50_000_000_000_000, None)?
            .execute()?;
        println!("{report:#?}");
        println!("{market:#?}");
        sleep(Duration::from_secs(2));
        let report = position
            .ops(&mut market)
            .decrease(prices, 50_000_000_000_000, None, 0, false, false)?
            .execute()?;
        println!("{report:#?}");
        println!("{market:#?}");
        Ok(())
    }
}
