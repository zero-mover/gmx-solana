use anchor_lang::prelude::*;

/// Instructions.
pub mod instructions;

use instructions::*;
use role_store::Authenticate;

declare_id!("AFxZM92h6tryw4hZx2puJRWjA4CQSkxmVkzJrDWJgJAL");

#[program]
pub mod market {
    use super::*;

    #[access_control(Authenticate::only_market_keeper(&ctx))]
    pub fn create_market(ctx: Context<CreateMarket>, index_token_mint: Pubkey) -> Result<()> {
        instructions::create_market(ctx, index_token_mint)
    }
}
