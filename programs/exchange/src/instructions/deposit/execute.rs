use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use data_store::{
    cpi::accounts::CheckRole,
    program::DataStore,
    states::{Deposit, Market},
    utils::Authentication,
};
use gmx_core::{Market as GmxCoreMarket, MarketExt};
use oracle::{
    program::Oracle,
    utils::{Chainlink, WithOracle, WithOracleExt},
};

use crate::{
    utils::market::{AsMarket, GmxCoreError},
    ExchangeError,
};

/// Execute a deposit.
pub fn execute_deposit<'info>(
    ctx: Context<'_, '_, 'info, 'info, ExecuteDeposit<'info>>,
) -> Result<()> {
    let deposit = &ctx.accounts.deposit;
    let long_token = deposit.tokens.initial_long_token;
    let short_token = deposit.tokens.initial_short_token;
    let remaining_accounts = ctx.remaining_accounts.to_vec();
    ctx.accounts.with_oracle_prices(
        vec![long_token, short_token],
        remaining_accounts,
        |accounts| {
            let oracle = &mut accounts.oracle;
            oracle.reload()?;
            let long_price = oracle.primary.get(&long_token).unwrap().max.to_unit_price();
            let short_price = oracle
                .primary
                .get(&short_token)
                .unwrap()
                .max
                .to_unit_price();
            msg!(&long_price.to_string());
            msg!(&short_price.to_string());
            let total_supply = accounts.as_market().total_supply();
            msg!(&total_supply.to_string());
            accounts
                .as_market()
                .deposit(1, 0, long_price, short_price)
                .map_err(GmxCoreError::from)?
                .execute()
                .map_err(|err| {
                    msg!(&err.to_string());
                    GmxCoreError::from(err)
                })?;
            Ok(())
        },
    )?;
    Ok(())
}

#[derive(Accounts)]
pub struct ExecuteDeposit<'info> {
    pub authority: Signer<'info>,
    /// CHECK: used and checked by CPI.
    pub only_order_keeper: UncheckedAccount<'info>,
    /// CHECK: used and checked by CPI.
    pub store: UncheckedAccount<'info>,
    pub data_store_program: Program<'info, DataStore>,
    pub oracle_program: Program<'info, Oracle>,
    pub chainlink_program: Program<'info, Chainlink>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    pub oracle: Account<'info, data_store::states::Oracle>,
    /// CHECK: used and checked by CPI.
    #[account(mut)]
    pub deposit: Account<'info, Deposit>,
    #[account(mut, constraint = receiver.key() == deposit.receivers.receiver)]
    pub receiver: Account<'info, TokenAccount>,
    #[account(mut, constraint = market.market_token_mint == market_token_mint.key())]
    pub market: Account<'info, Market>,
    #[account(mut, constraint = market_token_mint.key() == deposit.tokens.market_token)]
    pub market_token_mint: Account<'info, Mint>,
    /// CHECK: only used as signing PDA.
    pub market_sign: UncheckedAccount<'info>,
}

impl<'info> Authentication<'info> for ExecuteDeposit<'info> {
    fn authority(&self) -> &Signer<'info> {
        &self.authority
    }

    fn check_role_ctx(&self) -> CpiContext<'_, '_, '_, 'info, CheckRole<'info>> {
        CpiContext::new(
            self.data_store_program.to_account_info(),
            CheckRole {
                store: self.store.to_account_info(),
                roles: self.only_order_keeper.to_account_info(),
            },
        )
    }

    fn on_error(&self) -> Result<()> {
        Err(error!(ExchangeError::PermissionDenied))
    }
}

impl<'info> WithOracle<'info> for ExecuteDeposit<'info> {
    fn oracle_program(&self) -> AccountInfo<'info> {
        self.oracle_program.to_account_info()
    }

    fn chainlink_program(&self) -> AccountInfo<'info> {
        self.chainlink_program.to_account_info()
    }

    fn oracle(&self) -> AccountInfo<'info> {
        self.oracle.to_account_info()
    }
}

impl<'info> AsMarket<'info> for ExecuteDeposit<'info> {
    fn market(&self) -> &Account<'info, Market> {
        &self.market
    }

    fn market_mut(&mut self) -> &mut Account<'info, Market> {
        &mut self.market
    }

    fn market_token(&self) -> &Account<'info, Mint> {
        &self.market_token_mint
    }

    fn market_sign(&self) -> AccountInfo<'info> {
        self.market_sign.to_account_info()
    }

    fn receiver(&self) -> &Account<'info, TokenAccount> {
        &self.receiver
    }

    fn token_program(&self) -> AccountInfo<'info> {
        self.token_program.to_account_info()
    }
}
