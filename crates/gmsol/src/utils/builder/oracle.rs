use std::{future::Future, ops::Deref};

use anchor_client::solana_sdk::{pubkey::Pubkey, signer::Signer};
use gmsol_store::states::{common::TokensWithFeed, PriceProviderKind};

use crate::utils::{RpcBuilder, TransactionBuilder};

use super::{MakeTransactionBuilder, SetExecutionFee};

/// A mapping from feed id to the corresponding feed address.
pub type FeedAddressMap = std::collections::HashMap<Pubkey, Pubkey>;

/// Build with pull oracle instructions.
pub struct WithPullOracle<O, T>
where
    O: PullOracle,
{
    builder: T,
    pull_oracle: O,
    price_updates: O::PriceUpdates,
}

impl<O: PullOracle, T> WithPullOracle<O, T> {
    /// Construct transactions with the given pull oracle and price updates.
    pub fn with_price_updates(pull_oracle: O, builder: T, price_updates: O::PriceUpdates) -> Self {
        Self {
            builder,
            pull_oracle,
            price_updates,
        }
    }

    /// Fetch the required price updates and use them to construct transactions.
    pub async fn new(pull_oracle: O, mut builder: T) -> crate::Result<Self>
    where
        T: PullOraclePriceConsumer,
    {
        let feed_ids = builder.feed_ids().await?;
        let price_updates = pull_oracle.fetch_price_updates(&feed_ids).await?;

        Ok(Self::with_price_updates(
            pull_oracle,
            builder,
            price_updates,
        ))
    }
}

impl<'a, C: Deref<Target = impl Signer> + Clone, O, T> MakeTransactionBuilder<'a, C>
    for WithPullOracle<O, T>
where
    O: PullOracleOps<'a, C>,
    T: PullOraclePriceConsumer + MakeTransactionBuilder<'a, C>,
{
    async fn build(&mut self) -> crate::Result<TransactionBuilder<'a, C>> {
        let (instructions, map) = self
            .pull_oracle
            .fetch_price_update_instructions(&self.price_updates)
            .await?;

        self.builder
            .process_feeds(self.pull_oracle.provider_kind(), map)?;

        let consume = self.builder.build().await?;

        let PriceUpdateInstructions {
            post: mut tx,
            close,
        } = instructions;

        tx.append(consume, false)?;
        tx.append(close, true)?;

        Ok(tx)
    }
}

impl<O: PullOracle, T: SetExecutionFee> SetExecutionFee for WithPullOracle<O, T> {
    fn set_execution_fee(&mut self, lamports: u64) -> &mut Self {
        self.builder.set_execution_fee(lamports);
        self
    }
}

/// Feed IDs.
pub type FeedIds = TokensWithFeed;

/// Pull Oracle.
pub trait PullOracle {
    /// Price Updates.
    type PriceUpdates;

    /// Get price provider kind.
    fn provider_kind(&self) -> PriceProviderKind;

    /// Fetch Price Update.
    fn fetch_price_updates(
        &self,
        feed_ids: &FeedIds,
    ) -> impl Future<Output = crate::Result<Self::PriceUpdates>>;
}

/// Price Update Instructions.
pub struct PriceUpdateInstructions<'a, C> {
    post: TransactionBuilder<'a, C>,
    close: TransactionBuilder<'a, C>,
}

impl<'a, C: Deref<Target = impl Signer> + Clone> PriceUpdateInstructions<'a, C> {
    /// Create a new empty price update instructions.
    pub fn new(client: &'a crate::Client<C>) -> Self {
        Self {
            post: client.transaction(),
            close: client.transaction(),
        }
    }
}

impl<'a, C: Deref<Target = impl Signer> + Clone> PriceUpdateInstructions<'a, C> {
    /// Push a post price update instruction.
    pub fn try_push_post(
        &mut self,
        instruction: RpcBuilder<'a, C>,
    ) -> Result<(), (RpcBuilder<'a, C>, crate::Error)> {
        self.post.try_push(instruction)?;
        Ok(())
    }

    /// Push a close instruction.
    pub fn try_push_close(
        &mut self,
        instruction: RpcBuilder<'a, C>,
    ) -> Result<(), (RpcBuilder<'a, C>, crate::Error)> {
        self.close.try_push(instruction)?;
        Ok(())
    }
}

/// Pull Oracle Operations.
pub trait PullOracleOps<'a, C>: PullOracle {
    /// Fetch instructions to post the price updates.
    fn fetch_price_update_instructions(
        &self,
        price_updates: &Self::PriceUpdates,
    ) -> impl Future<Output = crate::Result<(PriceUpdateInstructions<'a, C>, FeedAddressMap)>>;
}

/// Pull Oracle Price Consumer.
pub trait PullOraclePriceConsumer {
    /// Returns a reference to tokens and their associated feed IDs that require price updates.
    fn feed_ids(&mut self) -> impl Future<Output = crate::Result<FeedIds>>;

    /// Processes the feed address map returned from the pull oracle.
    fn process_feeds(
        &mut self,
        provider: PriceProviderKind,
        map: FeedAddressMap,
    ) -> crate::Result<()>;
}
