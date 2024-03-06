use anchor_lang::{prelude::*, Bumps};
use gmx_solana_utils::to_seed;

declare_id!("H7L3aYUzR3joc6Heyonjt1thpWdtwYcTVQvCrB2xDsdi");

#[program]
pub mod role_store {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, key: String) -> Result<()> {
        ctx.accounts.store.init(&key, ctx.bumps.store);
        ctx.accounts.role_admin.grant(
            ctx.accounts.store.key(),
            Role::ROLE_ADMIN,
            ctx.bumps.role_admin,
            ctx.accounts.authority.key(),
        )
    }

    pub fn grant_role(ctx: Context<GrantRole>, role_name: String) -> Result<()> {
        ctx.accounts.role.grant(
            ctx.accounts.store.key(),
            &role_name,
            ctx.bumps.role,
            ctx.accounts.role_authority.key(),
        )?;
        if role_name == Role::ROLE_ADMIN {
            ctx.accounts.store.num_admins += 1;
        }
        Ok(())
    }

    pub fn revoke_role(ctx: Context<RevokeRole>) -> Result<()> {
        // require_gt!(
        //     ctx.accounts.store.num_admins,
        //     1,
        //     RoleStoreError::AtLeastOneAdminPerStore
        // );
        if ctx.accounts.role.is_role_admin() {
            ctx.accounts.store.num_admins -= 1;
        }
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(key: String)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + RoleStore::INIT_SPACE,
        seeds = [RoleStore::SEED, &to_seed(&key)],
        bump,
    )]
    pub store: Account<'info, RoleStore>,
    #[account(
        init,
        payer = authority,
        space = 8 + Role::INIT_SPACE,
        seeds = [Role::SEED, store.key().as_ref(), Role::ROLE_ADMIN.as_bytes(), authority.key().as_ref()],
        bump,
    )]
    pub role_admin: Account<'info, Role>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(role_name: String)]
pub struct GrantRole<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut)]
    pub store: Account<'info, RoleStore>,
    #[account(
        has_one = authority @ RoleStoreError::PermissionDenied,
        has_one = store @ RoleStoreError::MismatchedStore,
        constraint = only_role_admin.is_role_admin() @ RoleStoreError::PermissionDenied,
    )]
    pub only_role_admin: Account<'info, Role>,
    /// CHECK: We only use it as a pubkey.
    pub role_authority: UncheckedAccount<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + Role::INIT_SPACE,
        seeds = [Role::SEED, &store.key().to_bytes(), role_name.as_bytes(), role_authority.key().as_ref()],
        bump,
    )]
    pub role: Account<'info, Role>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RevokeRole<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut)]
    pub store: Account<'info, RoleStore>,
    #[account(
        has_one = authority @ RoleStoreError::PermissionDenied,
        has_one = store @ RoleStoreError::MismatchedStore,
        constraint = only_role_admin.is_role_admin() @ RoleStoreError::PermissionDenied,
    )]
    pub only_role_admin: Account<'info, Role>,
    #[account(
        mut,
        has_one = store @ RoleStoreError::MismatchedStore,
        constraint = !role.is_role_admin() || store.num_admins > 1 @ RoleStoreError::AtLeastOneAdminPerStore,
        close = authority,
    )]
    pub role: Account<'info, Role>,
}

#[account]
#[derive(InitSpace)]
pub struct RoleStore {
    #[max_len(32)]
    key: Vec<u8>,
    bump: u8,
    num_admins: u32,
}

impl RoleStore {
    /// Seed.
    pub const SEED: &'static [u8] = b"role_store";

    fn init(&mut self, key: &str, bump: u8) {
        self.key = to_seed(key).into();
        self.bump = bump;
        self.num_admins = 1;
    }
}

/// Maximum length in bytes of the name of role.
pub const MAX_ROLE_LEN: usize = 32;

#[account]
#[derive(InitSpace)]
pub struct Role {
    // Length in bytes <= MAX_ROLE_LEN.
    #[max_len(MAX_ROLE_LEN)]
    role: String,
    bump: u8,
    pub store: Pubkey,
    pub authority: Pubkey,
}

impl Role {
    /// Seed.
    pub const SEED: &'static [u8] = b"role";

    /// The ROLE_ADMIN role.
    pub const ROLE_ADMIN: &'static str = "ROLE_ADMIN";

    /// The CONTROLLER role.
    pub const CONTROLLER: &'static str = "CONTROLLER";

    fn grant(&mut self, store: Pubkey, role: &str, bump: u8, authority: Pubkey) -> Result<()> {
        require!(role.len() <= MAX_ROLE_LEN, RoleStoreError::RoleNameTooLarge);
        self.role = role.to_string();
        self.bump = bump;
        self.store = store;
        self.authority = authority;
        Ok(())
    }

    /// Check if it is a role admin.
    pub fn is_role_admin(&self) -> bool {
        matches!(self.role.as_str(), Self::ROLE_ADMIN)
    }

    /// Check if it is a controller.
    pub fn is_controller(&self) -> bool {
        matches!(self.role.as_str(), Self::CONTROLLER)
    }

    /// Bump.
    pub fn bump(&self) -> u8 {
        self.bump
    }
}

/// Authorization.
pub trait Authorization<'info> {
    /// Get the address of role store.
    fn store(&self) -> &Pubkey;

    /// Get the address of the authority.
    fn authority(&self) -> &Signer<'info>;

    /// Get the role to check.
    fn role(&self) -> &Account<'info, Role>;
}

/// Provides access control methods for [`Authorization`].
pub trait Authenticate<'info>: Authorization<'info> + Bumps + Sized {
    /// Check if the authorization is valid.
    fn valid(ctx: &Context<Self>) -> Result<()> {
        require_eq!(
            ctx.accounts.store().key(),
            ctx.accounts.role().store,
            RoleStoreError::MismatchedStore
        );
        require_eq!(
            *ctx.accounts.authority().key,
            ctx.accounts.role().authority,
            RoleStoreError::Unauthorized
        );
        Ok(())
    }

    /// Check if the authorization is valid and the role matches the given.
    fn only_role(ctx: &Context<Self>, role: &str) -> Result<()> {
        Self::valid(ctx)?;
        require!(role.len() <= MAX_ROLE_LEN, RoleStoreError::RoleNameTooLarge);
        require_eq!(
            role,
            &ctx.accounts.role().role,
            RoleStoreError::PermissionDenied
        );
        Ok(())
    }

    /// Check if the authorization is valid and the role is [`CONTROLLER`](Role::CONTROLLER).
    fn only_controller(ctx: &Context<Self>) -> Result<()> {
        Self::only_role(ctx, Role::CONTROLLER)
    }

    /// Check if the authorization is valid and the role is [`ROLE_ADMIN`](Role::ROLE_ADMIN).
    fn only_role_admin(ctx: &Context<Self>) -> Result<()> {
        Self::only_role(ctx, Role::ROLE_ADMIN)
    }
}

impl<'info, T: Authorization<'info> + Bumps> Authenticate<'info> for T {}

#[error_code]
pub enum RoleStoreError {
    #[msg("The length of the role name is too large")]
    RoleNameTooLarge,
    #[msg("Permission denied")]
    PermissionDenied,
    #[msg("Mismatched store")]
    MismatchedStore,
    #[msg("At least one admin per store")]
    AtLeastOneAdminPerStore,
    #[msg("Unauthorized")]
    Unauthorized,
}
