use std::fmt;

use eventsource_stream::Eventsource;
use futures_util::{Stream, TryStreamExt};
use pyth_sdk::Identifier;
use reqwest::{Client, IntoUrl, Url};

/// Default base URL for Hermes.
pub const DEFAULT_HERMES_BASE: &str = "https://hermes.pyth.network";

/// The SSE endpoint of price updates stream.
pub const PRICE_STREAM: &str = "/v2/updates/price/stream";

/// The endpoint of latest price update.
pub const PRICE_LATEST: &str = "/v2/updates/price/latest";

/// Hermes Client.
#[derive(Debug, Clone)]
pub struct Hermes {
    base: Url,
    client: Client,
}

impl Hermes {
    /// Create a new hermes client with the given base URL.
    pub fn try_new(base: impl IntoUrl) -> crate::Result<Self> {
        Ok(Self {
            base: base.into_url()?,
            client: Client::new(),
        })
    }

    /// Get a stream of price updates.
    pub async fn price_updates(
        &self,
        feed_ids: impl IntoIterator<Item = &Identifier>,
        encoding: Option<EncodingType>,
    ) -> crate::Result<impl Stream<Item = crate::Result<PriceUpdate>>> {
        let params = get_query(feed_ids, encoding);
        let stream = self
            .client
            .get(self.base.join(PRICE_STREAM)?)
            .query(&params)
            .send()
            .await?
            .bytes_stream()
            .eventsource()
            .map_err(crate::Error::from)
            .try_filter_map(|event| {
                let update = deserialize_price_update_event(&event)
                    .inspect_err(
                        |err| tracing::warn!(%err, ?event, "deserialize price update error"),
                    )
                    .ok();
                async { Ok(update) }
            });
        Ok(stream)
    }

    /// Get latest price updates.
    pub async fn latest_price_updates(
        &self,
        feed_ids: impl IntoIterator<Item = &Identifier>,
        encoding: Option<EncodingType>,
    ) -> crate::Result<PriceUpdate> {
        let params = get_query(feed_ids, encoding);
        let update = self
            .client
            .get(self.base.join(PRICE_LATEST)?)
            .query(&params)
            .send()
            .await?
            .json()
            .await?;
        Ok(update)
    }
}

impl Default for Hermes {
    fn default() -> Self {
        Self {
            base: DEFAULT_HERMES_BASE.parse().unwrap(),
            client: Default::default(),
        }
    }
}

fn deserialize_price_update_event(event: &eventsource_stream::Event) -> crate::Result<PriceUpdate> {
    Ok(serde_json::from_str(&event.data)?)
}

/// Price Update.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PriceUpdate {
    pub(crate) binary: BinaryPriceUpdate,
    #[serde(default)]
    parsed: Vec<ParsedPriceUpdate>,
}

impl PriceUpdate {
    /// Get the parsed price udpate.
    pub fn parsed(&self) -> &[ParsedPriceUpdate] {
        &self.parsed
    }

    /// Min timestamp.
    pub fn min_timestamp(&self) -> Option<i64> {
        self.parsed
            .iter()
            .map(|update| update.price.publish_time)
            .min()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BinaryPriceUpdate {
    pub(crate) encoding: EncodingType,
    pub(crate) data: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, serde::Deserialize, serde::Serialize)]
pub enum EncodingType {
    /// Hex.
    #[default]
    #[serde(rename = "hex")]
    Hex,
    /// Base64.
    #[serde(rename = "base64")]
    Base64,
}

impl fmt::Display for EncodingType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hex => write!(f, "hex"),
            Self::Base64 => write!(f, "base64"),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ParsedPriceUpdate {
    id: String,
    price: Price,
    ema_price: Price,
    metadata: Metadata,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Price {
    #[serde(with = "pyth_sdk::utils::as_string")]
    price: i64,
    #[serde(with = "pyth_sdk::utils::as_string")]
    conf: u64,
    expo: i32,
    publish_time: i64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Metadata {
    slot: Option<u64>,
    proof_available_time: Option<i64>,
    prev_publish_time: Option<i64>,
}

fn get_query<'a>(
    feed_ids: impl IntoIterator<Item = &'a Identifier>,
    encoding: Option<EncodingType>,
) -> Vec<(&'static str, String)> {
    feed_ids
        .into_iter()
        .map(|id| ("ids[]", id.to_hex()))
        .chain(encoding.map(|encoding| ("encoding", encoding.to_string())))
        .collect()
}
