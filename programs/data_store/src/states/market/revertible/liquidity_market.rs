use anchor_lang::prelude::*;
use anchor_spl::token::Mint;
use gmx_core::{
    params::{position::PositionImpactDistributionParams, PriceImpactParams},
    ClockKind, PoolKind,
};

use crate::{
    constants,
    states::{clock::AsClock, Store},
    utils::internal::TransferUtils,
};

use super::{swap_market::RevertibleSwapMarket, Revertible, RevertibleMarket, RevertiblePool};

/// Convert a [`RevertibleMarket`] to a [`LiquidityMarket`](gmx_core::LiquidityMarket).
pub struct AsLiquidityMarket<'a, 'info> {
    market: RevertibleSwapMarket<'a>,
    market_token: &'a mut Account<'info, Mint>,
    transfer: TransferUtils<'a, 'info>,
    receiver: Option<AccountInfo<'info>>,
    vault: AccountInfo<'info>,
    position_impact: RevertiblePool,
    position_impact_distribution_clock: i64,
    to_mint: u64,
    to_burn: u64,
}

impl<'a, 'info> AsLiquidityMarket<'a, 'info> {
    pub(crate) fn new(
        market: RevertibleMarket<'a>,
        market_token: &'a mut Account<'info, Mint>,
        vault: AccountInfo<'info>,
        token_program: AccountInfo<'info>,
        store: &'a AccountLoader<'info, Store>,
    ) -> Result<Self> {
        let position_impact = market.create_revertible_pool(PoolKind::PositionImpact)?;
        let position_impact_distribution_clock =
            market.get_clock(ClockKind::PriceImpactDistribution)?;
        Ok(Self {
            market: RevertibleSwapMarket::new(market)?,
            transfer: TransferUtils::new(
                token_program,
                store,
                Some(market_token.to_account_info()),
            ),
            market_token,
            receiver: None,
            vault,
            position_impact,
            position_impact_distribution_clock,
            to_mint: 0,
            to_burn: 0,
        })
    }

    pub(crate) fn enable_mint(mut self, receiver: AccountInfo<'info>) -> Self {
        self.receiver = Some(receiver);
        self
    }
}

impl<'a, 'info> gmx_core::BaseMarket<{ constants::MARKET_DECIMALS }>
    for AsLiquidityMarket<'a, 'info>
{
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
        self.market.swap_impact_pool()
    }

    fn open_interest_pool(&self, is_long: bool) -> gmx_core::Result<&Self::Pool> {
        self.market.open_interest_pool(is_long)
    }

    fn open_interest_in_tokens_pool(&self, is_long: bool) -> gmx_core::Result<&Self::Pool> {
        self.market.open_interest_in_tokens_pool(is_long)
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

impl<'a, 'info> gmx_core::SwapMarket<{ constants::MARKET_DECIMALS }>
    for AsLiquidityMarket<'a, 'info>
{
    fn swap_impact_params(
        &self,
    ) -> gmx_core::Result<gmx_core::params::PriceImpactParams<Self::Num>> {
        self.market.swap_impact_params()
    }

    fn swap_fee_params(&self) -> gmx_core::Result<gmx_core::params::FeeParams<Self::Num>> {
        self.market.swap_fee_params()
    }

    fn swap_impact_pool_mut(&mut self) -> gmx_core::Result<&mut Self::Pool> {
        self.market.swap_impact_pool_mut()
    }
}

impl<'a, 'info> gmx_core::LiquidityMarket<{ constants::MARKET_DECIMALS }>
    for AsLiquidityMarket<'a, 'info>
{
    fn total_supply(&self) -> Self::Num {
        self.market_token.supply.into()
    }

    fn max_pool_value_for_deposit(&self, is_long_token: bool) -> gmx_core::Result<Self::Num> {
        if is_long_token {
            Ok(self
                .market
                .market
                .config()
                .max_pool_value_for_deposit_for_long_token)
        } else {
            Ok(self
                .market
                .market
                .config()
                .max_pool_value_for_deposit_for_short_token)
        }
    }

    fn mint(&mut self, amount: &Self::Num) -> gmx_core::Result<()> {
        let new_mint: u64 = (*amount)
            .try_into()
            .map_err(|_| gmx_core::Error::Overflow)?;
        let to_mint = self
            .to_mint
            .checked_add(new_mint)
            .ok_or(gmx_core::Error::Overflow)?;
        // CHECK for overflow.
        self.market_token
            .supply
            .checked_add(to_mint)
            .ok_or(gmx_core::Error::Overflow)?;
        self.to_mint = to_mint;
        Ok(())
    }

    fn burn(&mut self, amount: &Self::Num) -> gmx_core::Result<()> {
        let new_burn: u64 = (*amount)
            .try_into()
            .map_err(|_| gmx_core::Error::Overflow)?;
        let to_burn = self
            .to_burn
            .checked_add(new_burn)
            .ok_or(gmx_core::Error::Overflow)?;
        // CHECK for underflow.
        self.market_token
            .supply
            .checked_sub(to_burn)
            .ok_or(gmx_core::Error::Underflow)?;
        self.to_burn = to_burn;
        Ok(())
    }
}

impl<'a, 'info> gmx_core::PositionImpactMarket<{ constants::MARKET_DECIMALS }>
    for AsLiquidityMarket<'a, 'info>
{
    fn position_impact_pool(&self) -> gmx_core::Result<&Self::Pool> {
        Ok(&self.position_impact)
    }

    fn position_impact_pool_mut(&mut self) -> gmx_core::Result<&mut Self::Pool> {
        Ok(&mut self.position_impact)
    }

    fn just_passed_in_seconds_for_position_impact_distribution(&mut self) -> gmx_core::Result<u64> {
        AsClock::from(&mut self.position_impact_distribution_clock).just_passed_in_seconds()
    }

    fn position_impact_params(&self) -> gmx_core::Result<PriceImpactParams<Self::Num>> {
        let config = self.market.market.config();
        PriceImpactParams::builder()
            .with_exponent(config.position_impact_exponent)
            .with_positive_factor(config.position_impact_positive_factor)
            .with_negative_factor(config.position_impact_negative_factor)
            .build()
    }

    fn position_impact_distribution_params(
        &self,
    ) -> gmx_core::Result<PositionImpactDistributionParams<Self::Num>> {
        let config = self.market.market.config();
        Ok(PositionImpactDistributionParams::builder()
            .distribute_factor(config.position_impact_distribute_factor)
            .min_position_impact_pool_amount(config.min_position_impact_pool_amount)
            .build())
    }
}

impl<'a, 'info> Revertible for AsLiquidityMarket<'a, 'info> {
    fn commit(self) {
        if self.to_mint != 0 {
            self.transfer
                .mint_to(&self.receiver.expect("mint is not enabled"), self.to_mint)
                .map_err(|err| panic!("mint error: {err}"))
                .unwrap();
        }
        if self.to_burn != 0 {
            self.transfer
                .burn_from(&self.vault, self.to_burn)
                .map_err(|err| panic!("burn error: {err}"))
                .unwrap();
        }
        self.market.commit_with(|market| {
            let position_impact = market
                .pools
                .get_mut(PoolKind::PositionImpact)
                .expect("must exist");
            self.position_impact
                .as_small_pool()
                .write_to_pool(position_impact);
            *market
                .clocks
                .get_mut(ClockKind::PriceImpactDistribution)
                .expect("must exist") = self.position_impact_distribution_clock;
        });
    }
}
