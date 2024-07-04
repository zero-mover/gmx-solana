import { useCallback, useContext, useMemo } from "react";
import { makeStoreProgram, makeExchangeProgram } from "gmsol";
import { AnchorStateContext } from ".";
import { useWalletModal } from "@solana/wallet-adapter-react-ui";

export const useStoreProgram = () => {
  const { provider, connection } = useAnchor();
  const program = useMemo(() => {
    return provider ? makeStoreProgram(provider) : makeStoreProgram({
      connection
    });
  }, [provider, connection]);

  return program;
}

export const useExchangeProgram = () => {
  const { provider, connection } = useAnchor();

  const program = useMemo(() => {
    return provider ? makeExchangeProgram(provider) : makeExchangeProgram({
      connection,
    });
  }, [provider, connection]);

  return program;
}

export const useAnchorProvider = () => {
  const { provider } = useAnchor();
  return provider;
}

export const useAnchor = () => {
  const ctx = useContext(AnchorStateContext);
  if (!ctx) {
    throw new Error("Used `useAnchor` outside of `AnchorStateProvider`");
  }
  return ctx;
};

export const useOpenConnectModal = () => {
  const { setVisible } = useWalletModal();

  return useCallback(() => {
    setVisible(true);
  }, [setVisible]);
};
