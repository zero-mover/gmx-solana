use std::{
    collections::HashMap,
    iter::{Peekable, Zip},
    slice::Iter,
};

use anchor_client::solana_sdk::{instruction::AccountMeta, pubkey::Pubkey};
use data_store::states::{common::TokensWithFeed, PriceProviderKind};

use crate::pyth::find_pyth_feed_account;

type Parser = Box<dyn Fn(Pubkey) -> crate::Result<AccountMeta>>;

/// Feeds parser.
pub struct FeedsParser {
    parsers: HashMap<PriceProviderKind, Parser>,
}

impl Default for FeedsParser {
    fn default() -> Self {
        Self {
            parsers: HashMap::from([(
                PriceProviderKind::Pyth,
                Box::new(|feed: Pubkey| {
                    let pubkey = find_pyth_feed_account(0, feed.to_bytes()).0;
                    Ok(AccountMeta {
                        pubkey,
                        is_signer: false,
                        is_writable: false,
                    })
                }) as Parser,
            )]),
        }
    }
}

impl FeedsParser {
    /// Parse a [`TokensWithFeed`]
    pub fn parse<'a>(
        &'a self,
        tokens_with_feed: &'a TokensWithFeed,
    ) -> impl Iterator<Item = crate::Result<AccountMeta>> + 'a {
        FeedAccountMetas::new(tokens_with_feed)
            .map(|res| res.and_then(|(provider, feed)| self.dispatch(&provider, &feed)))
    }

    fn dispatch(&self, provider: &PriceProviderKind, feed: &Pubkey) -> crate::Result<AccountMeta> {
        let Some(parser) = self.parsers.get(provider) else {
            return Ok(AccountMeta {
                pubkey: *feed,
                is_signer: false,
                is_writable: false,
            });
        };
        (parser)(*feed)
    }
}

#[cfg(feature = "pyth-pull-oracle")]
mod pyth_pull_oracle {
    use pyth_sdk::Identifier;

    use super::*;
    use crate::pyth::pull_oracle::Prices;

    impl FeedsParser {
        /// Parse Pyth feeds with price updates map.
        pub fn with_pyth_price_updates(&mut self, price_updates: &Prices) -> &mut Self {
            let price_updates = price_updates.clone();
            self.parsers.insert(
                PriceProviderKind::Pyth,
                Box::new(move |feed| {
                    let feed_id = Identifier::new(feed.to_bytes());
                    let price_update = price_updates.get(&feed_id).ok_or_else(|| {
                        crate::Error::invalid_argument(format!(
                            "price update account for {feed_id}"
                        ))
                    })?;

                    Ok(AccountMeta {
                        pubkey: *price_update,
                        is_signer: false,
                        is_writable: false,
                    })
                }),
            );
            self
        }
    }
}

/// Feed account metas.
pub struct FeedAccountMetas<'a> {
    provider_with_lengths: Peekable<Zip<Iter<'a, u8>, Iter<'a, u16>>>,
    feeds: Iter<'a, Pubkey>,
    current: usize,
    failed: bool,
}

impl<'a> FeedAccountMetas<'a> {
    /// Create from [`TokensWithFeed`].
    pub fn new(token_with_feeds: &'a TokensWithFeed) -> Self {
        let providers = token_with_feeds.providers.iter();
        let nums = token_with_feeds.nums.iter();
        let provider_with_lengths = providers.zip(nums).peekable();
        let feeds = token_with_feeds.feeds.iter();
        Self {
            feeds,
            provider_with_lengths,
            current: 0,
            failed: false,
        }
    }
}

impl<'a> Iterator for FeedAccountMetas<'a> {
    type Item = crate::Result<(PriceProviderKind, Pubkey)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        loop {
            let (provider, length) = self.provider_with_lengths.peek()?;
            if self.current == (**length as usize) {
                self.provider_with_lengths.next();
                self.current = 0;
                continue;
            }
            let Ok(provider) = PriceProviderKind::try_from(**provider) else {
                self.failed = true;
                return Some(Err(crate::Error::invalid_argument(
                    "invalid provider index",
                )));
            };
            let Some(feed) = self.feeds.next() else {
                return Some(Err(crate::Error::invalid_argument("not enough feeds")));
            };
            self.current += 1;
            return Some(Ok((provider, *feed)));
        }
    }
}
