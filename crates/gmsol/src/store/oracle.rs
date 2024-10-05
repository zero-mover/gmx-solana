use std::ops::Deref;

use anchor_client::{
    anchor_lang::system_program,
    solana_sdk::{pubkey::Pubkey, signer::Signer},
};
use gmsol_store::{accounts, instruction};

use crate::utils::RpcBuilder;

/// Oracle management for GMSOL.
pub trait OracleOps<C> {
    /// Initialize [`Oracle`](gmsol_store::states::Oracle) account.
    fn initialize_oracle(&self, store: &Pubkey, index: u8) -> (RpcBuilder<C>, Pubkey);
}

impl<C, S> OracleOps<C> for crate::Client<C>
where
    C: Deref<Target = S> + Clone,
    S: Signer,
{
    fn initialize_oracle(&self, store: &Pubkey, index: u8) -> (RpcBuilder<C>, Pubkey) {
        let authority = self.payer();
        let oracle = self.find_oracle_address(store, index);
        let builder = self
            .store_rpc()
            .accounts(accounts::InitializeOracle {
                authority,
                store: *store,
                oracle,
                system_program: system_program::ID,
            })
            .args(instruction::InitializeOracle { index });
        (builder, oracle)
    }
}
