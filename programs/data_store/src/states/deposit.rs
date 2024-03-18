use anchor_lang::prelude::*;

use super::{NonceBytes, Seed};

const MAX_SWAP_PATH_LEN: usize = 16;

/// Deposit.
#[account]
#[derive(InitSpace)]
pub struct Deposit {
    /// The bump seed.
    pub bump: u8,
    /// The nonce bytes for this deposit.
    pub nonce: [u8; 32],
    /// The account depositing liquidity.
    pub user: Pubkey,
    // /// Callback Contract.
    // pub callback: Pubkey,
    /// The market to deposit to.
    pub market: Pubkey,
    /// The receivers of the deposit.
    pub receivers: Receivers,
    /// The tokens and swap paths for the deposit.
    pub tokens: Tokens,
    /// The slot that the deposit was last updated at.
    pub updated_at_slot: u64,
    // /// The execution fee for keepers.
    // pub execution_fee: u64,
    // /// The fee limit for the callback contract.
    // pub callback_fee_limit: u64,
}

impl Seed for Deposit {
    const SEED: &'static [u8] = b"deposit";
}

impl Deposit {
    /// The max length of swap path.
    pub const MAX_SWAP_PATH_LEN: usize = MAX_SWAP_PATH_LEN;

    pub(crate) fn init(
        &mut self,
        bump: u8,
        nonce: NonceBytes,
        account: Pubkey,
        market: Pubkey,
        receivers: Receivers,
        tokens: Tokens,
    ) -> Result<()> {
        self.bump = bump;
        self.nonce = nonce;
        self.user = account;
        self.receivers = receivers;
        self.market = market;
        self.tokens = tokens;
        self.updated_at_slot = Clock::get()?.slot;
        Ok(())
    }
}

/// The receivers of the deposit.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct Receivers {
    /// The address to send the liquidity tokens to.
    pub receiver: Pubkey,
    /// The ui fee receiver.
    pub ui_fee_receiver: Pubkey,
}

/// The tokens and swap paths config for the deposit.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct Tokens {
    /// Initial long token.
    pub initial_long_token: Pubkey,
    /// Initial short token.
    pub initial_short_token: Pubkey,
    /// Swap path for long token.
    #[max_len(MAX_SWAP_PATH_LEN)]
    pub long_token_swap_path: Vec<Pubkey>,
    /// Swap path for short token.
    #[max_len(MAX_SWAP_PATH_LEN)]
    pub short_token_swap_path: Vec<Pubkey>,
    /// The amount of long tokens to deposit.
    pub initial_long_token_amount: u64,
    /// The amount of short tokens to deposit.
    pub initial_short_token_amount: u64,
    /// The minimum acceptable number of liquidity tokens.
    pub min_market_tokens: u64,
    /// Whether to unwrap the native token.
    pub should_unwrap_native_token: bool,
}
