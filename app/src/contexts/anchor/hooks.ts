import { useContext, useMemo } from "react";
import { makeDataStoreProgram, makeExchangeProgram } from "gmsol";
import { AnchorStateContext } from "./AnchorStateProvider";

export const useDataStore = () => {
  const { provider, connection } = useAnchor();
  const program = useMemo(() => {
    return provider ? makeDataStoreProgram(provider) : makeDataStoreProgram({
      connection
    });
  }, [provider, connection]);

  return program;
}

export const useExchange = () => {
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
