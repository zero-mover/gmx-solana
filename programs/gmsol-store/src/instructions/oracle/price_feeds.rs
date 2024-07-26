use std::ops::Deref;

use anchor_lang::prelude::*;

use crate::{
    states::{Oracle, PriceProvider, PriceValidator, Store, TokenMapHeader, TokenMapLoader},
    utils::internal,
};

#[derive(Accounts)]
pub struct SetPricesFromPriceFeed<'info> {
    pub authority: Signer<'info>,
    #[account(has_one = token_map)]
    pub store: AccountLoader<'info, Store>,
    #[account(
        mut,
        has_one = store,
        seeds = [Oracle::SEED, store.key().as_ref(), &[oracle.index]],
        bump = oracle.bump,
    )]
    pub oracle: Account<'info, Oracle>,
    #[account(has_one = store)]
    pub token_map: AccountLoader<'info, TokenMapHeader>,
    pub price_provider: Interface<'info, PriceProvider>,
}

/// Set the oracle prices from price feeds.
pub(crate) fn set_prices_from_price_feed<'info>(
    ctx: Context<'_, '_, 'info, 'info, SetPricesFromPriceFeed<'info>>,
    tokens: Vec<Pubkey>,
) -> Result<()> {
    let validator = PriceValidator::try_from(ctx.accounts.store.load()?.deref())?;
    let token_map = ctx.accounts.token_map.load_token_map()?;
    ctx.accounts.oracle.set_prices_from_remaining_accounts(
        validator,
        &ctx.accounts.price_provider,
        &token_map,
        &tokens,
        ctx.remaining_accounts,
    )
}

impl<'info> internal::Authentication<'info> for SetPricesFromPriceFeed<'info> {
    fn authority(&self) -> &Signer<'info> {
        &self.authority
    }

    fn store(&self) -> &AccountLoader<'info, Store> {
        &self.store
    }
}
