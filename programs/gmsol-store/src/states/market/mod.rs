use std::{collections::BTreeSet, str::FromStr};

use anchor_lang::{prelude::*, Bump};
use bitmaps::Bitmap;
use borsh::{BorshDeserialize, BorshSerialize};
use gmsol_model::{action::Prices, ClockKind, PoolKind};

use crate::{
    utils::fixed_str::{bytes_to_fixed_str, fixed_str_to_bytes},
    StoreError,
};

use super::{Factor, InitSpace, Oracle, Seed};

use self::pool::Pools;

pub use self::{
    config::{MarketConfig, MarketConfigBuffer, MarketConfigKey},
    ops::{AdlOps, ValidateMarketBalances},
    pool::Pool,
};

/// Market Operations.
pub mod ops;

/// Clock ops.
pub mod clock;

/// Market Config.
pub mod config;

/// Revertible Market Operations.
pub mod revertible;

/// Pool.
pub mod pool;

mod model;

/// Max number of flags.
pub const MAX_FLAGS: usize = 8;

/// Market Flag Value.
pub type MarketFlagValue = u8;

/// Market Flag Bitmap.
pub type MarketFlagBitmap = Bitmap<MAX_FLAGS>;

const MAX_NAME_LEN: usize = 64;

/// Find PDA for [`Market`] account.
pub fn find_market_address(
    store: &Pubkey,
    token: &Pubkey,
    store_program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[Market::SEED, store.as_ref(), token.as_ref()],
        store_program_id,
    )
}

/// Market.
#[account(zero_copy)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Market {
    /// Bump Seed.
    pub(crate) bump: u8,
    flag: MarketFlagValue,
    padding: [u8; 14],
    name: [u8; MAX_NAME_LEN],
    pub(crate) meta: MarketMeta,
    /// Store.
    pub store: Pubkey,
    pools: Pools,
    clocks: Clocks,
    state: MarketState,
    config: MarketConfig,
}

impl Bump for Market {
    fn seed(&self) -> u8 {
        self.bump
    }
}

impl Seed for Market {
    const SEED: &'static [u8] = b"market";
}

impl InitSpace for Market {
    const INIT_SPACE: usize = std::mem::size_of::<Self>();
}

impl Default for Market {
    fn default() -> Self {
        use bytemuck::Zeroable;
        Self::zeroed()
    }
}

impl Market {
    /// Initialize the market.
    #[allow(clippy::too_many_arguments)]
    pub fn init(
        &mut self,
        bump: u8,
        store: Pubkey,
        name: &str,
        market_token_mint: Pubkey,
        index_token_mint: Pubkey,
        long_token_mint: Pubkey,
        short_token_mint: Pubkey,
        is_enabled: bool,
    ) -> Result<()> {
        self.bump = bump;
        self.store = store;
        self.name = fixed_str_to_bytes(name)?;
        self.set_enabled(is_enabled);
        self.meta.market_token_mint = market_token_mint;
        self.meta.index_token_mint = index_token_mint;
        self.meta.long_token_mint = long_token_mint;
        self.meta.short_token_mint = short_token_mint;
        let is_pure = self.meta.long_token_mint == self.meta.short_token_mint;
        self.set_flag(MarketFlag::Pure, is_pure);
        self.pools.init(is_pure);
        self.clocks.init_to_current()?;
        self.config.init();
        Ok(())
    }

    /// Get meta.
    pub fn meta(&self) -> &MarketMeta {
        &self.meta
    }

    /// Get name.
    pub fn name(&self) -> Result<&str> {
        bytes_to_fixed_str(&self.name)
    }

    /// Description.
    pub fn description(&self) -> Result<String> {
        let name = self.name()?;
        Ok(format!(
            "Market {{ name = {name}, token = {}}}",
            self.meta.market_token_mint
        ))
    }

    /// Record transferred in by the given token.
    pub fn record_transferred_in_by_token(&mut self, token: &Pubkey, amount: u64) -> Result<()> {
        if self.meta.long_token_mint == *token {
            self.record_transferred_in(true, amount)
        } else if self.meta.short_token_mint == *token {
            self.record_transferred_in(false, amount)
        } else {
            Err(error!(StoreError::InvalidCollateralToken))
        }
    }

    /// Record transferred out by the given token.
    pub fn record_transferred_out_by_token(&mut self, token: &Pubkey, amount: u64) -> Result<()> {
        if self.meta.long_token_mint == *token {
            self.record_transferred_out(true, amount)
        } else if self.meta.short_token_mint == *token {
            self.record_transferred_out(false, amount)
        } else {
            Err(error!(StoreError::InvalidCollateralToken))
        }
    }

    /// Get flag.
    pub fn flag(&self, flag: MarketFlag) -> bool {
        let bitmap = MarketFlagBitmap::from_value(self.flag);
        bitmap.get(usize::from(flag as u8))
    }

    /// Set flag.
    pub fn set_flag(&mut self, flag: MarketFlag, value: bool) {
        let mut bitmap = MarketFlagBitmap::from_value(self.flag);
        bitmap.set(usize::from(flag as u8), value);
        self.flag = bitmap.into_value();
    }

    /// Is this market a pure market, i.e., a single token market.
    pub fn is_pure(&self) -> bool {
        self.flag(MarketFlag::Pure)
    }

    /// Is this market enabled.
    pub fn is_enabled(&self) -> bool {
        self.flag(MarketFlag::Enabled)
    }

    /// Set enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.set_flag(MarketFlag::Enabled, enabled);
    }

    /// Is ADL enabled.
    pub fn is_adl_enabled(&self, is_long: bool) -> bool {
        if is_long {
            self.flag(MarketFlag::AutoDeleveragingEnabledForLong)
        } else {
            self.flag(MarketFlag::AutoDeleveragingEnabledForShort)
        }
    }

    /// Set ADL enabled.
    pub fn set_adl_enabled(&mut self, is_long: bool, enabled: bool) {
        if is_long {
            self.set_flag(MarketFlag::AutoDeleveragingEnabledForLong, enabled)
        } else {
            self.set_flag(MarketFlag::AutoDeleveragingEnabledForShort, enabled)
        }
    }

    /// Record transferred in.
    fn record_transferred_in(&mut self, is_long_token: bool, amount: u64) -> Result<()> {
        // TODO: use event
        msg!(
            "{}: {},{}(+{},{})",
            self.meta.market_token_mint,
            self.state.long_token_balance,
            self.state.short_token_balance,
            amount,
            is_long_token
        );
        if self.is_pure() || is_long_token {
            self.state.long_token_balance = self
                .state
                .long_token_balance
                .checked_add(amount)
                .ok_or(error!(StoreError::AmountOverflow))?;
        } else {
            self.state.short_token_balance = self
                .state
                .short_token_balance
                .checked_add(amount)
                .ok_or(error!(StoreError::AmountOverflow))?;
        }
        msg!(
            "{}: {},{}",
            self.meta.market_token_mint,
            self.state.long_token_balance,
            self.state.short_token_balance
        );
        Ok(())
    }

    /// Record transferred out.
    fn record_transferred_out(&mut self, is_long_token: bool, amount: u64) -> Result<()> {
        // TODO: use event
        msg!(
            "{}: {},{}(-{},{})",
            self.meta.market_token_mint,
            self.state.long_token_balance,
            self.state.short_token_balance,
            amount,
            is_long_token
        );
        if self.is_pure() || is_long_token {
            self.state.long_token_balance = self
                .state
                .long_token_balance
                .checked_sub(amount)
                .ok_or(error!(StoreError::AmountOverflow))?;
        } else {
            self.state.short_token_balance = self
                .state
                .short_token_balance
                .checked_sub(amount)
                .ok_or(error!(StoreError::AmountOverflow))?;
        }
        msg!(
            "{}: {},{}",
            self.meta.market_token_mint,
            self.state.long_token_balance,
            self.state.short_token_balance
        );
        Ok(())
    }

    /// Get pool of the given kind.
    #[inline]
    pub fn pool(&self, kind: PoolKind) -> Option<Pool> {
        self.pools.get(kind).copied()
    }

    /// Try to get pool of the given kind.
    pub fn try_pool(&self, kind: PoolKind) -> gmsol_model::Result<&Pool> {
        self.pools
            .get(kind)
            .ok_or(gmsol_model::Error::MissingPoolKind(kind))
    }

    pub(crate) fn pool_mut(&mut self, kind: PoolKind) -> Option<&mut Pool> {
        self.pools.get_mut(kind)
    }

    /// Get clock of the given kind.
    pub fn clock(&self, kind: ClockKind) -> Option<i64> {
        self.clocks.get(kind).copied()
    }

    /// Validate the market.
    pub fn validate(&self, store: &Pubkey) -> Result<()> {
        require_eq!(*store, self.store, StoreError::InvalidMarket);
        require!(self.is_enabled(), StoreError::DisabledMarket);
        Ok(())
    }

    /// Get config.
    pub fn get_config(&self, key: &str) -> Result<&Factor> {
        let key = MarketConfigKey::from_str(key).map_err(|_| error!(StoreError::InvalidKey))?;
        Ok(self.get_config_by_key(key))
    }

    /// Get config by key.
    #[inline]
    pub fn get_config_by_key(&self, key: MarketConfigKey) -> &Factor {
        self.config.get(key)
    }

    /// Get config mutably by key
    pub fn get_config_mut(&mut self, key: &str) -> Result<&mut Factor> {
        let key = MarketConfigKey::from_str(key).map_err(|_| error!(StoreError::InvalidKey))?;
        Ok(self.config.get_mut(key))
    }

    /// Get market state.
    pub fn state(&self) -> &MarketState {
        &self.state
    }

    /// Get market state mutably.
    pub fn state_mut(&mut self) -> &mut MarketState {
        &mut self.state
    }

    /// Update config with buffer.
    pub fn update_config_with_buffer(&mut self, buffer: &MarketConfigBuffer) -> Result<()> {
        for entry in buffer.iter() {
            let key = entry.key()?;
            let current_value = self.config.get_mut(key);
            let new_value = entry.value();
            *current_value = new_value;
        }
        Ok(())
    }

    /// Get prices from oracle.
    pub fn prices(&self, oracle: &Oracle) -> Result<Prices<u128>> {
        oracle.market_prices(self)
    }

    /// Get max pool value for deposit.
    pub fn max_pool_value_for_deposit(&self, is_long_token: bool) -> gmsol_model::Result<Factor> {
        if is_long_token {
            Ok(self.config.max_pool_value_for_deposit_for_long_token)
        } else {
            Ok(self.config.max_pool_value_for_deposit_for_short_token)
        }
    }
}

/// Market Flags.
#[repr(u8)]
pub enum MarketFlag {
    /// Is enabled.
    Enabled,
    /// Is Pure.
    Pure,
    /// Is auto-deleveraging enabled for long.
    AutoDeleveragingEnabledForLong,
    /// Is auto-deleveraging enabled for short.
    AutoDeleveragingEnabledForShort,
    // CHECK: cannot have more than `MAX_FLAGS` flags.
}

/// Market State.
#[zero_copy]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct MarketState {
    long_token_balance: u64,
    short_token_balance: u64,
    funding_factor_per_second: i128,
    trade_count: u64,
    deposit_count: u64,
    withdrawal_count: u64,
    order_count: u64,
}

impl MarketState {
    /// Get long token balance.
    pub fn long_token_balance_raw(&self) -> u64 {
        self.long_token_balance
    }

    /// Get short token balance.
    pub fn short_token_balance_raw(&self) -> u64 {
        self.short_token_balance
    }

    /// Get funding factor per second.
    pub fn funding_factor_per_second(&self) -> i128 {
        self.funding_factor_per_second
    }

    /// Get current trade count.
    pub fn trade_count(&self) -> u64 {
        self.trade_count
    }

    /// Get current deposit count.
    pub fn deposit_count(&self) -> u64 {
        self.deposit_count
    }

    /// Get current withdrawal count.
    pub fn withdrawal_count(&self) -> u64 {
        self.withdrawal_count
    }

    /// Get current order count.
    pub fn order_count(&self) -> u64 {
        self.order_count
    }

    /// Next deposit id.
    pub fn next_deposit_id(&mut self) -> Result<u64> {
        let next_id = self
            .deposit_count
            .checked_add(1)
            .ok_or(error!(StoreError::AmountOverflow))?;
        self.deposit_count = next_id;
        Ok(next_id)
    }

    /// Next withdrawal id.
    pub fn next_withdrawal_id(&mut self) -> Result<u64> {
        let next_id = self
            .withdrawal_count
            .checked_add(1)
            .ok_or(error!(StoreError::AmountOverflow))?;
        self.withdrawal_count = next_id;
        Ok(next_id)
    }

    /// Next order id.
    pub fn next_order_id(&mut self) -> Result<u64> {
        let next_id = self
            .order_count
            .checked_add(1)
            .ok_or(error!(StoreError::AmountOverflow))?;
        self.order_count = next_id;
        Ok(next_id)
    }

    /// Next trade id.
    pub fn next_trade_id(&mut self) -> Result<u64> {
        let next_id = self
            .trade_count
            .checked_add(1)
            .ok_or(error!(StoreError::AmountOverflow))?;
        self.trade_count = next_id;
        Ok(next_id)
    }
}

/// Market Metadata.
#[zero_copy]
#[derive(BorshSerialize, BorshDeserialize)]
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

    /// Check if the given token is long token or short token, and return it's side.
    pub fn to_token_side(&self, token: &Pubkey) -> Result<bool> {
        if *token == self.long_token_mint {
            Ok(true)
        } else if *token == self.short_token_mint {
            Ok(false)
        } else {
            err!(StoreError::InvalidArgument)
        }
    }

    /// Get opposite token.
    pub fn opposite_token(&self, token: &Pubkey) -> Result<&Pubkey> {
        if *token == self.long_token_mint {
            Ok(&self.short_token_mint)
        } else if *token == self.short_token_mint {
            Ok(&self.long_token_mint)
        } else {
            err!(StoreError::InvalidArgument)
        }
    }

    /// Check if the given token is a valid collateral token,
    /// return error if it is not.
    pub fn validate_collateral_token(&self, token: &Pubkey) -> Result<()> {
        if self.is_collateral_token(token) {
            Ok(())
        } else {
            Err(StoreError::InvalidCollateralToken.into())
        }
    }

    /// Get ordered token set.
    pub fn ordered_tokens(&self) -> BTreeSet<Pubkey> {
        BTreeSet::from([
            self.index_token_mint,
            self.long_token_mint,
            self.short_token_mint,
        ])
    }
}

/// Type that has market meta.
pub trait HasMarketMeta {
    fn is_pure(&self) -> bool;

    fn market_meta(&self) -> &MarketMeta;
}

impl HasMarketMeta for Market {
    fn is_pure(&self) -> bool {
        self.is_pure()
    }

    fn market_meta(&self) -> &MarketMeta {
        &self.meta
    }
}

/// Market clocks.
#[zero_copy]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Clocks {
    /// Price impact distribution clock.
    price_impact_distribution: i64,
    /// Borrowing clock.
    borrowing: i64,
    /// Funding clock.
    funding: i64,
    /// ADL updated clock for long.
    adl_for_long: i64,
    /// ADL updated clock for short.
    adl_for_short: i64,
    reserved: [i64; 3],
}

impl Clocks {
    fn init_to_current(&mut self) -> Result<()> {
        let current = Clock::get()?.unix_timestamp;
        self.price_impact_distribution = current;
        self.borrowing = current;
        self.funding = current;
        Ok(())
    }

    fn get(&self, kind: ClockKind) -> Option<&i64> {
        let clock = match kind {
            ClockKind::PriceImpactDistribution => &self.price_impact_distribution,
            ClockKind::Borrowing => &self.borrowing,
            ClockKind::Funding => &self.funding,
            ClockKind::AdlForLong => &self.adl_for_long,
            ClockKind::AdlForShort => &self.adl_for_short,
            _ => return None,
        };
        Some(clock)
    }

    fn get_mut(&mut self, kind: ClockKind) -> Option<&mut i64> {
        let clock = match kind {
            ClockKind::PriceImpactDistribution => &mut self.price_impact_distribution,
            ClockKind::Borrowing => &mut self.borrowing,
            ClockKind::Funding => &mut self.funding,
            ClockKind::AdlForLong => &mut self.adl_for_long,
            ClockKind::AdlForShort => &mut self.adl_for_short,
            _ => return None,
        };
        Some(clock)
    }
}

#[event]
pub struct MarketChangeEvent {
    pub address: Pubkey,
    pub action: super::Action,
}
