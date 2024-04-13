use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

use crate::{
    states::{
        common::SwapParams,
        order::{Order, OrderKind, OrderParams, Receivers, Senders, Tokens},
        position::Position,
        DataStore, Market, NonceBytes, Roles, Seed,
    },
    DataStoreError,
};

#[derive(Accounts)]
#[instruction(
    nonce: [u8; 32],
    tokens_with_feed: Vec<(Pubkey, Pubkey)>,
    swap: SwapParams,
    params: OrderParams,
    output_token: Pubkey,
)]
pub struct InitializeOrder<'info> {
    pub authority: Signer<'info>,
    pub store: Account<'info, DataStore>,
    pub only_controller: Account<'info, Roles>,
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        space = 8 + Order::init_space(&tokens_with_feed, &swap),
        payer = payer,
        seeds = [Order::SEED, store.key().as_ref(), payer.key().as_ref(), &nonce],
        bump,
    )]
    pub order: Box<Account<'info, Order>>,
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + Position::INIT_SPACE,
        seeds = [
            Position::SEED,
            store.key().as_ref(),
            payer.key().as_ref(),
            market.meta.market_token_mint.as_ref(),
            output_token.as_ref(),
            &[params.to_position_kind()? as u8],
        ],
        bump,
    )]
    pub position: Option<AccountLoader<'info, Position>>,
    pub market: Box<Account<'info, Market>>,
    #[account(token::authority = payer)]
    pub initial_collateral_token_account: Option<Box<Account<'info, TokenAccount>>>,
    #[account(token::authority = payer)]
    pub final_output_token_account: Option<Box<Account<'info, TokenAccount>>>,
    #[account(token::authority = payer)]
    pub secondary_output_token_account: Option<Box<Account<'info, TokenAccount>>>,
    pub system_program: Program<'info, System>,
}

/// Initialize a new [`Order`] account.
pub fn initialize_order(
    ctx: Context<InitializeOrder>,
    nonce: NonceBytes,
    tokens_with_feed: Vec<(Pubkey, Pubkey)>,
    swap: SwapParams,
    params: OrderParams,
    output_token: Pubkey,
    ui_fee_receiver: Pubkey,
) -> Result<()> {
    let meta = ctx.accounts.market.meta();
    // Validate and create `Tokens`.
    let tokens = match &params.kind {
        OrderKind::MarketSwap => Tokens {
            market_token: meta.market_token_mint,
            initial_collateral_token: ctx.accounts.initial_collateral_token_account()?.mint,
            output_token: ctx.accounts.final_output_token_account()?.mint,
            secondary_output_token: ctx.accounts.final_output_token_account()?.mint,
            final_output_token: None,
        },
        OrderKind::MarketIncrease => {
            ctx.accounts.initialize_position_if_needed(
                &params,
                ctx.bumps.position,
                &output_token,
            )?;
            Tokens {
                market_token: meta.market_token_mint,
                initial_collateral_token: ctx.accounts.initial_collateral_token_account()?.mint,
                output_token,
                secondary_output_token: meta.pnl_token(params.is_long),
                final_output_token: None,
            }
        }
        OrderKind::MarketDecrease | OrderKind::Liquidation => {
            ctx.accounts
                .validate_position(ctx.bumps.position, &output_token)?;
            Tokens {
                market_token: meta.market_token_mint,
                initial_collateral_token: ctx.accounts.initial_collateral_token_account()?.mint,
                output_token,
                secondary_output_token: meta.pnl_token(params.is_long),
                final_output_token: None,
            }
        }
    };
    let (senders, receivers) = match &params.kind {
        OrderKind::MarketSwap => (
            Senders {
                initial_collateral_token_account: Some(
                    ctx.accounts.initial_collateral_token_account()?.key(),
                ),
            },
            Receivers {
                ui_fee: ui_fee_receiver,
                final_output_token_account: Some(ctx.accounts.final_output_token_account()?.key()),
                secondary_output_token_account: None,
            },
        ),
        OrderKind::MarketIncrease => (
            Senders {
                initial_collateral_token_account: Some(
                    ctx.accounts.initial_collateral_token_account()?.key(),
                ),
            },
            Receivers {
                ui_fee: ui_fee_receiver,
                final_output_token_account: None,
                secondary_output_token_account: None,
            },
        ),
        OrderKind::MarketDecrease | OrderKind::Liquidation => (
            Senders {
                initial_collateral_token_account: None,
            },
            Receivers {
                ui_fee: ui_fee_receiver,
                final_output_token_account: Some(ctx.accounts.final_output_token_account()?.key()),
                secondary_output_token_account: Some(
                    ctx.accounts.secondary_output_token_account()?.key(),
                ),
            },
        ),
    };

    ctx.accounts.order.init(
        ctx.bumps.order,
        &nonce,
        &ctx.accounts.market.key(),
        ctx.accounts.payer.key,
        &params,
        &tokens,
        &senders,
        &receivers,
        tokens_with_feed,
        swap,
    )?;
    Ok(())
}

impl<'info> InitializeOrder<'info> {
    fn initial_collateral_token_account(&self) -> Result<&Account<'info, TokenAccount>> {
        let account = self
            .initial_collateral_token_account
            .as_ref()
            .ok_or(DataStoreError::MissingSender)?;
        Ok(account)
    }

    fn final_output_token_account(&self) -> Result<&Account<'info, TokenAccount>> {
        let account = self
            .final_output_token_account
            .as_ref()
            .ok_or(DataStoreError::MissingSender)?;
        Ok(account)
    }

    fn secondary_output_token_account(&self) -> Result<&Account<'info, TokenAccount>> {
        let account = self
            .secondary_output_token_account
            .as_ref()
            .ok_or(DataStoreError::MissingSender)?;
        Ok(account)
    }

    fn initialize_position_if_needed(
        &self,
        params: &OrderParams,
        bump: u8,
        output_token: &Pubkey,
    ) -> Result<()> {
        self.market.meta().validate_collateral_token(output_token)?;
        let Some(position) = self.position.as_ref() else {
            return err!(DataStoreError::PositionIsNotProvided);
        };
        let maybe_initialized = match position.load_init() {
            Ok(mut position) => {
                position.try_init(
                    params.to_position_kind()?,
                    bump,
                    self.payer.key,
                    &self.market.meta.market_token_mint,
                    output_token,
                )?;
                false
            }
            Err(Error::AnchorError(err)) => {
                if err.error_code_number == ErrorCode::AccountDiscriminatorAlreadySet as u32 {
                    true
                } else {
                    return Err(Error::AnchorError(err));
                }
            }
            Err(err) => {
                return Err(err);
            }
        };
        if maybe_initialized {
            // We need to validate the position if it has been initialized.
            self.validate_position(bump, output_token)?;
        }
        Ok(())
    }

    /// Validate the position to make sure it is initialized and valid.
    fn validate_position(&self, bump: u8, output_token: &Pubkey) -> Result<()> {
        self.market.meta().validate_collateral_token(output_token)?;
        let Some(position) = self.position.as_ref() else {
            return err!(DataStoreError::PositionIsNotProvided);
        };
        let position = position.load()?;
        require_eq!(position.bump, bump, DataStoreError::InvalidPosition);
        Ok(())
    }
}
