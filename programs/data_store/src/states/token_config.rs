use std::{
    cell::{Ref, RefMut},
    mem::size_of,
};

use anchor_lang::prelude::*;
use bitmaps::Bitmap;

use crate::DataStoreError;

use super::{InitSpace, PriceProviderKind};

/// Default heartbeat duration for price updates.
pub const DEFAULT_HEARTBEAT_DURATION: u32 = 30;

/// Default precision for price.
pub const DEFAULT_PRECISION: u8 = 4;

/// Default timestamp adjustment.
pub const DEFAULT_TIMESTAMP_ADJUSTMENT: u32 = 0;

const MAX_FEEDS: usize = 4;
const MAX_FLAGS: usize = 8;
const MAX_TOKENS: usize = 256;

/// Token Flags.
#[repr(u8)]
#[non_exhaustive]
pub enum Flag {
    /// Is initialized.
    Initialized,
    /// Enabled.
    Enabled,
    /// Is a synthetic asset.
    Synthetic,
    // WARNING: Cannot have more than `MAX_FLAGS` flags.
}

type TokenFlags = Bitmap<MAX_FLAGS>;
type TokenFlagsValue = u8;

#[zero_copy]
#[derive(PartialEq, Eq)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct TokenConfigV2 {
    /// Flags.
    flags: TokenFlagsValue,
    /// Token decimals.
    token_decimals: u8,
    /// Precision.
    precision: u8,
    /// Expected provider.
    expected_provider: u8,
    /// Price Feeds.
    feeds: [Pubkey; MAX_FEEDS],
    /// Heartbeat duration.
    heartbeat_duration: u32,
    /// Timestamp adjustment.
    timestamp_adjustment: u32,
    reserved: [u8; 32],
}

impl TokenConfigV2 {
    /// Get the corresponding price feed address.
    pub fn get_feed(&self, kind: &PriceProviderKind) -> Result<Pubkey> {
        let index = *kind as usize;
        let feed = self
            .feeds
            .get(index)
            .ok_or(DataStoreError::PriceFeedNotSet)?;
        if *feed == Pubkey::default() {
            err!(DataStoreError::PriceFeedNotSet)
        } else {
            Ok(*feed)
        }
    }

    /// Set expected provider.
    pub fn set_expected_provider(&mut self, provider: PriceProviderKind) {
        self.expected_provider = provider as u8;
    }

    /// Get expected price provider kind.
    pub fn expected_provider(&self) -> Result<PriceProviderKind> {
        let kind = PriceProviderKind::try_from(self.expected_provider)
            .map_err(|_| DataStoreError::InvalidProviderKindIndex)?;
        Ok(kind)
    }

    /// Get price feed address for the expected provider.
    pub fn get_expected_feed(&self) -> Result<Pubkey> {
        self.get_feed(&self.expected_provider()?)
    }

    /// Set enabled.
    pub fn set_enabled(&mut self, enable: bool) {
        self.set_flag(Flag::Enabled, enable)
    }

    /// Set synthetic.
    pub fn set_synthetic(&mut self, is_synthetic: bool) {
        self.set_flag(Flag::Synthetic, is_synthetic)
    }

    /// Is enabled.
    pub fn is_enabled(&self) -> bool {
        self.flag(Flag::Enabled)
    }

    /// Is synthetic.
    pub fn is_synthetic(&self) -> bool {
        self.flag(Flag::Synthetic)
    }

    /// Set flag
    pub fn set_flag(&mut self, flag: Flag, value: bool) {
        let mut bitmap = TokenFlags::from_value(self.flags);
        let index = flag as usize;
        bitmap.set(index, value);
        self.flags = bitmap.into_value();
    }

    /// Get flag.
    pub fn flag(&self, flag: Flag) -> bool {
        let bitmap = TokenFlags::from_value(self.flags);
        let index = flag as usize;
        bitmap.get(index)
    }

    /// Create a new token config from builder.
    pub fn init(
        &mut self,
        synthetic: bool,
        token_decimals: u8,
        builder: TokenConfigBuilder,
        enable: bool,
    ) -> Result<()> {
        require!(
            !self.flag(Flag::Initialized),
            DataStoreError::InvalidArgument
        );
        self.set_flag(Flag::Initialized, true);
        let TokenConfigBuilder {
            heartbeat_duration,
            precision,
            feeds,
            expected_provider,
        } = builder;
        self.set_synthetic(synthetic);
        self.set_enabled(enable);
        self.token_decimals = token_decimals;
        self.precision = precision;
        self.feeds = feeds
            .try_into()
            .map_err(|_| error!(DataStoreError::InvalidArgument))?;
        self.expected_provider = expected_provider.unwrap_or(PriceProviderKind::default() as u8);
        self.heartbeat_duration = heartbeat_duration;
        self.timestamp_adjustment = DEFAULT_TIMESTAMP_ADJUSTMENT;
        Ok(())
    }

    /// Token decimals.
    pub fn token_decimals(&self) -> u8 {
        self.token_decimals
    }

    /// Price Precision.
    pub fn precision(&self) -> u8 {
        self.precision
    }

    /// Get timestamp adjustment.
    pub fn timestamp_adjustment(&self) -> u32 {
        self.timestamp_adjustment
    }

    /// Heartbeat duration.
    pub fn heartbeat_duration(&self) -> u32 {
        self.heartbeat_duration
    }
}

impl InitSpace for TokenConfigV2 {
    const INIT_SPACE: usize = std::mem::size_of::<Self>();
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct TokenConfigBuilder {
    heartbeat_duration: u32,
    precision: u8,
    feeds: Vec<Pubkey>,
    expected_provider: Option<u8>,
}

impl Default for TokenConfigBuilder {
    fn default() -> Self {
        Self {
            heartbeat_duration: DEFAULT_HEARTBEAT_DURATION,
            precision: DEFAULT_PRECISION,
            feeds: vec![Pubkey::default(); MAX_FEEDS],
            expected_provider: None,
        }
    }
}

impl TokenConfigBuilder {
    /// Update the feed address for the given price provider.
    /// Return error when the feed was not set before.
    pub fn update_price_feed(mut self, kind: &PriceProviderKind, new_feed: Pubkey) -> Result<Self> {
        let index = *kind as usize;
        let feed = self
            .feeds
            .get_mut(index)
            .ok_or(DataStoreError::PriceFeedNotSet)?;
        *feed = new_feed;
        Ok(self)
    }

    /// Set heartbeat duration.
    pub fn with_heartbeat_duration(mut self, duration: u32) -> Self {
        self.heartbeat_duration = duration;
        self
    }

    /// Set precision.
    pub fn with_precision(mut self, precision: u8) -> Self {
        self.precision = precision;
        self
    }

    /// Set expected provider.
    pub fn with_expected_provider(mut self, provider: PriceProviderKind) -> Self {
        self.expected_provider = Some(provider as u8);
        self
    }
}

crate::fixed_map!(
    Tokens,
    Pubkey,
    crate::utils::pubkey::to_bytes,
    u8,
    MAX_TOKENS,
    0
);

/// Header of `TokenMap`.
#[account(zero_copy)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct TokenMapHeader {
    /// The authorized store.
    pub store: Pubkey,
    tokens: Tokens,
    reserved: [u8; 64],
}

impl InitSpace for TokenMapHeader {
    const INIT_SPACE: usize = std::mem::size_of::<TokenMapHeader>();
}

impl TokenMapHeader {
    /// Get the space of the whole `TokenMap` required, excluding discriminator.
    pub fn space(num_configs: u8) -> usize {
        TokenMapHeader::INIT_SPACE + (usize::from(num_configs) * TokenConfigV2::INIT_SPACE)
    }

    /// Get the space after push.
    pub fn space_after_push(&self) -> Result<usize> {
        let num_configs: u8 = self
            .tokens
            .len()
            .checked_add(1)
            .ok_or(error!(DataStoreError::ExceedMaxLengthLimit))?
            .try_into()
            .map_err(|_| error!(DataStoreError::AmountOverflow))?;
        Ok(Self::space(num_configs))
    }
}

/// Reference to Token Map.
pub struct TokenMapRef<'a> {
    header: Ref<'a, TokenMapHeader>,
    configs: Ref<'a, [u8]>,
}

/// Mutable Reference to Token Map.
pub struct TokenMapMut<'a> {
    header: RefMut<'a, TokenMapHeader>,
    configs: RefMut<'a, [u8]>,
}

/// Token Map Loader.
pub trait TokenMapLoader<'info> {
    fn load_token_map(&self) -> Result<TokenMapRef>;
    fn load_token_map_mut(&self) -> Result<TokenMapMut>;
}

impl<'info> TokenMapLoader<'info> for AccountLoader<'info, TokenMapHeader> {
    fn load_token_map(&self) -> Result<TokenMapRef> {
        // Check the account.
        self.load()?;

        let data = self.as_ref().try_borrow_data()?;
        let (_disc, data) = Ref::map_split(data, |d| d.split_at(8));
        let (header, configs) = Ref::map_split(data, |d| d.split_at(size_of::<TokenMapHeader>()));

        Ok(TokenMapRef {
            header: Ref::map(header, bytemuck::from_bytes),
            configs,
        })
    }

    fn load_token_map_mut(&self) -> Result<TokenMapMut> {
        // Check the account for mutablely access.
        self.load_mut()?;

        let data = self.as_ref().try_borrow_mut_data()?;
        let (_disc, data) = RefMut::map_split(data, |d| d.split_at_mut(8));
        let (header, configs) =
            RefMut::map_split(data, |d| d.split_at_mut(size_of::<TokenMapHeader>()));
        Ok(TokenMapMut {
            header: RefMut::map(header, bytemuck::from_bytes_mut),
            configs,
        })
    }
}

/// Read Token Map.
pub trait TokenMapAccess {
    /// Get the config of the given token.
    fn get(&self, token: &Pubkey) -> Option<&TokenConfigV2>;
}

impl<'a> TokenMapAccess for TokenMapRef<'a> {
    fn get(&self, token: &Pubkey) -> Option<&TokenConfigV2> {
        let index = usize::from(*self.header.tokens.get(token)?);
        crate::utils::dynamic_access::get(&self.configs, index)
    }
}

/// Token Map Operations.
///
/// The token map is append-only.
pub trait TokenMapMutAccess {
    /// Get mutably the config of the given token.
    fn get_mut(&mut self, token: &Pubkey) -> Option<&mut TokenConfigV2>;

    /// Push a new token config.
    fn push_with(
        &mut self,
        token: &Pubkey,
        f: impl FnOnce(&mut TokenConfigV2) -> Result<()>,
    ) -> Result<()>;
}

impl<'a> TokenMapMutAccess for TokenMapMut<'a> {
    fn get_mut(&mut self, token: &Pubkey) -> Option<&mut TokenConfigV2> {
        let index = usize::from(*self.header.tokens.get(token)?);
        crate::utils::dynamic_access::get_mut(&mut self.configs, index)
    }

    fn push_with(
        &mut self,
        token: &Pubkey,
        f: impl FnOnce(&mut TokenConfigV2) -> Result<()>,
    ) -> Result<()> {
        let next_index = self.header.tokens.len();
        require!(
            next_index < MAX_TOKENS,
            DataStoreError::ExceedMaxLengthLimit
        );
        let index = next_index
            .try_into()
            .map_err(|_| error!(DataStoreError::AmountOverflow))?;
        self.header.tokens.insert_with_options(token, index, true)?;
        let Some(dst) =
            crate::utils::dynamic_access::get_mut::<TokenConfigV2>(&mut self.configs, next_index)
        else {
            return err!(DataStoreError::NoSpaceForNewData);
        };
        (f)(dst)
    }
}
