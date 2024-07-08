//! # The GMSOL Store Program
//!
//! The GMSOL Store Program is a Solana Program developed using [Anchor](anchor_lang).
//! It defines the data structure of the main Accounts for the GMSOL
//! and provides a role-based permission management framwork based on the [`Store`](states::Store) Account.
//!
//! It mainly consists of the following components:
//! - Core data strcutures defined in the [`states`] module.
//! - Actual implementation of instructions defined in the [`instructions`] module.
//! - Events generated during the execution of instructions defined in [`events`] module.
//! - Constants such as default market parameters found in the [`constants`] module.
//! - Various helper functions and implementations defined in the [`utils`] module.
//!   Notably, if external Programs wish to use the permission management feature provided by the Store Program,
//!   the relevant trait definitions (like [`Authentication`](utils::Authentication)) can be found in the [`utils`] module.
//! - The instructions generated by the [#\[program\]](macro@program) macro is defined in [`gmsol_store`].
//!
//! ## Overall Design
//! GMSOL primarily consists of the *Store Program*, *Exchange Program*, and off-chain *Keepers*.
//! The *Store Program* (which is defined in this crate) provides management instrcutions for core
//! data accounts and features a role-based permission system to management access to these data accounts.
//! We strive to keep the instructions in the Store Program as simple and straightforward as possible.
//! For example, although the creation of a [`Market`](states::Market) account, the creation
//! of the correspoding market token, and the creation of market vaults to be used typically need to
//! be used together to be meaningful, they are defined separately in the Store Program, with
//! independent precondition and permission validations. The actual market creation instruction,
//! which includes the invocation of the aforementioned instructions, is defined in the
//! Exchange Program. This design allows us to update the logic of specific operations by updating
//! the Exchange Program without needing to modify the Store Program.
//!
//! However, for the execution instructions of actions (such as the execution of orders), which often
//! require significant computational resources, we need to optimize their implementation carefully.
//! Particularly, we need to avoid the additional overhead caused by CPI (Cross-Program Invocation).
//! Therefore, their core parts are implemented directly in the Store Program
//! ([`exchange`]).
//!
//! Next we will introduce the data accounts defined in the Store Program and the instrucionts for
//! operating them one by one.
//!
//! ### Store Account
//! A [`Store`](states::Store) Account serves as both an authority and a global configuration
//! storage.
//!
//! #### Store Account as an Authority
//! The address of a Store Account is a PDA (Program Derived Address), which can be used to sign
//! instructions during CPI calls (see [cpi-with-pda-signer] for details). This allows the Store
//! Account to be used as an authority for extenral resources. For example, the Store Account is
//! the owner of the Market Vault (an SPL Token Account) and the mint authority for the Market Token
//! (an SPL Mint Account). When executing transfers in and out of the Market Vault, the Store Account
//! is used as the Signer for the corresponding CPI to authorize the opeartion.
//!
//! Besides serving as an authority for external resources, the Store Account is also the authority
//! for internal resources within GMSOL. Most Data Accounts in the Store Program have a `store` field
//! indicating *they are managed by this Store Account*. Additionally, the Store Account maintains a
//! permissions table that records all addresses with permissions in this store and the permissions
//! they hold.
//!
//! Only addresses with the required permissions in this store are allowed to modify the Data Accounts
//! under its management.
//!
//! More specifically, most instructions in the Store Program require a Signer and a Store Account in
//! addition to the Data Account being operated on. Before executing the operation, the instruction
//! verifies:
//! 1. The Data Account is managed by the Store (by checking if the `store` field of the Data Account
//! matches the address of the Store Account).
//! 2. The Signer has the required permissions in the given Store.
//!
//! For example, there is an instruction ([`PushToTokenMap`]) in the Store Program to add a new token
//! config to a token map. This instruction requires an `authority` (a signer, which can be considered
//! the initiator of the instruction), a store account, and the token map to which the new token config
//! will be added. The instruction verifies that the `store` field of the token map matches the provided
//! store account and checks the permissions of the `authority` has the
//! [`MARKET_KEEPER`](states::RoleKey::MARKET_KEEPER) permission in this store.
//!
//! Currently, the Store Program allows the creation of a Store Account permissionlessly. In the GMSOL
//! system, we refer to a Store Account and the accounts it manages as a *deployment* of GMSOL. A Store
//! Account corresponds to one deployment, and different deployments are independent of each other,
//! with completely isolated permission settings.
//!
//! #### Store Account as Global Configuration Storage
//! Since the Store Account is involved in almost all Store Program instructions, it is very suitable for
//! storing global configrations. Currently, we store the following configurations in the Store Account:
//! 1. The authority (admin) of the store and the permissions table for other addresses.
//! 2. The address of the token map used in this deployment.
//! 3. Treasury configuration.
//! 4. Verification configuration for oracles.
//! 5. Other global configurations.
//!
//! #### Store Account Address Derivation Rules
//! The address of the Store Account is a PDA. Sepecifically, the Store Account address is derived using
//! the following [seeds]:
//! 1. A constant [`Store::SEED`](states::Store).
//! 2. A hashed key string, hashed to 32 bytes.
//! This means that we can generate a Store Account address from any key string (with a length not
//! exceeding [`MAX_LEN`](states::Store::MAX_LEN)). However, this is not unique, as multiple key strings
//! may generate the same Store Account address. The store derived from an empty key string is referred to
//! as the default store, which si typically controlled by the GMSOL Program deployer.
//!
//! Given the possibility of multiple key strings deriving the same Store Account address, to ensure that
//! a store corresponds to a signle key string, the key string used during store creation is saved in the
//! account in its original form. Thus, regardless of how many key strings correspond to the same store,
//! only the key string specified by the store creator is saved in the store. This ensures that each
//! successfully created store has a unique key.
//!
//! [cpi-with-pda-signer]: https://solana.com/docs/core/cpi#cpi-with-pda-signer
//! [seeds]: https://solana.com/docs/core/pda#how-to-derive-a-pda

pub mod instructions;

/// States.
pub mod states;

/// Constants.
pub mod constants;

/// Utils.
pub mod utils;

/// Events.
pub mod events;

pub use self::states::Data;

use self::{
    instructions::*,
    states::{
        common::{SwapParams, TokenRecord},
        deposit::TokenParams as DepositTokenParams,
        market::{config::EntryArgs, MarketMeta},
        order::{OrderParams, TransferOut},
        token_config::TokenConfigBuilder,
        withdrawal::TokenParams as WithdrawalTokenParams,
        PriceProviderKind,
    },
    utils::internal,
};
use anchor_lang::prelude::*;
use gmsol_utils::price::Price;

#[cfg_attr(test, macro_use)]
extern crate static_assertions;

declare_id!("hndKzPMrB9Xzs3mwarnPdkSWpZPZN3gLeeNzHDcHotT");

#[program]
/// Instructions definitions of the GMSOL Store Program.
pub mod gmsol_store {
    use super::*;

    // Data Store.
    /// Initialize a new [`Store`](crate::states::Store) account.
    pub fn initialize(
        ctx: Context<Initialize>,
        key: String,
        authority: Option<Pubkey>,
    ) -> Result<()> {
        instructions::initialize(ctx, key, authority)
    }

    #[access_control(internal::Authenticate::only_admin(&ctx))]
    pub fn transfer_store_authority(
        ctx: Context<TransferStoreAuthority>,
        new_authority: Pubkey,
    ) -> Result<()> {
        instructions::unchecked_transfer_store_authority(ctx, new_authority)
    }

    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn set_token_map(ctx: Context<SetTokenMap>) -> Result<()> {
        instructions::unchecked_set_token_map(ctx)
    }

    pub fn get_token_map(ctx: Context<ReadStore>) -> Result<Option<Pubkey>> {
        instructions::get_token_map(ctx)
    }

    // Roles.
    pub fn check_admin(ctx: Context<CheckRole>) -> Result<bool> {
        instructions::check_admin(ctx)
    }

    pub fn check_role(ctx: Context<CheckRole>, role: String) -> Result<bool> {
        instructions::check_role(ctx, role)
    }

    pub fn has_admin(ctx: Context<HasRole>, authority: Pubkey) -> Result<bool> {
        instructions::has_admin(ctx, authority)
    }

    pub fn has_role(ctx: Context<HasRole>, authority: Pubkey, role: String) -> Result<bool> {
        instructions::has_role(ctx, authority, role)
    }

    #[access_control(internal::Authenticate::only_admin(&ctx))]
    pub fn enable_role(ctx: Context<EnableRole>, role: String) -> Result<()> {
        instructions::enable_role(ctx, role)
    }

    #[access_control(internal::Authenticate::only_admin(&ctx))]
    pub fn disable_role(ctx: Context<DisableRole>, role: String) -> Result<()> {
        instructions::disable_role(ctx, role)
    }

    #[access_control(internal::Authenticate::only_admin(&ctx))]
    pub fn grant_role(ctx: Context<GrantRole>, user: Pubkey, role: String) -> Result<()> {
        instructions::grant_role(ctx, user, role)
    }

    #[access_control(internal::Authenticate::only_admin(&ctx))]
    pub fn revoke_role(ctx: Context<RevokeRole>, user: Pubkey, role: String) -> Result<()> {
        instructions::revoke_role(ctx, user, role)
    }

    // Config.
    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn insert_amount(ctx: Context<InsertAmount>, key: String, amount: u64) -> Result<()> {
        instructions::insert_amount(ctx, &key, amount)
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn insert_factor(ctx: Context<InsertFactor>, key: String, factor: u128) -> Result<()> {
        instructions::insert_factor(ctx, &key, factor)
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn insert_address(ctx: Context<InsertAddress>, key: String, address: Pubkey) -> Result<()> {
        instructions::insert_address(ctx, &key, address)
    }

    // Token Config.
    /// Initialize a token map.
    pub fn initialize_token_map(ctx: Context<InitializeTokenMap>) -> Result<()> {
        instructions::initialize_token_map(ctx)
    }

    /// Push a new token config to the given token map.
    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn push_to_token_map(
        ctx: Context<PushToTokenMap>,
        name: String,
        builder: TokenConfigBuilder,
        enable: bool,
        new: bool,
    ) -> Result<()> {
        instructions::unchecked_push_to_token_map(ctx, &name, builder, enable, new)
    }

    /// Push a new synthetic token config to the given token map.
    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn push_to_token_map_synthetic(
        ctx: Context<PushToTokenMapSynthetic>,
        name: String,
        token: Pubkey,
        token_decimals: u8,
        builder: TokenConfigBuilder,
        enable: bool,
        new: bool,
    ) -> Result<()> {
        instructions::unchecked_push_to_token_map_synthetic(
            ctx,
            &name,
            token,
            token_decimals,
            builder,
            enable,
            new,
        )
    }

    pub fn is_token_config_enabled(ctx: Context<ReadTokenMap>, token: Pubkey) -> Result<bool> {
        instructions::is_token_config_enabled(ctx, &token)
    }

    pub fn token_expected_provider(ctx: Context<ReadTokenMap>, token: Pubkey) -> Result<u8> {
        instructions::token_expected_provider(ctx, &token).map(|kind| kind as u8)
    }

    pub fn token_feed(ctx: Context<ReadTokenMap>, token: Pubkey, provider: u8) -> Result<Pubkey> {
        instructions::token_feed(
            ctx,
            &token,
            &PriceProviderKind::try_from(provider)
                .map_err(|_| StoreError::InvalidProviderKindIndex)?,
        )
    }

    pub fn token_timestamp_adjustment(
        ctx: Context<ReadTokenMap>,
        token: Pubkey,
        provider: u8,
    ) -> Result<u32> {
        instructions::token_timestamp_adjustment(
            ctx,
            &token,
            &PriceProviderKind::try_from(provider)
                .map_err(|_| StoreError::InvalidProviderKindIndex)?,
        )
    }

    pub fn token_name(ctx: Context<ReadTokenMap>, token: Pubkey) -> Result<String> {
        instructions::token_name(ctx, &token)
    }

    pub fn token_decimals(ctx: Context<ReadTokenMap>, token: Pubkey) -> Result<u8> {
        instructions::token_decimals(ctx, &token)
    }

    pub fn token_precision(ctx: Context<ReadTokenMap>, token: Pubkey) -> Result<u8> {
        instructions::token_precision(ctx, &token)
    }

    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn toggle_token_config(
        ctx: Context<ToggleTokenConfig>,
        token: Pubkey,
        enable: bool,
    ) -> Result<()> {
        instructions::unchecked_toggle_token_config(ctx, token, enable)
    }

    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn set_expected_provider(
        ctx: Context<SetExpectedProvider>,
        token: Pubkey,
        provider: u8,
    ) -> Result<()> {
        instructions::unchecked_set_expected_provider(
            ctx,
            token,
            PriceProviderKind::try_from(provider)
                .map_err(|_| StoreError::InvalidProviderKindIndex)?,
        )
    }

    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn set_feed_config(
        ctx: Context<SetFeedConfig>,
        token: Pubkey,
        provider: u8,
        feed: Pubkey,
        timestamp_adjustment: u32,
    ) -> Result<()> {
        instructions::unchecked_set_feed_config(
            ctx,
            token,
            &PriceProviderKind::try_from(provider)
                .map_err(|_| StoreError::InvalidProviderKindIndex)?,
            feed,
            timestamp_adjustment,
        )
    }

    // Market.
    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn initialize_market(
        ctx: Context<InitializeMarket>,
        market_token_mint: Pubkey,
        index_token_mint: Pubkey,
        long_token_mint: Pubkey,
        short_token_mint: Pubkey,
        name: String,
        enable: bool,
    ) -> Result<()> {
        instructions::unchecked_initialize_market(
            ctx,
            market_token_mint,
            index_token_mint,
            long_token_mint,
            short_token_mint,
            &name,
            enable,
        )
    }

    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn remove_market(ctx: Context<RemoveMarket>) -> Result<()> {
        instructions::unchecked_remove_market(ctx)
    }

    pub fn get_validated_market_meta(ctx: Context<GetValidatedMarketMeta>) -> Result<MarketMeta> {
        instructions::get_validated_market_meta(ctx)
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn market_transfer_in(ctx: Context<MarketTransferIn>, amount: u64) -> Result<()> {
        instructions::unchecked_market_transfer_in(ctx, amount)
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn market_transfer_out(ctx: Context<MarketTransferOut>, amount: u64) -> Result<()> {
        instructions::unchecked_market_transfer_out(ctx, amount)
    }

    pub fn get_market_meta(ctx: Context<ReadMarket>) -> Result<MarketMeta> {
        instructions::get_market_meta(ctx)
    }

    pub fn get_market_config(ctx: Context<ReadMarket>, key: String) -> Result<u128> {
        instructions::get_market_config(ctx, &key)
    }

    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn update_market_config(
        ctx: Context<UpdateMarketConfig>,
        key: String,
        value: u128,
    ) -> Result<()> {
        instructions::unchecked_update_market_config(ctx, &key, value)
    }

    /// Update market config with the given buffer.
    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn update_market_config_with_buffer(
        ctx: Context<UpdateMarketConfigWithBuffer>,
    ) -> Result<()> {
        instructions::unchecked_update_market_config_with_buffer(ctx)
    }

    /// Initialize a market config buffer account.
    pub fn initialize_market_config_buffer(
        ctx: Context<InitializeMarketConfigBuffer>,
        expire_after_secs: u32,
    ) -> Result<()> {
        instructions::initialize_market_config_buffer(ctx, expire_after_secs)
    }

    /// Set the authority of the buffer account.
    pub fn set_market_config_buffer_authority(
        ctx: Context<SetMarketConfigBufferAuthority>,
        new_authority: Pubkey,
    ) -> Result<()> {
        instructions::set_market_config_buffer_authority(ctx, new_authority)
    }

    /// Close the buffer account.
    pub fn close_market_config_buffer(ctx: Context<CloseMarketConfigBuffer>) -> Result<()> {
        instructions::close_market_config_buffer(ctx)
    }

    /// Push to the buffer account.
    pub fn push_to_market_config_buffer(
        ctx: Context<PushToMarketConfigBuffer>,
        new_configs: Vec<EntryArgs>,
    ) -> Result<()> {
        instructions::push_to_market_config_buffer(ctx, new_configs)
    }

    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn toggle_market(ctx: Context<ToggleMarket>, enable: bool) -> Result<()> {
        instructions::unchecked_toggle_market(ctx, enable)
    }

    // Token.
    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn initialize_market_token(
        ctx: Context<InitializeMarketToken>,
        index_token_mint: Pubkey,
        long_token_mint: Pubkey,
        short_token_mint: Pubkey,
    ) -> Result<()> {
        instructions::unchecked_initialize_market_token(
            ctx,
            index_token_mint,
            long_token_mint,
            short_token_mint,
        )
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn mint_market_token_to(ctx: Context<MintMarketTokenTo>, amount: u64) -> Result<()> {
        instructions::unchecked_mint_market_token_to(ctx, amount)
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn burn_market_token_from(ctx: Context<BurnMarketTokenFrom>, amount: u64) -> Result<()> {
        instructions::unchecked_burn_market_token_from(ctx, amount)
    }

    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn initialize_market_vault(
        ctx: Context<InitializeMarketVault>,
        market_token_mint: Option<Pubkey>,
    ) -> Result<()> {
        instructions::unchecked_initialize_market_vault(ctx, market_token_mint)
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn market_vault_transfer_out(
        ctx: Context<MarketVaultTransferOut>,
        amount: u64,
    ) -> Result<()> {
        instructions::unchecked_market_vault_transfer_out(ctx, amount)
    }

    #[access_control(internal::Authenticate::only_order_keeper(&ctx))]
    pub fn use_claimable_account(
        ctx: Context<UseClaimableAccount>,
        timestamp: i64,
        amount: u64,
    ) -> Result<()> {
        instructions::unchecked_use_claimable_account(ctx, timestamp, amount)
    }

    #[access_control(internal::Authenticate::only_order_keeper(&ctx))]
    pub fn close_empty_claimable_account(
        ctx: Context<CloseEmptyClaimableAccount>,
        user: Pubkey,
        timestamp: i64,
    ) -> Result<()> {
        instructions::unchecked_close_empty_claimable_account(ctx, user, timestamp)
    }

    // Oracle.
    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn initialize_oracle(ctx: Context<InitializeOracle>, index: u8) -> Result<()> {
        instructions::unchecked_initialize_oracle(ctx, index)
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn clear_all_prices(ctx: Context<ClearAllPrices>) -> Result<()> {
        instructions::clear_all_prices(ctx)
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn set_price(ctx: Context<SetPrice>, token: Pubkey, price: Price) -> Result<()> {
        instructions::set_price(ctx, token, price)
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn set_prices_from_price_feed<'info>(
        ctx: Context<'_, '_, 'info, 'info, SetPricesFromPriceFeed<'info>>,
        tokens: Vec<Pubkey>,
    ) -> Result<()> {
        instructions::set_prices_from_price_feed(ctx, tokens)
    }

    // Deposit.
    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn initialize_deposit(
        ctx: Context<InitializeDeposit>,
        nonce: [u8; 32],
        tokens_with_feed: Vec<TokenRecord>,
        swap_params: SwapParams,
        token_params: DepositTokenParams,
        ui_fee_receiver: Pubkey,
    ) -> Result<()> {
        instructions::initialize_deposit(
            ctx,
            nonce,
            tokens_with_feed,
            swap_params,
            token_params,
            ui_fee_receiver,
        )
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn remove_deposit(ctx: Context<RemoveDeposit>, refund: u64, reason: String) -> Result<()> {
        instructions::remove_deposit(ctx, refund, &reason)
    }

    // Withdrawal.
    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn initialize_withdrawal(
        ctx: Context<InitializeWithdrawal>,
        nonce: [u8; 32],
        swap_params: SwapParams,
        tokens_with_feed: Vec<TokenRecord>,
        token_params: WithdrawalTokenParams,
        market_token_amount: u64,
        ui_fee_receiver: Pubkey,
    ) -> Result<()> {
        instructions::initialize_withdrawal(
            ctx,
            nonce,
            swap_params,
            tokens_with_feed,
            token_params,
            market_token_amount,
            ui_fee_receiver,
        )
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn remove_withdrawal(
        ctx: Context<RemoveWithdrawal>,
        refund: u64,
        reason: String,
    ) -> Result<()> {
        instructions::remove_withdrawal(ctx, refund, &reason)
    }

    // Exchange.
    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn execute_deposit<'info>(
        ctx: Context<'_, '_, 'info, 'info, ExecuteDeposit<'info>>,
        throw_on_execution_error: bool,
    ) -> Result<bool> {
        instructions::execute_deposit(ctx, throw_on_execution_error)
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn execute_withdrawal<'info>(
        ctx: Context<'_, '_, 'info, 'info, ExecuteWithdrawal<'info>>,
        throw_on_execution_error: bool,
    ) -> Result<(u64, u64)> {
        instructions::execute_withdrawal(ctx, throw_on_execution_error)
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn execute_order<'info>(
        ctx: Context<'_, '_, 'info, 'info, ExecuteOrder<'info>>,
        recent_timestamp: i64,
        throw_on_execution_error: bool,
    ) -> Result<(bool, Box<TransferOut>)> {
        instructions::execute_order(ctx, recent_timestamp, throw_on_execution_error)
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn initialize_order(
        ctx: Context<InitializeOrder>,
        nonce: [u8; 32],
        tokens_with_feed: Vec<TokenRecord>,
        swap: SwapParams,
        params: OrderParams,
        output_token: Pubkey,
        ui_fee_receiver: Pubkey,
    ) -> Result<()> {
        instructions::initialize_order(
            ctx,
            nonce,
            tokens_with_feed,
            swap,
            params,
            output_token,
            ui_fee_receiver,
        )
    }

    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn remove_order(ctx: Context<RemoveOrder>, refund: u64, reason: String) -> Result<()> {
        instructions::remove_order(ctx, refund, &reason)
    }

    // Position.
    #[access_control(internal::Authenticate::only_controller(&ctx))]
    pub fn remove_position(ctx: Context<RemovePosition>, refund: u64) -> Result<()> {
        instructions::remove_position(ctx, refund)
    }

    #[cfg(not(feature = "no-bug-fix"))]
    #[access_control(internal::Authenticate::only_market_keeper(&ctx))]
    pub fn turn_into_pure_pool(ctx: Context<TurnIntoPurePool>, kind: u8) -> Result<()> {
        instructions::unchecked_turn_into_pure_pool(
            ctx,
            kind.try_into()
                .map_err(|_| error!(StoreError::InvalidArgument))?,
        )
    }
}

#[error_code]
pub enum StoreError {
    // Common.
    #[msg("Invalid pda")]
    InvalidPDA,
    #[msg("Invalid key")]
    InvalidKey,
    #[msg("Aready exist")]
    AlreadyExist,
    #[msg("Exceed max length limit")]
    ExceedMaxLengthLimit,
    #[msg("Exceed max string length limit")]
    ExceedMaxStringLengthLimit,
    #[msg("No space for new data")]
    NoSpaceForNewData,
    #[msg("Invalid argument")]
    InvalidArgument,
    #[msg("Lamports not enough")]
    LamportsNotEnough,
    #[msg("Required resource not found")]
    RequiredResourceNotFound,
    #[msg("Amount overflow")]
    AmountOverflow,
    #[msg("Unknown error")]
    Unknown,
    #[msg("Gmx Core Error")]
    Model,
    #[msg("Missing amount")]
    MissingAmount,
    #[msg("Missing factor")]
    MissingFactor,
    #[msg("Cannot be zero")]
    CannotBeZero,
    #[msg("Missing Market Account")]
    MissingMarketAccount,
    #[msg("Load Account Error")]
    LoadAccountError,
    // Roles.
    #[msg("Too many admins")]
    TooManyAdmins,
    #[msg("At least one admin")]
    AtLeastOneAdmin,
    #[msg("Invalid data store")]
    InvalidDataStore,
    #[msg("Already be an admin")]
    AlreadyBeAnAdmin,
    #[msg("Not an admin")]
    NotAnAdmin,
    #[msg("Invalid role")]
    InvalidRole,
    #[msg("Invalid roles account")]
    InvalidRoles,
    #[msg("Permission denied")]
    PermissionDenied,
    #[msg("No such role")]
    NoSuchRole,
    #[msg("The role is disabled")]
    DisabledRole,
    // Oracle.
    #[msg("Oracle is not empty")]
    PricesAlreadySet,
    #[msg("Price of the given token already set")]
    PriceAlreadySet,
    #[msg("Invalid price feed account")]
    InvalidPriceFeedAccount,
    #[msg("Invalid price feed price")]
    InvalidPriceFeedPrice,
    #[msg("Price feed not updated")]
    PriceFeedNotUpdated,
    #[msg("Token config disabled")]
    TokenConfigDisabled,
    #[msg("Negative price is not allowed")]
    NegativePrice,
    #[msg("Price overflow")]
    PriceOverflow,
    #[msg("Price feed is not set for the given provider")]
    PriceFeedNotSet,
    #[msg("Not enough feeds")]
    NotEnoughFeeds,
    #[msg("Max price age exceeded")]
    MaxPriceAgeExceeded,
    #[msg("Invalid oracle timestamp range")]
    InvalidOracleTsTrange,
    #[msg("Max oracle timestamp range exceeded")]
    MaxOracleTimeStampRangeExceeded,
    #[msg("Oracle timestamps are smaller than required")]
    OracleTimestampsAreSmallerThanRequired,
    #[msg("Oracle timestamps are larger than requried")]
    OracleTimestampsAreLargerThanRequired,
    #[msg("Oracle not updated")]
    OracleNotUpdated,
    #[msg("Invalid oracle slot")]
    InvalidOracleSlot,
    // Market.
    #[msg("Computation error")]
    Computation,
    #[msg("Unsupported pool kind")]
    UnsupportedPoolKind,
    #[msg("Invalid collateral token")]
    InvalidCollateralToken,
    #[msg("Invalid market")]
    InvalidMarket,
    #[msg("Disabled market")]
    DisabledMarket,
    #[msg("Unknown swap out market")]
    UnknownSwapOutMarket,
    // Exchange Common.
    #[msg("Invalid swap path")]
    InvalidSwapPath,
    #[msg("Output amount too small")]
    OutputAmountTooSmall,
    #[msg("Amount is not zero but swap in token not provided")]
    AmountNonZeroMissingToken,
    #[msg("Missing token mint")]
    MissingTokenMint,
    #[msg("Missing oracle price")]
    MissingOracelPrice,
    // Withdrawal.
    #[msg("User mismach")]
    UserMismatch,
    #[msg("Empty withdrawal")]
    EmptyWithdrawal,
    #[msg("Invalid withdrawal to remove")]
    InvalidWithdrawalToRemove,
    #[msg("Unable to transfer out remaining withdrawal amount")]
    UnableToTransferOutRemainingWithdrawalAmount,
    // Deposit.
    #[msg("Empty deposit")]
    EmptyDeposit,
    #[msg("Missing deposit token account")]
    MissingDepositTokenAccount,
    #[msg("Invalid deposit to remove")]
    InvalidDepositToRemove,
    // Exchange.
    #[msg("Invalid position kind")]
    InvalidPositionKind,
    #[msg("Invalid position collateral token")]
    InvalidPositionCollateralToken,
    #[msg("Invalid position market")]
    InvalidPositionMarket,
    #[msg("Position account not provided")]
    PositionNotProvided,
    #[msg("Same secondary tokens not merged")]
    SameSecondaryTokensNotMerged,
    #[msg("Missing receivers")]
    MissingReceivers,
    // Position.
    #[msg("position is not initialized")]
    PositionNotInitalized,
    #[msg("position has been initialized")]
    PositionHasBeenInitialized,
    #[msg("position is not required")]
    PositionIsNotRequried,
    #[msg("position is not provided")]
    PositionIsNotProvided,
    #[msg("invalid position initialization params")]
    InvalidPositionInitailziationParams,
    #[msg("invalid position")]
    InvalidPosition,
    // Order.
    #[msg("missing initialial token account for order")]
    MissingInitializeTokenAccountForOrder,
    #[msg("missing claimable time window")]
    MissingClaimableTimeWindow,
    #[msg("missing recent time window")]
    MissingRecentTimeWindow,
    #[msg("missing holding address")]
    MissingHoldingAddress,
    #[msg("missing sender")]
    MissingSender,
    #[msg("missing position")]
    MissingPosition,
    #[msg("missing claimable long collateral account for user")]
    MissingClaimableLongCollateralAccountForUser,
    #[msg("missing claimable short collateral account for user")]
    MissingClaimableShortCollateralAccountForUser,
    #[msg("missing claimable pnl token account for holding")]
    MissingClaimablePnlTokenAccountForHolding,
    #[msg("claimable collateral in output token for holding is not supported")]
    ClaimbleCollateralInOutputTokenForHolding,
    #[msg("no delegated authority is set")]
    NoDelegatedAuthorityIsSet,
    #[msg("invalid order to remove")]
    InvalidOrderToRemove,
    // Token Config.
    #[msg("synthetic flag does not match")]
    InvalidSynthetic,
    #[msg("invalid token map")]
    InvalidTokenMap,
    // Invalid Provider Kind.
    #[msg("invalid provider kind index")]
    InvalidProviderKindIndex,
}

impl StoreError {
    #[inline]
    pub(crate) const fn invalid_position_kind(_kind: u8) -> Self {
        Self::InvalidPositionKind
    }
}

/// Data Store Resut.
pub type StoreResult<T> = std::result::Result<T, StoreError>;
