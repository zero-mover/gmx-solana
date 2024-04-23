import { MarketInfo } from "@/onchain/market";
import { TokenData, Tokens } from "@/onchain/token";
import React, { Dispatch, ReactNode, useMemo } from "react";
import { useImmerReducer } from "use-immer";
import { createContext } from "use-context-selector";
import { Operation } from "./utils";
import { useStateSelector } from "@/contexts/state";
import { getTokenData } from "@/onchain/token/utils";
import { useSortedPoolsWithIndexToken } from "@/hooks";

export const GmStateContext = createContext<GmState | null>(null);
export const GmStateDispatchContext = React.createContext<Dispatch<Action> | null>(null);

export default function GmStateProvider({
  children,
  market,
  operation,
  firstToken,
  secondToken,
}: {
  children: ReactNode,
  market: MarketInfo,
  operation: Operation,
  firstToken?: TokenData,
  secondToken?: TokenData,
}) {
  const [input, dispath] = useImmerReducer(stateReducer, {
    firstTokenInputValue: "",
    secondTokenInputValue: "",
    marketTokenInputValue: "",
  });

  const { marketTokens, marketInfos } = useStateSelector(({ marketTokens, marketInfos }) => {
    return {
      marketTokens,
      marketInfos,
    }
  });

  const marketToken = getTokenData(marketTokens, market.marketTokenAddress);
  const { marketsInfo: sortedMarketsInfoByIndexToken } = useSortedPoolsWithIndexToken(
    marketInfos,
    marketTokens
  );

  const state = useMemo(() => {
    return {
      input,
      market,
      operation,
      firstToken,
      secondToken,
      marketToken,
      marketTokens,
      sortedMarketsInfoByIndexToken,
    };
  }, [firstToken, input, market, marketToken, marketTokens, operation, secondToken, sortedMarketsInfoByIndexToken]);

  return (
    <GmStateContext.Provider value={state}>
      <GmStateDispatchContext.Provider value={dispath}>
        {children}
      </GmStateDispatchContext.Provider>
    </GmStateContext.Provider>
  );
}

export interface GmState {
  market: MarketInfo,
  operation: Operation,
  firstToken?: TokenData,
  secondToken?: TokenData,
  marketToken?: TokenData,
  marketTokens?: Tokens,
  sortedMarketsInfoByIndexToken: MarketInfo[],
  input: InputState,
}

interface InputState {
  firstTokenInputValue: string,
  secondTokenInputValue: string,
  marketTokenInputValue: string,
}

export interface Action {
  type:
  "reset"
  | "set-first-token-input-value"
  | "set-second-token-input-value"
  | "set-market-token-input-value",
  value?: string,
}

const stateReducer = (state: InputState, action: Action) => {
  switch (action.type) {
    case 'reset': {
      state.firstTokenInputValue = "";
      state.secondTokenInputValue = "";
      state.marketTokenInputValue = "";
      break;
    }
    case "set-first-token-input-value": {
      state.firstTokenInputValue = action.value ?? "";
      break;
    }
    case "set-second-token-input-value": {
      state.secondTokenInputValue = action.value ?? "";
      break;
    }
    case "set-market-token-input-value": {
      state.marketTokenInputValue = action.value ?? "";
      break;
    }
  }
};
