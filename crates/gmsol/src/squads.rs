use std::{collections::HashMap, future::Future, ops::Deref};

use anchor_client::{anchor_lang, solana_client::nonblocking::rpc_client::RpcClient};
use gmsol_solana_utils::{
    bundle_builder::{BundleBuilder, BundleOptions},
    transaction_builder::TransactionBuilder,
};
use solana_sdk::{
    address_lookup_table::{state::AddressLookupTable, AddressLookupTableAccount},
    hash::Hash,
    instruction::{AccountMeta, CompiledInstruction, Instruction},
    message::{
        v0::{Message, MessageAddressTableLookup},
        MessageHeader, VersionedMessage,
    },
    pubkey::Pubkey,
    signer::Signer,
};
use squads_multisig::{
    client::{
        get_multisig, ProposalApproveData, ProposalCreateAccounts, ProposalCreateArgs,
        ProposalCreateData, ProposalVoteAccounts, ProposalVoteArgs, VaultTransactionCreateAccounts,
        VaultTransactionCreateArgs, VaultTransactionCreateData, VaultTransactionExecuteAccounts,
        VaultTransactionExecuteData,
    },
    pda::get_ephemeral_signer_pda,
    squads_multisig_program::{self, VaultTransaction},
    state::{Proposal, TransactionMessage, VaultTransactionMessage},
};

pub use squads_multisig::pda::{get_proposal_pda, get_transaction_pda, get_vault_pda};

use crate::utils::builder::MakeBundleBuilder;

/// Squads Vault Transaction.
pub struct SquadsVaultTransaction(VaultTransaction);

impl Deref for SquadsVaultTransaction {
    type Target = VaultTransaction;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl anchor_lang::Discriminator for SquadsVaultTransaction {
    const DISCRIMINATOR: [u8; 8] =
        <VaultTransaction as squads_multisig::anchor_lang::Discriminator>::DISCRIMINATOR;
}

impl anchor_lang::AccountDeserialize for SquadsVaultTransaction {
    fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
        let inner = <VaultTransaction as squads_multisig::anchor_lang::AccountDeserialize>::try_deserialize_unchecked(buf).map_err(|_err| {
            anchor_lang::error::ErrorCode::AccountDidNotDeserialize
        })?;

        Ok(Self(inner))
    }
}

impl SquadsVaultTransaction {
    /// Convert to transaction message.
    pub fn to_message(&self) -> Message {
        let message = &self.0.message;
        let instructions = message
            .instructions
            .iter()
            .map(|ix| CompiledInstruction {
                program_id_index: ix.program_id_index,
                accounts: ix.account_indexes.clone(),
                data: ix.data.clone(),
            })
            .collect();
        let address_table_lookups = message
            .address_table_lookups
            .iter()
            .map(|atl| MessageAddressTableLookup {
                account_key: atl.account_key,
                writable_indexes: atl.writable_indexes.clone(),
                readonly_indexes: atl.readonly_indexes.clone(),
            })
            .collect();
        let num_non_signers = message.account_keys.len() as u8 - message.num_signers;
        Message {
            header: MessageHeader {
                num_required_signatures: message.num_signers,
                num_readonly_signed_accounts: message.num_signers - message.num_writable_signers,
                num_readonly_unsigned_accounts: num_non_signers - message.num_writable_non_signers,
            },
            account_keys: message.account_keys.clone(),
            recent_blockhash: Hash::default(),
            instructions,
            address_table_lookups,
        }
    }
}

/// Squads Proposal.
pub struct SquadsProposal(Proposal);

impl Deref for SquadsProposal {
    type Target = Proposal;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl anchor_lang::Discriminator for SquadsProposal {
    const DISCRIMINATOR: [u8; 8] =
        <Proposal as squads_multisig::anchor_lang::Discriminator>::DISCRIMINATOR;
}

impl anchor_lang::AccountDeserialize for SquadsProposal {
    fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
        let inner = <Proposal as squads_multisig::anchor_lang::AccountDeserialize>::try_deserialize_unchecked(buf).map_err(|_err| {
            anchor_lang::error::ErrorCode::AccountDidNotDeserialize
        })?;

        Ok(Self(inner))
    }
}

/// Squads Multisig Ops.
pub trait SquadsOps<C> {
    /// Create Vault Transaction with the given transaction index and return the message.
    fn squads_create_vault_transaction_and_return_data(
        &self,
        multisig: &Pubkey,
        vault_index: u8,
        transaction_index: u64,
        message: &VersionedMessage,
        memo: Option<String>,
        draft: bool,
    ) -> crate::Result<TransactionBuilder<C, (Pubkey, VaultTransaction)>>;

    /// Create Vault Transaction with the given transaction index.
    fn squads_create_vault_transaction_with_index(
        &self,
        multisig: &Pubkey,
        vault_index: u8,
        transaction_index: u64,
        message: &VersionedMessage,
        memo: Option<String>,
        draft: bool,
    ) -> crate::Result<TransactionBuilder<C, Pubkey>>;

    /// Create Vault Transaction with next transaction index.
    fn squads_create_vault_transaction(
        &self,
        multisig: &Pubkey,
        vault_index: u8,
        message: &VersionedMessage,
        memo: Option<String>,
        draft: bool,
        offset: Option<u64>,
    ) -> impl Future<Output = crate::Result<TransactionBuilder<C, Pubkey>>>;

    /// Approve a proposal.
    fn squads_approve_proposal(
        &self,
        multisig: &Pubkey,
        proposal: &Pubkey,
        memo: Option<String>,
    ) -> crate::Result<TransactionBuilder<C>>;

    /// Execute a vault transaction.
    fn squads_execute_vault_transaction(
        &self,
        multisig: &Pubkey,
        data: VaultTransaction,
        luts_cache: Option<&HashMap<Pubkey, AddressLookupTableAccount>>,
    ) -> impl Future<Output = crate::Result<TransactionBuilder<C>>>;

    /// Create a [`Squads`] from the given [`BundleBuilder`].
    fn squads_from_bundle<'a, T>(
        &'a self,
        multisig: &Pubkey,
        vault_index: u8,
        bundle: T,
    ) -> Squads<'a, C, T>;
}

impl<C: Deref<Target = impl Signer> + Clone> SquadsOps<C> for crate::Client<C> {
    fn squads_create_vault_transaction_and_return_data(
        &self,
        multisig: &Pubkey,
        vault_index: u8,
        transaction_index: u64,
        message: &VersionedMessage,
        memo: Option<String>,
        draft: bool,
    ) -> crate::Result<TransactionBuilder<C, (Pubkey, VaultTransaction)>> {
        use squads_multisig::{
            anchor_lang::{AnchorSerialize, InstructionData, ToAccountMetas},
            squads_multisig_program::ID,
        };

        let payer = self.payer();
        let transaction_pda = get_transaction_pda(multisig, transaction_index, Some(&ID));
        let proposal_pda = get_proposal_pda(multisig, transaction_index, Some(&ID)).0;
        let vault_pda = get_vault_pda(multisig, vault_index, Some(&ID));

        let transaction_message = versioned_message_to_transaction_message(message);
        let rpc = self.store_transaction().pre_instructions(vec![
            Instruction {
                program_id: ID,
                accounts: VaultTransactionCreateAccounts {
                    creator: payer,
                    rent_payer: payer,
                    transaction: transaction_pda.0,
                    multisig: *multisig,
                    system_program: solana_sdk::system_program::id(),
                }
                .to_account_metas(Some(false)),
                data: VaultTransactionCreateData {
                    args: VaultTransactionCreateArgs {
                        ephemeral_signers: 0,
                        vault_index,
                        memo,
                        transaction_message: transaction_message.try_to_vec()?,
                    },
                }
                .data(),
            },
            Instruction {
                program_id: ID,
                accounts: ProposalCreateAccounts {
                    creator: payer,
                    rent_payer: payer,
                    proposal: proposal_pda,
                    multisig: *multisig,
                    system_program: solana_sdk::system_program::id(),
                }
                .to_account_metas(Some(false)),
                data: ProposalCreateData {
                    args: ProposalCreateArgs {
                        draft,
                        transaction_index,
                    },
                }
                .data(),
            },
        ]);

        let data = VaultTransaction {
            multisig: *multisig,
            creator: payer,
            index: transaction_index,
            bump: transaction_pda.1,
            vault_index,
            vault_bump: vault_pda.1,
            ephemeral_signer_bumps: vec![],
            message: transaction_message
                .try_into()
                .map_err(crate::Error::unknown)?,
        };

        Ok(rpc.output((transaction_pda.0, data)))
    }

    fn squads_create_vault_transaction_with_index(
        &self,
        multisig: &Pubkey,
        vault_index: u8,
        transaction_index: u64,
        message: &VersionedMessage,
        memo: Option<String>,
        draft: bool,
    ) -> crate::Result<TransactionBuilder<C, Pubkey>> {
        let (txn, (transaction, _)) = self
            .squads_create_vault_transaction_and_return_data(
                multisig,
                vault_index,
                transaction_index,
                message,
                memo,
                draft,
            )?
            .swap_output(());

        Ok(txn.output(transaction))
    }

    async fn squads_create_vault_transaction(
        &self,
        multisig: &Pubkey,
        vault_index: u8,
        message: &VersionedMessage,
        memo: Option<String>,
        draft: bool,
        offset: Option<u64>,
    ) -> crate::Result<TransactionBuilder<C, Pubkey>> {
        let multisig_data = get_multisig(&self.store_program().rpc(), multisig)
            .await
            .map_err(crate::Error::unknown)?;

        self.squads_create_vault_transaction_with_index(
            multisig,
            vault_index,
            multisig_data.transaction_index + 1 + offset.unwrap_or(0),
            message,
            memo,
            draft,
        )
    }

    fn squads_approve_proposal(
        &self,
        multisig: &Pubkey,
        proposal: &Pubkey,
        memo: Option<String>,
    ) -> crate::Result<TransactionBuilder<C>> {
        use squads_multisig::{
            anchor_lang::{InstructionData, ToAccountMetas},
            squads_multisig_program::ID,
        };

        let txn = self
            .store_transaction()
            .program(ID)
            .args(
                ProposalApproveData {
                    args: ProposalVoteArgs { memo },
                }
                .data(),
            )
            .accounts(
                ProposalVoteAccounts {
                    multisig: *multisig,
                    member: self.payer(),
                    proposal: *proposal,
                }
                .to_account_metas(Some(false)),
            );

        Ok(txn)
    }

    async fn squads_execute_vault_transaction(
        &self,
        multisig: &Pubkey,
        data: VaultTransaction,
        luts_cache: Option<&HashMap<Pubkey, AddressLookupTableAccount>>,
    ) -> crate::Result<TransactionBuilder<C>> {
        use squads_multisig::{
            anchor_lang::{InstructionData, ToAccountMetas},
            squads_multisig_program::ID,
        };

        let program_id = ID;

        let vault_transaction = data;
        let vault = get_vault_pda(multisig, vault_transaction.vault_index, Some(&program_id)).0;
        let transaction =
            get_transaction_pda(multisig, vault_transaction.index, Some(&program_id)).0;
        let proposal = get_proposal_pda(multisig, vault_transaction.index, Some(&program_id)).0;

        let (remaining_accounts, luts) = message_to_execute_account_metas(
            &self.store_program().rpc(),
            vault_transaction.message,
            vault_transaction.ephemeral_signer_bumps,
            &vault,
            &transaction,
            Some(&program_id),
            luts_cache,
        )
        .await;

        let txn = self
            .store_transaction()
            .program(ID)
            .args(VaultTransactionExecuteData {}.data())
            .accounts(
                VaultTransactionExecuteAccounts {
                    multisig: *multisig,
                    proposal,
                    transaction,
                    member: self.payer(),
                }
                .to_account_metas(Some(false)),
            )
            .accounts(remaining_accounts)
            .lookup_tables(luts.into_iter().map(|lut| (lut.key, lut.addresses)));

        Ok(txn)
    }

    fn squads_from_bundle<'a, T>(
        &'a self,
        multisig: &Pubkey,
        vault_index: u8,
        bundle: T,
    ) -> Squads<'a, C, T> {
        Squads {
            client: self,
            multisig: *multisig,
            vault_index,
            builder: bundle,
            approve: false,
            execute: false,
        }
    }
}

fn versioned_message_to_transaction_message(message: &VersionedMessage) -> TransactionMessage {
    match message {
        VersionedMessage::Legacy(message) => {
            let num_accounts = message.account_keys.len() as u8;
            let num_signers = message.header.num_required_signatures;
            let num_non_signers = num_accounts - num_signers;
            let instructions = message
                .instructions
                .iter()
                .map(|ix| squads_multisig_program::CompiledInstruction {
                    program_id_index: ix.program_id_index,
                    account_indexes: ix.accounts.clone().into(),
                    data: ix.data.clone().into(),
                })
                .collect::<Vec<_>>();
            TransactionMessage {
                num_signers,
                num_writable_signers: num_signers - message.header.num_readonly_signed_accounts,
                num_writable_non_signers: num_non_signers
                    - message.header.num_readonly_unsigned_accounts,
                account_keys: message.account_keys.clone().into(),
                instructions: instructions.into(),
                address_table_lookups: Vec::default().into(),
            }
        }
        VersionedMessage::V0(message) => {
            let num_accounts = message.account_keys.len() as u8;
            let num_signers = message.header.num_required_signatures;
            let num_non_signers = num_accounts - num_signers;
            let instructions = message
                .instructions
                .iter()
                .map(|ix| squads_multisig_program::CompiledInstruction {
                    program_id_index: ix.program_id_index,
                    account_indexes: ix.accounts.clone().into(),
                    data: ix.data.clone().into(),
                })
                .collect::<Vec<_>>();
            let address_table_lookups = message
                .address_table_lookups
                .iter()
                .map(|atl| squads_multisig_program::MessageAddressTableLookup {
                    account_key: atl.account_key,
                    writable_indexes: atl.writable_indexes.clone().into(),
                    readonly_indexes: atl.readonly_indexes.clone().into(),
                })
                .collect::<Vec<_>>();
            TransactionMessage {
                num_signers,
                num_writable_signers: num_signers - message.header.num_readonly_signed_accounts,
                num_writable_non_signers: num_non_signers
                    - message.header.num_readonly_unsigned_accounts,
                account_keys: message.account_keys.clone().into(),
                instructions: instructions.into(),
                address_table_lookups: address_table_lookups.into(),
            }
        }
    }
}

/// Squads bundle builder.
#[derive(Clone)]
pub struct Squads<'a, C, T> {
    client: &'a crate::Client<C>,
    multisig: Pubkey,
    vault_index: u8,
    builder: T,
    approve: bool,
    execute: bool,
}

impl<C, T> Squads<'_, C, T> {
    /// Set whether to approve the proposals.
    pub fn approve(&mut self, approve: bool) -> &mut Self {
        self.approve = approve;
        self
    }

    /// Set whether to execute the transactions.
    pub fn execute(&mut self, execute: bool) -> &mut Self {
        self.execute = execute;
        self
    }
}

impl<'a, C: Deref<Target = impl Signer> + Clone, T> MakeBundleBuilder<'a, C> for Squads<'a, C, T>
where
    T: MakeBundleBuilder<'a, C>,
{
    async fn build_with_options(
        &mut self,
        options: BundleOptions,
    ) -> crate::Result<BundleBuilder<'a, C>> {
        let inner = self.builder.build_with_options(options).await?;

        let mut luts_cache = HashMap::<_, _>::default();

        let multisig_data = get_multisig(&self.client.store_program().rpc(), &self.multisig)
            .await
            .map_err(crate::Error::unknown)?;
        let mut txn_idx = multisig_data.transaction_index;

        let mut bundle = inner.try_clone_empty()?;

        let mut transactions = vec![];
        let mut transaction_indexes = vec![];
        let mut transaction_datas = vec![];
        let mut compute_budgets = vec![];

        for mut txn in inner.into_builders() {
            txn_idx += 1;
            for (key, addresses) in txn.get_luts() {
                luts_cache.entry(*key).or_insert(AddressLookupTableAccount {
                    key: *key,
                    addresses: addresses.clone(),
                });
            }
            let message = txn.message_with_blockhash_and_options(Default::default(), true, None)?;
            let (rpc, (transaction, data)) = self
                .client
                .squads_create_vault_transaction_and_return_data(
                    &self.multisig,
                    self.vault_index,
                    txn_idx,
                    &message,
                    None,
                    false,
                )?
                .swap_output(());
            bundle.push(rpc)?;
            transactions.push(transaction);
            transaction_indexes.push(txn_idx);
            transaction_datas.push(data);
            compute_budgets.push(*txn.compute_budget_mut());
        }

        if !transactions.is_empty() {
            tracing::info!(
                start_index = multisig_data.transaction_index + 1,
                end_index = txn_idx,
                "Creating vault transactions: {transactions:#?}"
            );

            if self.approve {
                for idx in transaction_indexes.iter() {
                    let proposal = get_proposal_pda(&self.multisig, *idx, None).0;
                    bundle.push(self.client.squads_approve_proposal(
                        &self.multisig,
                        &proposal,
                        None,
                    )?)?;
                }
            }

            if self.execute {
                for (idx, data) in transaction_datas.into_iter().enumerate() {
                    let compute_budget = compute_budgets[idx];
                    let mut txn = self
                        .client
                        .squads_execute_vault_transaction(&self.multisig, data, Some(&luts_cache))
                        .await?;
                    *txn.compute_budget_mut() += compute_budget;
                    bundle.push(txn)?;
                }
            }
        }

        Ok(bundle)
    }
}

/// Extracts account metadata from the given message.
// Adapted from:
// https://github.com/Squads-Protocol/v4/blob/4f864f8ff1bfabaa0d7367ae33de085e9fe202cf/cli/src/command/vault_transaction_execute.rs#L193
pub async fn message_to_execute_account_metas(
    rpc_client: &RpcClient,
    message: VaultTransactionMessage,
    ephemeral_signer_bumps: Vec<u8>,
    vault_pda: &Pubkey,
    transaction_pda: &Pubkey,
    program_id: Option<&Pubkey>,
    luts_cache: Option<&HashMap<Pubkey, AddressLookupTableAccount>>,
) -> (Vec<AccountMeta>, Vec<AddressLookupTableAccount>) {
    let mut account_metas = Vec::with_capacity(message.account_keys.len());

    let mut address_lookup_table_accounts: Vec<AddressLookupTableAccount> = Vec::new();

    let ephemeral_signer_pdas: Vec<Pubkey> = (0..ephemeral_signer_bumps.len())
        .map(|additional_signer_index| {
            let (pda, _bump_seed) = get_ephemeral_signer_pda(
                transaction_pda,
                additional_signer_index as u8,
                program_id,
            );
            pda
        })
        .collect();

    let address_lookup_table_keys = message
        .address_table_lookups
        .iter()
        .map(|lookup| lookup.account_key)
        .collect::<Vec<_>>();

    for key in address_lookup_table_keys {
        let address_lookup_table_account = match luts_cache.as_ref().and_then(|map| map.get(&key)) {
            Some(lut) => lut.clone(),
            None => {
                let account_data = rpc_client.get_account(&key).await.unwrap().data;
                let lookup_table = AddressLookupTable::deserialize(&account_data).unwrap();

                AddressLookupTableAccount {
                    addresses: lookup_table.addresses.to_vec(),
                    key,
                }
            }
        };

        address_lookup_table_accounts.push(address_lookup_table_account);
        account_metas.push(AccountMeta::new(key, false));
    }

    for (account_index, account_key) in message.account_keys.iter().enumerate() {
        let is_writable =
            VaultTransactionMessage::is_static_writable_index(&message, account_index);
        let is_signer = VaultTransactionMessage::is_signer_index(&message, account_index)
            && !account_key.eq(vault_pda)
            && !ephemeral_signer_pdas.contains(account_key);

        account_metas.push(AccountMeta {
            pubkey: *account_key,
            is_signer,
            is_writable,
        });
    }

    for lookup in &message.address_table_lookups {
        let lookup_table_account = address_lookup_table_accounts
            .iter()
            .find(|account| account.key == lookup.account_key)
            .unwrap();

        for &account_index in &lookup.writable_indexes {
            let account_index_usize = account_index as usize;

            let pubkey = lookup_table_account
                .addresses
                .get(account_index_usize)
                .unwrap();

            account_metas.push(AccountMeta::new(*pubkey, false));
        }

        for &account_index in &lookup.readonly_indexes {
            let account_index_usize = account_index as usize;

            let pubkey = lookup_table_account
                .addresses
                .get(account_index_usize)
                .unwrap();

            account_metas.push(AccountMeta::new_readonly(*pubkey, false));
        }
    }

    (account_metas, address_lookup_table_accounts)
}
