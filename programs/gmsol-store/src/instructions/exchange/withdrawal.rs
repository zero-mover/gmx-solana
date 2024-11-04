use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer_checked, Mint, Token, TokenAccount, TransferChecked},
};
use gmsol_utils::InitSpace;

use crate::{
    events::WithdrawalCreated,
    ops::withdrawal::{CreateWithdrawalOperation, CreateWithdrawalParams},
    states::{
        common::action::ActionExt, withdrawal::Withdrawal, Market, NonceBytes, RoleKey, Seed, Store,
    },
    utils::{internal, token::is_associated_token_account},
    CoreError,
};

/// The accounts definition for the `create_withdrawal` instruction.
///
/// Remaining accounts expected by this instruction:
///
///   - 0..M. `[]` M market accounts, where M represents the length
///     of the swap path for final long token.
///   - M..M+N. `[]` N market accounts, where N represents the length
///     of the swap path for final short token.
#[derive(Accounts)]
#[instruction(nonce: [u8; 32])]
pub struct CreateWithdrawal<'info> {
    /// The owner.
    #[account(mut)]
    pub owner: Signer<'info>,
    /// Store.
    pub store: AccountLoader<'info, Store>,
    /// Market.
    #[account(mut, has_one = store)]
    pub market: AccountLoader<'info, Market>,
    /// The withdrawal to be created.
    #[account(
        init,
        space = 8 + Withdrawal::INIT_SPACE,
        payer = owner,
        seeds = [Withdrawal::SEED, store.key().as_ref(), owner.key().as_ref(), &nonce],
        bump,
    )]
    pub withdrawal: AccountLoader<'info, Withdrawal>,
    /// Market token.
    #[account(constraint = market.load()?.meta().market_token_mint == market_token.key() @ CoreError::MarketTokenMintMismatched)]
    pub market_token: Box<Account<'info, Mint>>,
    /// Final long token.
    pub final_long_token: Box<Account<'info, Mint>>,
    /// Final short token.
    pub final_short_token: Box<Account<'info, Mint>>,
    /// The escrow account for receving market tokens to burn.
    #[account(
        mut,
        associated_token::mint = market_token,
        associated_token::authority = withdrawal,
    )]
    pub market_token_escrow: Box<Account<'info, TokenAccount>>,
    /// The escrow account for receiving withdrawed final long tokens.
    #[account(
        mut,
        associated_token::mint = final_long_token,
        associated_token::authority = withdrawal,
    )]
    pub final_long_token_escrow: Box<Account<'info, TokenAccount>>,
    /// The escrow account for receiving withdrawed final short tokens.
    #[account(
        mut,
        associated_token::mint = final_short_token,
        associated_token::authority = withdrawal,
    )]
    pub final_short_token_escrow: Box<Account<'info, TokenAccount>>,
    /// The source market token account.
    #[account(
        mut,
        token::mint = market_token,
    )]
    pub market_token_source: Box<Account<'info, TokenAccount>>,
    /// The ATA for receiving the final long tokens.
    #[account(
        associated_token::mint = final_long_token,
        associated_token::authority = owner,
    )]
    pub final_long_token_ata: Box<Account<'info, TokenAccount>>,
    /// The ATA for receiving the final short tokens.
    #[account(
        associated_token::mint = final_short_token,
        associated_token::authority = owner,
    )]
    pub final_short_token_ata: Box<Account<'info, TokenAccount>>,
    /// The system program.
    pub system_program: Program<'info, System>,
    /// The token program.
    pub token_program: Program<'info, Token>,
    /// The associated token program.
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> internal::Create<'info, Withdrawal> for CreateWithdrawal<'info> {
    type CreateParams = CreateWithdrawalParams;

    fn action(&self) -> AccountInfo<'info> {
        self.withdrawal.to_account_info()
    }

    fn payer(&self) -> AccountInfo<'info> {
        self.owner.to_account_info()
    }

    fn system_program(&self) -> AccountInfo<'info> {
        self.system_program.to_account_info()
    }

    fn create_impl(
        &mut self,
        params: &Self::CreateParams,
        nonce: &NonceBytes,
        bumps: &Self::Bumps,
        remaining_accounts: &'info [AccountInfo<'info>],
    ) -> Result<()> {
        self.transfer_tokens(params)?;
        CreateWithdrawalOperation::builder()
            .withdrawal(self.withdrawal.clone())
            .market(self.market.clone())
            .store(self.store.clone())
            .owner(&self.owner)
            .nonce(nonce)
            .bump(bumps.withdrawal)
            .final_long_token(&self.final_long_token_escrow)
            .final_short_token(&self.final_short_token_escrow)
            .market_token(&self.market_token_escrow)
            .params(params)
            .swap_paths(remaining_accounts)
            .build()
            .execute()?;
        emit!(WithdrawalCreated::new(
            self.store.key(),
            self.withdrawal.key(),
        )?);
        Ok(())
    }
}

impl<'info> CreateWithdrawal<'info> {
    fn transfer_tokens(&mut self, params: &CreateWithdrawalParams) -> Result<()> {
        let amount = params.market_token_amount;
        let source = &self.market_token_source;
        let target = &mut self.market_token_escrow;
        let mint = &self.market_token;
        if amount != 0 {
            transfer_checked(
                CpiContext::new(
                    self.token_program.to_account_info(),
                    TransferChecked {
                        from: source.to_account_info(),
                        mint: mint.to_account_info(),
                        to: target.to_account_info(),
                        authority: self.owner.to_account_info(),
                    },
                ),
                amount,
                mint.decimals,
            )?;
            target.reload()?;
        }
        Ok(())
    }
}

/// The accounts definition for the `close_withdrawal` instruction.
#[event_cpi]
#[derive(Accounts)]
pub struct CloseWithdrawal<'info> {
    /// The executor of this instruction.
    pub executor: Signer<'info>,
    /// The store.
    pub store: AccountLoader<'info, Store>,
    /// The owner of the withdrawal.
    /// CHECK: only use to validate and receive fund.
    #[account(mut)]
    pub owner: UncheckedAccount<'info>,
    /// Market token.
    #[account(
        constraint = withdrawal.load()?.tokens.market_token() == market_token.key() @ CoreError::MarketTokenMintMismatched
    )]
    pub market_token: Box<Account<'info, Mint>>,
    /// Final long token.
    #[account(constraint = withdrawal.load()?.tokens.final_long_token() == final_long_token.key() @ CoreError::TokenMintMismatched)]
    pub final_long_token: Box<Account<'info, Mint>>,
    /// Final short token.
    #[account(constraint = withdrawal.load()?.tokens.final_long_token() == final_long_token.key() @ CoreError::TokenMintMismatched)]
    pub final_short_token: Box<Account<'info, Mint>>,
    /// The withdrawal to close.
    #[account(
        mut,
        constraint = withdrawal.load()?.header.owner == owner.key() @ CoreError::OwnerMismatched,
        constraint = withdrawal.load()?.header.store == store.key() @ CoreError::StoreMismatched,
        constraint = withdrawal.load()?.tokens.market_token_account() == market_token_escrow.key() @ CoreError::MarketTokenAccountMismatched,
        constraint = withdrawal.load()?.tokens.final_long_token_account() == final_long_token_escrow.key() @ CoreError::MarketTokenAccountMismatched,
        constraint = withdrawal.load()?.tokens.final_short_token_account() == final_short_token_escrow.key() @ CoreError::MarketTokenAccountMismatched,
    )]
    pub withdrawal: AccountLoader<'info, Withdrawal>,
    /// The escrow account for receving market tokens to burn.
    #[account(
        mut,
        associated_token::mint = market_token,
        associated_token::authority = withdrawal,
    )]
    pub market_token_escrow: Box<Account<'info, TokenAccount>>,
    /// The escrow account for receiving withdrawed final long tokens.
    #[account(
        mut,
        associated_token::mint = final_long_token,
        associated_token::authority = withdrawal,
    )]
    pub final_long_token_escrow: Box<Account<'info, TokenAccount>>,
    /// The escrow account for receiving withdrawed final short tokens.
    #[account(
        mut,
        associated_token::mint = final_short_token,
        associated_token::authority = withdrawal,
    )]
    pub final_short_token_escrow: Box<Account<'info, TokenAccount>>,
    /// The ATA for market token of owner.
    /// CHECK: should be checked during the execution.
    #[account(
        mut,
        constraint = is_associated_token_account(market_token_ata.key, owner.key, &market_token.key()) @ CoreError::NotAnATA,
    )]
    pub market_token_ata: UncheckedAccount<'info>,
    /// The ATA for final long token of owner.
    /// CHECK: should be checked during the execution
    #[account(
        mut,
        constraint = is_associated_token_account(final_long_token_ata.key, owner.key, &final_long_token.key()) @ CoreError::NotAnATA,
    )]
    pub final_long_token_ata: UncheckedAccount<'info>,
    /// The ATA for final short token of owner.
    /// CHECK: should be checked during the execution
    #[account(
        mut,
        constraint = is_associated_token_account(final_short_token_ata.key, owner.key, &final_short_token.key()) @ CoreError::NotAnATA,
    )]
    pub final_short_token_ata: UncheckedAccount<'info>,
    /// The system program.
    pub system_program: Program<'info, System>,
    /// The token program.
    pub token_program: Program<'info, Token>,
    /// The associated token program.
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> internal::Authentication<'info> for CloseWithdrawal<'info> {
    fn authority(&self) -> &Signer<'info> {
        &self.executor
    }

    fn store(&self) -> &AccountLoader<'info, Store> {
        &self.store
    }
}

impl<'info> internal::Close<'info, Withdrawal> for CloseWithdrawal<'info> {
    fn expected_keeper_role(&self) -> &str {
        RoleKey::ORDER_KEEPER
    }

    fn fund_receiver(&self) -> AccountInfo<'info> {
        self.owner.to_account_info()
    }

    fn process(&self, init_if_needed: bool) -> Result<internal::Success> {
        use crate::utils::token::TransferAllFromEscrowToATA;

        let signer = self.withdrawal.load()?.signer();
        let seeds = signer.as_seeds();

        let builder = TransferAllFromEscrowToATA::builder()
            .system_program(self.system_program.to_account_info())
            .token_program(self.token_program.to_account_info())
            .associated_token_program(self.associated_token_program.to_account_info())
            .payer(self.executor.to_account_info())
            .owner(self.owner.to_account_info())
            .escrow_authority(self.withdrawal.to_account_info())
            .seeds(&seeds)
            .init_if_needed(init_if_needed);

        // Transfer market tokens.
        if !builder
            .clone()
            .mint(self.market_token.to_account_info())
            .decimals(self.market_token.decimals)
            .ata(self.market_token_ata.to_account_info())
            .escrow(self.market_token_escrow.to_account_info())
            .build()
            .execute()?
        {
            return Ok(false);
        }

        // Transfer final long tokens.
        if !builder
            .clone()
            .mint(self.final_long_token.to_account_info())
            .decimals(self.final_long_token.decimals)
            .ata(self.final_long_token_ata.to_account_info())
            .escrow(self.final_long_token_escrow.to_account_info())
            .build()
            .execute()?
        {
            return Ok(false);
        }

        if self.final_long_token_escrow.key() != self.final_short_token_escrow.key() {
            // Transfer final short tokens.
            if !builder
                .clone()
                .mint(self.final_short_token.to_account_info())
                .decimals(self.final_short_token.decimals)
                .ata(self.final_short_token_ata.to_account_info())
                .escrow(self.final_short_token_escrow.to_account_info())
                .build()
                .execute()?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn event_authority(&self, bumps: &Self::Bumps) -> (AccountInfo<'info>, u8) {
        (
            self.event_authority.to_account_info(),
            bumps.event_authority,
        )
    }

    fn action(&self) -> &AccountLoader<'info, Withdrawal> {
        &self.withdrawal
    }
}
