use std::borrow::Borrow;

use anchor_client::{
    solana_client::{
        nonblocking::rpc_client::RpcClient, rpc_client::GetConfirmedSignaturesForAddress2Config,
        rpc_config::RpcTransactionConfig,
    },
    solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Signature},
    ClientError,
};
use async_stream::{stream, try_stream};
use futures_util::Stream;
use gmsol_decode::decoder::{CPIEvents, TransactionDecoder};
use solana_transaction_status::UiTransactionEncoding;

use crate::utils::WithSlot;

/// Fetch transaction history for an address.
pub async fn fetch_transaction_history_with_config(
    client: impl Borrow<RpcClient>,
    address: &Pubkey,
    commitment: CommitmentConfig,
    until: Option<Signature>,
    mut before: Option<Signature>,
    batch: Option<usize>,
) -> crate::Result<impl Stream<Item = crate::Result<WithSlot<Signature>>>> {
    let limit = batch;
    let commitment = Some(commitment);
    let address = *address;

    let stream = try_stream! {
        loop {
            let txns = client.borrow().get_signatures_for_address_with_config(&address, GetConfirmedSignaturesForAddress2Config {
                before,
                until,
                limit,
                commitment,
            }).await.map_err(ClientError::from)?;
            match txns.last() {
                Some(next) => {
                    let next = next.signature.parse().map_err(crate::Error::unknown)?;
                    for txn in txns {
                        let slot = txn.slot;
                        let signature = txn.signature.parse().map_err(crate::Error::unknown)?;
                        yield WithSlot::new(slot, signature);
                    }
                    before = Some(next);
                },
                None => {
                    break;
                }
            }
        }
    };
    Ok(stream)
}

/// Encoded CPI Events.
#[derive(Debug, Clone)]
pub struct EncodedCPIEvents {
    program_id: Pubkey,
    signature: Signature,
    events: Vec<Vec<u8>>,
}

impl EncodedCPIEvents {
    /// Get the program id.
    pub fn program_id(&self) -> &Pubkey {
        &self.program_id
    }

    /// Get the transaction signature.
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// Get events.
    pub fn events(&self) -> &[Vec<u8>] {
        &self.events
    }

    #[cfg(feature = "decode")]
    pub fn decode<T: crate::decode::Decode>(&self) -> impl Iterator<Item = crate::Result<T>> + '_ {
        use crate::decode::value::OwnedDataDecoder;
        self.events.iter().map(|data| {
            let decoder = OwnedDataDecoder::new(&self.program_id, data);
            Ok(T::decode(decoder)?)
        })
    }
}

/// Extract encoded CPI events from transaction history.
pub fn extract_cpi_events(
    stream: impl Stream<Item = crate::Result<WithSlot<Signature>>>,
    client: impl Borrow<RpcClient>,
    program_id: &Pubkey,
    event_authority: &Pubkey,
    commitment: CommitmentConfig,
    max_supported_transaction_version: Option<u8>,
) -> impl Stream<Item = crate::Result<WithSlot<CPIEvents>>> {
    let program_id = *program_id;
    let event_authority = *event_authority;
    stream! {
        for await res in stream {
            match res {
                Ok(ctx) => {
                    let signature = *ctx.value();
                    tracing::debug!(%signature, "fetching transaction");
                    let tx = client
                        .borrow()
                        .get_transaction_with_config(
                            &signature,
                            RpcTransactionConfig {
                                encoding: Some(UiTransactionEncoding::Base64),
                                commitment: Some(commitment),
                                max_supported_transaction_version,
                            },
                        )
                        .await
                        .map_err(ClientError::from)?;
                    let mut decoder = TransactionDecoder::new(signature, &tx);
                    match decoder
                        .add_cpi_event_authority_and_program_id(event_authority, program_id)
                        .and_then(|decoder| decoder.extract_cpi_events())
                    {
                        Ok(events) => {
                            yield Ok(ctx.map(|_| events));
                        },
                        Err(err) => {
                            yield Err(err.into());
                        }
                    }
                },
                Err(err) => {
                    yield Err(err);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;

    use super::*;
    use crate::test::{default_cluster, setup_fmt_tracing};

    #[tokio::test]
    async fn test_transaction_hisotry_fetching() -> eyre::Result<()> {
        let _guard = setup_fmt_tracing("info");
        let cluster = default_cluster();
        let client = RpcClient::new(cluster.url().to_string());
        let stream = fetch_transaction_history_with_config(
            client,
            &crate::program_ids::DEFAULT_GMSOL_STORE_ID,
            CommitmentConfig::confirmed(),
            None,
            None,
            Some(5),
        )
        .await?
        .take(5);
        futures_util::pin_mut!(stream);
        while let Some(Ok(signature)) = stream.next().await {
            tracing::info!("{signature:?}");
        }
        Ok(())
    }
}
