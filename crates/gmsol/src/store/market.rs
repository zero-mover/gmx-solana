use std::ops::Deref;

use anchor_client::{
    anchor_lang::system_program,
    solana_sdk::{pubkey::Pubkey, signer::Signer},
    Program, RequestBuilder,
};
use data_store::{
    accounts, constants, instruction,
    states::{Market, Seed},
};

use super::roles::find_roles_address;

/// Find PDA for the market vault.
pub fn find_market_vault_address(store: &Pubkey, token: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            constants::MARKET_VAULT_SEED,
            store.as_ref(),
            token.as_ref(),
            &[],
        ],
        &data_store::id(),
    )
}

/// Find PDA for Market token mint account.
pub fn find_market_token_address(
    store: &Pubkey,
    index_token: &Pubkey,
    long_token: &Pubkey,
    short_token: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            constants::MAREKT_TOKEN_MINT_SEED,
            store.as_ref(),
            index_token.as_ref(),
            long_token.as_ref(),
            short_token.as_ref(),
        ],
        &data_store::id(),
    )
}

/// Find PDA for [`Market`] account.
pub fn find_market_address(store: &Pubkey, token: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[Market::SEED, store.as_ref(), token.as_ref()],
        &data_store::id(),
    )
}

/// Vault Operations.
pub trait VaultOps<C> {
    /// Initialize a market vault for the given token.
    fn initialize_market_vault(
        &self,
        store: &Pubkey,
        token: &Pubkey,
    ) -> (RequestBuilder<C>, Pubkey);

    /// Transfer tokens out from the given market vault.
    fn market_vault_transfer_out(
        &self,
        store: &Pubkey,
        token: &Pubkey,
        to: &Pubkey,
        amount: u64,
    ) -> RequestBuilder<C>;
}

impl<C, S> VaultOps<C> for Program<C>
where
    C: Deref<Target = S> + Clone,
    S: Signer,
{
    fn initialize_market_vault(
        &self,
        store: &Pubkey,
        token: &Pubkey,
    ) -> (RequestBuilder<C>, Pubkey) {
        let authority = self.payer();
        let vault = find_market_vault_address(store, token).0;
        let builder = self
            .request()
            .accounts(accounts::InitializeMarketVault {
                authority,
                only_market_keeper: find_roles_address(store, &authority).0,
                store: *store,
                mint: *token,
                vault,
                system_program: system_program::ID,
                token_program: anchor_spl::token::ID,
            })
            .args(instruction::InitializeMarketVault {
                market_token_mint: None,
            });
        (builder, vault)
    }

    fn market_vault_transfer_out(
        &self,
        store: &Pubkey,
        token: &Pubkey,
        to: &Pubkey,
        amount: u64,
    ) -> RequestBuilder<C> {
        let authority = self.payer();
        self.request()
            .accounts(accounts::MarketVaultTransferOut {
                authority,
                only_controller: find_roles_address(store, &authority).0,
                store: *store,
                market_vault: find_market_vault_address(store, token).0,
                to: *to,
                token_program: anchor_spl::token::ID,
            })
            .args(instruction::MarketVaultTransferOut { amount })
    }
}

impl<C, S> VaultOps<C> for crate::Client<C>
where
    C: Deref<Target = S> + Clone,
    S: Signer,
{
    fn initialize_market_vault(
        &self,
        store: &Pubkey,
        token: &Pubkey,
    ) -> (RequestBuilder<C>, Pubkey) {
        let authority = self.payer();
        let vault = self.find_market_vault_address(store, token);
        let builder = self
            .data_store()
            .request()
            .accounts(accounts::InitializeMarketVault {
                authority,
                only_market_keeper: self.payer_roles_address(store),
                store: *store,
                mint: *token,
                vault,
                system_program: system_program::ID,
                token_program: anchor_spl::token::ID,
            })
            .args(instruction::InitializeMarketVault {
                market_token_mint: None,
            });
        (builder, vault)
    }

    fn market_vault_transfer_out(
        &self,
        store: &Pubkey,
        token: &Pubkey,
        to: &Pubkey,
        amount: u64,
    ) -> RequestBuilder<C> {
        let authority = self.payer();
        self.data_store()
            .request()
            .accounts(accounts::MarketVaultTransferOut {
                authority,
                only_controller: self.payer_roles_address(store),
                store: *store,
                market_vault: self.find_market_vault_address(store, token),
                to: *to,
                token_program: anchor_spl::token::ID,
            })
            .args(instruction::MarketVaultTransferOut { amount })
    }
}
