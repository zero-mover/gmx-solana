use crate::PoolKind;

/// Error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Empty deposit.
    #[error("empty deposit")]
    EmptyDeposit,
    /// Empty withdrawal.
    #[error("empty withdrawal")]
    EmptyWithdrawal,
    /// Invalid prices.
    #[error("invalid prices")]
    InvalidPrices,
    /// Unknown computation error.
    #[error("unknown computation error")]
    Computation,
    /// Overflow.
    #[error("overflow")]
    Overflow,
    /// Underflow.
    #[error("underflow")]
    Underflow,
    /// Divided by zero.
    #[error("divided by zero")]
    DividedByZero,
    /// Invalid pool value.
    #[error("invalid pool value {0}")]
    InvalidPoolValue(String),
    /// Convert error.
    #[error("convert value error")]
    Convert,
    /// Anchor error.
    #[cfg(feature = "solana")]
    #[error(transparent)]
    Solana(#[from] anchor_lang::prelude::Error),
    /// Build params error.
    #[error("build params: {0}")]
    BuildParams(String),
    /// Missing pool kind.
    #[error("missing pool of kind: {0}")]
    MissingPoolKind(PoolKind),
    /// Mint receiver not set.
    #[error("mint receiver not set")]
    MintReceiverNotSet,
    /// Withdrawal vault not set.
    #[error("withdrawal vault not set")]
    WithdrawalVaultNotSet,
}

impl Error {
    /// Build params.
    pub fn build_params(msg: impl ToString) -> Self {
        Self::BuildParams(msg.to_string())
    }

    /// Invalid pool value.
    pub fn invalid_pool_value(msg: impl ToString) -> Self {
        Self::InvalidPoolValue(msg.to_string())
    }
}
