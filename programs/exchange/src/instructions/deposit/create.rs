use std::collections::BTreeSet;

use anchor_lang::{prelude::*, system_program};
use anchor_spl::token::{Token, TokenAccount};
use data_store::{
    cpi::accounts::{GetTokenConfig, GetValidatedMarketMeta, InitializeDeposit, MarketTransferIn},
    program::DataStore,
    states::{
        common::{SwapParams, TokenRecord},
        deposit::TokenParams,
        NonceBytes,
    },
};

use crate::{
    events::DepositCreatedEvent,
    utils::{market::get_and_validate_swap_path, ControllerSeeds},
    ExchangeError,
};

/// Create Deposit Params.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateDepositParams {
    pub ui_fee_receiver: Pubkey,
    pub execution_fee: u64,
    pub long_token_swap_length: u8,
    pub short_token_swap_length: u8,
    pub initial_long_token_amount: u64,
    pub initial_short_token_amount: u64,
    pub min_market_token: u64,
    pub should_unwrap_native_token: bool,
}

#[derive(Accounts)]
pub struct CreateDeposit<'info> {
    /// CHECK: only used as signing PDA.
    #[account(
        seeds = [
            crate::constants::CONTROLLER_SEED,
            store.key().as_ref(),
        ],
        bump,
    )]
    pub authority: UncheckedAccount<'info>,
    /// CHECK: only used to invoke CPI.
    pub store: UncheckedAccount<'info>,
    /// CHECK: only used to invoke CPI.
    pub only_controller: UncheckedAccount<'info>,
    pub data_store_program: Program<'info, DataStore>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    /// CHECK: only used to invoke CPI which will then initialize the account.
    #[account(mut)]
    pub deposit: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub receiver: Box<Account<'info, TokenAccount>>,
    /// CHECK: check by CPI.
    pub token_config_map: UncheckedAccount<'info>,
    /// CHECK: only used to invoke CPI and should be checked by it.
    pub market: UncheckedAccount<'info>,
    /// CHECK: only used to invoke CPI and should be checked by it.
    #[account(mut)]
    pub initial_long_market: Option<UncheckedAccount<'info>>,
    /// CHECK: only used to invoke CPI and should be checked by it.
    #[account(mut)]
    pub initial_short_market: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    pub initial_long_token_account: Option<Box<Account<'info, TokenAccount>>>,
    #[account(mut)]
    pub initial_short_token_account: Option<Box<Account<'info, TokenAccount>>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub initial_long_token_vault: Option<UncheckedAccount<'info>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub initial_short_token_vault: Option<UncheckedAccount<'info>>,
}

/// Create Deposit.
pub fn create_deposit<'info>(
    ctx: Context<'_, '_, 'info, 'info, CreateDeposit<'info>>,
    nonce: NonceBytes,
    params: CreateDepositParams,
) -> Result<()> {
    use data_store::cpi;

    require!(
        params.initial_long_token_amount != 0 || params.initial_short_token_amount != 0,
        ExchangeError::EmptyDepositAmounts
    );

    let controller = ControllerSeeds::new(ctx.accounts.store.key, ctx.bumps.authority);

    if params.initial_long_token_amount != 0 {
        cpi::market_transfer_in(
            ctx.accounts
                .market_transfer_in_ctx(true)?
                .with_signer(&[&controller.as_seeds()]),
            params.initial_long_token_amount,
        )?;
    }

    if params.initial_short_token_amount != 0 {
        cpi::market_transfer_in(
            ctx.accounts
                .market_transfer_in_ctx(false)?
                .with_signer(&[&controller.as_seeds()]),
            params.initial_short_token_amount,
        )?;
    }

    let mut tokens = BTreeSet::default();
    let initial_long_token_mint = ctx
        .accounts
        .initial_long_token_account
        .as_ref()
        .map(|a| a.mint);
    let initial_short_token_mint = ctx
        .accounts
        .initial_short_token_account
        .as_ref()
        .map(|a| a.mint);
    if let Some(mint) = initial_long_token_mint {
        tokens.insert(mint);
    }
    if let Some(mint) = initial_short_token_mint {
        tokens.insert(mint);
    }

    let market_meta =
        cpi::get_validated_market_meta(ctx.accounts.get_validated_market_meta_ctx())?.get();
    tokens.insert(market_meta.index_token_mint);
    tokens.insert(market_meta.long_token_mint);
    tokens.insert(market_meta.short_token_mint);

    // Handle the swap paths.
    let long_swap_length = params.long_token_swap_length as usize;
    let short_swap_length = params.short_token_swap_length as usize;
    require_gte!(
        ctx.remaining_accounts.len(),
        long_swap_length + short_swap_length,
        ExchangeError::NotEnoughRemainingAccounts,
    );
    let long_token_swap_path = get_and_validate_swap_path(
        &ctx.accounts.data_store_program,
        ctx.accounts.store.to_account_info(),
        &ctx.remaining_accounts[..long_swap_length],
        initial_long_token_mint
            .as_ref()
            .unwrap_or(&market_meta.long_token_mint),
        &market_meta.long_token_mint,
        &mut tokens,
    )?;
    let short_token_swap_path = get_and_validate_swap_path(
        &ctx.accounts.data_store_program,
        ctx.accounts.store.to_account_info(),
        &ctx.remaining_accounts[long_swap_length..(long_swap_length + short_swap_length)],
        initial_short_token_mint
            .as_ref()
            .unwrap_or(&market_meta.short_token_mint),
        &market_meta.short_token_mint,
        &mut tokens,
    )?;

    let tokens_with_feed = tokens
        .into_iter()
        .map(|token| {
            let config = cpi::get_token_config(
                ctx.accounts.get_token_config_ctx(),
                ctx.accounts.store.key(),
                token,
            )?
            .get()
            .ok_or(ExchangeError::ResourceNotFound)?;
            TokenRecord::from_config(token, &config)
        })
        .collect::<Result<Vec<_>>>()?;
    cpi::initialize_deposit(
        ctx.accounts
            .initialize_deposit_ctx()
            .with_signer(&[&controller.as_seeds()]),
        nonce,
        tokens_with_feed,
        SwapParams {
            long_token_swap_path,
            short_token_swap_path,
        },
        TokenParams {
            initial_long_token_amount: params.initial_long_token_amount,
            initial_short_token_amount: params.initial_short_token_amount,
            min_market_tokens: params.min_market_token,
            should_unwrap_native_token: params.should_unwrap_native_token,
        },
        params.ui_fee_receiver,
    )?;
    // FIXME: should we allow using WNT to pay for the execution fee?
    require_gte!(
        ctx.accounts.deposit.lamports() + params.execution_fee,
        super::MAX_DEPOSIT_EXECUTION_FEE,
        ExchangeError::NotEnoughExecutionFee
    );
    if params.execution_fee != 0 {
        system_program::transfer(ctx.accounts.transfer_ctx(), params.execution_fee)?;
    }
    emit!(DepositCreatedEvent {
        ts: Clock::get()?.unix_timestamp,
        store: ctx.accounts.store.key(),
        deposit: ctx.accounts.deposit.key(),
    });
    Ok(())
}

impl<'info> CreateDeposit<'info> {
    fn get_validated_market_meta_ctx(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, GetValidatedMarketMeta<'info>> {
        CpiContext::new(
            self.data_store_program.to_account_info(),
            GetValidatedMarketMeta {
                store: self.store.to_account_info(),
                market: self.market.to_account_info(),
            },
        )
    }

    fn get_token_config_ctx(&self) -> CpiContext<'_, '_, '_, 'info, GetTokenConfig<'info>> {
        CpiContext::new(
            self.data_store_program.to_account_info(),
            GetTokenConfig {
                map: self.token_config_map.to_account_info(),
            },
        )
    }

    fn initialize_deposit_ctx(&self) -> CpiContext<'_, '_, '_, 'info, InitializeDeposit<'info>> {
        CpiContext::new(
            self.data_store_program.to_account_info(),
            InitializeDeposit {
                authority: self.authority.to_account_info(),
                payer: self.payer.to_account_info(),
                only_controller: self.only_controller.to_account_info(),
                store: self.store.to_account_info(),
                deposit: self.deposit.to_account_info(),
                initial_long_token_account: self
                    .initial_long_token_account
                    .as_ref()
                    .map(|a| a.to_account_info()),
                initial_short_token_account: self
                    .initial_short_token_account
                    .as_ref()
                    .map(|a| a.to_account_info()),
                market: self.market.to_account_info(),
                receiver: self.receiver.to_account_info(),
                system_program: self.system_program.to_account_info(),
                token_program: self.token_program.to_account_info(),
            },
        )
    }

    fn transfer_ctx(&self) -> CpiContext<'_, '_, '_, 'info, system_program::Transfer<'info>> {
        CpiContext::new(
            self.system_program.to_account_info(),
            system_program::Transfer {
                from: self.payer.to_account_info(),
                to: self.deposit.to_account_info(),
            },
        )
    }

    fn market_transfer_in_ctx(
        &self,
        is_long: bool,
    ) -> Result<CpiContext<'_, '_, '_, 'info, MarketTransferIn<'info>>> {
        let (market, from, vault) = if is_long {
            (
                self.initial_long_market
                    .as_ref()
                    .ok_or(error!(ExchangeError::InvalidArgument))?
                    .to_account_info(),
                self.initial_long_token_account
                    .as_ref()
                    .ok_or(error!(ExchangeError::InvalidArgument))?
                    .to_account_info(),
                self.initial_long_token_vault
                    .as_ref()
                    .ok_or(error!(ExchangeError::InvalidArgument))?
                    .to_account_info(),
            )
        } else {
            (
                self.initial_short_market
                    .as_ref()
                    .ok_or(error!(ExchangeError::InvalidArgument))?
                    .to_account_info(),
                self.initial_short_token_account
                    .as_ref()
                    .ok_or(error!(ExchangeError::InvalidArgument))?
                    .to_account_info(),
                self.initial_short_token_vault
                    .as_ref()
                    .ok_or(error!(ExchangeError::InvalidArgument))?
                    .to_account_info(),
            )
        };
        Ok(CpiContext::new(
            self.data_store_program.to_account_info(),
            MarketTransferIn {
                authority: self.authority.to_account_info(),
                store: self.store.to_account_info(),
                only_controller: self.only_controller.to_account_info(),
                from_authority: self.payer.to_account_info(),
                market,
                from,
                vault,
                token_program: self.token_program.to_account_info(),
            },
        ))
    }
}
