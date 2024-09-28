use anchor_lang::prelude::*;

use super::{
    common::{
        action::{Action, ActionHeader},
        token::TokenAndAccount,
    },
    Seed,
};

/// Shift.
#[account(zero_copy)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Shift {
    /// Action header.
    pub(crate) header: ActionHeader,
    /// Token accounts.
    pub(crate) tokens: TokenAccounts,
    /// Shift params.
    pub(crate) params: ShiftParams,
}

impl Seed for Shift {
    const SEED: &'static [u8] = b"shift";
}

impl Action for Shift {
    fn header(&self) -> &ActionHeader {
        &self.header
    }
}

impl Shift {
    /// Init Space.
    pub const INIT_SPACE: usize = core::mem::size_of::<Self>();

    /// Min execution lamports.
    pub const MIN_EXECUTION_LAMPORTS: u64 = 200_000;
}

#[account(zero_copy)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct TokenAccounts {
    pub(crate) from_market_token: TokenAndAccount,
    pub(crate) to_market_token: TokenAndAccount,
    pub(crate) long_token: Pubkey,
    pub(crate) short_token: Pubkey,
}

impl TokenAccounts {
    /// Get from market token.
    pub fn from_market_token(&self) -> Pubkey {
        self.from_market_token.token().expect("must exist")
    }

    /// Get from market token account.
    pub fn from_market_token_account(&self) -> Pubkey {
        self.from_market_token.account().expect("msut exist")
    }

    /// Get to market token.
    pub fn to_market_token(&self) -> Pubkey {
        self.to_market_token.token().expect("must exist")
    }

    /// Get to market token account.
    pub fn to_market_token_account(&self) -> Pubkey {
        self.to_market_token.account().expect("msut exist")
    }

    /// Get long token.
    pub fn long_token(&self) -> &Pubkey {
        &self.long_token
    }

    /// Get short token.
    pub fn short_token(&self) -> &Pubkey {
        &self.short_token
    }
}

#[account(zero_copy)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct ShiftParams {
    from_market_token_amount: u64,
    min_to_market_token_amount: u64,
}

impl ShiftParams {
    /// Get from market token amount.
    pub fn from_market_token_amount(&self) -> u64 {
        self.from_market_token_amount
    }

    /// Get min to market token amount.
    pub fn min_to_market_token_amount(&self) -> u64 {
        self.min_to_market_token_amount
    }
}
