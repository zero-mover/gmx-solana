use std::{
    collections::{hash_map::Entry, HashMap},
    fmt,
    future::Future,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use anchor_client::{
    solana_client::rpc_config::RpcSendTransactionConfig,
    solana_sdk::{
        address_lookup_table::AddressLookupTableAccount,
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        signature::{Keypair, Signature},
        signer::{EncodableKey, Signer},
        system_instruction,
    },
    Cluster,
};
use event_listener::Event;
use eyre::{eyre, OptionExt};
use gmsol::{
    alt::AddressLookupTableOps,
    client::SystemProgramOps,
    constants::MARKET_USD_UNIT,
    exchange::ExchangeOps,
    pyth::{pull_oracle::ExecuteWithPythPrices, Hermes, PythPullOracle},
    store::{
        gt::GTOps, market::MarketOps, oracle::OracleOps, roles::RolesOps, store_ops::StoreOps,
        token_config::TokenConfigOps,
    },
    types::{FactorKey, MarketConfigKey, PriceProviderKind, RoleKey, TokenConfigBuilder},
    utils::{shared_signer, SignerRef, TransactionBuilder},
    Client, ClientOptions,
};
use pyth_sdk::Identifier;
use rand::{rngs::StdRng, CryptoRng, RngCore, SeedableRng};
use tokio::{
    sync::{Mutex, OnceCell, OwnedMutexGuard},
    time::sleep,
};
use tracing::{level_filters::LevelFilter, Instrument};
use tracing_subscriber::EnvFilter;

const ENV_ANCHOR_PROVIDER: &str = "ANCHOR_PROVIDER_URL";
const ENV_ANCHOR_WALLET: &str = "ANCHOR_WALLET";
const ENV_GMSOL_RANDOM_STORE: &str = "GMSOL_RANDOM_STORE";
const ENV_GMSOL_RNG: &str = "GMSOL_RNG";
const ENV_GMSOL_KEEPER: &str = "GMSOL_KEEPER";
const ENV_GMSOL_REFUND_WAIT: &str = "GMSOL_REFUND_WAIT";

/// Deployment.
pub struct Deployment {
    rng: StdRng,
    /// Hermes.
    pub hermes: Hermes,
    /// Pyth Oracle.
    pub pyth: PythPullOracle<SignerRef>,
    /// Client.
    pub client: Client<SignerRef>,
    /// Users.
    pub users: Users,
    /// Store key.
    pub store_key: String,
    /// Store.
    pub store: Pubkey,
    /// Token Map.
    token_map: Keypair,
    /// Oracle index.
    pub oracle_index: u8,
    /// Oracle.
    pub oracle: Pubkey,
    /// GT mint.
    pub gt: Pubkey,
    /// Tokens.
    tokens: HashMap<String, Token>,
    /// Synthetic tokens.
    synthetic_tokens: HashMap<String, Token>,
    /// Market tokens.
    market_tokens: HashMap<[String; 3], Pubkey>,
    common_alt: AddressLookupTableAccount,
    market_alt: AddressLookupTableAccount,
}

impl fmt::Debug for Deployment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Deployment")
            .field("cluster", self.client.cluster())
            .field("payer", &self.client.payer())
            .field("users", &self.users)
            .field("store", &self.store)
            .field("token_map", &self.token_map.pubkey())
            .field("oracle", &self.oracle)
            .field("tokens", &self.tokens)
            .field("synthetic_tokens", &self.synthetic_tokens)
            .finish_non_exhaustive()
    }
}

impl Deployment {
    /// Default user.
    pub const DEFAULT_USER: &'static str = "user_0";

    /// Default keeper.
    pub const DEFAULT_KEEPER: &'static str = "keeper_0";

    /// Market selector for liquidation test.
    pub const SELECT_LIQUIDATION_MARKET: [&'static str; 3] = ["fBTC", "USDG", "fBTC"];

    /// Market selector for ADL test.
    pub const SELECT_ADL_MARKET: [&'static str; 3] = ["SOL", "USDG", "fBTC"];

    const SOL_PYTH_FEED_ID: [u8; 32] = [
        0xef, 0x0d, 0x8b, 0x6f, 0xda, 0x2c, 0xeb, 0xa4, 0x1d, 0xa1, 0x5d, 0x40, 0x95, 0xd1, 0xda,
        0x39, 0x2a, 0x0d, 0x2f, 0x8e, 0xd0, 0xc6, 0xc7, 0xbc, 0x0f, 0x4c, 0xfa, 0xc8, 0xc2, 0x80,
        0xb5, 0x6d,
    ];
    const BTC_PYTH_FEED_ID: [u8; 32] = [
        0xe6, 0x2d, 0xf6, 0xc8, 0xb4, 0xa8, 0x5f, 0xe1, 0xa6, 0x7d, 0xb4, 0x4d, 0xc1, 0x2d, 0xe5,
        0xdb, 0x33, 0x0f, 0x7a, 0xc6, 0x6b, 0x72, 0xdc, 0x65, 0x8a, 0xfe, 0xdf, 0x0f, 0x4a, 0x41,
        0x5b, 0x43,
    ];
    const USDC_PYTH_FEED_ID: [u8; 32] = [
        0xea, 0xa0, 0x20, 0xc6, 0x1c, 0xc4, 0x79, 0x71, 0x28, 0x13, 0x46, 0x1c, 0xe1, 0x53, 0x89,
        0x4a, 0x96, 0xa6, 0xc0, 0x0b, 0x21, 0xed, 0x0c, 0xfc, 0x27, 0x98, 0xd1, 0xf9, 0xa9, 0xe9,
        0xc9, 0x4a,
    ];

    async fn connect() -> eyre::Result<Self> {
        let mut rng = Self::get_rng()?;
        let (client, store_key, store) = Self::get_client_and_store().await?;
        let oracle_index = 255;
        let oracle = client.find_oracle_address(&store, oracle_index);
        let token_map = Keypair::generate(&mut rng);

        let (rpc, common_alt) = client.create_alt().await?;
        let signature = rpc.send_without_preflight().await?;
        tracing::info!(%common_alt, %signature, "created an ALT for common addresses");

        let (rpc, market_alt) = client.create_alt().await?;
        let signature = rpc.send_without_preflight().await?;
        tracing::info!(%market_alt, %signature, "created an ALT for market addresses");

        Ok(Self {
            users: Users::new(&mut rng),
            rng,
            hermes: Default::default(),
            pyth: PythPullOracle::try_new(&client)?,
            client,
            store_key,
            store,
            token_map,
            oracle_index,
            oracle,
            gt: Default::default(),
            tokens: Default::default(),
            synthetic_tokens: Default::default(),
            market_tokens: Default::default(),
            common_alt: AddressLookupTableAccount {
                key: common_alt,
                addresses: vec![],
            },
            market_alt: AddressLookupTableAccount {
                key: market_alt,
                addresses: vec![],
            },
        })
    }

    async fn init() -> eyre::Result<Self> {
        Self::init_tracing()?;

        let mut deployment = Self::connect().await?;

        deployment.setup().await?;

        Ok(deployment)
    }

    async fn setup(&mut self) -> eyre::Result<()> {
        tracing::info!("[Setting up everything...]");
        self.add_users()?;

        let _guard = self.use_accounts().await?;

        self.create_tokens([
            (
                "fBTC",
                TokenConfig {
                    decimals: 6,
                    pyth_feed_id: Identifier::new(Self::BTC_PYTH_FEED_ID),
                    precision: 3,
                },
            ),
            (
                "USDG",
                TokenConfig {
                    decimals: 8,
                    pyth_feed_id: Identifier::new(Self::USDC_PYTH_FEED_ID),
                    precision: 6,
                },
            ),
        ])
        .await?;
        self.add_synthetic_tokens([(
            "SOL",
            Pubkey::default(),
            TokenConfig {
                decimals: 9,
                pyth_feed_id: Identifier::new(Self::SOL_PYTH_FEED_ID),
                precision: 4,
            },
        )]);
        self.create_token_accounts().await?;
        self.initialize_store().await?;
        self.initialize_token_map().await?;
        self.initialize_markets([
            ["SOL", "WSOL", "USDG"],
            ["SOL", "WSOL", "WSOL"],
            ["fBTC", "fBTC", "USDG"],
            ["fBTC", "WSOL", "USDG"],
            ["SOL", "fBTC", "fBTC"],
            // For liquidation test
            Self::SELECT_LIQUIDATION_MARKET,
            // For ADL test
            Self::SELECT_ADL_MARKET,
        ])
        .await?;

        self.initialize_alts().await?;

        self.initialize_gt(7).await?;

        Ok(())
    }

    async fn get_client_and_store() -> eyre::Result<(Client<SignerRef>, String, Pubkey)> {
        use rand::{distributions::Alphanumeric, thread_rng, Rng};
        use std::env;

        let endpoint = env::var(ENV_ANCHOR_PROVIDER)
            .map_err(|_| eyre!("env `{ENV_ANCHOR_PROVIDER}` is not set"))?;
        let wallet = env::var(ENV_ANCHOR_WALLET)
            .map_err(|_| eyre!("env `{ENV_ANCHOR_WALLET}` is not set"))?;
        let wallet = shellexpand::full(&wallet)?;

        let random_store =
            env::var(ENV_GMSOL_RANDOM_STORE).is_ok() || endpoint == Cluster::Devnet.url();
        let store_key = random_store
            .then(|| {
                let mut rng = thread_rng();
                (&mut rng)
                    .sample_iter(Alphanumeric)
                    .take(16)
                    .map(char::from)
                    .collect::<String>()
            })
            .unwrap_or_default();

        let client = Client::new_with_options(
            endpoint.parse().map_err(eyre::Error::msg)?,
            shared_signer(
                Keypair::read_from_file(&*wallet)
                    .map_err(|err| eyre::Error::msg(err.to_string()))?,
            ),
            ClientOptions::builder()
                .commitment(CommitmentConfig::confirmed())
                .build(),
        )?;
        let store = client.find_store_address(&store_key);
        Ok((client, store_key, store))
    }

    fn get_rng() -> eyre::Result<StdRng> {
        match std::env::var(ENV_GMSOL_RNG) {
            Ok(value) => {
                let seed: u64 = value.parse()?;
                Ok(StdRng::seed_from_u64(seed))
            }
            Err(_) => Ok(StdRng::from_entropy()),
        }
    }

    fn init_tracing() -> eyre::Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::builder()
                    .with_default_directive(LevelFilter::ERROR.into())
                    .from_env_lossy(),
            )
            .try_init()
            .map_err(eyre::Error::msg)?;
        Ok(())
    }

    fn add_users(&mut self) -> eyre::Result<()> {
        self.users.add_user(Self::DEFAULT_USER, &mut self.rng);

        match std::env::var(ENV_GMSOL_KEEPER) {
            Ok(path) => {
                let path = shellexpand::full(&path)?;
                let keypair = Keypair::read_from_file(&*path)
                    .map_err(|err| eyre::Error::msg(err.to_string()))?;
                self.users
                    .add_user_with_keypair(Self::DEFAULT_KEEPER, keypair);
            }
            Err(_) => {
                self.users.add_user(Self::DEFAULT_KEEPER, &mut self.rng);
            }
        }

        Ok(())
    }

    fn add_synthetic_tokens<T: ToString>(
        &mut self,
        configs: impl IntoIterator<Item = (T, Pubkey, TokenConfig)>,
    ) {
        for (name, address, config) in configs {
            self.synthetic_tokens
                .insert(name.to_string(), Token { address, config });
        }
    }

    async fn create_tokens<T: ToString>(
        &mut self,
        configs: impl IntoIterator<Item = (T, TokenConfig)>,
    ) -> eyre::Result<()> {
        use spl_token::native_mint;

        self.tokens = self.do_create_tokens(configs).await?;
        if let Entry::Vacant(entry) = self.tokens.entry("WSOL".to_string()) {
            entry.insert(Token {
                address: native_mint::ID,
                config: TokenConfig {
                    decimals: native_mint::DECIMALS,
                    pyth_feed_id: Identifier::new(Self::SOL_PYTH_FEED_ID),
                    precision: 4,
                },
            });
        }
        Ok(())
    }

    async fn do_create_tokens<T>(
        &mut self,
        configs: impl IntoIterator<Item = (T, TokenConfig)>,
    ) -> eyre::Result<HashMap<String, Token>>
    where
        T: ToString,
    {
        use anchor_spl::token::{Mint, ID};
        use spl_token::instruction;

        let client = self.client.data_store().solana_rpc();
        let rent = client
            .get_minimum_balance_for_rent_exemption(Mint::LEN)
            .await?;
        let mut builder = TransactionBuilder::new(client);

        let tokens = configs
            .into_iter()
            .map(|(name, config)| (name.to_string(), (Keypair::generate(&mut self.rng), config)))
            .collect::<HashMap<_, _>>();

        let payer = self.client.payer();

        for (name, (token, config)) in tokens.iter() {
            let pubkey = token.pubkey();
            tracing::info!(%name, "creating mint account {pubkey}");
            let rpc = self
                .client
                .data_store_rpc()
                .signer(token)
                .pre_instruction(system_instruction::create_account(
                    &payer,
                    &pubkey,
                    rent,
                    Mint::LEN as u64,
                    &ID,
                ))
                .pre_instruction(instruction::initialize_mint2(
                    &ID,
                    &token.pubkey(),
                    &payer,
                    None,
                    config.decimals,
                )?);
            builder.try_push(rpc).map_err(|(_, err)| err)?;
        }

        match builder.send_all().await {
            Ok(signatures) => {
                tracing::info!("created tokens with {signatures:#?}");
            }
            Err((signatures, err)) => {
                tracing::error!(%err, "failed to create tokens, successful txns: {signatures:#?}");
            }
        }

        Ok(tokens
            .into_iter()
            .map(|(name, (keypair, config))| {
                (
                    name,
                    Token {
                        address: keypair.pubkey(),
                        config,
                    },
                )
            })
            .collect())
    }

    async fn create_token_accounts(&self) -> eyre::Result<()> {
        use anchor_spl::token::ID;
        use spl_associated_token_account::instruction;

        let client = self.client.data_store().solana_rpc();
        let mut builder = TransactionBuilder::new(client);

        let payer = self.client.payer();

        for (name, token) in self.tokens.iter() {
            for user in self.users.keypairs().await {
                let pubkey = user.pubkey();
                tracing::info!(token=%name, mint=%token.address, "creating token account for {pubkey}");
                let rpc = self.client.data_store_rpc().pre_instruction(
                    instruction::create_associated_token_account(
                        &payer,
                        &pubkey,
                        &token.address,
                        &ID,
                    ),
                );
                builder.try_push(rpc).map_err(|(_, err)| err)?;
            }
        }

        match builder.send_all().await {
            Ok(signatures) => {
                tracing::info!("created token accounts with {signatures:#?}");
            }
            Err((signatures, err)) => {
                tracing::error!(%err, "failed to create token accounts, successful txns: {signatures:#?}");
            }
        }

        Ok(())
    }

    async fn initialize_store(&self) -> eyre::Result<()> {
        let client = &self.client;
        let store = &self.store;
        let controller = client.controller_address(store);
        let keeper_keypair = self
            .user_keypair(Self::DEFAULT_KEEPER)
            .ok_or_eyre("the default keeper is not initialized")?;
        let keeper = keeper_keypair.pubkey();

        let mut builder = client.transaction();

        builder
            .push(client.initialize_store(&self.store_key, None))?
            .push(client.initialize_controller(store))?
            .push_many(
                [
                    RoleKey::CONTROLLER,
                    RoleKey::MARKET_KEEPER,
                    RoleKey::ORDER_KEEPER,
                ]
                .iter()
                .map(|role| client.enable_role(store, role)),
                false,
            )?
            .push(client.grant_role(store, &controller, RoleKey::CONTROLLER))?
            .push(client.grant_role(store, &keeper, RoleKey::MARKET_KEEPER))?
            .push(client.grant_role(store, &keeper, RoleKey::ORDER_KEEPER))?;

        _ = builder
            .send_all()
            .await.
            inspect(|signatures| {
                tracing::info!("initialized store with txns: {signatures:#?}");
            })
            .inspect_err(|(signatures, err)| {
                tracing::error!(%err, "failed to initialize store, successful txns: {signatures:#?}");
            });

        Ok(())
    }

    async fn initialize_token_map(&self) -> eyre::Result<()> {
        let client = self.user_client(Self::DEFAULT_KEEPER)?;
        let store = &self.store;

        let mut builder = client.transaction();

        let (rpc, address) = client.initialize_token_map(store, &self.token_map);
        builder
            .push(rpc)?
            .push(client.set_token_map(store, &address))?
            .push_many(
                self.tokens
                    .iter()
                    .map(|(name, token)| (name, token, false))
                    .chain(
                        self.synthetic_tokens
                            .iter()
                            .map(|(name, token)| (name, token, true)),
                    )
                    .map(|(name, token, synthetic)| {
                        let config = TokenConfigBuilder::default()
                            .update_price_feed(
                                &PriceProviderKind::Pyth,
                                Pubkey::new_from_array(token.config.pyth_feed_id.to_bytes()),
                            )?
                            .with_expected_provider(PriceProviderKind::Pyth)
                            .with_precision(token.config.precision);
                        if synthetic {
                            Ok(client.insert_synthetic_token_config(
                                store,
                                &address,
                                name,
                                &token.address,
                                token.config.decimals,
                                config,
                                true,
                                true,
                            ))
                        } else {
                            Ok(client.insert_token_config(
                                store,
                                &address,
                                name,
                                &token.address,
                                config,
                                true,
                                true,
                            ))
                        }
                    })
                    .collect::<eyre::Result<Vec<_>>>()?,
                false,
            )?
            .push(client.initialize_oracle(store, self.oracle_index).0)?;

        _ = builder
            .send_all()
            .await.
            inspect(|signatures| {
                tracing::info!("initialized token map with txns: {signatures:#?}");
            })
            .inspect_err(|(signatures, err)| {
                tracing::error!(%err, "failed to initialize token map, successful txns: {signatures:#?}");
            });

        Ok(())
    }

    async fn initialize_markets<T: AsRef<str>>(
        &mut self,
        triples: impl IntoIterator<Item = [T; 3]>,
    ) -> eyre::Result<()> {
        let market_triples = triples.into_iter().filter_map(|[index, long, short]| {
            let index_token = self
                .synthetic_tokens
                .get(index.as_ref())
                .or(self.tokens.get(index.as_ref()))?;
            let long_token = self.tokens.get(long.as_ref())?;
            let short_token = self.tokens.get(short.as_ref())?;
            let name = [
                index.as_ref().to_string(),
                long.as_ref().to_string(),
                short.as_ref().to_string(),
            ];
            Some((
                name,
                [index_token.address, long_token.address, short_token.address],
            ))
        });

        let client = self.user_client(Self::DEFAULT_KEEPER)?;
        let mut builder = client.transaction();
        let store = &self.store;
        let token_map = self.token_map.pubkey();
        for (name, [index, long, short]) in market_triples {
            let market_name = format!("{}/USD[{}-{}]", name[0], name[1], name[2]);
            let Entry::Vacant(entry) = self.market_tokens.entry(name) else {
                continue;
            };
            let (rpc, market_token) = client
                .create_market(
                    store,
                    &market_name,
                    &index,
                    &long,
                    &short,
                    true,
                    Some(&token_map),
                )
                .await?;
            builder.push(rpc)?;
            entry.insert(market_token);
            let rpc = client.update_market_config_by_key(
                store,
                &market_token,
                MarketConfigKey::MaxPoolAmountForLongToken,
                &1_000_000_000_000_000_000,
            )?;
            builder.push(rpc)?;
            let rpc = client.update_market_config_by_key(
                store,
                &market_token,
                MarketConfigKey::MaxPoolAmountForShortToken,
                &1_000_000_000_000_000_000,
            )?;
            builder.push(rpc)?;
            let rpc = client.toggle_gt_minting(store, &market_token, true);
            builder.push(rpc)?;
        }
        _ = builder
            .send_all()
            .await
            .inspect(|signatures| {
                tracing::info!("created markets with txns: {signatures:#?}");
            })
            .inspect_err(|(signatures, err)| {
                tracing::error!(%err, "failed to create markets, successful txns: {signatures:#?}");
            });
        Ok(())
    }

    async fn initialize_alts(&mut self) -> eyre::Result<()> {
        debug_assert!(self.common_alt.addresses.is_empty());
        debug_assert!(self.market_alt.addresses.is_empty());

        // Init common ALT.
        let event_authority = self.client.data_store_event_authority();
        let gt_mint = self.client.find_gt_mint_address(&self.store);
        let mut addresses = vec![
            self.store,
            self.token_map(),
            self.oracle,
            event_authority,
            gt_mint,
            anchor_spl::token::ID,
            anchor_spl::token_2022::ID,
            anchor_spl::associated_token::ID,
            anchor_client::anchor_lang::system_program::ID,
        ];

        for token in self.tokens.values() {
            addresses.push(token.address);
            addresses.push(
                self.client
                    .find_market_vault_address(&self.store, &token.address),
            );
        }

        let signature = self
            .client
            .extend_alt(&self.common_alt.key, addresses.clone())
            .send_without_preflight()
            .await?;

        tracing::info!(len=%addresses.len(), %signature, "common ALT extended");
        self.common_alt.addresses = addresses;

        // Init market ALT.
        let mut addresses = vec![];
        for market_token in self.market_tokens.values() {
            let market = self.client.find_market_address(&self.store, market_token);
            addresses.push(market);
            addresses.push(*market_token);
            addresses.push(
                self.client
                    .find_market_vault_address(&self.store, market_token),
            );
        }

        let signature = self
            .client
            .extend_alt(&self.market_alt.key, addresses.clone())
            .send_without_preflight()
            .await?;

        tracing::info!(len=%addresses.len(), %signature, "market ALT extended");
        self.market_alt.addresses = addresses;

        Ok(())
    }

    async fn initialize_gt(&mut self, decimals: u8) -> eyre::Result<()> {
        let client = self.user_client(Self::DEFAULT_KEEPER)?;
        let store = &self.store;
        let gt = client.find_gt_mint_address(store);
        self.gt = gt;

        let mut tx = client.transaction();

        let gt_unit = 10u64.pow(decimals as u32);

        tx.push(client.initialize_gt(
            store,
            decimals,
            100 * MARKET_USD_UNIT / 10u128.pow(decimals as u32),
            101 * MARKET_USD_UNIT / 100,
            10 * gt_unit,
            vec![
                30 * gt_unit,
                100 * gt_unit,
                300 * gt_unit,
                1_000 * gt_unit,
                3_000 * gt_unit,
                10_000 * gt_unit,
                30_000 * gt_unit,
                100_000 * gt_unit,
                300_000 * gt_unit,
            ],
        ))?
        .push(client.insert_factor(
            store,
            FactorKey::GtMintingCostReferredDiscount,
            10 * MARKET_USD_UNIT / 100,
        ))?
        .push(
            client.gt_set_order_fee_discount_factors(
                store,
                [0, 25, 50, 75, 100, 125, 150, 175, 200, 225]
                    .iter()
                    .map(|base| *base * MARKET_USD_UNIT / 1_000)
                    .collect(),
            ),
        )?
        .push(
            client.gt_set_referral_reward_factors(
                store,
                [5, 10, 11, 12, 13, 15, 18, 23, 31, 44]
                    .iter()
                    .map(|base| *base * MARKET_USD_UNIT / 100)
                    .collect(),
            ),
        )?;

        tx.send_all()
            .instrument(tracing::info_span!("initalize GT", gt=%gt))
            .await
            .inspect(|signatures| {
                tracing::info!("initialized GT with txns: {signatures:#?}");
            })
            .inspect_err(|(signatures, err)| {
                tracing::error!(%err, "failed to initialize GT, successful txns: {signatures:#?}");
            })
            .ok();

        Ok(())
    }

    async fn fund_users(&self) -> eyre::Result<()> {
        const LAMPORTS: u64 = 500_000_000;

        let client = self.client.data_store().solana_rpc();
        let payer = self.client.payer();
        let lamports = client.get_balance(&payer).await?;
        tracing::info!(%payer, "before funding users: {lamports}");

        let mut builder = TransactionBuilder::new(client);
        builder.push_many(
            self.users
                .pubkeys()
                .into_iter()
                .inspect(|user| tracing::info!(%user, "funding user with lamports {LAMPORTS}"))
                .map(|user| {
                    self.client
                        .transfer(&user, LAMPORTS)
                        .expect("amount must not be zero")
                }),
            false,
        )?;

        match builder.send_all().await {
            Ok(signatures) => {
                tracing::info!("funded users with {signatures:#?}");
            }
            Err((signatures, err)) => {
                tracing::error!(%err, "failed to fund users, successful txns: {signatures:#?}");
            }
        }

        Ok(())
    }
    async fn close_native_token_accounts(&self) -> eyre::Result<()> {
        use anchor_spl::token::{TokenAccount, ID};
        use spl_associated_token_account::get_associated_token_address;
        use spl_token::{instruction, native_mint};

        let payer = self.client.payer();
        let client = self.client.data_store().solana_rpc();
        let mut builder = TransactionBuilder::new(client);

        let users = self.users.keypairs().await.into_iter().collect::<Vec<_>>();
        for user in &users {
            let pubkey = user.pubkey();
            let address = get_associated_token_address(&pubkey, &native_mint::ID);
            let Some(_account) = self
                .client
                .account_with_config::<TokenAccount>(&address, Default::default())
                .await?
                .into_value()
            else {
                continue;
            };
            builder
                .try_push(self.client.data_store_rpc().signer(user).pre_instruction(
                    instruction::close_account(&ID, &address, &payer, &pubkey, &[&pubkey])?,
                ))
                .map_err(|(_, err)| err)?;
        }

        match builder.send_all().await {
            Ok(signatures) => {
                tracing::info!("closed native token accounts with {signatures:#?}");
            }
            Err((signatures, err)) => {
                tracing::error!(%err, "failed to close native token accounts, successful txns: {signatures:#?}");
            }
        }
        Ok(())
    }

    async fn refund_payer(&self) -> eyre::Result<()> {
        let client = self.client.data_store().solana_rpc();
        let payer = self.client.payer();

        let mut builder = TransactionBuilder::new(self.client.data_store().solana_rpc());

        let users = self.users.keypairs().await.into_iter().collect::<Vec<_>>();
        for user in &users {
            let pubkey = user.pubkey();
            let lamports = client.get_balance(&user.pubkey()).await?;
            if lamports == 0 {
                continue;
            }
            tracing::info!(user = %pubkey, %lamports, "refund from user");
            let ix = system_instruction::transfer(&user.pubkey(), &payer, lamports);
            builder
                .try_push(
                    self.client
                        .data_store_rpc()
                        .signer(user)
                        .pre_instruction(ix),
                )
                .map_err(|(_, err)| err)?;
        }

        match builder.send_all().await {
            Ok(signatures) => {
                tracing::info!("refunded the payer with {signatures:#?}");
            }
            Err((signatures, err)) => {
                tracing::error!(%err, "failed to refund the payer, successful txns: {signatures:#?}");
            }
        }

        self.users.funded.store(false, Ordering::SeqCst);

        let lamports = client.get_balance(&payer).await?;
        tracing::info!(%payer, "after refunding the payer: {lamports}");
        Ok(())
    }

    pub(crate) async fn use_accounts(&self) -> eyre::Result<Guard> {
        let guard = self.users.use_accounts();

        if self
            .users
            .funded
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            self.fund_users().await?;
        }

        Ok(guard)
    }

    pub(crate) async fn refund_payer_when_not_in_use(&self, wait: Duration) -> eyre::Result<()> {
        tokio::time::sleep(wait).await;
        self.users.wait_until_not_in_use().await;
        tracing::info!("[Cleanup...]");
        _ = self
            .close_native_token_accounts()
            .await
            .inspect_err(|err| tracing::error!(%err, "close native token accounts error"));
        self.refund_payer().await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn token_map(&self) -> Pubkey {
        self.token_map.pubkey()
    }

    pub(crate) fn token(&self, token: &str) -> Option<&Token> {
        self.tokens.get(token)
    }

    /// `None` means the locked user.
    pub(crate) fn user(&self, name: &str) -> eyre::Result<Pubkey> {
        let user = self.users.user(name).ok_or_eyre("no such suer")?;
        Ok(user)
    }

    /// Get a mutex-protected signer to prevent concurrent transactions.
    pub(crate) async fn locked_user_client(&self) -> eyre::Result<Client<Arc<LockedSigner>>> {
        let signer = self.users.locked_user_signer().await;
        Ok(self.client.try_clone_with_payer(signer)?)
    }

    fn user_keypair(&self, name: &str) -> Option<&SignerRef> {
        self.users.users.get(name)
    }

    pub(crate) fn user_client(&self, name: &str) -> eyre::Result<Client<SignerRef>> {
        let Some(signer) = self.user_keypair(name) else {
            eyre::bail!("no such user");
        };
        Ok(self.client.try_clone_with_payer(signer.clone())?)
    }

    pub(crate) async fn get_ata_amount(
        &self,
        token: &Pubkey,
        user: &Pubkey,
    ) -> eyre::Result<Option<u64>> {
        use anchor_spl::token::TokenAccount;
        use spl_associated_token_account::get_associated_token_address;

        let ata = get_associated_token_address(user, token);
        let account = self
            .client
            .account_with_config::<TokenAccount>(&ata, Default::default())
            .await?
            .into_value();
        Ok(account.map(|a| a.amount))
    }

    pub(crate) async fn get_user_ata_amount(
        &self,
        token: &Pubkey,
        user: Option<&str>,
    ) -> eyre::Result<Option<u64>> {
        let user = match user {
            Some(name) => self.user(name)?,
            None => self.users.locked_user_pubkey,
        };
        self.get_ata_amount(token, &user).await
    }

    pub(crate) async fn mint_or_transfer_to(
        &self,
        token_name: &str,
        user: &Pubkey,
        amount: u64,
    ) -> eyre::Result<Signature> {
        use anchor_spl::token::ID;
        use spl_associated_token_account::get_associated_token_address;
        use spl_token::{instruction, native_mint};

        let token = self.token(token_name).ok_or_eyre("no such token")?;
        let account = get_associated_token_address(user, &token.address);
        let payer = self.client.payer();

        let signature = if token.address == native_mint::ID {
            self.client
                .data_store_rpc()
                .pre_instruction(system_instruction::transfer(&payer, &account, amount))
                .pre_instruction(instruction::sync_native(&ID, &account)?)
                .into_anchor_request()
                .send()
                .await?
        } else {
            self.client
                .data_store_rpc()
                .pre_instruction(instruction::mint_to_checked(
                    &ID,
                    &token.address,
                    &account,
                    &payer,
                    &[],
                    amount,
                    token.config.decimals,
                )?)
                .into_anchor_request()
                .send()
                .await?
        };

        tracing::info!(%signature, token=%token_name, "minted or tranferred {amount} to {user}");

        Ok(signature)
    }

    pub(crate) async fn mint_or_transfer_to_user(
        &self,
        token_name: &str,
        user: &str,
        amount: u64,
    ) -> eyre::Result<()> {
        let user = self.user(user)?;

        self.mint_or_transfer_to(token_name, &user, amount).await?;

        Ok(())
    }

    pub(crate) fn market_token(&self, index: &str, long: &str, short: &str) -> Option<&Pubkey> {
        self.market_tokens
            .get(&[index.to_string(), long.to_string(), short.to_string()])
    }

    pub(crate) async fn execute_with_pyth<'a, T>(
        &'a self,
        execute: &mut T,
        compute_unit_price_micro_lamports: Option<u64>,
        skip_preflight: bool,
    ) -> eyre::Result<()>
    where
        T: ExecuteWithPythPrices<'a, SignerRef>,
    {
        use gmsol::pyth::{pull_oracle::hermes::EncodingType, PythPullOracleOps};

        let ctx = execute.context().await?;
        let feed_ids = ctx.feed_ids();

        sleep(Duration::from_secs(2)).await;
        let update = self
            .hermes
            .latest_price_updates(feed_ids, Some(EncodingType::Base64))
            .await?;
        self.pyth
            .execute_with_pyth_price_updates(
                Some(update.binary()),
                execute,
                compute_unit_price_micro_lamports,
                skip_preflight,
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn prepare_market(
        &self,
        selector: [&str; 3],
        mut long_token_amount: u64,
        mut short_token_amount: u64,
        skip_preflight: bool,
    ) -> eyre::Result<&Pubkey> {
        long_token_amount += rand::random::<u8>() as u64;
        short_token_amount += rand::random::<u8>() as u64;

        let [index, long_token, short_token] = selector;
        let market_token = self
            .market_token(index, long_token, short_token)
            .ok_or_eyre("market not found")?;
        self.mint_or_transfer_to_user(long_token, Deployment::DEFAULT_USER, long_token_amount)
            .await?;
        self.mint_or_transfer_to_user(short_token, Deployment::DEFAULT_USER, short_token_amount)
            .await?;

        let client = self.user_client(Self::DEFAULT_USER)?;
        let keeper = self.user_client(Self::DEFAULT_KEEPER)?;

        let (rpc, deposit) = client
            .create_deposit(&self.store, market_token)
            .long_token(long_token_amount, None, None)
            .short_token(short_token_amount, None, None)
            .build_with_address()
            .await?;
        let signature = rpc
            .into_anchor_request()
            .send_with_spinner_and_config(RpcSendTransactionConfig {
                skip_preflight,
                ..Default::default()
            })
            .await?;
        tracing::info!(%deposit, %signature, "created a deposit");
        let mut builder = keeper.execute_deposit(&self.store, &self.oracle, &deposit, false);
        self.execute_with_pyth(&mut builder, None, skip_preflight)
            .await?;
        Ok(market_token)
    }

    pub(crate) fn common_alt(&self) -> &AddressLookupTableAccount {
        &self.common_alt
    }

    pub(crate) fn market_alt(&self) -> &AddressLookupTableAccount {
        &self.market_alt
    }
}

/// Users.
pub struct Users {
    locked_user_pubkey: Pubkey,
    locked_user: Arc<Mutex<SignerRef>>,
    users: HashMap<String, SignerRef>,
    funded: Arc<AtomicBool>,
    used: Arc<AtomicUsize>,
    event: Arc<Event>,
}

impl fmt::Debug for Users {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pubkeys = self
            .users
            .iter()
            .map(|(name, k)| (name, k.pubkey()))
            .collect::<HashMap<_, _>>();
        f.debug_struct("Users")
            .field("locked_user", &self.locked_user_pubkey)
            .field("users", &pubkeys)
            .finish_non_exhaustive()
    }
}

impl Users {
    fn new<R>(rng: &mut R) -> Self
    where
        R: CryptoRng + RngCore,
    {
        let keypair = Keypair::generate(rng);
        Self {
            locked_user_pubkey: keypair.pubkey(),
            locked_user: Arc::new(Mutex::new(shared_signer(keypair))),
            users: Default::default(),
            funded: Arc::new(AtomicBool::new(false)),
            used: Arc::new(AtomicUsize::new(0)),
            event: Arc::new(Event::new()),
        }
    }

    fn add_user<R>(&mut self, name: &str, rng: &mut R) -> bool
    where
        R: CryptoRng + RngCore,
    {
        let keypair = Keypair::generate(rng);
        self.add_user_with_keypair(name, keypair)
    }

    fn add_user_with_keypair(&mut self, name: &str, keypair: Keypair) -> bool {
        let Entry::Vacant(entry) = self.users.entry(name.to_string()) else {
            return false;
        };
        tracing::info!(%name, pubkey=%keypair.pubkey(), "added a new user");
        entry.insert(shared_signer(keypair));
        true
    }

    fn use_accounts(&self) -> Guard {
        self.used.fetch_add(1, Ordering::SeqCst);
        self.event.notify(usize::MAX);
        Guard {
            used: self.used.clone(),
            event: self.event.clone(),
        }
    }

    fn wait_until_not_in_use(&self) -> impl Future<Output = ()> {
        let used = self.used.clone();
        let event = self.event.clone();

        async move {
            loop {
                if used.load(Ordering::SeqCst) == 0 {
                    break;
                }

                let listener = event.listen();

                if used.load(Ordering::SeqCst) == 0 {
                    break;
                }

                listener.await;
            }
        }
    }

    fn user(&self, name: &str) -> Option<Pubkey> {
        self.users.get(name).map(|k| k.pubkey())
    }

    fn pubkeys(&self) -> impl IntoIterator<Item = Pubkey> + '_ {
        self.users
            .values()
            .map(|k| k.pubkey())
            .chain(Some(self.locked_user_pubkey))
    }

    async fn keypairs(&self) -> impl IntoIterator<Item = SignerRef> + '_ {
        let locked = self.locked_user_signer().await;
        self.users
            .values()
            .cloned()
            .chain(Some(shared_signer(locked)))
    }

    async fn locked_user_signer(&self) -> Arc<LockedSigner> {
        let locked = self.locked_user.clone().lock_owned().await;
        Arc::new(LockedSigner { locked })
    }
}

pub(crate) struct LockedSigner {
    locked: OwnedMutexGuard<SignerRef>,
}

impl Signer for LockedSigner {
    fn try_pubkey(&self) -> Result<Pubkey, anchor_client::solana_sdk::signer::SignerError> {
        self.locked.try_pubkey()
    }

    fn try_sign_message(
        &self,
        message: &[u8],
    ) -> Result<
        anchor_client::solana_sdk::signature::Signature,
        anchor_client::solana_sdk::signer::SignerError,
    > {
        self.locked.try_sign_message(message)
    }

    fn is_interactive(&self) -> bool {
        self.locked.is_interactive()
    }

    fn pubkey(&self) -> Pubkey {
        self.locked.pubkey()
    }

    fn sign_message(&self, message: &[u8]) -> anchor_client::solana_sdk::signature::Signature {
        self.locked.sign_message(message)
    }
}

#[must_use]
pub(crate) struct Guard {
    used: Arc<AtomicUsize>,
    event: Arc<Event>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.used.fetch_sub(1, Ordering::SeqCst);
        self.event.notify(usize::MAX);
    }
}

#[derive(Debug)]
pub(crate) struct Token {
    pub(crate) address: Pubkey,
    pub(crate) config: TokenConfig,
}

#[derive(Debug)]
pub(crate) struct TokenConfig {
    pub(crate) decimals: u8,
    pub(crate) pyth_feed_id: Identifier,
    pub(crate) precision: u8,
}

/// Get current deployment.
pub async fn current_deployment() -> eyre::Result<&'static Deployment> {
    static DEPLOYMENT: OnceCell<Deployment> = OnceCell::const_new();
    DEPLOYMENT.get_or_try_init(Deployment::init).await
}

#[tokio::test]
async fn refund_payer() -> eyre::Result<()> {
    let wait = std::env::var(ENV_GMSOL_REFUND_WAIT)
        .ok()
        .and_then(|wait| wait.parse().ok())
        .unwrap_or(1);
    let deployment = current_deployment().await?;

    deployment
        .refund_payer_when_not_in_use(Duration::from_secs(wait))
        .await?;

    Ok(())
}
