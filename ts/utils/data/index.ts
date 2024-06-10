import { anchor } from "../endpoint";
import { keyToSeed } from "../seed";
import { EventManager } from "../event";
import { Keypair, PublicKey } from "@solana/web3.js";
import { BTC_FEED, BTC_FEED_ID, BTC_FEED_PYTH, BTC_TOKEN_MINT, SOL_FEED, SOL_FEED_ID, SOL_FEED_PYTH, SOL_TOKEN_MINT, USDC_FEED, USDC_FEED_ID, USDC_FEED_PYTH } from "../token";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";

import { dataStore } from "./program";
import { invokeInitializeTokenMap, invokePushToTokenMap } from "./token_config";
import { createRolesPDA } from "./roles";
import { createControllerPDA } from "../exchange";
import { invokeInsertAddress, invokeInsertAmount, invokeInsertFactor } from "./config";
import { TIME_WINDOW } from "./constants";
import { invokeSetTokenMap } from "./store";

export const encodeUtf8 = anchor.utils.bytes.utf8.encode;

// Data Store seed.
export const DATA_STORE_SEED = encodeUtf8("data_store");

// Market seeds.
export const MARKET_SEED = encodeUtf8("market");
export const MARKET_TOKEN_MINT_SEED = encodeUtf8("market_token_mint");
export const MARKET_VAULT_SEED = encodeUtf8("market_vault");
// Oracle seed.
export const ORACLE_SEED = encodeUtf8("oracle");
// Nonce seed.
export const NONCE_SEED = encodeUtf8("nonce");
// Deposit seed.
export const DEPOSIT_SEED = encodeUtf8("deposit");
// Withdrawal seed.
export const WITHDRAWAL_SEED = encodeUtf8("withdrawal");
// Order seed.
export const ORDER_SEED = encodeUtf8("order");
// Position seed.
export const POSITION_SEED = encodeUtf8("position");

// Role keys.
export const CONTROLLER = "CONTROLLER";
export const MARKET_KEEPER = "MARKET_KEEPER";
export const ORDER_KEEPER = "ORDER_KEEPER";

export const createDataStorePDA = (key: string) => anchor.web3.PublicKey.findProgramAddressSync([
    DATA_STORE_SEED,
    keyToSeed(key),
], dataStore.programId);

export const createMarketPDA = (store: PublicKey, marketToken: PublicKey) => PublicKey.findProgramAddressSync([
    MARKET_SEED,
    store.toBytes(),
    marketToken.toBytes(),
], dataStore.programId);

export const createMarketTokenMintPDA = (
    store: PublicKey,
    indexTokenMint: PublicKey,
    longTokenMint: PublicKey,
    shortTokenMint: PublicKey,
) => PublicKey.findProgramAddressSync([
    MARKET_TOKEN_MINT_SEED,
    store.toBytes(),
    indexTokenMint.toBytes(),
    longTokenMint.toBytes(),
    shortTokenMint.toBytes(),
], dataStore.programId);

export const createMarketVaultPDA = (store: PublicKey, tokenMint: PublicKey, marketTokenMint?: PublicKey) => PublicKey.findProgramAddressSync([
    MARKET_VAULT_SEED,
    store.toBytes(),
    tokenMint.toBytes(),
    marketTokenMint?.toBytes() ?? new Uint8Array(),
], dataStore.programId);

export const createOraclePDA = (store: PublicKey, index: number) => PublicKey.findProgramAddressSync([
    ORACLE_SEED,
    store.toBytes(),
    new Uint8Array([index]),
], dataStore.programId);

export const createNoncePDA = (store: PublicKey) => PublicKey.findProgramAddressSync([
    NONCE_SEED,
    store.toBytes(),
], dataStore.programId);

export const createDepositPDA = (store: PublicKey, user: PublicKey, nonce: Uint8Array) => PublicKey.findProgramAddressSync([
    DEPOSIT_SEED,
    store.toBytes(),
    user.toBytes(),
    nonce,
], dataStore.programId);

export const createWithdrawalPDA = (store: PublicKey, user: PublicKey, nonce: Uint8Array) => PublicKey.findProgramAddressSync([
    WITHDRAWAL_SEED,
    store.toBytes(),
    user.toBytes(),
    nonce,
], dataStore.programId);

export const createOrderPDA = (store: PublicKey, user: PublicKey, nonce: Uint8Array) => PublicKey.findProgramAddressSync([
    ORDER_SEED,
    store.toBytes(),
    user.toBytes(),
    nonce,
], dataStore.programId);

export const createPositionPDA = (store: PublicKey, user: PublicKey, marketToken: PublicKey, collateralToken: PublicKey, isLong: boolean) => PublicKey.findProgramAddressSync([
    POSITION_SEED,
    store.toBytes(),
    user.toBytes(),
    marketToken.toBytes(),
    collateralToken.toBytes(),
    new Uint8Array([isLong ? 1 : 2]),
], dataStore.programId);

export const createMarketVault = async (provider: anchor.AnchorProvider, signer: Keypair, dataStoreAddress: PublicKey, mint: PublicKey) => {
    const [vault] = createMarketVaultPDA(dataStoreAddress, mint);
    const [roles] = createRolesPDA(dataStoreAddress, signer.publicKey);

    await dataStore.methods.initializeMarketVault(null).accountsPartial({
        authority: signer.publicKey,
        store: dataStoreAddress,
        mint,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([signer]).rpc();
    return vault;
};

export const initializeDataStore = async (
    provider: anchor.AnchorProvider,
    eventManager: EventManager,
    signer: anchor.web3.Keypair,
    user: Keypair,
    dataStoreKey: string,
    oracleIndex: number,
    fakeToken: PublicKey,
    usdG: PublicKey,
) => {
    const [dataStorePDA] = createDataStorePDA(dataStoreKey);
    const [rolesPDA] = createRolesPDA(dataStorePDA, provider.publicKey);
    const [signerRoles] = createRolesPDA(dataStorePDA, signer.publicKey);

    eventManager.subscribe(dataStore, "DataStoreInitEvent");
    eventManager.subscribe(dataStore, "MarketChangeEvent");

    // Initialize a DataStore with the given key.
    try {
        const tx = await dataStore.methods.initialize(dataStoreKey).accountsPartial({
            authority: provider.publicKey,
            dataStore: dataStorePDA,
        }).rpc();
        console.log(`Initialized a new data store account ${dataStorePDA.toBase58()} in tx: ${tx}`);
    } catch (error) {
        console.warn("Failed to initialize a data store with the given key:", error);
    }

    // Initiliaze a roles account for Exchange Program.
    const [controller] = createControllerPDA(dataStorePDA);

    // Enable the required roles and grant to `signer` and `controller`
    const enabled_roles = [CONTROLLER, MARKET_KEEPER, ORDER_KEEPER];
    for (let index = 0; index < enabled_roles.length; index++) {
        const role = enabled_roles[index];
        {
            const tx = await dataStore.methods.enableRole(role).accounts({
                authority: provider.publicKey,
                store: dataStorePDA,
            }).rpc();
            console.log(`Enabled ${role} in tx: ${tx}`);
        }
        {
            const tx = await dataStore.methods.grantRole(signer.publicKey, role).accountsPartial({
                authority: provider.publicKey,
                store: dataStorePDA,
            }).rpc();
            console.log(`Grant ${role} to signer in tx: ${tx}`);
        }
        {
            const tx = await dataStore.methods.grantRole(controller, role).accountsPartial({
                authority: provider.publicKey,
                store: dataStorePDA,
            }).rpc();
            console.log(`Grant ${role} to exchange program in tx: ${tx}`);
        }
    }

    // Initialize token config map.
    const tokenMapKeypair = Keypair.generate();
    const tokenMap = tokenMapKeypair.publicKey;
    try {
        await invokeInitializeTokenMap(dataStore, { payer: signer, tokenMap: tokenMapKeypair, store: dataStorePDA });
        console.log(`Intialized token map: ${tokenMap}`);
        const [tx] = await invokeSetTokenMap(dataStore, { authority: signer, tokenMap, store: dataStorePDA });
        console.log(`The new token map has been set to the store, tx: ${tx}`);
    } catch (error) {
        console.log("Failed to initialize token map", error);
    }

    const HEARTBEAT = 240;

    // Insert BTC token config.
    try {
        await invokePushToTokenMap(dataStore, {
            authority: signer,
            store: dataStorePDA,
            token: BTC_TOKEN_MINT,
            tokenMap,
            heartbeatDuration: HEARTBEAT,
            precision: 2,
            feeds: {
                pythFeedId: BTC_FEED_ID,
            }
        });
        console.log(`Init a token config for ${BTC_TOKEN_MINT}`);
    } catch (error) {
        console.warn("Failed to init the token config account", error);
    }

    // Insert SOL token config.
    try {
        await invokePushToTokenMap(dataStore, {
            authority: signer,
            store: dataStorePDA,
            token: SOL_TOKEN_MINT,
            tokenMap,
            heartbeatDuration: HEARTBEAT,
            precision: 4,
            feeds: {
                pythFeedId: SOL_FEED_ID,
            }
        });
        console.log(`Init a token config for ${SOL_TOKEN_MINT}`);
    } catch (error) {
        console.warn("Failed to init the token config account", error);
    }

    // Insert FakeToken token config.
    try {
        await invokePushToTokenMap(dataStore, {
            authority: signer,
            store: dataStorePDA,
            token: fakeToken,
            tokenMap,
            heartbeatDuration: HEARTBEAT,
            precision: 2,
            feeds: {
                pythFeedId: BTC_FEED_ID,
            }
        });
        console.log(`Init a token config for ${fakeToken}`);
    } catch (error) {
        console.warn("Failed to init the token config account", error);
    }

    // Insert UsdG token config.
    try {
        await invokePushToTokenMap(dataStore, {
            authority: signer,
            store: dataStorePDA,
            token: usdG,
            tokenMap,
            heartbeatDuration: HEARTBEAT,
            precision: 4,
            feeds: {
                pythFeedId: USDC_FEED_ID,
            }
        });
        console.log(`Init a token config for ${usdG}`);
    } catch (error) {
        console.warn("Failed to init the token config account", error);
    }

    // Init an oracle.
    try {
        const [oraclePDA] = createOraclePDA(dataStorePDA, oracleIndex);
        const tx = await dataStore.methods.initializeOracle(oracleIndex).accounts({
            authority: signer.publicKey,
            store: dataStorePDA,
            oracle: oraclePDA,
        }).signers([signer]).rpc();
        console.log(`Inited an oracle account ${oraclePDA} in tx: ${tx}`);
    } catch (error) {
        console.warn(`Failed to init an oracle account with index ${oracleIndex}:`, error);
    }

    // Init the config.
    try {
        invokeInsertAmount(dataStore, { authority: signer, store: dataStorePDA, key: "oracle_max_age", amount: 3600 });
        invokeInsertAmount(dataStore, { authority: signer, store: dataStorePDA, key: "request_expiration", amount: 3600 });
        invokeInsertAmount(dataStore, { authority: signer, store: dataStorePDA, key: "oracle_max_timestamp_range", amount: 300 });
        invokeInsertAmount(dataStore, { authority: signer, store: dataStorePDA, key: "claimable_time_window", amount: TIME_WINDOW });
        invokeInsertAmount(dataStore, { authority: signer, store: dataStorePDA, key: "recent_time_window", amount: 120 });
        invokeInsertFactor(dataStore, { authority: signer, store: dataStorePDA, key: "oracle_ref_price_deviation", factor: 1_000_000_000_000_000 });
        invokeInsertAddress(dataStore, { authority: signer, store: dataStorePDA, key: "holding", address: dataStore.provider.publicKey });
    } catch (error) {
        console.warn("Failed to init config account", error);
    }
};

export * from "./program";
export * from "./token_config";
export * from "./roles";
export * from "./store";
