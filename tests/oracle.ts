import * as anchor from "@coral-xyz/anchor";
import { getAddresses, getExternalPrograms, getPrograms, getProvider, getUsers, expect } from "../utils/fixtures";
import { createRolesPDA, createTokenConfigMapPDA, dataStore } from "../utils/data";
import { BTC_FEED, BTC_TOKEN_MINT, SOL_FEED, SOL_TOKEN_MINT, USDC_FEED } from "../utils/token";
import { PublicKey } from "@solana/web3.js";

describe("oracle", () => {
    const provider = getProvider();

    const { chainlink } = getExternalPrograms();

    const { signer0 } = getUsers();

    const mockFeedAccount = anchor.web3.Keypair.generate();

    let dataStoreAddress: PublicKey;
    let oracleAddress: PublicKey;
    let roles: PublicKey;
    let fakeTokenMint: PublicKey;
    let usdGTokenMint: PublicKey;
    before(async () => {
        ({ dataStoreAddress, oracleAddress, fakeTokenMint, usdGTokenMint } = await getAddresses());
        [roles] = createRolesPDA(dataStoreAddress, signer0.publicKey);
    });

    it("create a new price feed", async () => {
        try {
            await chainlink.methods.createFeed("FOO", 1, 2, 3).accounts({
                feed: mockFeedAccount.publicKey,
                authority: provider.wallet.publicKey,
            }).signers([mockFeedAccount]).preInstructions([
                // @ts-ignore: ignore because the field name of `transmissions` account generated is wrong.
                await chainlink.account.transmissions.createInstruction(
                    mockFeedAccount,
                    8 + 192 + (3 + 3) * 48
                ),
            ]).rpc();
        } catch (error) {
            console.error(error);
            throw error;
        }
    });

    it("set price from feed and then clear", async () => {
        await dataStore.methods.setPricesFromPriceFeed([
            BTC_TOKEN_MINT,
            SOL_TOKEN_MINT,
            fakeTokenMint,
            usdGTokenMint,
        ]).accounts({
            store: dataStoreAddress,
            authority: signer0.publicKey,
            chainlinkProgram: chainlink.programId,
            onlyController: roles,
            oracle: oracleAddress,
            tokenConfigMap: createTokenConfigMapPDA(dataStoreAddress)[0],
        }).remainingAccounts([
            {
                pubkey: BTC_FEED,
                isSigner: false,
                isWritable: false,
            },
            {
                pubkey: SOL_FEED,
                isSigner: false,
                isWritable: false,
            },
            {
                pubkey: BTC_FEED,
                isSigner: false,
                isWritable: false,
            },
            {
                pubkey: USDC_FEED,
                isSigner: false,
                isWritable: false,
            },
        ]).signers([signer0]).rpc();
        const setData = await dataStore.account.oracle.fetch(oracleAddress);
        expect(setData.primary.prices.length).to.equal(4);

        await dataStore.methods.clearAllPrices().accounts({
            store: dataStoreAddress,
            authority: signer0.publicKey,
            onlyController: roles,
            oracle: oracleAddress,
        }).signers([signer0]).rpc();
        const clearedData = await dataStore.account.oracle.fetch(oracleAddress);
        expect(clearedData.primary.prices.length).to.equal(0);
        expect(clearedData.primary.tokens.length).to.equal(0);
    });
});
