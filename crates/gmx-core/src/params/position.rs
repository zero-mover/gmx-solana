/// Basic Position Parameters.
#[derive(Debug, Clone, Copy)]
pub struct PositionParams<T> {
    min_position_size_usd: T,
    min_collateral_value: T,
    min_collateral_factor: T,
}

impl<T> PositionParams<T> {
    /// Create a new [`PositionParams`].
    pub fn new(
        min_position_size_usd: T,
        min_collateral_value: T,
        min_collateral_factor: T,
    ) -> Self {
        Self {
            min_collateral_value,
            min_position_size_usd,
            min_collateral_factor,
        }
    }

    /// Get min position size usd.
    pub fn min_position_size_usd(&self) -> &T {
        &self.min_position_size_usd
    }

    /// Get min collateral value.
    pub fn min_collateral_value(&self) -> &T {
        &self.min_collateral_value
    }

    /// Get min collateral factor.
    pub fn min_collateral_factor(&self) -> &T {
        &self.min_collateral_factor
    }
}
