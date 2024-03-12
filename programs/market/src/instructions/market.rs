use anchor_lang::prelude::*;
use data_store::{cpi::accounts::InitializeMarket, states::DataStore};
use role_store::{Authorization, Role};

/// Create a new market.
pub fn create_market(
    ctx: Context<CreateMarket>,
    index_token: Pubkey,
    long_token: Pubkey,
    short_token: Pubkey,
    market_token: Pubkey,
) -> Result<()> {
    data_store::cpi::initialize_market(
        ctx.accounts.initialize_market_ctx(),
        index_token,
        long_token,
        short_token,
        market_token,
    )?;
    Ok(())
}

#[derive(Accounts)]
pub struct CreateMarket<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub only_market_keeper: Account<'info, Role>,
    pub data_store: Account<'info, DataStore>,
    /// CHECK: checked by CPI.
    #[account(mut)]
    pub market: UncheckedAccount<'info>,
    pub data_store_program: Program<'info, data_store::program::DataStore>,
    pub system_program: Program<'info, System>,
}

impl<'info> CreateMarket<'info> {
    fn initialize_market_ctx(&self) -> CpiContext<'_, '_, '_, 'info, InitializeMarket<'info>> {
        CpiContext::new(
            self.data_store_program.to_account_info(),
            InitializeMarket {
                authority: self.authority.to_account_info(),
                only_market_keeper: self.only_market_keeper.to_account_info(),
                store: self.data_store.to_account_info(),
                market: self.market.to_account_info(),
                system_program: self.system_program.to_account_info(),
            },
        )
    }
}

impl<'info> Authorization<'info> for CreateMarket<'info> {
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
