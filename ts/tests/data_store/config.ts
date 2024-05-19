import { PublicKey } from "@solana/web3.js";
import { getAddresses, getUsers } from "../../utils/fixtures";
import { dataStore } from "../../utils/data";
import { findConfigPDA, invokeInsertAmount, invokeInsertFactor } from "../../utils/data/config";

describe("data store: Config", () => {
    const { signer0 } = getUsers();

    let dataStoreAddress: PublicKey;
    before("init", async () => {
        ({ dataStoreAddress } = await getAddresses());
    });

    it("insert an amount to the config", async () => {
        const [config] = findConfigPDA(dataStoreAddress);
        await invokeInsertAmount(dataStore, {
            authority: signer0,
            store: dataStoreAddress,
            key: "foo_max_age",
            amount: 3600,
            insertNew: true,
        });
        const account = await dataStore.account.config.fetch(config);
        console.log(account);
    });

    it("insert a factor to the config", async () => {
        const [config] = findConfigPDA(dataStoreAddress);
        await invokeInsertFactor(dataStore, {
            authority: signer0,
            store: dataStoreAddress,
            key: "foo_ref_price_deviation",
            factor: 1_000_000_000_000_000,
            insertNew: true,
        });
        const account = await dataStore.account.config.fetch(config);
        console.log(account);
    });
});
