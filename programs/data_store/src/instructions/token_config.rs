use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::{
    states::{DataStore, Roles, Seed, TokenConfig, TokenConfigMap},
    utils::internal,
};

#[derive(Accounts)]
#[instruction(len: u16)]
pub struct InitializeTokenConfigMap<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub only_controller: Account<'info, Roles>,
    pub store: Account<'info, DataStore>,
    #[account(
        init,
        space = 8 + TokenConfigMap::init_space(len as usize),
        payer = authority,
        seeds = [TokenConfigMap::SEED, store.key().as_ref()],
        bump,
    )]
    pub map: Account<'info, TokenConfigMap>,
    pub system_program: Program<'info, System>,
}

/// Initialize token config map with the given length.
pub fn initialize_token_config_map(
    ctx: Context<InitializeTokenConfigMap>,
    _len: u16,
) -> Result<()> {
    ctx.accounts.map.init(ctx.bumps.map);
    Ok(())
}

impl<'info> internal::Authentication<'info> for InitializeTokenConfigMap<'info> {
    fn authority(&self) -> &Signer<'info> {
        &self.authority
    }

    fn store(&self) -> &Account<'info, DataStore> {
        &self.store
    }

    fn roles(&self) -> &Account<'info, Roles> {
        &self.only_controller
    }
}

#[derive(Accounts)]
pub struct InsertTokenConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub only_controller: Account<'info, Roles>,
    pub store: Account<'info, DataStore>,
    #[account(
        mut,
        seeds = [TokenConfigMap::SEED, store.key().as_ref()],
        bump = map.bump,
        realloc = 8 + TokenConfigMap::init_space(map.length_after_insert(&token.key())),
        realloc::zero = false,
        realloc::payer = authority,
    )]
    pub map: Account<'info, TokenConfigMap>,
    pub token: Account<'info, Mint>,
    pub system_program: Program<'info, System>,
}

/// Insert or update the config of the given token.
pub fn insert_token_config(
    ctx: Context<InsertTokenConfig>,
    price_feed: Pubkey,
    heartbeat_duration: u32,
    precision: u8,
    enable: bool,
) -> Result<()> {
    let token = &ctx.accounts.token;
    ctx.accounts.map.checked_insert(
        token.key(),
        TokenConfig {
            enabled: enable,
            synthetic: false,
            price_feed,
            heartbeat_duration,
            precision,
            token_decimals: token.decimals,
        },
    )
}

impl<'info> internal::Authentication<'info> for InsertTokenConfig<'info> {
    fn authority(&self) -> &Signer<'info> {
        &self.authority
    }

    fn store(&self) -> &Account<'info, DataStore> {
        &self.store
    }

    fn roles(&self) -> &Account<'info, Roles> {
        &self.only_controller
    }
}

#[derive(Accounts)]
#[instruction(token: Pubkey)]
pub struct InsertSyntheticTokenConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub only_controller: Account<'info, Roles>,
    pub store: Account<'info, DataStore>,
    #[account(
        mut,
        seeds = [TokenConfigMap::SEED, store.key().as_ref()],
        bump = map.bump,
        realloc = 8 + TokenConfigMap::init_space(map.length_after_insert(&token)),
        realloc::zero = false,
        realloc::payer = authority,
    )]
    pub map: Account<'info, TokenConfigMap>,
    pub system_program: Program<'info, System>,
}

/// Insert or update the config of the given synthetic token.
pub fn insert_synthetic_token_config(
    ctx: Context<InsertSyntheticTokenConfig>,
    token: Pubkey,
    decimals: u8,
    price_feed: Pubkey,
    heartbeat_duration: u32,
    precision: u8,
    enable: bool,
) -> Result<()> {
    ctx.accounts.map.checked_insert(
        token.key(),
        TokenConfig {
            enabled: enable,
            synthetic: true,
            price_feed,
            heartbeat_duration,
            precision,
            token_decimals: decimals,
        },
    )
}

impl<'info> internal::Authentication<'info> for InsertSyntheticTokenConfig<'info> {
    fn authority(&self) -> &Signer<'info> {
        &self.authority
    }

    fn store(&self) -> &Account<'info, DataStore> {
        &self.store
    }

    fn roles(&self) -> &Account<'info, Roles> {
        &self.only_controller
    }
}

#[derive(Accounts)]
pub struct ToggleTokenConfig<'info> {
    pub authority: Signer<'info>,
    pub store: Account<'info, DataStore>,
    pub only_controller: Account<'info, Roles>,
    #[account(
        mut,
        seeds = [TokenConfigMap::SEED, store.key().as_ref()],
        bump = map.bump,
    )]
    pub map: Account<'info, TokenConfigMap>,
}

/// Toggle the config for the given token.
pub fn toggle_token_config(
    ctx: Context<ToggleTokenConfig>,
    token: Pubkey,
    enable: bool,
) -> Result<()> {
    ctx.accounts.map.toggle_token_config(&token, enable)
}

impl<'info> internal::Authentication<'info> for ToggleTokenConfig<'info> {
    fn authority(&self) -> &Signer<'info> {
        &self.authority
    }

    fn store(&self) -> &Account<'info, DataStore> {
        &self.store
    }

    fn roles(&self) -> &Account<'info, Roles> {
        &self.only_controller
    }
}

#[derive(Accounts)]
#[instruction(store: Pubkey)]
pub struct GetTokenConfig<'info> {
    #[account(
        seeds = [TokenConfigMap::SEED, store.as_ref()],
        bump = map.bump,
    )]
    pub map: Account<'info, TokenConfigMap>,
}

/// Get token config of the given token.
pub fn get_token_config(
    ctx: Context<GetTokenConfig>,
    _store: Pubkey,
    token: Pubkey,
) -> Result<Option<TokenConfig>> {
    Ok(ctx.accounts.map.as_map().get(&token).cloned())
}

#[derive(Accounts)]
#[instruction(len: u16)]
pub struct ExtendTokenConfigMap<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub only_controller: Account<'info, Roles>,
    pub store: Account<'info, DataStore>,
    #[account(
        mut,
        seeds = [TokenConfigMap::SEED, store.key().as_ref()],
        bump = map.bump,
        realloc = 8 + TokenConfigMap::init_space(map.as_map().len() + len as usize),
        realloc::payer = authority,
        realloc::zero = false,
    )]
    pub map: Account<'info, TokenConfigMap>,
    pub system_program: Program<'info, System>,
}

/// Extend the length of the map with the given length.
pub fn extend_token_config_map(_ctx: Context<ExtendTokenConfigMap>, _len: u16) -> Result<()> {
    Ok(())
}

impl<'info> internal::Authentication<'info> for ExtendTokenConfigMap<'info> {
    fn authority(&self) -> &Signer<'info> {
        &self.authority
    }

    fn store(&self) -> &Account<'info, DataStore> {
        &self.store
    }

    fn roles(&self) -> &Account<'info, Roles> {
        &self.only_controller
    }
}
