use anchor_lang::prelude::*;
use gmsol_utils::price::Price;

use crate::{
    states::{Amount, Store, TokenConfig},
    CoreError,
};

use super::PriceProviderKind;

/// Default timestamp adjustment.
pub const DEFAULT_TIMESTAMP_ADJUSTMENT: u64 = 1;

/// Price Validator.
pub struct PriceValidator {
    clock: Clock,
    max_age: Amount,
    max_oracle_timestamp_range: Amount,
    max_future_timestamp_excess: Amount,
    min_oracle_ts: i64,
    max_oracle_ts: i64,
    min_oracle_slot: Option<u64>,
}

impl PriceValidator {
    pub(super) fn clock(&self) -> &Clock {
        &self.clock
    }

    pub(super) fn validate_one(
        &mut self,
        token_config: &TokenConfig,
        provider: &PriceProviderKind,
        oracle_ts: i64,
        oracle_slot: u64,
        _price: &Price,
    ) -> Result<()> {
        let timestamp_adjustment = token_config.timestamp_adjustment(provider)?.into();
        let ts = oracle_ts
            .checked_sub_unsigned(timestamp_adjustment)
            .ok_or_else(|| error!(CoreError::TokenAmountOverflow))?;

        let expiration_ts = ts
            .checked_add_unsigned(self.max_age)
            .ok_or_else(|| error!(CoreError::TokenAmountOverflow))?;

        let current_ts = self.clock.unix_timestamp;

        require_gte!(expiration_ts, current_ts, CoreError::MaxPriceAgeExceeded);
        require_gte!(
            current_ts.saturating_add_unsigned(self.max_future_timestamp_excess),
            oracle_ts,
            CoreError::MaxPriceTimestampExceeded
        );

        // Note: we may add ref price validation here in the future.

        self.merge_range(Some(oracle_slot), ts, ts);

        Ok(())
    }

    pub(super) fn merge_range(
        &mut self,
        min_oracle_slot: Option<u64>,
        min_oracle_ts: i64,
        max_oracle_ts: i64,
    ) {
        self.min_oracle_slot = match (self.min_oracle_slot, min_oracle_slot) {
            (Some(current), Some(other)) => Some(current.min(other)),
            (None, Some(slot)) | (Some(slot), None) => Some(slot),
            (None, None) => None,
        };
        self.min_oracle_ts = self.min_oracle_ts.min(min_oracle_ts);
        self.max_oracle_ts = self.max_oracle_ts.max(max_oracle_ts);
    }

    pub(super) fn finish(self) -> Result<Option<(u64, i64, i64)>> {
        let range: u64 = self
            .max_oracle_ts
            .checked_sub(self.min_oracle_ts)
            .ok_or_else(|| error!(CoreError::TokenAmountOverflow))?
            .try_into()
            .map_err(|_| error!(CoreError::InvalidOracleTimestampsRange))?;
        require_gte!(
            self.max_oracle_timestamp_range,
            range,
            CoreError::MaxOracleTimestampsRangeExceeded
        );
        Ok(self
            .min_oracle_slot
            .map(|slot| (slot, self.min_oracle_ts, self.max_oracle_ts)))
    }
}

impl<'a> TryFrom<&'a Store> for PriceValidator {
    type Error = anchor_lang::error::Error;

    fn try_from(config: &'a Store) -> Result<Self> {
        let max_age = config.amount.oracle_max_age;
        // TODO: enable validation with ref price.
        let _max_ref_price_deviation_factor = config.factor.oracle_ref_price_deviation;
        let max_oracle_timestamp_range = config.amount.oracle_max_timestamp_range;
        let max_future_timestamp_excess = config.amount.oracle_max_future_timestamp_excess;
        Ok(Self {
            clock: Clock::get()?,
            max_age,
            // max_ref_price_deviation_factor,
            max_oracle_timestamp_range,
            max_future_timestamp_excess,
            min_oracle_ts: i64::MAX,
            max_oracle_ts: i64::MIN,
            min_oracle_slot: None,
        })
    }
}
