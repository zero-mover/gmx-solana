use num_traits::{CheckedAdd, Zero};

use crate::{fixed::FixedPointOps, utils};

/// Fee Parameters.
#[derive(Debug, Clone, Copy)]
pub struct FeeParams<T> {
    positive_impact_fee_factor: T,
    negative_impact_fee_factor: T,
    fee_receiver_factor: T,
}

impl<T> FeeParams<T> {
    /// Builder for [`FeeParams`].
    pub fn builder() -> Builder<T>
    where
        T: Zero,
    {
        Builder {
            positive_impact_factor: Zero::zero(),
            negative_impact_factor: Zero::zero(),
            fee_receiver_factor: Zero::zero(),
        }
    }
}

/// Fees.
#[derive(Debug, Clone, Copy)]
pub struct Fees<T> {
    fee_receiver_amount: T,
    fee_amount_for_pool: T,
}

impl<T: Zero> Default for Fees<T> {
    fn default() -> Self {
        Self {
            fee_receiver_amount: Zero::zero(),
            fee_amount_for_pool: Zero::zero(),
        }
    }
}

impl<T> Fees<T> {
    /// Create a new [`Fees`].
    pub fn new(pool: T, receiver: T) -> Self {
        Self {
            fee_amount_for_pool: pool,
            fee_receiver_amount: receiver,
        }
    }

    /// Get fee receiver amount.
    pub fn fee_receiver_amount(&self) -> &T {
        &self.fee_receiver_amount
    }

    /// Get fee amount for pool.
    pub fn fee_amount_for_pool(&self) -> &T {
        &self.fee_amount_for_pool
    }
}

impl<T> FeeParams<T> {
    #[inline]
    fn factor(&self, is_positive_impact: bool) -> &T {
        if is_positive_impact {
            &self.positive_impact_fee_factor
        } else {
            &self.negative_impact_fee_factor
        }
    }

    /// Get basic fee.
    #[inline]
    pub fn fee<const DECIMALS: u8>(&self, is_positive_impact: bool, amount: &T) -> Option<T>
    where
        T: FixedPointOps<DECIMALS>,
    {
        let factor = self.factor(is_positive_impact);
        utils::apply_factor(amount, factor)
    }

    /// Get receiver fee.
    #[inline]
    pub fn receiver_fee<const DECIMALS: u8>(&self, fee_amount: &T) -> Option<T>
    where
        T: FixedPointOps<DECIMALS>,
    {
        utils::apply_factor(fee_amount, &self.fee_receiver_factor)
    }

    /// Apply fees to `amount`.
    /// - `DECIMALS` is the decimals of the parameters.
    ///
    /// Returns `None` if the computation fails, otherwise `amount` after fees and the fees are returned.
    pub fn apply_fees<const DECIMALS: u8>(
        &self,
        is_positive_impact: bool,
        amount: &T,
    ) -> Option<(T, Fees<T>)>
    where
        T: FixedPointOps<DECIMALS>,
    {
        let fee_amount = self.fee(is_positive_impact, amount)?;
        let fee_receiver_amount = self.receiver_fee(&fee_amount)?;
        let fees = Fees {
            fee_amount_for_pool: fee_amount.checked_sub(&fee_receiver_amount)?,
            fee_receiver_amount,
        };
        Some((amount.checked_sub(&fee_amount)?, fees))
    }

    /// Get order fees.
    fn order_fees<const DECIMALS: u8>(
        &self,
        collateral_token_price: &T,
        size_delta_usd: &T,
        is_positive_impact: bool,
    ) -> crate::Result<OrderFees<T>>
    where
        T: FixedPointOps<DECIMALS>,
    {
        if collateral_token_price.is_zero() {
            return Err(crate::Error::InvalidPrices);
        }

        // TODO: use min price.
        let fee_amount = self
            .fee(is_positive_impact, size_delta_usd)
            .ok_or(crate::Error::Computation("calculating order fee usd"))?
            / collateral_token_price.clone();

        // TODO: apply rebase.

        let receiver_fee_amount = self
            .receiver_fee(&fee_amount)
            .ok_or(crate::Error::Computation("calculating order receiver fee"))?;
        Ok(OrderFees {
            base: Fees::new(
                fee_amount
                    .checked_sub(&receiver_fee_amount)
                    .ok_or(crate::Error::Computation("calculating order fee for pool"))?,
                receiver_fee_amount,
            ),
        })
    }

    /// Get position fees.
    pub fn position_fees<const DECIMALS: u8>(
        &self,
        collateral_token_price: &T,
        size_delta_usd: &T,
        is_positive_impact: bool,
    ) -> crate::Result<PositionFees<T>>
    where
        T: FixedPointOps<DECIMALS>,
    {
        let OrderFees { base } =
            self.order_fees(collateral_token_price, size_delta_usd, is_positive_impact)?;
        Ok(PositionFees { base })
    }
}

/// Order Fees.
pub struct OrderFees<T> {
    base: Fees<T>,
}

/// Position Fees.
#[derive(Debug, Clone, Copy)]
pub struct PositionFees<T> {
    base: Fees<T>,
}

impl<T> PositionFees<T> {
    /// Get base fees.
    pub fn base(&self) -> &Fees<T> {
        &self.base
    }

    /// Get total cost amount in collateral tokens.
    pub fn total_cost_amount(&self) -> crate::Result<T>
    where
        T: CheckedAdd,
    {
        self.total_cost_excluding_funding()
    }

    /// Get total cost excluding funding fee.
    pub fn total_cost_excluding_funding(&self) -> crate::Result<T>
    where
        T: CheckedAdd,
    {
        self.base
            .fee_amount_for_pool
            .checked_add(&self.base.fee_receiver_amount)
            .ok_or(crate::Error::Overflow)
    }
}

impl<T: Zero> Default for PositionFees<T> {
    fn default() -> Self {
        Self {
            base: Default::default(),
        }
    }
}

/// Builder for [`FeeParams`].
pub struct Builder<T> {
    positive_impact_factor: T,
    negative_impact_factor: T,
    fee_receiver_factor: T,
}

impl<T> Builder<T> {
    /// Set the fee factor for positive impact.
    pub fn with_positive_impact_fee_factor(mut self, factor: T) -> Self {
        self.positive_impact_factor = factor;
        self
    }

    /// Set the fee factor for negative impact.
    pub fn with_negative_impact_fee_factor(mut self, factor: T) -> Self {
        self.negative_impact_factor = factor;
        self
    }

    /// Set the fee receiver factor.
    pub fn with_fee_receiver_factor(mut self, factor: T) -> Self {
        self.fee_receiver_factor = factor;
        self
    }

    /// Build [`FeeParams`].
    pub fn build(self) -> FeeParams<T> {
        FeeParams {
            positive_impact_fee_factor: self.positive_impact_factor,
            negative_impact_fee_factor: self.negative_impact_factor,
            fee_receiver_factor: self.fee_receiver_factor,
        }
    }
}
