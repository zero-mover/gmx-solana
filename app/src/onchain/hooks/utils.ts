import { useAnchorProvider } from "@/contexts/anchor";
import { createAssociatedTokenAccountInstruction, createCloseAccountInstruction, createSyncNativeInstruction, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { useConnection } from "@solana/wallet-adapter-react";
import { PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { useCallback } from "react";
import useSWR, { useSWRConfig } from "swr";
import useSWRMutation from "swr/mutation";
import { filterBalances } from "../token";
import { BN } from "@coral-xyz/anchor";
import { WRAPPED_NATIVE_TOKEN_ADDRESS } from "@/config/tokens";
import { usePendingTransactions } from "@/contexts/pendingTransactions";

export const useGenesisHash = () => {
  const connection = useConnection();
  const endpoint = connection.connection.rpcEndpoint;
  const { data } = useSWR(`genesis/${endpoint}`, async () => {
    console.debug("genesis hash updated");
    return await connection.connection.getGenesisHash();
  }, {
    refreshInterval: 0,
  });

  return data;
};

export const useInitializeTokenAccount = () => {
  const provider = useAnchorProvider();
  const { mutate } = useSWRConfig();
  const { setPendingTxs } = usePendingTransactions();

  const { trigger } = useSWRMutation("init-token-account", async (_key, { arg }: { arg: PublicKey }) => {
    if (provider && provider.publicKey) {
      const address = getAssociatedTokenAddressSync(arg, provider.publicKey);
      const ix = createAssociatedTokenAccountInstruction(provider.publicKey, address, provider.publicKey, arg);
      const tx = new Transaction().add(ix);
      tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
      const signature = await provider.sendAndConfirm(tx, undefined, {
        commitment: "processed",
      });
      setPendingTxs((txs) => {
        return [...txs, signature];
      });
      return signature;
    } else {
      throw Error("Wallet not connected");
    }
  });

  return useCallback((token: PublicKey) => {
    void trigger(token, {
      onSuccess: () => {
        console.log(`token account for ${token.toBase58()} is initialized`);
        void mutate(filterBalances);
      },
      onError: (error) => {
        console.error(error)
      }
    });
  }, [trigger, mutate]);
};

export const useWrapNativeToken = (callback: () => void) => {
  const provider = useAnchorProvider();
  const { mutate } = useSWRConfig();
  const { setPendingTxs } = usePendingTransactions();

  const { trigger } = useSWRMutation("wrap-native-token", async (_key, { arg }: { arg: BN }) => {
    if (provider && provider.publicKey) {
      const address = getAssociatedTokenAddressSync(WRAPPED_NATIVE_TOKEN_ADDRESS, provider.publicKey);
      const tx = new Transaction().add(
        SystemProgram.transfer({
          fromPubkey: provider.publicKey,
          toPubkey: address,
          lamports: BigInt(arg.toString()),
        }),
        createSyncNativeInstruction(address),
      );
      tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
      const signature = await provider.sendAndConfirm(tx, undefined, {
        commitment: "processed",
      });
      setPendingTxs((txs) => {
        return [...txs, signature];
      });
      return signature;
    } else {
      throw Error("Wallet not connected");
    }
  });

  return useCallback((lamports: BN) => {
    void trigger(lamports, {
      onSuccess: (signature) => {
        console.log(`wrapped SOL at tx ${signature}`);
        callback();
        void mutate(filterBalances);
      },
      onError: (error) => {
        console.error(error);
        callback();
      }
    });
  }, [trigger, mutate, callback]);
};

export const useUnwrapNativeToken = (callback: () => void) => {
  const provider = useAnchorProvider();
  const { mutate } = useSWRConfig();
  const { setPendingTxs } = usePendingTransactions();

  const { trigger } = useSWRMutation("unwrap-native-token", async () => {
    if (provider && provider.publicKey) {
      const address = getAssociatedTokenAddressSync(WRAPPED_NATIVE_TOKEN_ADDRESS, provider.publicKey);
      const tx = new Transaction().add(
        createCloseAccountInstruction(address, provider.publicKey, provider.publicKey),
      );
      tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
      const signature = await provider.sendAndConfirm(tx, undefined, {
        commitment: "processed",
      });
      setPendingTxs((txs) => {
        return [...txs, signature];
      });
      return signature;
    } else {
      throw Error("Wallet not connected");
    }
  });

  return useCallback(() => {
    void trigger(undefined, {
      onSuccess: (signature) => {
        console.log(`unwrapped SOL at tx ${signature}`);
        callback();
        void mutate(filterBalances);
      },
      onError: (error) => {
        console.error(error);
        callback();
      }
    });
  }, [trigger, mutate, callback]);
};
