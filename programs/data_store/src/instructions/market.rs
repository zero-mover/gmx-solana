use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, MintTo, Token, TokenAccount};
use role_store::{Authorization, Role};

use crate::constants;
use crate::states::{Action, Data, DataStore, Market, MarketChangeEvent};

/// Initialize the account for [`Market`].
pub fn initialize_market(
    ctx: Context<InitializeMarket>,
    market_token: Pubkey,
    index_token: Pubkey,
    long_token: Pubkey,
    short_token: Pubkey,
) -> Result<()> {
    let market = &mut ctx.accounts.market;
    market.bump = ctx.bumps.market;
    market.index_token = index_token;
    market.long_token = long_token;
    market.short_token = short_token;
    market.market_token = market_token;
    emit!(MarketChangeEvent {
        address: market.key(),
        action: Action::Init,
        market: (**market).clone(),
    });
    Ok(())
}

#[derive(Accounts)]
#[instruction(market_token: Pubkey)]
pub struct InitializeMarket<'info> {
    #[account(mut)]
    authority: Signer<'info>,
    only_market_keeper: Account<'info, Role>,
    store: Account<'info, DataStore>,
    #[account(
        init,
        payer = authority,
        space = 8 + Market::INIT_SPACE,
        seeds = [
            Market::SEED,
            store.key().as_ref(),
            &Market::create_key_seed(&market_token),
        ],
        bump,
    )]
    market: Account<'info, Market>,
    system_program: Program<'info, System>,
}

impl<'info> Authorization<'info> for InitializeMarket<'info> {
    fn role_store(&self) -> Pubkey {
        self.store.role_store
    }

    fn authority(&self) -> &Signer<'info> {
        &self.authority
    }

    fn role(&self) -> &Account<'info, Role> {
        &self.only_market_keeper
    }
}

/// Remove market.
pub fn remove_market(ctx: Context<RemoveMarket>) -> Result<()> {
    let market = &mut ctx.accounts.market;
    emit!(MarketChangeEvent {
        address: market.key(),
        action: Action::Remove,
        market: (**market).clone(),
    });
    Ok(())
}

#[derive(Accounts)]
pub struct RemoveMarket<'info> {
    #[account(mut)]
    authority: Signer<'info>,
    only_market_keeper: Account<'info, Role>,
    store: Account<'info, DataStore>,
    #[account(
        mut,
        seeds = [Market::SEED, store.key().as_ref(), &market.expected_key_seed()],
        bump = market.bump,
        close = authority,
    )]
    market: Account<'info, Market>,
}

impl<'info> Authorization<'info> for RemoveMarket<'info> {
    fn role_store(&self) -> Pubkey {
        self.store.role_store
    }

    fn authority(&self) -> &Signer<'info> {
        &self.authority
    }

    fn role(&self) -> &Account<'info, Role> {
        &self.only_market_keeper
    }
}

/// Initialize a new market token.
#[allow(unused_variables)]
pub fn initialize_market_token(
    ctx: Context<InitializeMarketToken>,
    index_token_mint: Pubkey,
    long_token_mint: Pubkey,
    short_token_mint: Pubkey,
) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
#[instruction(index_token_mint: Pubkey, long_token_mint: Pubkey, short_token_mint: Pubkey)]
pub struct InitializeMarketToken<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub only_market_keeper: Account<'info, Role>,
    pub data_store: Account<'info, DataStore>,
    #[account(
        init,
        payer = authority,
        mint::decimals = constants::MARKET_TOKEN_DECIMALS,
        mint::authority = market_sign,
        seeds = [
            constants::MAREKT_TOKEN_MINT_SEED,
            data_store.key().as_ref(),
            index_token_mint.as_ref(),
            long_token_mint.key().as_ref(),
            short_token_mint.key().as_ref(),
        ],
        bump,
    )]
    pub market_token_mint: Account<'info, Mint>,
    /// CHECK: only used as a signing PDA.
    #[account(seeds = [constants::MARKET_SIGN_SEED], bump)]
    pub market_sign: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

impl<'info> Authorization<'info> for InitializeMarketToken<'info> {
    fn role_store(&self) -> Pubkey {
        self.data_store.role_store
    }

    fn authority(&self) -> &Signer<'info> {
        &self.authority
    }

    fn role(&self) -> &Account<'info, Role> {
        &self.only_market_keeper
    }
}

/// Mint the given amount of market tokens to the destination account.
pub fn mint_market_token_to(ctx: Context<MintMarketTokenTo>, amount: u64) -> Result<()> {
    anchor_spl::token::mint_to(
        ctx.accounts
            .mint_to_ctx()
            .with_signer(&[&[&[ctx.bumps.market_sign]]]),
        amount,
    )
}

#[derive(Accounts)]
pub struct MintMarketTokenTo<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub only_market_keeper: Account<'info, Role>,
    pub data_store: Account<'info, DataStore>,
    // We don't have to check the mint is really a market token,
    // since the owner must be derived from `MARKET_SIGN`.
    pub market_token_mint: Account<'info, Mint>,
    /// CHECK: only used as a signing PDA.
    #[account(seeds = [constants::MARKET_SIGN_SEED], bump)]
    pub market_sign: UncheckedAccount<'info>,
    #[account(token::mint = market_token_mint)]
    pub to: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

impl<'info> Authorization<'info> for MintMarketTokenTo<'info> {
    fn role_store(&self) -> Pubkey {
        self.data_store.role_store
    }

    fn authority(&self) -> &Signer<'info> {
        &self.authority
    }

    fn role(&self) -> &Account<'info, Role> {
        &self.only_market_keeper
    }
}

impl<'info> MintMarketTokenTo<'info> {
    fn mint_to_ctx(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            MintTo {
                mint: self.market_token_mint.to_account_info(),
                to: self.to.to_account_info(),
                authority: self.market_sign.to_account_info(),
            },
        )
    }
}

/// Initialize a vault of the given token for a market.
/// The address is derived from token mint addresses (the `market_token_mint` seed is optional).
#[allow(unused_variables)]
pub fn initialize_vault(
    ctx: Context<InitializeVault>,
    market_token_mint: Option<Pubkey>,
) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
#[instruction(market_token_mint: Option<Pubkey>)]
pub struct InitializeVault<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub only_market_keeper: Account<'info, Role>,
    pub data_store: Account<'info, DataStore>,
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        payer = authority,
        token::mint = mint,
        token::authority = market_sign,
        seeds = [
            constants::MARKET_VAULT_SEED,
            mint.key().as_ref(),
            market_token_mint.as_ref().map(|key| key.as_ref()).unwrap_or_default(),
        ],
        bump,
    )]
    pub vault: Account<'info, TokenAccount>,
    /// CHECK: only used as a signing PDA.
    #[account(seeds = [constants::MARKET_SIGN_SEED], bump)]
    pub market_sign: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

impl<'info> Authorization<'info> for InitializeVault<'info> {
    fn role_store(&self) -> Pubkey {
        self.data_store.role_store
    }

    fn authority(&self) -> &Signer<'info> {
        &self.authority
    }

    fn role(&self) -> &Account<'info, Role> {
        &self.only_market_keeper
    }
}
