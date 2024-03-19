import { BN } from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";
import { createDepositPDA, createNoncePDA, createRolesPDA } from "../../utils/data";
import { getAddresses, getMarkets, getPrograms, getProvider, getUsers, waitForSetup } from "../../utils/fixtures";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";

describe("exchange: deposit", async () => {
    await waitForSetup();
    const { market, dataStore } = getPrograms();
    const { signer0, user0 } = getUsers();
    const {
        dataStoreAddress,
        user0FakeTokenAccount,
        user0UsdGTokenAccount,
        fakeTokenVault,
        usdGVault,
    } = getAddresses();

    const { marketFakeFakeUsdG } = getMarkets();

    const [roles] = createRolesPDA(dataStoreAddress, signer0.publicKey);

    it("create deposit", async () => {
        const depositNonce = Keypair.generate().publicKey.toBytes();
        const receiver = Keypair.generate().publicKey;
        const [deposit] = createDepositPDA(dataStoreAddress, user0.publicKey, depositNonce);
        const tx = await market.methods.createDeposit(
            [...depositNonce],
            {
                receivers: {
                    receiver,
                    uiFeeReceiver: receiver,
                },
                longTokenSwapPath: [],
                shortTokenSwapPath: [],
                initialLongTokenAmount: new BN(1),
                initialShortTokenAmount: new BN(1),
                minMarketToken: new BN(0),
                shouldUnwrapNativeToken: false,
            },
        ).accounts({
            market: marketFakeFakeUsdG,
            authority: signer0.publicKey,
            store: dataStoreAddress,
            onlyController: roles,
            dataStoreProgram: dataStore.programId,
            deposit,
            payer: user0.publicKey,
            initialLongToken: user0FakeTokenAccount,
            initialShortToken: user0UsdGTokenAccount,
            longTokenDepositVault: fakeTokenVault,
            shortTokenDepositVault: usdGVault,
            tokenProgram: TOKEN_PROGRAM_ID,
        }).signers([signer0, user0]).rpc();
        console.log(tx);
    });
});
