use std::ops::Deref;

use anchor_client::{
    anchor_lang::system_program,
    solana_sdk::{instruction::AccountMeta, pubkey::Pubkey, signer::Signer},
    Program, RequestBuilder,
};
use data_store::states::{Deposit, NonceBytes, Seed};
use exchange::{accounts, instruction, instructions::CreateDepositParams, utils::ControllerSeeds};
use rand::{distributions::Standard, Rng};

use crate::store::{
    data_store::{find_market_address, find_market_vault_address, find_token_config_map},
    roles::find_roles_address,
};

/// Create PDA for deposit.
pub fn find_deposit_address(store: &Pubkey, user: &Pubkey, nonce: &NonceBytes) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[Deposit::SEED, store.as_ref(), user.as_ref(), nonce],
        &data_store::id(),
    )
}

/// Create Deposit Builder.
pub struct CreateDepositBuilder<'a, C> {
    program: &'a Program<C>,
    store: Pubkey,
    market_token: Pubkey,
    receiver: Option<Pubkey>,
    ui_fee_receiver: Pubkey,
    execution_fee: u64,
    long_token_swap_path: Vec<Pubkey>,
    short_token_swap_path: Vec<Pubkey>,
    initial_long_token: Option<Pubkey>,
    initial_short_token: Option<Pubkey>,
    initial_long_token_account: Option<Pubkey>,
    initial_short_token_account: Option<Pubkey>,
    initial_long_token_amount: u64,
    initial_short_token_amount: u64,
    min_market_token: u64,
    should_unwrap_native_token: bool,
    nonce: Option<NonceBytes>,
}

impl<'a, C, S> CreateDepositBuilder<'a, C>
where
    C: Deref<Target = S> + Clone,
    S: Signer,
{
    pub(super) fn new(program: &'a Program<C>, store: Pubkey, market_token: Pubkey) -> Self {
        Self {
            program,
            store,
            nonce: None,
            market_token,
            receiver: None,
            ui_fee_receiver: Pubkey::default(),
            execution_fee: 0,
            long_token_swap_path: vec![],
            short_token_swap_path: vec![],
            initial_long_token: None,
            initial_short_token: None,
            initial_long_token_account: None,
            initial_short_token_account: None,
            initial_long_token_amount: 0,
            initial_short_token_amount: 0,
            min_market_token: 0,
            should_unwrap_native_token: false,
        }
    }

    /// Set the token account for receiving minted market tokens.
    ///
    /// Defaults to use associated token account.
    pub fn receiver(&mut self, token_account: &Pubkey) -> &mut Self {
        self.receiver = Some(*token_account);
        self
    }

    /// Set min market token to mint.
    pub fn min_market_token(&mut self, amount: u64) -> &mut Self {
        self.min_market_token = amount;
        self
    }

    /// Set allowed execution fee.
    pub fn execution_fee(&mut self, fee: u64) -> &mut Self {
        self.execution_fee = fee;
        self
    }

    /// Set long swap path.
    pub fn long_token_swap_path(&mut self, market_tokens: Vec<Pubkey>) -> &mut Self {
        self.long_token_swap_path = market_tokens;
        self
    }

    /// Set short swap path.
    pub fn short_token_swap_path(&mut self, market_tokens: Vec<Pubkey>) -> &mut Self {
        self.short_token_swap_path = market_tokens;
        self
    }

    async fn get_token_mint_if_not_provided(
        &self,
        token_account: &Pubkey,
        token: Option<&Pubkey>,
    ) -> crate::Result<Pubkey> {
        let token = if let Some(token) = token {
            *token
        } else {
            let account = self
                .program
                .account::<anchor_spl::token::TokenAccount>(*token_account)
                .await?;
            account.mint
        };
        Ok(token)
    }

    /// Set the initial long token params for deposit.
    ///
    /// - It will fetch the token of the given account if `token` not provided.
    pub async fn long_token(
        &mut self,
        token_account: &Pubkey,
        amount: u64,
        token: Option<&Pubkey>,
    ) -> crate::Result<&mut Self> {
        self.initial_long_token = Some(*token_account);
        self.initial_long_token_amount = amount;
        self.initial_long_token = Some(
            self.get_token_mint_if_not_provided(token_account, token)
                .await?,
        );
        Ok(self)
    }

    /// Set the initial short token params for deposit.
    ///
    /// - It will fetch the token of the given account if `token` not provided.
    pub async fn short_token(
        &mut self,
        token_account: &Pubkey,
        amount: u64,
        token: Option<&Pubkey>,
    ) -> crate::Result<&mut Self> {
        self.initial_short_token = Some(*token_account);
        self.initial_short_token_amount = amount;
        self.initial_short_token = Some(
            self.get_token_mint_if_not_provided(token_account, token)
                .await?,
        );
        Ok(self)
    }

    fn get_receiver(&self) -> Pubkey {
        match self.receiver {
            Some(token_account) => token_account,
            None => anchor_spl::associated_token::get_associated_token_address(
                &self.program.payer(),
                &self.market_token,
            ),
        }
    }

    /// Build a [`RequesitBuilder`] and return deposit address.
    pub fn build_with_address(&self) -> crate::Result<(RequestBuilder<'a, C>, Pubkey)> {
        let receiver = self.get_receiver();
        let Self {
            program,
            store,
            nonce,
            market_token,
            ui_fee_receiver,
            execution_fee,
            long_token_swap_path,
            short_token_swap_path,
            initial_long_token,
            initial_short_token,
            initial_long_token_account,
            initial_short_token_account,
            initial_long_token_amount,
            initial_short_token_amount,
            min_market_token,
            should_unwrap_native_token,
            ..
        } = self;
        let nonce = nonce.unwrap_or_else(|| {
            rand::thread_rng()
                .sample_iter(Standard)
                .take(32)
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap()
        });
        let payer = program.payer();
        let deposit = find_deposit_address(store, &payer, &nonce).0;
        let (_, authority) = ControllerSeeds::find_with_address(store);
        let only_controller = find_roles_address(store, &authority).0;
        let builder = program
            .request()
            .accounts(accounts::CreateDeposit {
                authority,
                store: *store,
                only_controller,
                data_store_program: data_store::id(),
                system_program: system_program::ID,
                token_program: anchor_spl::token::ID,
                deposit,
                payer,
                receiver,
                token_config_map: find_token_config_map(store).0,
                market: find_market_address(store, market_token).0,
                initial_long_token_account: *initial_long_token_account,
                initial_short_token_account: *initial_short_token_account,
                long_token_deposit_vault: initial_long_token
                    .map(|token| find_market_vault_address(store, &token).0),
                short_token_deposit_vault: initial_short_token
                    .map(|token| find_market_vault_address(store, &token).0),
            })
            .args(instruction::CreateDeposit {
                nonce,
                params: CreateDepositParams {
                    ui_fee_receiver: *ui_fee_receiver,
                    execution_fee: *execution_fee,
                    long_token_swap_length: long_token_swap_path
                        .len()
                        .try_into()
                        .map_err(|_| crate::Error::NumberOutOfRange)?,
                    short_token_swap_length: short_token_swap_path
                        .len()
                        .try_into()
                        .map_err(|_| crate::Error::NumberOutOfRange)?,
                    initial_long_token_amount: *initial_long_token_amount,
                    initial_short_token_amount: *initial_short_token_amount,
                    min_market_token: *min_market_token,
                    should_unwrap_native_token: *should_unwrap_native_token,
                },
            })
            .accounts(
                long_token_swap_path
                    .iter()
                    .chain(short_token_swap_path.iter())
                    .map(|mint| AccountMeta {
                        pubkey: find_market_address(store, mint).0,
                        is_signer: false,
                        is_writable: false,
                    })
                    .collect::<Vec<_>>(),
            );
        Ok((builder, deposit))
    }
}
