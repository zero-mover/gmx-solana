/// Error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Empty deposit.
    #[error("empty deposit")]
    EmptyDeposit,
    /// Unknown computation error.
    #[error("unknown computation error")]
    Computation,
    /// Overflow.
    #[error("overflow")]
    Overflow,
    /// Underflow.
    #[error("underflow")]
    Underflow,
    /// Invalid pool value for deposit.
    #[error("invalid pool value for deposit")]
    InvalidPoolValueForDeposit,
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
}

impl Error {
    /// Build params.
    pub fn build_params(msg: impl ToString) -> Self {
        Self::BuildParams(msg.to_string())
    }
}
