use anchor_lang::{prelude::*, Bump};
use anchor_spl::token::Mint;
use dual_vec_map::DualVecMap;
use gmx_core::{
    params::{
        fee::{BorrowingFeeParams, FundingFeeParams},
        position::PositionImpactDistributionParams,
        FeeParams, PositionParams, PriceImpactParams,
    },
    ClockKind, PoolKind,
};
use gmx_solana_utils::to_seed;

use crate::{constants, utils::internal::TransferUtils, DataStoreError};

use super::{
    common::map::{pools::Pools, DynamicMapStore},
    position::{Position, PositionOps},
    Data, DataStore, InitSpace, Seed,
};

/// Market.
#[account]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Market {
    /// Bump Seed.
    pub(crate) bump: u8,
    pub(crate) meta: MarketMeta,
    pools: Pools,
    clocks: DynamicMapStore<u8, i64>,
    funding_factor_per_second: i128,
}

impl Market {
    pub(crate) fn init_space(num_pools: u8, num_clocks: u8) -> usize {
        1 + 16
            + MarketMeta::INIT_SPACE
            + DynamicMapStore::<u8, Pool>::init_space(num_pools)
            + DynamicMapStore::<u8, i64>::init_space(num_clocks)
    }

    /// Get meta.
    pub fn meta(&self) -> &MarketMeta {
        &self.meta
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct MarketMeta {
    /// Market token.
    pub market_token_mint: Pubkey,
    /// Index token.
    pub index_token_mint: Pubkey,
    /// Long token.
    pub long_token_mint: Pubkey,
    /// Short token.
    pub short_token_mint: Pubkey,
}

impl MarketMeta {
    /// Check if the given token is a valid collateral token.
    #[inline]
    pub fn is_collateral_token(&self, token: &Pubkey) -> bool {
        *token == self.long_token_mint || *token == self.short_token_mint
    }

    /// Get pnl token.
    pub fn pnl_token(&self, is_long: bool) -> Pubkey {
        if is_long {
            self.long_token_mint
        } else {
            self.short_token_mint
        }
    }

    /// Check if the given token is a valid collateral token,
    /// return error if it is not.
    pub fn validate_collateral_token(&self, token: &Pubkey) -> Result<()> {
        if self.is_collateral_token(token) {
            Ok(())
        } else {
            Err(DataStoreError::InvalidCollateralToken.into())
        }
    }
}

const NOT_PURE_POOLS: [u8; 1] = [PoolKind::BorrowingFactor as u8];

impl Market {
    /// Initialize the market.
    #[allow(clippy::too_many_arguments)]
    pub fn init(
        &mut self,
        bump: u8,
        market_token_mint: Pubkey,
        index_token_mint: Pubkey,
        long_token_mint: Pubkey,
        short_token_mint: Pubkey,
        num_pools: u8,
        num_clocks: u8,
    ) -> Result<()> {
        self.bump = bump;
        self.meta.market_token_mint = market_token_mint;
        self.meta.index_token_mint = index_token_mint;
        self.meta.long_token_mint = long_token_mint;
        self.meta.short_token_mint = short_token_mint;
        let is_pure = self.meta.long_token_mint == self.meta.short_token_mint;
        self.pools.init_with(num_pools, |kind| {
            Pool::default().with_is_pure(is_pure && !(NOT_PURE_POOLS.contains(&kind)))
        });
        let current = Clock::get()?.unix_timestamp;
        self.clocks.init_with(num_clocks, |_| current);
        self.funding_factor_per_second = 0;
        Ok(())
    }

    /// Get pool of the given kind.
    #[inline]
    pub fn pool(&self, kind: PoolKind) -> Option<Pool> {
        self.pools.get_with(kind, |pool| pool.cloned())
    }

    /// Get mutable reference to the pool of the given kind.
    #[inline]
    pub(crate) fn with_pool_mut<T>(
        &mut self,
        kind: PoolKind,
        f: impl FnOnce(&mut Pool) -> T,
    ) -> Option<T> {
        self.pools.get_mut_with(kind, |pool| pool.map(f))
    }

    /// Get the expected key.
    pub fn expected_key(&self) -> String {
        Self::create_key(&self.meta.market_token_mint)
    }

    /// Get the expected key seed.
    pub fn expected_key_seed(&self) -> [u8; 32] {
        to_seed(&self.expected_key())
    }

    /// Create key from tokens.
    pub fn create_key(market_token: &Pubkey) -> String {
        market_token.to_string()
    }

    /// Create key seed from tokens.
    pub fn create_key_seed(market_token: &Pubkey) -> [u8; 32] {
        let key = Self::create_key(market_token);
        to_seed(&key)
    }

    pub(crate) fn as_market<'a, 'info>(
        &'a mut self,
        mint: &'a mut Account<'info, Mint>,
    ) -> AsMarket<'a, 'info> {
        AsMarket {
            meta: &self.meta,
            pools: self.pools.as_map_mut(),
            clocks: self.clocks.as_map_mut(),
            mint,
            transfer: None,
            receiver: None,
            vault: None,
            funding_factor_per_second: &mut self.funding_factor_per_second,
        }
    }
}

impl Bump for Market {
    fn seed(&self) -> u8 {
        self.bump
    }
}

impl Seed for Market {
    const SEED: &'static [u8] = b"market";
}

impl Data for Market {
    fn verify(&self, key: &str) -> Result<()> {
        // FIXME: is there a better way to verify the key?
        let expected = self.expected_key();
        require_eq!(key, &expected, crate::DataStoreError::InvalidKey);
        Ok(())
    }
}

/// A pool for market.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace, Default)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Pool {
    /// Whether the pool only contains one kind of token,
    /// i.e. a pure pool.
    /// For a pure pool, only the `long_token_amount` field is used.
    pub is_pure: bool,
    /// Long token amount.
    long_token_amount: u128,
    /// Short token amount.
    short_token_amount: u128,
}

impl InitSpace for Pool {
    const INIT_SPACE: usize = <Self as Space>::INIT_SPACE;
}

impl Pool {
    /// Set the pure flag.
    fn with_is_pure(mut self, is_pure: bool) -> Self {
        self.is_pure = is_pure;
        self
    }
}

impl gmx_core::Balance for Pool {
    type Num = u128;

    type Signed = i128;

    /// Get the long token amount.
    fn long_amount(&self) -> gmx_core::Result<Self::Num> {
        if self.is_pure {
            debug_assert_eq!(
                self.short_token_amount, 0,
                "short token amount must be zero"
            );
            Ok(self.long_token_amount / 2)
        } else {
            Ok(self.long_token_amount)
        }
    }

    /// Get the short token amount.
    fn short_amount(&self) -> gmx_core::Result<Self::Num> {
        if self.is_pure {
            debug_assert_eq!(
                self.short_token_amount, 0,
                "short token amount must be zero"
            );
            Ok(self.long_token_amount / 2)
        } else {
            Ok(self.short_token_amount)
        }
    }
}

impl gmx_core::Pool for Pool {
    fn apply_delta_to_long_amount(&mut self, delta: &Self::Signed) -> gmx_core::Result<()> {
        self.long_token_amount = self
            .long_token_amount
            .checked_add_signed(*delta)
            .ok_or(gmx_core::Error::Computation("apply delta to long amount"))?;
        Ok(())
    }

    fn apply_delta_to_short_amount(&mut self, delta: &Self::Signed) -> gmx_core::Result<()> {
        let amount = if self.is_pure {
            &mut self.long_token_amount
        } else {
            &mut self.short_token_amount
        };
        *amount = amount
            .checked_add_signed(*delta)
            .ok_or(gmx_core::Error::Computation("apply delta to short amount"))?;
        Ok(())
    }
}

type PoolsMap<'a> = DualVecMap<&'a mut Vec<u8>, &'a mut Vec<Pool>>;
type ClocksMap<'a> = DualVecMap<&'a mut Vec<u8>, &'a mut Vec<i64>>;

/// Convert to a [`Market`](gmx_core::Market).
pub struct AsMarket<'a, 'info> {
    meta: &'a MarketMeta,
    funding_factor_per_second: &'a mut i128,
    pools: PoolsMap<'a>,
    clocks: ClocksMap<'a>,
    mint: &'a mut Account<'info, Mint>,
    transfer: Option<TransferUtils<'a, 'info>>,
    receiver: Option<AccountInfo<'info>>,
    vault: Option<AccountInfo<'info>>,
}

impl<'a, 'info> AsMarket<'a, 'info> {
    pub(crate) fn enable_transfer(
        mut self,
        token_program: AccountInfo<'info>,
        store: &'a Account<'info, DataStore>,
    ) -> Self {
        self.transfer = Some(TransferUtils::new(
            token_program,
            store,
            Some(self.mint.to_account_info()),
        ));
        self
    }

    pub(crate) fn with_receiver(mut self, receiver: AccountInfo<'info>) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub(crate) fn with_vault(mut self, vault: AccountInfo<'info>) -> Self {
        self.vault = Some(vault);
        self
    }

    pub(crate) fn meta(&self) -> &MarketMeta {
        self.meta
    }

    pub(crate) fn into_position_ops(
        self,
        position: &'a mut AccountLoader<'info, Position>,
    ) -> Result<PositionOps<'a, 'info>> {
        PositionOps::try_new(self, position)
    }
}

impl<'a, 'info> gmx_core::Market<{ constants::MARKET_DECIMALS }> for AsMarket<'a, 'info> {
    type Num = u128;

    type Signed = i128;

    type Pool = Pool;

    fn pool(&self, kind: PoolKind) -> gmx_core::Result<Option<&Self::Pool>> {
        Ok(self.pools.get(&(kind as u8)))
    }

    fn pool_mut(&mut self, kind: PoolKind) -> gmx_core::Result<Option<&mut Self::Pool>> {
        Ok(self.pools.get_mut(&(kind as u8)))
    }

    fn total_supply(&self) -> Self::Num {
        self.mint.supply.into()
    }

    fn mint(&mut self, amount: &Self::Num) -> gmx_core::Result<()> {
        let Some(transfer) = self.transfer.as_ref() else {
            return Err(gmx_core::Error::invalid_argument("transfer not enabled"));
        };
        let Some(receiver) = self.receiver.as_ref() else {
            return Err(gmx_core::Error::MintReceiverNotSet);
        };
        transfer.mint_to(
            receiver,
            (*amount)
                .try_into()
                .map_err(|_| gmx_core::Error::Overflow)?,
        )?;
        self.mint.reload()?;
        Ok(())
    }

    fn burn(&mut self, amount: &Self::Num) -> gmx_core::Result<()> {
        let Some(transfer) = self.transfer.as_ref() else {
            return Err(gmx_core::Error::invalid_argument("transfer not enabled"));
        };
        let Some(vault) = self.vault.as_ref() else {
            return Err(gmx_core::Error::WithdrawalVaultNotSet);
        };
        transfer.burn_from(
            vault,
            (*amount)
                .try_into()
                .map_err(|_| gmx_core::Error::Overflow)?,
        )?;
        self.mint.reload()?;
        Ok(())
    }

    fn just_passed_in_seconds(&mut self, clock: ClockKind) -> gmx_core::Result<u64> {
        let current = Clock::get().map_err(Error::from)?.unix_timestamp;
        let last = self
            .clocks
            .get_mut(&(clock as u8))
            .ok_or(gmx_core::Error::MissingClockKind(clock))?;
        let duration = current.saturating_sub(*last);
        if duration > 0 {
            *last = current;
        }
        Ok(duration as u64)
    }

    fn usd_to_amount_divisor(&self) -> Self::Num {
        constants::MARKET_USD_TO_AMOUNT_DIVISOR
    }

    fn funding_amount_per_size_adjustment(&self) -> Self::Num {
        constants::FUNDING_AMOUNT_PER_SIZE_ADJUSTMENT
    }

    fn swap_impact_params(&self) -> gmx_core::params::PriceImpactParams<Self::Num> {
        PriceImpactParams::builder()
            .with_exponent(2 * constants::MARKET_USD_UNIT)
            .with_positive_factor(400_000_000_000)
            .with_negative_factor(800_000_000_000)
            .build()
            .unwrap()
    }

    fn swap_fee_params(&self) -> gmx_core::params::FeeParams<Self::Num> {
        FeeParams::builder()
            .with_fee_receiver_factor(37_000_000_000_000_000_000)
            .with_positive_impact_fee_factor(50_000_000_000_000_000)
            .with_negative_impact_fee_factor(70_000_000_000_000_000)
            .build()
    }

    fn position_params(&self) -> gmx_core::params::PositionParams<Self::Num> {
        PositionParams::new(
            constants::MARKET_USD_UNIT,
            constants::MARKET_USD_UNIT,
            constants::MARKET_USD_UNIT / 100,
            500_000_000_000_000_000,
            500_000_000_000_000_000,
            250_000_000_000_000_000,
        )
    }

    fn position_impact_params(&self) -> PriceImpactParams<Self::Num> {
        PriceImpactParams::builder()
            .with_exponent(2 * constants::MARKET_USD_UNIT)
            .with_positive_factor(9_000_000_000)
            .with_negative_factor(15_000_000_000)
            .build()
            .unwrap()
    }

    fn order_fee_params(&self) -> FeeParams<Self::Num> {
        FeeParams::builder()
            .with_fee_receiver_factor(37_000_000_000_000_000_000)
            .with_positive_impact_fee_factor(50_000_000_000_000_000)
            .with_negative_impact_fee_factor(70_000_000_000_000_000)
            .build()
    }

    fn position_impact_distribution_params(&self) -> PositionImpactDistributionParams<Self::Num> {
        PositionImpactDistributionParams::builder()
            .distribute_factor(constants::MARKET_USD_UNIT)
            .min_position_impact_pool_amount(1_000_000_000)
            .build()
    }

    fn borrowing_fee_params(&self) -> BorrowingFeeParams<Self::Num> {
        BorrowingFeeParams::builder()
            .factor_for_long(2_820_000_000_000)
            .factor_for_short(2_820_000_000_000)
            .exponent_for_long(100_000_000_000_000_000_000)
            .exponent_for_short(100_000_000_000_000_000_000)
            .build()
    }

    fn funding_fee_params(&self) -> FundingFeeParams<Self::Num> {
        FundingFeeParams::builder()
            .exponent(100_000_000_000_000_000_000)
            .funding_factor(2_000_000_000_000)
            .max_factor_per_second(1_000_000_000_000)
            .min_factor_per_second(30_000_000_000)
            .increase_factor_per_second(790_000_000)
            .decrease_factor_per_second(0)
            .threshold_for_stable_funding(5_000_000_000_000_000_000)
            .threshold_for_decrease_funding(0)
            .build()
    }

    fn funding_factor_per_second(&self) -> &Self::Signed {
        self.funding_factor_per_second
    }

    fn funding_factor_per_second_mut(&mut self) -> &mut Self::Signed {
        self.funding_factor_per_second
    }

    fn reserve_factor(&self) -> &Self::Num {
        &constants::MARKET_USD_UNIT
    }

    fn open_interest_reserve_factor(&self) -> &Self::Num {
        &constants::MARKET_USD_UNIT
    }

    fn max_pnl_factor(
        &self,
        kind: gmx_core::PnlFactorKind,
        _is_long: bool,
    ) -> gmx_core::Result<Self::Num> {
        use gmx_core::PnlFactorKind;

        match kind {
            PnlFactorKind::Deposit => Ok(60_000_000_000_000_000_000),
            PnlFactorKind::Withdrawal => Ok(30_000_000_000_000_000_000),
            _ => Err(error!(DataStoreError::RequiredResourceNotFound).into()),
        }
    }

    fn max_pool_amount(&self, _is_long_token: bool) -> gmx_core::Result<Self::Num> {
        Ok(1_000_000_000 * constants::MARKET_USD_UNIT)
    }

    fn max_pool_value_for_deposit(&self, _is_long_token: bool) -> gmx_core::Result<Self::Num> {
        Ok(1_000_000_000_000_000 * constants::MARKET_USD_UNIT)
    }

    fn max_open_interest(&self, _is_long: bool) -> gmx_core::Result<Self::Num> {
        Ok(1_000_000_000 * constants::MARKET_USD_UNIT)
    }
}

#[event]
pub struct MarketChangeEvent {
    pub address: Pubkey,
    pub action: super::Action,
    pub(crate) market: Market,
}
