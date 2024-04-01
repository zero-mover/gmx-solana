use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

use super::{
    common::{SwapParams, TokensWithFeed},
    Market, NonceBytes, Seed,
};

/// Deposit.
#[account]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Deposit {
    /// Fixed part.
    pub fixed: Fixed,
    /// Dynamic part.
    pub dynamic: Dynamic,
}

impl Deposit {
    pub(crate) fn init_space(
        tokens_with_feed: &[(Pubkey, Pubkey)],
        swap_params: &SwapParams,
    ) -> usize {
        Fixed::INIT_SPACE + Dynamic::init_space(tokens_with_feed, swap_params)
    }
}

/// Fixed part of [`Deposit`].
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Fixed {
    /// The bump seed.
    pub bump: u8,
    /// The nonce bytes for this deposit.
    pub nonce: [u8; 32],
    /// The slot that the deposit was last updated at.
    pub updated_at_slot: u64,
    /// Market.
    pub market: Pubkey,
    /// Senders.
    pub senders: Senders,
    /// The receivers of the deposit.
    pub receivers: Receivers,
    /// Tokens config.
    pub tokens: Tokens,
}

/// Senders of [`Deposit`].
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Senders {
    /// The user depositing liquidity.
    pub user: Pubkey,
    /// Initial long token account.
    pub initial_long_token_account: Pubkey,
    /// Initial short token account.
    pub initial_short_token_account: Pubkey,
}

/// Tokens config of [`Deposit`].
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Tokens {
    /// The market token of the market.
    pub market_token: Pubkey,
    /// Initial long token.
    pub initial_long_token: Pubkey,
    /// Initial short token.
    pub initial_short_token: Pubkey,
    /// Params.
    pub params: TokenParams,
}

/// Dynamic part of [`Deposit`].
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Dynamic {
    /// Tokens with feed.
    pub tokens_with_feed: TokensWithFeed,
    /// Swap params.
    pub swap_params: SwapParams,
}

impl Dynamic {
    fn init_space(tokens_with_feed: &[(Pubkey, Pubkey)], swap_params: &SwapParams) -> usize {
        TokensWithFeed::init_space(tokens_with_feed)
            + SwapParams::init_space(
                swap_params.long_token_swap_path.len(),
                swap_params.short_token_swap_path.len(),
            )
    }
}

impl Seed for Deposit {
    const SEED: &'static [u8] = b"deposit";
}

impl Deposit {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn init(
        &mut self,
        bump: u8,
        market: &Account<Market>,
        nonce: NonceBytes,
        tokens_with_feed: Vec<(Pubkey, Pubkey)>,
        user: Pubkey,
        initial_long_token_account: &Account<TokenAccount>,
        initial_short_token_account: &Account<TokenAccount>,
        receivers: Receivers,
        token_params: TokenParams,
        swap_params: SwapParams,
    ) -> Result<()> {
        *self = Self {
            fixed: Fixed {
                bump,
                nonce,
                updated_at_slot: Clock::get()?.slot,
                market: market.key(),
                senders: Senders {
                    user,
                    initial_long_token_account: initial_long_token_account.key(),
                    initial_short_token_account: initial_short_token_account.key(),
                },
                receivers,
                tokens: Tokens {
                    market_token: market.meta.market_token_mint,
                    initial_long_token: initial_long_token_account.mint,
                    initial_short_token: initial_short_token_account.mint,
                    params: token_params,
                },
            },
            dynamic: Dynamic {
                tokens_with_feed: TokensWithFeed::from_vec(tokens_with_feed),
                swap_params,
            },
        };
        Ok(())
    }
}

/// The receivers of the deposit.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Receivers {
    /// The address to send the liquidity tokens to.
    pub receiver: Pubkey,
    /// The ui fee receiver.
    pub ui_fee_receiver: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct TokenParams {
    /// The amount of long tokens to deposit.
    pub initial_long_token_amount: u64,
    /// The amount of short tokens to deposit.
    pub initial_short_token_amount: u64,
    /// The minimum acceptable number of liquidity tokens.
    pub min_market_tokens: u64,
    /// Whether to unwrap the native token.
    pub should_unwrap_native_token: bool,
}
