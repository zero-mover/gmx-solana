use anchor_lang::prelude::*;
use gmsol_utils::price::{Decimal, Price};
use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;

use crate::{states::TokenConfig, CoreError};

/// The Pyth receiver program.
pub struct Pyth;

impl Id for Pyth {
    fn id() -> Pubkey {
        pyth_solana_receiver_sdk::ID
    }
}

impl Pyth {
    /// Push Oracle ID.
    pub const PUSH_ORACLE_ID: Pubkey = pyth_solana_receiver_sdk::PYTH_PUSH_ORACLE_ID;

    pub(super) fn check_and_get_price<'info>(
        clock: &Clock,
        token_config: &TokenConfig,
        feed: &'info AccountInfo<'info>,
        feed_id: &Pubkey,
    ) -> Result<(u64, i64, Price)> {
        let feed = Account::<PriceUpdateV2>::try_from(feed)?;
        let feed_id = feed_id.to_bytes();
        let price = feed.get_price_no_older_than(
            clock,
            token_config.heartbeat_duration().into(),
            &feed_id,
        )?;
        let parsed_price = pyth_price_with_confidence_to_price(
            price.price,
            price.conf,
            price.exponent,
            token_config,
        )?;
        Ok((feed.posted_slot, price.publish_time, parsed_price))
    }
}

/// The legacy Pyth program.
pub struct PythLegacy;

impl PythLegacy {
    pub(super) fn check_and_get_price<'info>(
        clock: &Clock,
        token_config: &TokenConfig,
        feed: &'info AccountInfo<'info>,
    ) -> Result<(u64, i64, Price)> {
        use pyth_sdk_solana::state::SolanaPriceAccount;
        let feed = SolanaPriceAccount::account_info_to_feed(feed).map_err(|err| {
            msg!("Pyth Error: {}", err);
            CoreError::Internal
        })?;

        let Some(price) = feed.get_price_no_older_than(
            clock.unix_timestamp,
            token_config.heartbeat_duration().into(),
        ) else {
            return err!(CoreError::PriceFeedNotUpdated);
        };

        let parsed_price =
            pyth_price_with_confidence_to_price(price.price, price.conf, price.expo, token_config)?;

        // Pyth legacy price feed does not provide `posted_slot`,
        // so we use current slot to ignore the slot check.
        Ok((clock.slot, price.publish_time, parsed_price))
    }
}

/// Convert pyth price value with confidence to [`Price`].
pub fn pyth_price_with_confidence_to_price(
    price: i64,
    confidence: u64,
    exponent: i32,
    token_config: &TokenConfig,
) -> Result<Price> {
    let mid_price: u64 = price
        .try_into()
        .map_err(|_| error!(CoreError::InvalidPriceFeedPrice))?;
    let min_price = mid_price
        .checked_sub(confidence)
        .ok_or(error!(CoreError::InvalidPriceFeedPrice))?;
    let max_price = mid_price
        .checked_add(confidence)
        .ok_or(CoreError::InvalidPriceFeedPrice)?;
    Ok(Price {
        min: pyth_price_value_to_decimal(min_price, exponent, token_config)?,
        max: pyth_price_value_to_decimal(max_price, exponent, token_config)?,
    })
}

/// Pyth price value to decimal.
pub fn pyth_price_value_to_decimal(
    mut value: u64,
    exponent: i32,
    token_config: &TokenConfig,
) -> Result<Decimal> {
    // actual price == value * 10^exponent
    // - If `exponent` is not positive, then the `decimals` is set to `-exponent`.
    // - Otherwise, we should use `value * 10^exponent` as `price` argument, and let `decimals` be `0`.
    let decimals: u8 = if exponent <= 0 {
        (-exponent)
            .try_into()
            .map_err(|_| CoreError::InvalidPriceFeedPrice)?
    } else {
        let factor = 10u64
            .checked_pow(exponent as u32)
            .ok_or(CoreError::InvalidPriceFeedPrice)?;
        value = value
            .checked_mul(factor)
            .ok_or(CoreError::InvalidPriceFeedPrice)?;
        0
    };
    let price = Decimal::try_from_price(
        value as u128,
        decimals,
        token_config.token_decimals(),
        token_config.precision(),
    )
    .map_err(|_| CoreError::InvalidPriceFeedPrice)?;
    Ok(price)
}

/// The address of legacy Pyth program.
#[cfg(not(feature = "devnet"))]
pub const PYTH_LEGACY_ID: Pubkey = Pubkey::new_from_array([
    220, 229, 235, 225, 228, 156, 59, 159, 17, 76, 181, 84, 76, 80, 169, 158, 192, 214, 146, 214,
    63, 86, 121, 90, 224, 41, 172, 131, 217, 234, 139, 226,
]);

#[cfg(feature = "devnet")]
pub const PYTH_LEGACY_ID: Pubkey = Pubkey::new_from_array([
    10, 26, 152, 51, 163, 118, 85, 43, 86, 183, 202, 13, 237, 25, 41, 23, 0, 87, 232, 39, 160, 198,
    39, 244, 182, 71, 185, 238, 144, 153, 175, 180,
]);

impl Id for PythLegacy {
    fn id() -> Pubkey {
        PYTH_LEGACY_ID
    }
}
