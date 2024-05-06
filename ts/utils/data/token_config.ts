import { PublicKey, Signer } from "@solana/web3.js";
import { dataStore } from "./program";
import { createRolesPDA } from ".";
import { utils } from "@coral-xyz/anchor";

// Token Config map seed.
export const TOKEN_CONFIG_MAP_SEED = utils.bytes.utf8.encode("token_config_map");

export const createTokenConfigMapPDA = (store: PublicKey) => PublicKey.findProgramAddressSync([
    TOKEN_CONFIG_MAP_SEED,
    store.toBytes(),
], dataStore.programId);

export const initializeTokenConfigMap = async (authority: Signer, store: PublicKey, len: number) => {
    const [map] = createTokenConfigMapPDA(store);
    await dataStore.methods.initializeTokenConfigMap(len).accounts({
        authority: authority.publicKey,
        store,
        onlyController: createRolesPDA(store, authority.publicKey)[0],
    }).signers([authority]).rpc();
    return map;
};

const hexStringToPublicKey = (hex: string) => {
    const decoded = utils.bytes.hex.decode(hex);
    return new PublicKey(decoded);
};

export const insertTokenConfig = async (
    authority: Signer,
    store: PublicKey,
    token: PublicKey,
    heartbeatDuration: number,
    precision: number,
    feeds: {
        pythFeedId?: string,
        chainlinkFeed?: PublicKey,
        pythDevFeed?: PublicKey,
    }
) => {
    await dataStore.methods.insertTokenConfig({
        heartbeatDuration,
        precision,
        feeds: [
            feeds.pythFeedId ? hexStringToPublicKey(feeds.pythFeedId) : PublicKey.default,
            feeds.chainlinkFeed ?? PublicKey.default,
            feeds.pythDevFeed ?? PublicKey.default,
        ]
    }, true).accounts({
        authority: authority.publicKey,
        store,
        onlyController: createRolesPDA(store, authority.publicKey)[0],
        token,
    }).signers([authority]).rpc();
};

export const insertSyntheticTokenConfig = async (
    authority: Signer,
    store: PublicKey,
    token: PublicKey,
    decimals: number,
    heartbeatDuration: number,
    precision: number,
    feeds: {
        pythFeedId?: string,
        chainlinkFeed?: PublicKey,
        pythDevFeed?: PublicKey,
    }
) => {
    await dataStore.methods.insertSyntheticTokenConfig(token, decimals, {
        heartbeatDuration,
        precision,
        feeds: [
            feeds.pythFeedId ? hexStringToPublicKey(feeds.pythFeedId) : PublicKey.default,
            feeds.chainlinkFeed ?? PublicKey.default,
            feeds.pythDevFeed ?? PublicKey.default,
        ]
    }, true).accounts({
        authority: authority.publicKey,
        store,
        onlyController: createRolesPDA(store, authority.publicKey)[0],
    }).signers([authority]).rpc();
};

export const toggleTokenConfig = async (
    authority: Signer,
    store: PublicKey,
    token: PublicKey,
    enable: boolean,
) => {
    await dataStore.methods.toggleTokenConfig(token, enable).accounts({
        authority: authority.publicKey,
        store,
        onlyController: createRolesPDA(store, authority.publicKey)[0],
    }).signers([authority]).rpc();
};

export interface TokenConfig {
    enabled: boolean,
    heartbeatDuration: number,
    tokenDecimals: number,
    precision: number,
    feeds: PublicKey[],
}

export const getTokenConfig = async (store: PublicKey, token: PublicKey) => {
    const config: TokenConfig = await dataStore.methods.getTokenConfig(store, token).accounts({
        map: createTokenConfigMapPDA(store)[0],
    }).view();
    return config;
}

export const extendTokenConfigMap = async (authority: Signer, store: PublicKey, extendLen: number) => {
    await dataStore.methods.extendTokenConfigMap(extendLen).accounts({
        authority: authority.publicKey,
        store,
        onlyController: createRolesPDA(store, authority.publicKey)[0],
    }).signers([authority]).rpc();
};
