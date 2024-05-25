use std::fmt;

use crate::{
    action::{
        update_borrowing_state::UpdateBorrowingReport, update_funding_state::UpdateFundingReport,
    },
    num::Unsigned,
    params::fee::PositionFees,
};

use super::{DecreasePositionParams, ProcessCollateralResult};

/// Report of the execution of posiiton decreasing.
#[must_use = "`should_remove`, `output_amount`, `secondary_output_amount` must use"]
pub struct DecreasePositionReport<T: Unsigned> {
    params: DecreasePositionParams<T>,
    price_impact_value: T::Signed,
    price_impact_diff: T,
    execution_price: T,
    size_delta_in_tokens: T,
    fees: PositionFees<T>,
    withdrawable_collateral_amount: T,
    size_delta_usd: T,
    borrowing: UpdateBorrowingReport<T>,
    funding: UpdateFundingReport<T>,

    // Output.
    should_remove: bool,
    is_output_token_long: bool,
    output_amount: T,
    secondary_output_amount: T,
    claimble_funding_long_token_amount: T,
    claimble_funding_short_token_amount: T,
}

impl<T: Unsigned + fmt::Debug> fmt::Debug for DecreasePositionReport<T>
where
    T::Signed: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecreasePositionReport")
            .field("should_remove", &self.should_remove)
            .field("params", &self.params)
            .field("price_impact_value", &self.price_impact_value)
            .field("price_impact_diff", &self.price_impact_diff)
            .field("execution_price", &self.execution_price)
            .field("size_delta_in_tokens", &self.size_delta_in_tokens)
            .field("fees", &self.fees)
            .field(
                "withdrawable_collateral_amount",
                &self.withdrawable_collateral_amount,
            )
            .field("size_delta_usd", &self.size_delta_usd)
            .field("borrowing", &self.borrowing)
            .field("funding", &self.funding)
            .field("is_output_token_long", &self.is_output_token_long)
            .field("output_amount", &self.output_amount)
            .field("secondary_output_amount", &self.secondary_output_amount)
            .field(
                "claimble_funding_long_token_amount",
                &self.claimble_funding_long_token_amount,
            )
            .field(
                "claimble_funding_short_token_amount",
                &self.claimble_funding_short_token_amount,
            )
            .finish()
    }
}

impl<T: Unsigned + Clone> DecreasePositionReport<T> {
    pub(super) fn new(
        should_remove: bool,
        params: DecreasePositionParams<T>,
        execution: ProcessCollateralResult<T>,
        withdrawable_collateral_amount: T,
        size_delta_usd: T,
        borrowing: UpdateBorrowingReport<T>,
        funding: UpdateFundingReport<T>,
    ) -> Self {
        Self {
            should_remove,
            params,
            price_impact_value: execution.price_impact_value,
            execution_price: execution.execution_price,
            size_delta_in_tokens: execution.size_delta_in_tokens,
            borrowing,
            funding,
            is_output_token_long: execution.is_output_token_long,
            output_amount: execution.collateral.output_amount,
            secondary_output_amount: execution.collateral.secondary_output_amount,
            withdrawable_collateral_amount,
            size_delta_usd,
            price_impact_diff: execution.price_impact_diff,
            claimble_funding_long_token_amount: execution
                .fees
                .funding_fees()
                .claimable_long_token_amount()
                .clone(),
            claimble_funding_short_token_amount: execution
                .fees
                .funding_fees()
                .claimable_short_token_amount()
                .clone(),
            fees: execution.fees,
        }
    }

    /// Get params.
    pub fn params(&self) -> &DecreasePositionParams<T> {
        &self.params
    }

    /// Get size delta in tokens.
    pub fn size_delta_in_tokens(&self) -> &T {
        &self.size_delta_in_tokens
    }

    /// Get execution price.
    pub fn execution_price(&self) -> &T {
        &self.execution_price
    }

    /// Get price impact value.
    pub fn price_impact_value(&self) -> &T::Signed {
        &self.price_impact_value
    }

    /// Get execution fees.
    pub fn fees(&self) -> &PositionFees<T> {
        &self.fees
    }

    /// Returns whether the output token is long token.
    ///
    /// ## Must Use
    /// Must use by the caller.
    pub fn is_output_token_long(&self) -> bool {
        self.is_output_token_long
    }

    /// Get output amount.
    ///
    /// ## Must Use
    /// Must use by the caller.
    pub fn output_amount(&self) -> &T {
        &self.output_amount
    }

    /// Get secondary output amount.
    ///
    /// ## Must Use
    /// Must use by the caller.
    pub fn secondary_output_amount(&self) -> &T {
        &self.secondary_output_amount
    }

    /// Get should remove.
    ///
    /// ## Must Use
    /// Must use by the caller.
    pub fn should_remove(&self) -> bool {
        self.should_remove
    }

    /// Get withdrawable collateral amount.
    ///
    /// ## Must Use
    /// Must use by the caller.
    pub fn withdrawable_collateral_amount(&self) -> &T {
        &self.withdrawable_collateral_amount
    }

    /// Get claimable funding amounts.
    ///
    /// ## Must Use
    /// Must use by the caller.
    pub fn cliamble_funding_amounts(&self) -> (&T, &T) {
        (
            &self.claimble_funding_long_token_amount,
            &self.claimble_funding_short_token_amount,
        )
    }

    /// Get borrowing report.
    pub fn borrowing(&self) -> &UpdateBorrowingReport<T> {
        &self.borrowing
    }

    /// Get funding report.
    pub fn funding(&self) -> &UpdateFundingReport<T> {
        &self.funding
    }
}
