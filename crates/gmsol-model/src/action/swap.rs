use crate::{
    market::{BaseMarket, BaseMarketExt, BaseMarketMutExt, SwapMarketMutExt},
    num::{MulDiv, Unsigned},
    params::Fees,
    BalanceExt, PnlFactorKind, PoolExt, SwapMarketMut,
};

use num_traits::{CheckedAdd, CheckedMul, CheckedSub, Signed, Zero};

use super::Prices;

/// A swap.
#[must_use]
pub struct Swap<M: BaseMarket<DECIMALS>, const DECIMALS: u8> {
    market: M,
    params: SwapParams<M::Num>,
}

/// Swap params.
#[derive(Debug, Clone, Copy)]
pub struct SwapParams<T> {
    is_token_in_long: bool,
    token_in_amount: T,
    prices: Prices<T>,
}

impl<T> SwapParams<T> {
    /// Get long token price.
    pub fn long_token_price(&self) -> &T {
        &self.prices.long_token_price
    }

    /// Get short token price.
    pub fn short_token_price(&self) -> &T {
        &self.prices.short_token_price
    }

    /// Whether the in token is long token.
    pub fn is_token_in_long(&self) -> bool {
        self.is_token_in_long
    }

    /// Get the amount of in token.
    pub fn token_in_amount(&self) -> &T {
        &self.token_in_amount
    }
}

/// Report of the execution of swap.
#[must_use = "`token_out_amount` must use"]
#[derive(Debug, Clone, Copy)]
pub struct SwapReport<T: Unsigned> {
    params: SwapParams<T>,
    token_in_fees: Fees<T>,
    token_out_amount: T,
    price_impact_value: T::Signed,
    price_impact_amount: T,
}

impl<T: Unsigned> SwapReport<T> {
    /// Get swap params.
    pub fn params(&self) -> &SwapParams<T> {
        &self.params
    }

    /// Get token in fees.
    pub fn token_in_fees(&self) -> &Fees<T> {
        &self.token_in_fees
    }

    /// Get the amount of out token.
    pub fn token_out_amount(&self) -> &T {
        &self.token_out_amount
    }

    /// Get the price impact for the swap.
    pub fn price_impact(&self) -> &T::Signed {
        &self.price_impact_value
    }

    /// Get the price impact amount.
    pub fn price_impact_amount(&self) -> &T {
        &self.price_impact_amount
    }
}

struct ReassignedValues<T: Unsigned> {
    long_token_delta_value: T::Signed,
    short_token_delta_value: T::Signed,
    token_in_price: T,
    token_out_price: T,
}

impl<T: Unsigned> ReassignedValues<T> {
    fn new(
        long_token_delta_value: T::Signed,
        short_token_delta_value: T::Signed,
        token_in_price: T,
        token_out_price: T,
    ) -> Self {
        Self {
            long_token_delta_value,
            short_token_delta_value,
            token_in_price,
            token_out_price,
        }
    }
}

impl<const DECIMALS: u8, M: SwapMarketMut<DECIMALS>> Swap<M, DECIMALS> {
    /// Create a new swap in the given market.
    pub fn try_new(
        market: M,
        is_token_in_long: bool,
        token_in_amount: M::Num,
        prices: Prices<M::Num>,
    ) -> crate::Result<Self> {
        if token_in_amount.is_zero() {
            return Err(crate::Error::EmptySwap);
        }
        prices.validate()?;
        Ok(Self {
            market,
            params: SwapParams {
                is_token_in_long,
                token_in_amount,
                prices,
            },
        })
    }

    /// Assign the amounts of `token_in` and `token_out` to `long_token` and `short_token`, respectively,
    /// and assgin the prices of `long_token` and `short_token` to `token_in` and `token_out`.
    fn reassign_values(&self) -> crate::Result<ReassignedValues<M::Num>> {
        if self.params.is_token_in_long {
            let long_delta_value: M::Signed = self
                .params
                .token_in_amount
                .checked_mul(self.params.long_token_price())
                .ok_or(crate::Error::Computation("long delta value"))?
                .try_into()
                .map_err(|_| crate::Error::Convert)?;
            Ok(ReassignedValues::new(
                long_delta_value.clone(),
                -long_delta_value,
                self.params.long_token_price().clone(),
                self.params.short_token_price().clone(),
            ))
        } else {
            let short_delta_value: M::Signed = self
                .params
                .token_in_amount
                .checked_mul(self.params.short_token_price())
                .ok_or(crate::Error::Computation("short delta value"))?
                .try_into()
                .map_err(|_| crate::Error::Convert)?;
            Ok(ReassignedValues::new(
                -short_delta_value.clone(),
                short_delta_value,
                self.params.short_token_price().clone(),
                self.params.long_token_price().clone(),
            ))
        }
    }

    fn charge_fees(&mut self, is_positive_impact: bool) -> crate::Result<(M::Num, Fees<M::Num>)> {
        let (amount_after_fees, fees) = self
            .market
            .swap_fee_params()?
            .apply_fees(is_positive_impact, &self.params.token_in_amount)
            .ok_or(crate::Error::Computation("apply fees"))?;
        self.market.claimable_fee_pool_mut()?.apply_delta_amount(
            self.params.is_token_in_long,
            &fees
                .fee_receiver_amount()
                .clone()
                .try_into()
                .map_err(|_| crate::Error::Convert)?,
        )?;
        Ok((amount_after_fees, fees))
    }

    /// Execute the swap.
    pub fn execute(mut self) -> crate::Result<SwapReport<M::Num>> {
        let ReassignedValues {
            long_token_delta_value,
            short_token_delta_value,
            token_in_price,
            token_out_price,
        } = self.reassign_values()?;

        // Calculate price impact.
        let price_impact = self
            .market
            .liquidity_pool()?
            .pool_delta_with_values(
                long_token_delta_value,
                short_token_delta_value,
                self.params.long_token_price(),
                self.params.short_token_price(),
            )?
            .price_impact(&self.market.swap_impact_params()?)?;

        let (amount_after_fees, fees) = self.charge_fees(price_impact.is_positive())?;

        // Calculate final amounts && apply delta to price impact pool.
        let token_in_amount;
        let token_out_amount;
        let pool_amount_out;
        let price_impact_amount;
        if price_impact.is_positive() {
            price_impact_amount = self.market.apply_swap_impact_value_with_cap(
                !self.params.is_token_in_long,
                &token_out_price,
                &price_impact,
            )?;
            token_in_amount = amount_after_fees;
            pool_amount_out = token_in_amount
                .checked_mul_div(&token_in_price, &token_out_price)
                .ok_or(crate::Error::Computation(
                    "pool amount out for positive impact",
                ))?;
            // Extra amount is deducted from the swap impact pool.
            token_out_amount = pool_amount_out.checked_add(&price_impact_amount).ok_or(
                crate::Error::Computation("token out amount for positive impact"),
            )?;
        } else {
            price_impact_amount = self.market.apply_swap_impact_value_with_cap(
                self.params.is_token_in_long,
                &token_in_price,
                &price_impact,
            )?;
            token_in_amount = amount_after_fees.checked_sub(&price_impact_amount).ok_or(
                crate::Error::Computation("swap: not enough fund to pay price impact"),
            )?;
            token_out_amount = token_in_amount
                .checked_mul_div(&token_in_price, &token_out_price)
                .ok_or(crate::Error::Computation(
                    "token out amount for negative impact",
                ))?;
            pool_amount_out = token_out_amount.clone();
        }

        // Apply delta to primary pools.
        // `token_in_amount` is assumed to have been transferred in.
        self.market.apply_delta(
            self.params.is_token_in_long,
            &token_in_amount
                .checked_add(fees.fee_amount_for_pool())
                .ok_or(crate::Error::Overflow)?
                .try_into()
                .map_err(|_| crate::Error::Convert)?,
        )?;
        self.market.apply_delta(
            !self.params.is_token_in_long,
            &-pool_amount_out
                .try_into()
                .map_err(|_| crate::Error::Convert)?,
        )?;

        self.market
            .validate_pool_amount(self.params.is_token_in_long)?;
        self.market
            .validate_reserve(&self.params.prices, !self.params.is_token_in_long)?;

        let (long_kind, short_kind) = if self.params.is_token_in_long {
            (
                PnlFactorKind::MaxAfterDeposit,
                PnlFactorKind::MaxAfterWithdrawal,
            )
        } else {
            (
                PnlFactorKind::MaxAfterWithdrawal,
                PnlFactorKind::MaxAfterDeposit,
            )
        };
        self.market
            .validate_max_pnl(&self.params.prices, long_kind, short_kind)?;

        Ok(SwapReport {
            params: self.params,
            price_impact_value: price_impact,
            token_in_fees: fees,
            token_out_amount,
            price_impact_amount,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        action::Prices,
        market::{LiquidityMarketExt, SwapMarketMutExt},
        pool::Balance,
        test::TestMarket,
        BaseMarket, LiquidityMarket,
    };

    #[test]
    fn basic() -> crate::Result<()> {
        let mut market = TestMarket::<u64, 9>::default();
        let mut prices = Prices {
            index_token_price: 120,
            long_token_price: 120,
            short_token_price: 1,
        };
        market.deposit(1_000_000_000, 0, prices)?.execute()?;
        prices.index_token_price = 121;
        prices.long_token_price = 121;
        market.deposit(1_000_000_000, 0, prices)?.execute()?;
        prices.index_token_price = 122;
        prices.long_token_price = 122;
        market.deposit(0, 1_000_000_000, prices)?.execute()?;
        println!("{market:#?}");

        let prices = Prices {
            index_token_price: 123,
            long_token_price: 123,
            short_token_price: 1,
        };

        // Test for positive impact.
        let before_market = market.clone();
        let token_in_amount = 100_000_000;
        let report = market.swap(false, token_in_amount, prices)?.execute()?;
        println!("{report:#?}");
        println!("{market:#?}");

        assert_eq!(before_market.total_supply(), market.total_supply());

        assert_eq!(
            before_market.liquidity_pool()?.long_amount()?,
            market.liquidity_pool()?.long_amount()? + report.token_out_amount
                - report.price_impact_amount,
        );
        assert_eq!(
            before_market.liquidity_pool()?.short_amount()? + token_in_amount
                - report.token_in_fees.fee_receiver_amount(),
            market.liquidity_pool()?.short_amount()?,
        );

        assert_eq!(
            before_market.swap_impact_pool()?.long_amount()?,
            market.swap_impact_pool()?.long_amount()? + report.price_impact_amount,
        );
        assert_eq!(
            before_market.swap_impact_pool()?.short_amount()?,
            market.swap_impact_pool()?.short_amount()?
        );

        assert_eq!(
            before_market.claimable_fee_pool()?.long_amount()?,
            market.claimable_fee_pool()?.long_amount()?,
        );
        assert_eq!(
            before_market.claimable_fee_pool()?.short_amount()?
                + report.token_in_fees.fee_receiver_amount(),
            market.claimable_fee_pool()?.short_amount()?,
        );

        // Test for negative impact.
        let before_market = market.clone();
        let token_in_amount = 100_000;

        let prices = Prices {
            index_token_price: 119,
            long_token_price: 119,
            short_token_price: 1,
        };

        let report = market.swap(true, token_in_amount, prices)?.execute()?;
        println!("{report:#?}");
        println!("{market:#?}");

        assert_eq!(before_market.total_supply(), market.total_supply());

        assert_eq!(
            before_market.liquidity_pool()?.long_amount()? + token_in_amount
                - report.price_impact_amount
                - report.token_in_fees.fee_receiver_amount(),
            market.liquidity_pool()?.long_amount()?,
        );
        assert_eq!(
            before_market.liquidity_pool()?.short_amount()? - report.token_out_amount,
            market.liquidity_pool()?.short_amount()?,
        );

        assert_eq!(
            before_market.swap_impact_pool()?.long_amount()? + report.price_impact_amount,
            market.swap_impact_pool()?.long_amount()?,
        );
        assert_eq!(
            before_market.swap_impact_pool()?.short_amount()?,
            market.swap_impact_pool()?.short_amount()?
        );

        assert_eq!(
            before_market.claimable_fee_pool()?.long_amount()?
                + report.token_in_fees.fee_receiver_amount(),
            market.claimable_fee_pool()?.long_amount()?,
        );
        assert_eq!(
            before_market.claimable_fee_pool()?.short_amount()?,
            market.claimable_fee_pool()?.short_amount()?,
        );
        Ok(())
    }

    //test for zero swap
    #[test]
    fn zero_amount_swap() -> crate::Result<()> {
        let mut market = TestMarket::<u64, 9>::default();
        let prices = Prices {
            index_token_price: 120,
            long_token_price: 120,
            short_token_price: 1,
        };
        market.deposit(1_000_000_000, 0, prices)?.execute()?;
        market.deposit(0, 1_000_000_000, prices)?.execute()?;
        println!("{market:#?}");

        let result = market.swap(true, 0, prices)?.execute();
        assert!(result.is_err());

        println!("{market:#?}");
        Ok(())
    }
    
    //test for over amount
    #[test]
    fn over_amount_swap() -> crate::Result<()> {
        let mut market = TestMarket::<u64, 9>::default();
        let prices = Prices {
            index_token_price: 120,
            long_token_price: 120,
            short_token_price: 1,
        };
        market.deposit(1_000_000_000, 0, prices)?.execute()?;
        market.deposit(0, 1_000_000_000, prices)?.execute()?;
        println!("{market:#?}");

        let result = market.swap(true, 2_000_000_000, prices)?.execute();
        assert!(result.is_err());
        println!("{market:#?}");
        Ok(())
    }
    
    //test for small amount
    //error
    #[test]
    fn small_amount_swap() -> crate::Result<()> {
        let mut market = TestMarket::<u64, 9>::default();
        let prices = Prices {
            index_token_price: 120,
            long_token_price: 120,
            short_token_price: 1,
        };
        market.deposit(1_000_000_000, 0, prices)?.execute()?;
        println!("{market:#?}");
        
        let small_amount = 1;
    
        let report = market.swap(false, small_amount, prices)?.execute()?;
        println!("{report:#?}");
        println!("{market:#?}");

        assert_eq!(market.liquidity_pool()?.short_amount()?, 0);
        Ok(())
    }

}
