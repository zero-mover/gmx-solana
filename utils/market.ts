import { workspace, Program } from "@coral-xyz/anchor";
import { Market } from "../target/types/market";
import { Keypair, PublicKey } from "@solana/web3.js";
import { createMarketPDA, createMarketTokenMintPDA, createMarketVaultPDA, createRolesPDA, dataStore, getMarketSignPDA } from "./data";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { BTC_TOKEN_MINT, SOL_TOKEN_MINT, SignedToken } from "./token";

export const market = workspace.Market as Program<Market>;

export const createMarket = async (
    signer: Keypair,
    dataStoreAddress: PublicKey,
    indexTokenMint: PublicKey,
    longTokenMint: PublicKey,
    shortTokenMint: PublicKey,
) => {
    const [marketTokenMint] = createMarketTokenMintPDA(dataStoreAddress, indexTokenMint, longTokenMint, shortTokenMint);
    const [longToken] = createMarketVaultPDA(dataStoreAddress, longTokenMint, marketTokenMint);
    const [shortToken] = createMarketVaultPDA(dataStoreAddress, shortTokenMint, marketTokenMint);
    const [marketSign] = getMarketSignPDA();
    const [roles] = createRolesPDA(dataStoreAddress, signer.publicKey);
    const [marketAddress] = createMarketPDA(dataStoreAddress, marketTokenMint);

    await market.methods.createMarket(indexTokenMint).accounts({
        authority: signer.publicKey,
        onlyMarketKeeper: roles,
        dataStore: dataStoreAddress,
        market: marketAddress,
        marketTokenMint,
        longTokenMint,
        shortTokenMint,
        longToken,
        shortToken,
        marketSign,
        dataStoreProgram: dataStore.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([signer]).rpc();

    return marketAddress;
};

export const initializeMarkets = async (signer: Keypair, dataStoreAddress: PublicKey, fakeTokenMint: PublicKey, usdGMint: PublicKey) => {
    let marketSolSolBtc: PublicKey;
    try {
        marketSolSolBtc = await createMarket(signer, dataStoreAddress, SOL_TOKEN_MINT, SOL_TOKEN_MINT, BTC_TOKEN_MINT);
        console.log(`New market has been created: ${marketSolSolBtc}`);
    } catch (error) {
        console.warn("Failed to initialize market", error);
    }

    let marketFakeFakeUsdG: PublicKey;
    try {
        marketFakeFakeUsdG = await createMarket(signer, dataStoreAddress, fakeTokenMint, fakeTokenMint, usdGMint);
        console.log(`New market has been created: ${marketFakeFakeUsdG}`);
    } catch (error) {
        console.warn("Failed to initialize market", error);
    }

    return {
        marketSolSolBtc,
        marketFakeFakeUsdG,
    }
};
