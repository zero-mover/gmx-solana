import { Dispatch, SetStateAction, useCallback, useEffect, useMemo, useState } from "react";
import { AvailableTokenOptions, TradeMode, TradeOptions, TradeType } from "./types";
import { useLocalStorageSerializeKey } from "@/utils/localStorage";
import { getSyntheticsTradeOptionsKey } from "@/config/localStorage";
import { MarketInfos } from "../market";
import { Tokens } from "../token";
import { useAvailableTokenOptions } from "./use-available-token-options";
import { mapValues, pick } from "lodash";
import { getByKey } from "@/utils/objects";
import { createTradeFlags } from "./utils";

const INITIAL_TRADE_OPTIONS: TradeOptions = {
  tradeType: TradeType.Long,
  tradeMode: TradeMode.Market,
  tokens: {},
  markets: {},
  collateralAddress: undefined,
};

const useTradeOptions = (chainId: string | undefined, availableTokensOptions: AvailableTokenOptions, marketInfos?: MarketInfos) => {
  const [storedOptions, setStoredOptions] = useLocalStorageSerializeKey(
    getSyntheticsTradeOptionsKey(chainId ?? ""),
    INITIAL_TRADE_OPTIONS,
  );

  const [syncedChainId, setSyncedChainId] = useState<string | undefined>(undefined);

  //#region Manual useMemo zone begin
  // Prevent reinitialization when only the order of the tokens changes.
  /* eslint-disable react-hooks/exhaustive-deps */
  const unstableRefAvailableSwapTokensAddresses = availableTokensOptions.swapTokens.map((t) => t.address.toBase58());
  const swapKey = unstableRefAvailableSwapTokensAddresses.sort().join(",");

  const availableSwapTokenAddresses = useMemo(() => {
    return unstableRefAvailableSwapTokensAddresses;
  }, [swapKey]);

  const unstableRefAvailableIndexTokensAddresses = availableTokensOptions.indexTokens.map((t) => t.address.toBase58());
  const indexKey = unstableRefAvailableIndexTokensAddresses.sort().join(",");

  const availableIndexTokenAddresses = useMemo(() => {
    return unstableRefAvailableIndexTokensAddresses;
  }, [indexKey]);

  const unstableRefStrippedMarketInfos = mapValues(marketInfos || {}, (info) =>
    pick(info, ["longTokenAddress", "shortTokenAddress"])
  );

  const strippedMarketInfos = useMemo(() => {
    return unstableRefStrippedMarketInfos;
  }, [JSON.stringify(unstableRefStrippedMarketInfos)]);
  /* eslint-enable react-hooks/exhaustive-deps */
  //#endregion Manual useMemo zone end

  // Handle chain change.
  useEffect(() => {
    if (syncedChainId === chainId) {
      return;
    }

    if (availableIndexTokenAddresses.length === 0) {
      return;
    }

    if (storedOptions?.tokens.indexTokenAddress && availableIndexTokenAddresses.includes(storedOptions.tokens.indexTokenAddress)) {
      setSyncedChainId(chainId);
      return;
    }

    const market = availableTokensOptions.sortedAllMarkets?.at(0);

    if (!market) {
      return;
    }

    setStoredOptions({
      ...INITIAL_TRADE_OPTIONS,
      markets: {
        [market.marketTokenAddress.toBase58()]: {
          longTokenAddress: market.longTokenAddress.toBase58(),
          shortTokenAddress: market.shortTokenAddress.toBase58(),
        }
      },
      tokens: {
        indexTokenAddress: market.indexTokenAddress.toBase58(),
        fromTokenAddress: market.shortTokenAddress.toBase58(),
      }
    });
    setSyncedChainId(chainId);
  }, [availableIndexTokenAddresses, availableTokensOptions.sortedAllMarkets, chainId, setStoredOptions, storedOptions, syncedChainId]);

  const setTradeOptions = useCallback((action: SetStateAction<TradeOptions | undefined>) => {
    setStoredOptions((oldState) => {
      let newState = typeof action === "function" ? action(oldState)! : action!;

      if (newState && (newState.tradeType === TradeType.Long || newState.tradeType === TradeType.Short)) {
        newState = fallbackPositionTokens(newState);
      }

      console.log(newState);

      return newState;
    });

    function fallbackPositionTokens(newState: TradeOptions) {
      const needFromUpdate = !availableSwapTokenAddresses.find((t) => t === newState.tokens.fromTokenAddress);
      const nextFromTokenAddress =
        needFromUpdate && availableSwapTokenAddresses.length
          ? availableSwapTokenAddresses[0]
          : newState.tokens.fromTokenAddress;

      if (nextFromTokenAddress && nextFromTokenAddress !== newState.tokens.fromTokenAddress) {
        newState = {
          ...newState,
          tokens: {
            ...newState.tokens,
            fromTokenAddress: nextFromTokenAddress,
          },
        };
      }

      const needIndexUpdateByAvailableTokens = !availableIndexTokenAddresses.find(
        (t) => t === newState.tokens.indexTokenAddress
      );

      if (needIndexUpdateByAvailableTokens && availableIndexTokenAddresses.length) {
        const updater = setToTokenAddressUpdaterBuilder(
          newState.tradeType,
          availableIndexTokenAddresses[0],
          undefined
        );

        newState = updater(newState);
      }

      const toTokenAddress =
        newState.tradeType === TradeType.Swap
          ? newState.tokens.toTokenAddress
          : newState.tokens.indexTokenAddress;
      const marketAddress = toTokenAddress
        ? newState.markets[toTokenAddress]?.[newState.tradeType === TradeType.Long ? "longTokenAddress" : "shortTokenAddress"]
        : undefined;
      const marketInfo = getByKey(strippedMarketInfos, marketAddress);

      const currentCollateralIncludedInCurrentMarket =
        marketInfo &&
        (marketInfo.longTokenAddress.toBase58() === newState.collateralAddress ||
          marketInfo.shortTokenAddress.toBase58() === newState.collateralAddress);

      const needCollateralUpdate = !newState.collateralAddress || !currentCollateralIncludedInCurrentMarket;

      if (needCollateralUpdate && marketInfo) {
        newState = {
          ...newState,
          collateralAddress: marketInfo.shortTokenAddress.toBase58(),
        };
      }

      return newState;
    }
  }, [availableIndexTokenAddresses, availableSwapTokenAddresses, setStoredOptions, strippedMarketInfos]);

  return [storedOptions!, setTradeOptions] as [TradeOptions, Dispatch<SetStateAction<TradeOptions>>];
};

export function useTradeBoxState(
  chainId: string | undefined,
  {
    marketInfos,
    tokens
  }: {
    marketInfos?: MarketInfos,
    tokens?: Tokens,
  },
) {

  const avaiableTokensOptions = useAvailableTokenOptions({ marketInfos, tokens });
  const [tradeOptions, setTradeOptions] = useTradeOptions(chainId, avaiableTokensOptions);

  const tradeType = tradeOptions.tradeType;
  const tradeMode = tradeOptions.tradeMode;

  const availalbleTradeModes = useMemo(() => {
    if (!tradeType) {
      return [];
    }
    return {
      [TradeType.Long]: [TradeMode.Market, TradeMode.Limit, TradeMode.Trigger],
      [TradeType.Short]: [TradeMode.Market, TradeMode.Limit, TradeMode.Trigger],
      [TradeType.Swap]: [TradeMode.Market, TradeMode.Limit],
    }[tradeType];
  }, [tradeType]);

  const tradeFlags = useMemo(() => createTradeFlags(tradeType, tradeMode), [tradeType, tradeMode]);
  const { isSwap } = tradeFlags;

  const setTradeType = useCallback((tradeType: TradeType) => {
    setTradeOptions((state) => {
      return {
        ...state,
        tradeType,
      }
    });
  }, [setTradeOptions]);

  const setTradeMode = useCallback((tradeMode: TradeMode) => {
    setTradeOptions((state) => {
      return {
        ...state,
        tradeMode,
      }
    });
  }, [setTradeOptions]);

  return {
    tradeType,
    tradeMode,
    avaiableTokensOptions,
    availalbleTradeModes,
    setTradeType,
    setTradeMode,
  };
}

function setToTokenAddressUpdaterBuilder(
  tradeType: TradeType | undefined,
  tokenAddress: string,
  marketTokenAddress: string | undefined
): (oldState: TradeOptions | undefined) => TradeOptions {
  return function setToTokenAddressUpdater(oldState: TradeOptions | undefined): TradeOptions {
    const isSwap = oldState?.tradeType === TradeType.Swap;
    const newState = JSON.parse(JSON.stringify(oldState)) as TradeOptions;
    if (!newState) {
      return newState;
    }

    if (tradeType) {
      newState.tradeType = tradeType;
    }

    if (isSwap) {
      newState.tokens.toTokenAddress = tokenAddress;
    } else {
      newState.tokens.indexTokenAddress = tokenAddress;
      if (tokenAddress && marketTokenAddress) {
        newState.markets[tokenAddress] = newState.markets[tokenAddress] || {};
        if (newState.tradeType === TradeType.Long) {
          newState.markets[tokenAddress].longTokenAddress = marketTokenAddress;
        } else if (newState.tradeType === TradeType.Short) {
          newState.markets[tokenAddress].shortTokenAddress = marketTokenAddress;
        }
      }
    }

    return newState;
  };
}
