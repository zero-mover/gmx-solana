use anchor_lang::prelude::*;

/// Tokens with feed.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TokensWithFeed {
    /// Tokens that require prices,
    /// which must be of the same length with `feeds`.
    pub tokens: Vec<Pubkey>,
    /// Token feeds for the tokens,
    /// which must be of  the same length with `tokens`.
    pub feeds: Vec<Pubkey>,
}

impl TokensWithFeed {
    /// Create from vec.
    pub fn from_vec(tokens_with_feed: Vec<(Pubkey, Pubkey)>) -> Self {
        let (tokens, feeds) = tokens_with_feed.into_iter().unzip();
        Self { tokens, feeds }
    }

    pub(crate) fn init_space(tokens_with_feed: &[(Pubkey, Pubkey)]) -> usize {
        (4 + 32 * tokens_with_feed.len()) + (4 + 32 * tokens_with_feed.len())
    }
}

/// Swap params.
#[derive(AnchorDeserialize, AnchorSerialize, Clone)]
pub struct SwapParams {
    /// Swap path for long token.
    pub long_token_swap_path: Vec<Pubkey>,
    /// Swap path for short token.
    pub short_token_swap_path: Vec<Pubkey>,
}

impl SwapParams {
    pub(crate) fn init_space(long_path_len: usize, short_path_len: usize) -> usize {
        (4 + 32 * long_path_len) + (4 + 32 * short_path_len)
    }
}
