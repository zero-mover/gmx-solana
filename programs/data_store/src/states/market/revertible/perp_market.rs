use anchor_lang::prelude::*;
use gmx_core::{
    params::{
        fee::{BorrowingFeeParams, FundingFeeParams},
        position::PositionImpactDistributionParams,
        FeeParams, PositionParams, PriceImpactParams,
    },
    ClockKind, PoolKind,
};

use crate::{
    constants,
    states::{clock::AsClock, HasMarketMeta, Market},
};

use super::{Revertible, RevertibleMarket, RevertiblePool};

/// Convert a [`RevetibleMarket`] to a [`PerpMarket`](gmx_core::PerpMarket).
pub struct RevertiblePerpMarket<'a> {
    market: RevertibleMarket<'a>,
    clocks: Clocks,
    pools: Pools,
    state: State,
}

struct Clocks {
    position_impact_distribution_clock: i64,
    borrowing_clock: i64,
    funding_clock: i64,
}

impl<'a, 'market> TryFrom<&'a RevertibleMarket<'market>> for Clocks {
    type Error = Error;

    fn try_from(market: &'a RevertibleMarket<'market>) -> Result<Self> {
        Ok(Self {
            position_impact_distribution_clock: market
                .get_clock(ClockKind::PriceImpactDistribution)?,
            borrowing_clock: market.get_clock(ClockKind::Borrowing)?,
            funding_clock: market.get_clock(ClockKind::Funding)?,
        })
    }
}

impl Clocks {
    fn write_to_market(&self, market: &mut Market) {
        *market
            .clocks
            .get_mut(ClockKind::PriceImpactDistribution)
            .expect("must exist") = self.position_impact_distribution_clock;
        *market
            .clocks
            .get_mut(ClockKind::Borrowing)
            .expect("must exist") = self.borrowing_clock;
        *market
            .clocks
            .get_mut(ClockKind::Funding)
            .expect("must exist") = self.funding_clock;
    }
}

struct Pools {
    swap_impact: RevertiblePool,
    position_impact: RevertiblePool,
    open_interest: (RevertiblePool, RevertiblePool),
    open_interest_in_tokens: (RevertiblePool, RevertiblePool),
    borrowing_factor: RevertiblePool,
    funding_amount_per_size: (RevertiblePool, RevertiblePool),
    claimable_funding_amount_per_size: (RevertiblePool, RevertiblePool),
}

impl<'a, 'market> TryFrom<&'a RevertibleMarket<'market>> for Pools {
    type Error = Error;

    fn try_from(market: &'a RevertibleMarket<'market>) -> Result<Self> {
        Ok(Self {
            swap_impact: market.create_revertible_pool(PoolKind::SwapImpact)?,
            position_impact: market.create_revertible_pool(PoolKind::PositionImpact)?,
            open_interest: (
                market.create_revertible_pool(PoolKind::OpenInterestForLong)?,
                market.create_revertible_pool(PoolKind::OpenInterestForShort)?,
            ),
            open_interest_in_tokens: (
                market.create_revertible_pool(PoolKind::OpenInterestInTokensForLong)?,
                market.create_revertible_pool(PoolKind::OpenInterestInTokensForShort)?,
            ),
            borrowing_factor: market.create_revertible_pool(PoolKind::BorrowingFactor)?,
            funding_amount_per_size: (
                market.create_revertible_pool(PoolKind::FundingAmountPerSizeForLong)?,
                market.create_revertible_pool(PoolKind::FundingAmountPerSizeForShort)?,
            ),
            claimable_funding_amount_per_size: (
                market.create_revertible_pool(PoolKind::ClaimableFundingAmountPerSizeForLong)?,
                market.create_revertible_pool(PoolKind::ClaimableFundingAmountPerSizeForShort)?,
            ),
        })
    }
}

impl Pools {
    fn write_to_market(&self, market: &mut Market) {
        self.swap_impact.as_small_pool().write_to_pool(
            market
                .pools
                .get_mut(PoolKind::SwapImpact)
                .expect("must exist"),
        );

        self.position_impact.as_small_pool().write_to_pool(
            market
                .pools
                .get_mut(PoolKind::PositionImpact)
                .expect("must exist"),
        );

        self.open_interest.0.as_small_pool().write_to_pool(
            market
                .pools
                .get_mut(PoolKind::OpenInterestForLong)
                .expect("must exist"),
        );
        self.open_interest.1.as_small_pool().write_to_pool(
            market
                .pools
                .get_mut(PoolKind::OpenInterestForShort)
                .expect("must exist"),
        );

        self.open_interest_in_tokens
            .0
            .as_small_pool()
            .write_to_pool(
                market
                    .pools
                    .get_mut(PoolKind::OpenInterestInTokensForLong)
                    .expect("must exist"),
            );
        self.open_interest_in_tokens
            .1
            .as_small_pool()
            .write_to_pool(
                market
                    .pools
                    .get_mut(PoolKind::OpenInterestInTokensForShort)
                    .expect("must exist"),
            );

        self.borrowing_factor.as_small_pool().write_to_pool(
            market
                .pools
                .get_mut(PoolKind::BorrowingFactor)
                .expect("must exist"),
        );

        self.funding_amount_per_size
            .0
            .as_small_pool()
            .write_to_pool(
                market
                    .pools
                    .get_mut(PoolKind::FundingAmountPerSizeForLong)
                    .expect("must exist"),
            );
        self.funding_amount_per_size
            .1
            .as_small_pool()
            .write_to_pool(
                market
                    .pools
                    .get_mut(PoolKind::FundingAmountPerSizeForShort)
                    .expect("must exist"),
            );

        self.claimable_funding_amount_per_size
            .0
            .as_small_pool()
            .write_to_pool(
                market
                    .pools
                    .get_mut(PoolKind::ClaimableFundingAmountPerSizeForLong)
                    .expect("must exist"),
            );
        self.claimable_funding_amount_per_size
            .1
            .as_small_pool()
            .write_to_pool(
                market
                    .pools
                    .get_mut(PoolKind::ClaimableFundingAmountPerSizeForShort)
                    .expect("must exist"),
            );
    }
}

struct State {
    funding_factor_per_second: i128,
}

impl<'a, 'market> From<&'a RevertibleMarket<'market>> for State {
    fn from(market: &'a RevertibleMarket<'market>) -> Self {
        Self {
            funding_factor_per_second: market.state().funding_factor_per_second,
        }
    }
}

impl State {
    fn write_to_market(&self, market: &mut Market) {
        market.state.funding_factor_per_second = self.funding_factor_per_second;
    }
}

impl<'a> Key for RevertiblePerpMarket<'a> {
    fn key(&self) -> anchor_lang::prelude::Pubkey {
        self.market.key()
    }
}

impl<'a> HasMarketMeta for RevertiblePerpMarket<'a> {
    fn is_pure(&self) -> bool {
        self.market.is_pure()
    }

    fn market_meta(&self) -> &crate::states::MarketMeta {
        self.market.market_meta()
    }
}

impl<'a> gmx_core::Bank<Pubkey> for RevertiblePerpMarket<'a> {
    type Num = u64;

    fn record_transferred_in_by_token<Q: std::borrow::Borrow<Pubkey> + ?Sized>(
        &mut self,
        token: &Q,
        amount: &Self::Num,
    ) -> gmx_core::Result<()> {
        self.market.record_transferred_in_by_token(token, amount)
    }

    fn record_transferred_out_by_token<Q: std::borrow::Borrow<Pubkey> + ?Sized>(
        &mut self,
        token: &Q,
        amount: &Self::Num,
    ) -> gmx_core::Result<()> {
        self.market.record_transferred_out_by_token(token, amount)
    }

    fn balance<Q: std::borrow::Borrow<Pubkey> + ?Sized>(
        &self,
        token: &Q,
    ) -> gmx_core::Result<Self::Num> {
        self.market.balance(token)
    }
}

impl<'a> RevertiblePerpMarket<'a> {
    pub(crate) fn new<'info>(loader: &'a AccountLoader<'info, Market>) -> Result<Self> {
        let market = loader.try_into()?;
        Self::from_market(market)
    }

    pub(crate) fn from_market(market: RevertibleMarket<'a>) -> Result<Self> {
        Ok(Self {
            clocks: (&market).try_into()?,
            pools: (&market).try_into()?,
            state: (&market).into(),
            market,
        })
    }
}

impl<'a> Revertible for RevertiblePerpMarket<'a> {
    fn commit(self) {
        self.market.commit_with(|market| {
            self.clocks.write_to_market(market);
            self.pools.write_to_market(market);
            self.state.write_to_market(market);
        });
    }
}

impl<'a> gmx_core::BaseMarket<{ constants::MARKET_DECIMALS }> for RevertiblePerpMarket<'a> {
    type Num = u128;

    type Signed = i128;

    type Pool = RevertiblePool;

    fn liquidity_pool(&self) -> gmx_core::Result<&Self::Pool> {
        self.market.liquidity_pool()
    }

    fn liquidity_pool_mut(&mut self) -> gmx_core::Result<&mut Self::Pool> {
        self.market.liquidity_pool_mut()
    }

    fn claimable_fee_pool(&self) -> gmx_core::Result<&Self::Pool> {
        self.market.claimable_fee_pool()
    }

    fn claimable_fee_pool_mut(&mut self) -> gmx_core::Result<&mut Self::Pool> {
        self.market.claimable_fee_pool_mut()
    }

    fn swap_impact_pool(&self) -> gmx_core::Result<&Self::Pool> {
        Ok(&self.pools.swap_impact)
    }

    fn open_interest_pool(&self, is_long: bool) -> gmx_core::Result<&Self::Pool> {
        if is_long {
            Ok(&self.pools.open_interest.0)
        } else {
            Ok(&self.pools.open_interest.1)
        }
    }

    fn open_interest_in_tokens_pool(&self, is_long: bool) -> gmx_core::Result<&Self::Pool> {
        if is_long {
            Ok(&self.pools.open_interest_in_tokens.0)
        } else {
            Ok(&self.pools.open_interest_in_tokens.1)
        }
    }

    fn usd_to_amount_divisor(&self) -> Self::Num {
        self.market.usd_to_amount_divisor()
    }

    fn max_pool_amount(&self, is_long_token: bool) -> gmx_core::Result<Self::Num> {
        self.market.max_pool_amount(is_long_token)
    }

    fn max_pnl_factor(
        &self,
        kind: gmx_core::PnlFactorKind,
        is_long: bool,
    ) -> gmx_core::Result<Self::Num> {
        self.market.max_pnl_factor(kind, is_long)
    }

    fn reserve_factor(&self) -> gmx_core::Result<Self::Num> {
        self.market.reserve_factor()
    }
}

impl<'a> gmx_core::SwapMarket<{ constants::MARKET_DECIMALS }> for RevertiblePerpMarket<'a> {
    fn swap_impact_params(
        &self,
    ) -> gmx_core::Result<gmx_core::params::PriceImpactParams<Self::Num>> {
        self.market.swap_impact_params()
    }

    fn swap_fee_params(&self) -> gmx_core::Result<gmx_core::params::FeeParams<Self::Num>> {
        self.market.swap_fee_params()
    }

    fn swap_impact_pool_mut(&mut self) -> gmx_core::Result<&mut Self::Pool> {
        Ok(&mut self.pools.swap_impact)
    }
}

impl<'a> gmx_core::PositionImpactMarket<{ constants::MARKET_DECIMALS }>
    for RevertiblePerpMarket<'a>
{
    fn position_impact_pool(&self) -> gmx_core::Result<&Self::Pool> {
        Ok(&self.pools.position_impact)
    }

    fn position_impact_pool_mut(&mut self) -> gmx_core::Result<&mut Self::Pool> {
        Ok(&mut self.pools.position_impact)
    }

    fn just_passed_in_seconds_for_position_impact_distribution(&mut self) -> gmx_core::Result<u64> {
        AsClock::from(&mut self.clocks.position_impact_distribution_clock).just_passed_in_seconds()
    }

    fn position_impact_params(&self) -> gmx_core::Result<PriceImpactParams<Self::Num>> {
        self.market.position_impact_params()
    }

    fn position_impact_distribution_params(
        &self,
    ) -> gmx_core::Result<PositionImpactDistributionParams<Self::Num>> {
        self.market.position_impact_distribution_params()
    }
}

impl<'a> gmx_core::PerpMarket<{ constants::MARKET_DECIMALS }> for RevertiblePerpMarket<'a> {
    fn just_passed_in_seconds_for_borrowing(&mut self) -> gmx_core::Result<u64> {
        AsClock::from(&mut self.clocks.borrowing_clock).just_passed_in_seconds()
    }

    fn just_passed_in_seconds_for_funding(&mut self) -> gmx_core::Result<u64> {
        AsClock::from(&mut self.clocks.funding_clock).just_passed_in_seconds()
    }

    fn funding_factor_per_second(&self) -> &Self::Signed {
        &self.state.funding_factor_per_second
    }

    fn funding_factor_per_second_mut(&mut self) -> &mut Self::Signed {
        &mut self.state.funding_factor_per_second
    }

    fn open_interest_pool_mut(&mut self, is_long: bool) -> gmx_core::Result<&mut Self::Pool> {
        if is_long {
            Ok(&mut self.pools.open_interest.0)
        } else {
            Ok(&mut self.pools.open_interest.1)
        }
    }

    fn open_interest_in_tokens_pool_mut(
        &mut self,
        is_long: bool,
    ) -> gmx_core::Result<&mut Self::Pool> {
        if is_long {
            Ok(&mut self.pools.open_interest_in_tokens.0)
        } else {
            Ok(&mut self.pools.open_interest_in_tokens.1)
        }
    }

    fn borrowing_factor_pool(&self) -> gmx_core::Result<&Self::Pool> {
        Ok(&self.pools.borrowing_factor)
    }

    fn borrowing_factor_pool_mut(&mut self) -> gmx_core::Result<&mut Self::Pool> {
        Ok(&mut self.pools.borrowing_factor)
    }

    fn funding_amount_per_size_pool(&self, is_long: bool) -> gmx_core::Result<&Self::Pool> {
        if is_long {
            Ok(&self.pools.funding_amount_per_size.0)
        } else {
            Ok(&self.pools.funding_amount_per_size.1)
        }
    }

    fn funding_amount_per_size_pool_mut(
        &mut self,
        is_long: bool,
    ) -> gmx_core::Result<&mut Self::Pool> {
        if is_long {
            Ok(&mut self.pools.funding_amount_per_size.0)
        } else {
            Ok(&mut self.pools.funding_amount_per_size.1)
        }
    }

    fn claimable_funding_amount_per_size_pool(
        &self,
        is_long: bool,
    ) -> gmx_core::Result<&Self::Pool> {
        if is_long {
            Ok(&self.pools.claimable_funding_amount_per_size.0)
        } else {
            Ok(&self.pools.claimable_funding_amount_per_size.1)
        }
    }

    fn claimable_funding_amount_per_size_pool_mut(
        &mut self,
        is_long: bool,
    ) -> gmx_core::Result<&mut Self::Pool> {
        if is_long {
            Ok(&mut self.pools.claimable_funding_amount_per_size.0)
        } else {
            Ok(&mut self.pools.claimable_funding_amount_per_size.1)
        }
    }

    fn borrowing_fee_params(&self) -> gmx_core::Result<BorrowingFeeParams<Self::Num>> {
        self.market.borrowing_fee_params()
    }

    fn funding_amount_per_size_adjustment(&self) -> Self::Num {
        self.market.funding_amount_per_size_adjustment()
    }

    fn funding_fee_params(&self) -> gmx_core::Result<FundingFeeParams<Self::Num>> {
        self.market.funding_fee_params()
    }

    fn position_params(&self) -> gmx_core::Result<PositionParams<Self::Num>> {
        self.market.position_params()
    }

    fn order_fee_params(&self) -> gmx_core::Result<FeeParams<Self::Num>> {
        self.market.order_fee_params()
    }

    fn open_interest_reserve_factor(&self) -> gmx_core::Result<Self::Num> {
        Ok(self.market.config().open_interest_reserve_factor)
    }

    fn max_open_interest(&self, is_long: bool) -> gmx_core::Result<Self::Num> {
        if is_long {
            Ok(self.market.config().max_open_interest_for_long)
        } else {
            Ok(self.market.config().max_open_interest_for_short)
        }
    }
}
