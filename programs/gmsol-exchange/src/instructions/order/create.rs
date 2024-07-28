use std::collections::BTreeSet;

use anchor_lang::{prelude::*, system_program};
use anchor_spl::token::{Token, TokenAccount};
use gmsol_store::{
    cpi::accounts::{GetValidatedMarketMeta, InitializeOrder, MarketTransferIn},
    program::GmsolStore,
    states::{
        common::{SwapParams, TokenRecord},
        order::{OrderKind, OrderParams},
        NonceBytes, Store, TokenMapHeader, TokenMapLoader,
    },
};

use crate::{
    events::OrderCreatedEvent,
    utils::{market::get_and_validate_swap_path, token_records, ControllerSeeds},
    ExchangeError,
};

#[derive(Accounts)]
pub struct CreateOrder<'info> {
    /// CHECK: only used as signing PDA.
    #[account(
        seeds = [
            crate::constants::CONTROLLER_SEED,
            store.key().as_ref(),
        ],
        bump,
    )]
    pub authority: UncheckedAccount<'info>,
    #[account(has_one = token_map)]
    pub store: AccountLoader<'info, Store>,
    #[account(has_one = store)]
    pub token_map: AccountLoader<'info, TokenMapHeader>,
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: only used to invoke CPI and then checked and initilized by it.
    #[account(mut)]
    pub order: UncheckedAccount<'info>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub position: Option<UncheckedAccount<'info>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub market: UncheckedAccount<'info>,
    #[account(mut)]
    pub initial_collateral_token_account: Option<Box<Account<'info, TokenAccount>>>,
    pub final_output_token_account: Option<Box<Account<'info, TokenAccount>>>,
    pub secondary_output_token_account: Option<Box<Account<'info, TokenAccount>>>,
    /// CHECK: check by CPI.
    #[account(mut)]
    pub initial_collateral_token_vault: Option<UncheckedAccount<'info>>,
    pub long_token_account: Box<Account<'info, TokenAccount>>,
    pub short_token_account: Box<Account<'info, TokenAccount>>,
    pub data_store_program: Program<'info, GmsolStore>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

/// Create Order.
pub fn create_order<'info>(
    ctx: Context<'_, '_, 'info, 'info, CreateOrder<'info>>,
    nonce: NonceBytes,
    params: CreateOrderParams,
) -> Result<()> {
    let order = &params.order;
    let store = ctx.accounts.store.key();
    let controller = ControllerSeeds::new(&store, ctx.bumps.authority);

    let (tokens, swap, need_to_transfer_in) = match &order.kind {
        OrderKind::MarketIncrease
        | OrderKind::MarketSwap
        | OrderKind::LimitIncrease
        | OrderKind::LimitSwap => ctx.accounts.handle_tokens_for_increase_or_swap_order(
            &params.output_token,
            ctx.remaining_accounts,
            params.swap_length as usize,
        )?,
        OrderKind::MarketDecrease | OrderKind::LimitDecrease | OrderKind::StopLossDecrease => {
            let (tokens, swap) = ctx.accounts.handle_tokens_for_decrease_order(
                &params.output_token,
                ctx.remaining_accounts,
                params.swap_length as usize,
            )?;
            (tokens, swap, None)
        }
        _ => {
            return err!(ExchangeError::UnsupportedOrderKind);
        }
    };

    if let Some(market) = need_to_transfer_in {
        if order.initial_collateral_delta_amount != 0 {
            gmsol_store::cpi::market_transfer_in(
                ctx.accounts
                    .market_transfer_in_ctx(market)?
                    .with_signer(&[&controller.as_seeds()]),
                order.initial_collateral_delta_amount,
            )?;
        }
    }

    gmsol_store::cpi::initialize_order(
        ctx.accounts
            .initialize_order_ctx()
            .with_signer(&[&controller.as_seeds()]),
        ctx.accounts.payer.key(),
        nonce,
        ctx.accounts.to_tokens_with_feed(tokens)?,
        swap,
        order.clone(),
        params.output_token,
        params.ui_fee_receiver,
    )?;

    require_gte!(
        ctx.accounts.order.lamports() + params.execution_fee,
        super::MAX_ORDER_EXECUTION_FEE,
        ExchangeError::NotEnoughExecutionFee
    );
    if params.execution_fee != 0 {
        system_program::transfer(ctx.accounts.transfer_ctx(), params.execution_fee)?;
    }

    emit!(OrderCreatedEvent {
        ts: Clock::get()?.unix_timestamp,
        store: ctx.accounts.store.key(),
        order: ctx.accounts.order.key(),
        position: ctx.accounts.position.as_ref().map(|a| a.key()),
    });
    Ok(())
}

/// Create Order Params.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateOrderParams {
    /// Order Params.
    pub order: OrderParams,
    /// Swap out token or collateral token.
    pub output_token: Pubkey,
    /// Ui fee receiver.
    pub ui_fee_receiver: Pubkey,
    /// Execution fee.
    pub execution_fee: u64,
    /// Swap path length.
    pub swap_length: u8,
}

impl<'info> CreateOrder<'info> {
    fn initialize_order_ctx(&self) -> CpiContext<'_, '_, '_, 'info, InitializeOrder<'info>> {
        CpiContext::new(
            self.data_store_program.to_account_info(),
            InitializeOrder {
                authority: self.authority.to_account_info(),
                store: self.store.to_account_info(),
                payer: self.payer.to_account_info(),
                order: self.order.to_account_info(),
                position: self.position.as_ref().map(|a| a.to_account_info()),
                market: self.market.to_account_info(),
                initial_collateral_token_account: self
                    .initial_collateral_token_account
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
                long_token_account: self.long_token_account.to_account_info(),
                short_token_account: self.short_token_account.to_account_info(),
                system_program: self.system_program.to_account_info(),
                initial_collateral_token_vault: self
                    .initial_collateral_token_vault
                    .as_ref()
                    .map(|a| a.to_account_info()),
                token_program: self.token_program.to_account_info(),
            },
        )
    }

    fn transfer_ctx(&self) -> CpiContext<'_, '_, '_, 'info, system_program::Transfer<'info>> {
        CpiContext::new(
            self.system_program.to_account_info(),
            system_program::Transfer {
                from: self.payer.to_account_info(),
                to: self.order.to_account_info(),
            },
        )
    }

    fn market_transfer_in_ctx(
        &self,
        market: AccountInfo<'info>,
    ) -> Result<CpiContext<'_, '_, '_, 'info, MarketTransferIn<'info>>> {
        Ok(CpiContext::new(
            self.data_store_program.to_account_info(),
            MarketTransferIn {
                authority: self.authority.to_account_info(),
                store: self.store.to_account_info(),
                from_authority: self.payer.to_account_info(),
                market,
                from: self
                    .initial_collateral_token_account
                    .as_ref()
                    .ok_or(error!(ExchangeError::InvalidArgument))?
                    .to_account_info(),
                vault: self
                    .initial_collateral_token_vault
                    .as_ref()
                    .ok_or(error!(ExchangeError::InvalidArgument))?
                    .to_account_info(),
                token_program: self.token_program.to_account_info(),
            },
        ))
    }

    fn common_tokens(
        &self,
        output_token: &Pubkey,
        include_index_token: bool,
    ) -> Result<BTreeSet<Pubkey>> {
        let mut tokens = BTreeSet::default();
        let ctx = CpiContext::new(
            self.data_store_program.to_account_info(),
            GetValidatedMarketMeta {
                store: self.store.to_account_info(),
                market: self.market.to_account_info(),
            },
        );
        let meta = gmsol_store::cpi::get_validated_market_meta(ctx)?.get();
        tokens.insert(meta.long_token_mint);
        tokens.insert(meta.short_token_mint);
        if include_index_token {
            tokens.insert(meta.index_token_mint);
        }
        if let Some(account) = self.initial_collateral_token_account.as_ref() {
            tokens.insert(account.mint);
        }
        if let Some(account) = self.final_output_token_account.as_ref() {
            tokens.insert(account.mint);
        }
        if let Some(account) = self.secondary_output_token_account.as_ref() {
            require!(
                tokens.contains(&account.mint),
                ExchangeError::InvalidSecondaryOutputToken
            );
        }
        require!(
            tokens.contains(output_token),
            ExchangeError::InvalidOutputToken
        );
        Ok(tokens)
    }

    fn handle_tokens_for_increase_or_swap_order(
        &self,
        output_token: &Pubkey,
        remaining_accounts: &[AccountInfo<'info>],
        length: usize,
    ) -> Result<(BTreeSet<Pubkey>, SwapParams, Option<AccountInfo<'info>>)> {
        let mut tokens = self.common_tokens(output_token, true)?;
        require_gte!(
            remaining_accounts.len(),
            length,
            ExchangeError::NotEnoughRemainingAccounts
        );
        let initial_token = self
            .initial_collateral_token_account
            .as_ref()
            .map(|a| a.mint)
            .ok_or(ExchangeError::MissingTokenAccountForOrder)?;
        let swap_path = get_and_validate_swap_path(
            &self.data_store_program,
            self.store.to_account_info(),
            &remaining_accounts[..length],
            &initial_token,
            output_token,
            &mut tokens,
        )?;
        let transfer_in_market = remaining_accounts
            .first()
            .cloned()
            .unwrap_or_else(|| self.market.to_account_info());
        Ok((
            tokens,
            SwapParams {
                long_token_swap_path: swap_path,
                short_token_swap_path: vec![],
            },
            Some(transfer_in_market),
        ))
    }

    fn handle_tokens_for_decrease_order(
        &self,
        output_token: &Pubkey,
        remaining_accounts: &[AccountInfo<'info>],
        length: usize,
    ) -> Result<(BTreeSet<Pubkey>, SwapParams)> {
        let mut tokens = self.common_tokens(output_token, true)?;
        require_gte!(
            remaining_accounts.len(),
            length,
            ExchangeError::NotEnoughRemainingAccounts
        );
        let final_token = self
            .final_output_token_account
            .as_ref()
            .map(|a| a.mint)
            .ok_or(ExchangeError::MissingTokenAccountForOrder)?;
        let swap_path = get_and_validate_swap_path(
            &self.data_store_program,
            self.store.to_account_info(),
            &remaining_accounts[..length],
            output_token,
            &final_token,
            &mut tokens,
        )?;
        // FIXME: allow swap for secondary output token.
        Ok((
            tokens,
            SwapParams {
                long_token_swap_path: swap_path,
                short_token_swap_path: vec![],
            },
        ))
    }

    fn to_tokens_with_feed(&self, tokens: BTreeSet<Pubkey>) -> Result<Vec<TokenRecord>> {
        let token_map = self.token_map.load_token_map()?;
        token_records(&token_map, &tokens)
    }
}
