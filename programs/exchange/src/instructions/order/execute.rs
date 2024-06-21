use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};
use data_store::{
    cpi::accounts::{MarketTransferOut, RemovePosition},
    program::DataStore,
    states::{order::TransferOut, Oracle, Order, PriceProvider},
    utils::{Authentication, WithOracle, WithOracleExt},
};

use crate::{utils::ControllerSeeds, ExchangeError};

use super::utils::CancelOrderUtil;

#[derive(Accounts)]
pub struct ExecuteOrder<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    /// CHECK: only used as signing PDA.
    #[account(
        seeds = [
            crate::constants::CONTROLLER_SEED,
            store.key().as_ref(),
        ],
        bump,
    )]
    pub controller: UncheckedAccount<'info>,
    /// CHECK: used and checked by CPI.
    pub store: UncheckedAccount<'info>,
    /// CHECK: check by CPI.
    pub token_map: UncheckedAccount<'info>,
    #[account(mut)]
    pub oracle: Account<'info, Oracle>,
    /// CHECK: only used to invoke CPI and should be checked by it.
    #[account(mut)]
    pub market: UncheckedAccount<'info>,
    #[account(mut, constraint = market_token_mint.key() == order.fixed.tokens.market_token)]
    pub market_token_mint: Account<'info, Mint>,
    #[account(
        mut,
        constraint = order.fixed.senders.initial_collateral_token_account == initial_collateral_token_account.as_ref().map(|a| a.key()),
    )]
    pub order: Account<'info, Order>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub position: Option<UncheckedAccount<'info>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub user: UncheckedAccount<'info>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub final_output_token_vault: Option<UncheckedAccount<'info>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub secondary_output_token_vault: Option<UncheckedAccount<'info>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub final_output_token_account: Option<UncheckedAccount<'info>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub secondary_output_token_account: Option<UncheckedAccount<'info>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub long_token_vault: UncheckedAccount<'info>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub short_token_vault: UncheckedAccount<'info>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub long_token_account: UncheckedAccount<'info>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub short_token_account: UncheckedAccount<'info>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub claimable_long_token_account_for_user: Option<UncheckedAccount<'info>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub claimable_short_token_account_for_user: Option<UncheckedAccount<'info>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub claimable_pnl_token_account_for_holding: Option<UncheckedAccount<'info>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub initial_collateral_token_account: Option<UncheckedAccount<'info>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub initial_collateral_token_vault: Option<UncheckedAccount<'info>>,
    /// CHECK: check by CPI and cancel utils.
    #[account(mut)]
    pub initial_market: Option<UncheckedAccount<'info>>,
    pub data_store_program: Program<'info, DataStore>,
    pub token_program: Program<'info, Token>,
    pub price_provider: Interface<'info, PriceProvider>,
    pub system_program: Program<'info, System>,
}

/// Execute an order.
pub fn execute_order<'info>(
    ctx: Context<'_, '_, 'info, 'info, ExecuteOrder<'info>>,
    recent_timestamp: i64,
    execution_fee: u64,
    cancel_on_execution_error: bool,
) -> Result<()> {
    let store = ctx.accounts.store.key();
    let controller = ControllerSeeds::find(&store);
    let order = &ctx.accounts.order;
    // TODO: validate the pre-condition of transferring out before execution.
    let (should_remove_position, transfer_out, final_output_market, final_secondary_output_market) =
        ctx.accounts.with_oracle_prices(
            order.prices.tokens.clone(),
            ctx.remaining_accounts,
            &controller.as_seeds(),
            |accounts, remaining_accounts| {
                let store = accounts.store.key;
                let swap = &accounts.order.swap;
                let final_output_market = swap
                    .find_last_market(store, true, remaining_accounts)
                    .unwrap_or(accounts.market.to_account_info());
                let final_secondary_output_market = swap
                    .find_last_market(store, false, remaining_accounts)
                    .unwrap_or(accounts.market.to_account_info());
                let (should_remove_position, transfer_out) = data_store::cpi::execute_order(
                    accounts
                        .execute_order_ctx()
                        .with_signer(&[&controller.as_seeds()])
                        .with_remaining_accounts(remaining_accounts.to_vec()),
                    recent_timestamp,
                    !cancel_on_execution_error,
                )?
                .get();
                accounts.order.reload()?;
                Ok((
                    should_remove_position,
                    transfer_out,
                    final_output_market,
                    final_secondary_output_market,
                ))
            },
        )?;
    ctx.accounts.process_transfer_out(
        &controller,
        &transfer_out,
        final_output_market,
        final_secondary_output_market,
    )?;
    if should_remove_position {
        // Refund all lamports.
        let refund = ctx.accounts.position()?.get_lamports();
        data_store::cpi::remove_position(
            ctx.accounts
                .remove_position_ctx()?
                .with_signer(&[&controller.as_seeds()]),
            refund,
        )?;
    }
    ctx.accounts.cancel_util().execute(
        ctx.accounts.authority.to_account_info(),
        &controller,
        execution_fee,
    )?;
    Ok(())
}

impl<'info> Authentication<'info> for ExecuteOrder<'info> {
    fn authority(&self) -> AccountInfo<'info> {
        self.authority.to_account_info()
    }

    fn on_error(&self) -> Result<()> {
        Err(error!(ExchangeError::PermissionDenied))
    }

    fn data_store_program(&self) -> AccountInfo<'info> {
        self.data_store_program.to_account_info()
    }

    fn store(&self) -> AccountInfo<'info> {
        self.store.to_account_info()
    }
}

impl<'info> WithOracle<'info> for ExecuteOrder<'info> {
    fn price_provider(&self) -> AccountInfo<'info> {
        self.price_provider.to_account_info()
    }

    fn oracle(&self) -> AccountInfo<'info> {
        self.oracle.to_account_info()
    }

    fn token_map(&self) -> AccountInfo<'info> {
        self.token_map.to_account_info()
    }

    fn controller(&self) -> AccountInfo<'info> {
        self.controller.to_account_info()
    }
}

impl<'info> ExecuteOrder<'info> {
    fn cancel_util(&self) -> CancelOrderUtil<'_, 'info> {
        CancelOrderUtil {
            data_store_program: self.data_store_program.to_account_info(),
            token_program: self.token_program.to_account_info(),
            system_program: self.system_program.to_account_info(),
            controller: self.controller.to_account_info(),
            store: self.store.to_account_info(),
            user: self.user.to_account_info(),
            order: &self.order,
            initial_market: self.initial_market.as_ref().map(|a| a.to_account_info()),
            initial_collateral_token_account: self
                .initial_collateral_token_account
                .as_ref()
                .map(|a| a.to_account_info()),
            initial_collateral_token_vault: self
                .initial_collateral_token_vault
                .as_ref()
                .map(|a| a.to_account_info()),
        }
    }

    fn execute_order_ctx(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, data_store::cpi::accounts::ExecuteOrder<'info>> {
        CpiContext::new(
            self.data_store_program.to_account_info(),
            data_store::cpi::accounts::ExecuteOrder {
                authority: self.controller.to_account_info(),
                store: self.store.to_account_info(),
                oracle: self.oracle.to_account_info(),
                order: self.order.to_account_info(),
                market: self.market.to_account_info(),
                market_token_mint: self.market_token_mint.to_account_info(),
                position: self.position.as_ref().map(|a| a.to_account_info()),
                final_output_token_vault: self
                    .final_output_token_vault
                    .as_ref()
                    .map(|a| a.to_account_info()),
                secondary_output_token_vault: self
                    .secondary_output_token_vault
                    .as_ref()
                    .map(|a| a.to_account_info()),
                final_output_token_account: self
                    .final_output_token_account
                    .as_ref()
                    .map(|a| a.to_account_info()),
                secondary_output_token_account: self
                    .secondary_output_token_account
                    .as_ref()
                    .map(|a| a.to_account_info()),
                long_token_vault: self.long_token_vault.to_account_info(),
                short_token_vault: self.short_token_vault.to_account_info(),
                long_token_account: self.long_token_account.to_account_info(),
                short_token_account: self.short_token_account.to_account_info(),
                token_program: self.token_program.to_account_info(),
                claimable_long_token_account_for_user: self
                    .claimable_long_token_account_for_user
                    .as_ref()
                    .map(|a| a.to_account_info()),
                claimable_short_token_account_for_user: self
                    .claimable_short_token_account_for_user
                    .as_ref()
                    .map(|a| a.to_account_info()),
                claimable_pnl_token_account_for_holding: self
                    .claimable_pnl_token_account_for_holding
                    .as_ref()
                    .map(|a| a.to_account_info()),
            },
        )
    }

    fn position(&self) -> Result<&UncheckedAccount<'info>> {
        let Some(position) = self.position.as_ref() else {
            return err!(ExchangeError::PositionNotProvided);
        };
        Ok(position)
    }

    fn remove_position_ctx(&self) -> Result<CpiContext<'_, '_, '_, 'info, RemovePosition<'info>>> {
        Ok(CpiContext::new(
            self.data_store_program.to_account_info(),
            RemovePosition {
                payer: self.authority.to_account_info(),
                authority: self.controller.to_account_info(),
                store: self.store.to_account_info(),
                position: self.position()?.to_account_info(),
                user: self.user.to_account_info(),
                system_program: self.system_program.to_account_info(),
            },
        ))
    }

    fn market_transfer_out_ctx(
        &self,
        market: AccountInfo<'info>,
        vault: AccountInfo<'info>,
        to: AccountInfo<'info>,
    ) -> CpiContext<'_, '_, '_, 'info, MarketTransferOut<'info>> {
        CpiContext::new(
            self.data_store_program.to_account_info(),
            MarketTransferOut {
                authority: self.controller.to_account_info(),
                store: self.store.to_account_info(),
                market,
                to,
                vault,
                token_program: self.token_program.to_account_info(),
            },
        )
    }

    fn market_transfer_out(
        &self,
        controller: &ControllerSeeds,
        market: Option<AccountInfo<'info>>,
        vault: Option<AccountInfo<'info>>,
        to: Option<AccountInfo<'info>>,
        amount: u64,
    ) -> Result<()> {
        data_store::cpi::market_transfer_out(
            self.market_transfer_out_ctx(
                market.ok_or(error!(ExchangeError::InvalidArgument))?,
                vault.ok_or(error!(ExchangeError::InvalidArgument))?,
                to.ok_or(error!(ExchangeError::InvalidArgument))?,
            )
            .with_signer(&[&controller.as_seeds()]),
            amount,
        )?;
        Ok(())
    }

    fn process_transfer_out(
        &self,
        controller: &ControllerSeeds,
        transfer_out: &TransferOut,
        final_output_market: AccountInfo<'info>,
        final_secondary_output_market: AccountInfo<'info>,
    ) -> Result<()> {
        let TransferOut {
            final_output_token,
            final_secondary_output_token,
            long_token,
            short_token,
            long_token_for_claimable_account_of_user,
            short_token_for_claimable_account_of_user,
            long_token_for_claimable_account_of_holding,
            short_token_for_claimable_account_of_holding,
        } = transfer_out;

        if *final_output_token != 0 {
            // Must have been validated during the execution.
            self.market_transfer_out(
                controller,
                Some(final_output_market),
                self.final_output_token_vault
                    .as_ref()
                    .map(|a| a.to_account_info()),
                self.final_output_token_account
                    .as_ref()
                    .map(|a| a.to_account_info()),
                *final_output_token,
            )?;
        }

        if *final_secondary_output_token != 0 {
            // Must have been validated during the execution.
            self.market_transfer_out(
                controller,
                Some(final_secondary_output_market),
                self.secondary_output_token_vault
                    .as_ref()
                    .map(|a| a.to_account_info()),
                self.secondary_output_token_account
                    .as_ref()
                    .map(|a| a.to_account_info()),
                *final_secondary_output_token,
            )?;
        }

        if *long_token != 0 {
            self.market_transfer_out(
                controller,
                Some(self.market.to_account_info()),
                Some(self.long_token_vault.to_account_info()),
                Some(self.long_token_account.to_account_info()),
                *long_token,
            )?;
        }

        if *short_token != 0 {
            self.market_transfer_out(
                controller,
                Some(self.market.to_account_info()),
                Some(self.short_token_vault.to_account_info()),
                Some(self.short_token_account.to_account_info()),
                *short_token,
            )?;
        }

        if *long_token_for_claimable_account_of_user != 0 {
            self.market_transfer_out(
                controller,
                Some(self.market.to_account_info()),
                Some(self.long_token_vault.to_account_info()),
                self.claimable_long_token_account_for_user
                    .as_ref()
                    .map(|a| a.to_account_info()),
                *long_token_for_claimable_account_of_user,
            )?;
        }

        if *short_token_for_claimable_account_of_user != 0 {
            self.market_transfer_out(
                controller,
                Some(self.market.to_account_info()),
                Some(self.short_token_vault.to_account_info()),
                self.claimable_short_token_account_for_user
                    .as_ref()
                    .map(|a| a.to_account_info()),
                *short_token_for_claimable_account_of_user,
            )?;
        }

        if *long_token_for_claimable_account_of_holding != 0 {
            self.market_transfer_out(
                controller,
                Some(self.market.to_account_info()),
                Some(self.long_token_vault.to_account_info()),
                self.claimable_pnl_token_account_for_holding
                    .as_ref()
                    .map(|a| a.to_account_info()),
                *long_token_for_claimable_account_of_holding,
            )?;
        }

        if *short_token_for_claimable_account_of_holding != 0 {
            self.market_transfer_out(
                controller,
                Some(self.market.to_account_info()),
                Some(self.short_token_vault.to_account_info()),
                self.claimable_pnl_token_account_for_holding
                    .as_ref()
                    .map(|a| a.to_account_info()),
                *short_token_for_claimable_account_of_holding,
            )?;
        }

        Ok(())
    }
}
