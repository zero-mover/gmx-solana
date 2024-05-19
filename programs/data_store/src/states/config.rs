use anchor_lang::prelude::*;

use crate::{
    constants::keys::{GLOBAL, REQUEST_EXPIRATION_TIME},
    DataStoreError,
};

use super::{common::MapStore, Amount, Factor, Seed};

/// Default request expiration time.
pub const DEFAULT_REQUEST_EXPIRATION_TIME: u64 = 300;

/// Config.
#[account]
#[derive(InitSpace)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Config {
    /// Bump.
    pub bump: u8,
    /// Factors.
    factors: MapStore<[u8; 32], u128, 32>,
    /// Amounts or seconds.
    amounts: MapStore<[u8; 32], u64, 32>,
}

impl Seed for Config {
    const SEED: &'static [u8] = b"config";
}

impl Config {
    /// Insert a new factor.
    pub fn insert_factor(
        &mut self,
        namespace: &str,
        key: &str,
        factor: u128,
        new: bool,
    ) -> Result<Option<Factor>> {
        if new {
            self.factors.insert_new(namespace, key, factor)?;
            Ok(None)
        } else {
            Ok(self.factors.insert(namespace, key, factor))
        }
    }

    /// Insert a new amount.
    pub fn insert_amount(
        &mut self,
        namespace: &str,
        key: &str,
        amount: u64,
        new: bool,
    ) -> Result<Option<Amount>> {
        if new {
            self.amounts.insert_new(namespace, key, amount)?;
            Ok(None)
        } else {
            Ok(self.amounts.insert(namespace, key, amount))
        }
    }

    /// Get amount.
    pub fn amount(&self, namespace: &str, key: &str) -> Option<Amount> {
        self.amounts
            .get_with(namespace, key, |amount| amount.copied())
    }

    /// Get Factor.
    pub fn factor(&self, namespace: &str, key: &str) -> Option<Factor> {
        self.factors
            .get_with(namespace, key, |factor| factor.copied())
    }

    /// Get request expiration time config.
    pub fn request_expiration(&self) -> u64 {
        self.amount(GLOBAL, REQUEST_EXPIRATION_TIME)
            .unwrap_or(DEFAULT_REQUEST_EXPIRATION_TIME)
    }

    /// Calculate the request expiration time.
    pub fn request_expiration_at(&self, start: i64) -> Result<i64> {
        start
            .checked_add_unsigned(self.request_expiration())
            .ok_or(error!(DataStoreError::AmountOverflow))
    }
}
