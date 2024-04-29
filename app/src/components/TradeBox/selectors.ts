
import { selectTradeBoxAvailableTokensOptions, selectTradeBoxFromTokenAddress, selectTradeBoxState, selectTradeBoxToTokenAddress, selectTradeBoxTradeFlags } from "@/contexts/shared/selectors/trade-box-selectors";
import { Token, TokenData, TokenPrices, getTokenData } from "@/onchain/token";
import { convertToUsd, parseValue } from "@/utils/number";
import { createSharedStatesSelector } from "@/contexts/shared/utils";
import { selectMarketStateMarketInfos, selectMarketStateTokens } from "@/contexts/shared/selectors/market-selectors";
import { BN_ZERO, ONE_USD, USD_DECIMALS } from "@/config/constants";
import { BN } from "@coral-xyz/anchor";
import { selectTradeBoxCollateralTokenAddress } from "@/contexts/shared/selectors/trade-box-selectors/select-trade-box-collateral-token-address";
import { TokensRatio } from "@/onchain/trade";

const parseAmount = (value: string, token?: Token) => (token ? parseValue(value || "0", token.decimals) : BN_ZERO) ?? BN_ZERO;
const calcUsd = (amount: BN, token?: TokenData) => convertToUsd(amount, token?.decimals, token?.prices.minPrice);

export const selectFromToken = createSharedStatesSelector([selectMarketStateTokens, selectTradeBoxFromTokenAddress], getTokenData);
export const selectToToken = createSharedStatesSelector([selectMarketStateTokens, selectTradeBoxToTokenAddress], getTokenData);
export const selectCollateralToken = createSharedStatesSelector([selectMarketStateTokens, selectTradeBoxCollateralTokenAddress], getTokenData)
export const selectFromTokenInputValue = createSharedStatesSelector([selectTradeBoxState], state => state.fromTokenInputValue);
export const selectSetFromTokenInputValue = createSharedStatesSelector([selectTradeBoxState], state => state.setFromTokenInputValue);
export const selectToTokenInputValue = createSharedStatesSelector([selectTradeBoxState], state => state.toTokenInputValue);
export const selectSetToTokenInputValue = createSharedStatesSelector([selectTradeBoxState], state => state.setToTokenInputValue);
export const selectTriggerRatioInputValue = createSharedStatesSelector([selectTradeBoxState], state => state.triggerRatioInputValue);
export const selectSetTriggerRatioInputValue = createSharedStatesSelector([selectTradeBoxState], state => state.setTriggerRatioInputValue);
export const selectFocusedInput = createSharedStatesSelector([selectTradeBoxState], state => state.focusedInput);
export const selectSetFocusedInput = createSharedStatesSelector([selectTradeBoxState], state => state.setFocusedInput);
export const selectFromTokenInputAmount = createSharedStatesSelector([selectFromTokenInputValue, selectFromToken], parseAmount);
export const selectFromTokenUsd = createSharedStatesSelector([selectFromTokenInputAmount, selectFromToken], calcUsd);
export const selectToTokenInputAmount = createSharedStatesSelector([selectToTokenInputValue, selectToToken], parseAmount);
export const selectSwapTokens = createSharedStatesSelector([selectTradeBoxAvailableTokensOptions], options => options.swapTokens);
export const selectSortedLongAndShortTokens = createSharedStatesSelector([selectTradeBoxAvailableTokensOptions], options => options.sortedLongAndShortTokens);
export const selectSwitchTokenAddresses = createSharedStatesSelector([selectTradeBoxState], state => state.switchTokenAddresses);
export const selectTriggerRatioValue = createSharedStatesSelector([selectTriggerRatioInputValue], value => parseValue(value, USD_DECIMALS));
export const selectSortedAllMarkets = createSharedStatesSelector([selectTradeBoxAvailableTokensOptions], options => options.sortedAllMarkets);

export const selectMarkPrice = createSharedStatesSelector([
  selectTradeBoxTradeFlags,
  selectToToken,
], ({ isSwap, isIncrease, isLong }, toToken) => {
  if (!toToken) return;
  if (isSwap) return toToken.prices.minPrice;
  return getMarkPrice({ prices: toToken.prices, isIncrease, isLong })
});

export const selectTradeRatios = createSharedStatesSelector([
  selectTradeBoxTradeFlags,
  selectFromToken,
  selectToToken,
  selectMarkPrice,
  selectTriggerRatioValue,
], ({ isSwap }, fromToken, toToken, markPrice, triggerRatioValue) => {
  const fromTokenPrice = fromToken?.prices.minPrice;

  if (!isSwap || !fromToken || !toToken || !fromTokenPrice || !markPrice) {
    return {};
  }

  const markRatio = getTokensRatioByPrice({
    fromToken,
    toToken,
    fromPrice: fromTokenPrice,
    toPrice: markPrice,
  });

  if (!triggerRatioValue) return { markPrice };

  const triggerRatio: TokensRatio = {
    ratio: triggerRatioValue.gt(BN_ZERO) ? triggerRatioValue : markRatio.ratio,
    largestToken: markRatio.largestToken,
    smallestToken: markRatio.smallestToken,
  };

  return {
    markRatio,
    triggerRatio,
  };
});

export const selectSwapRoutes = createSharedStatesSelector([
  selectMarketStateMarketInfos,
  selectTradeBoxFromTokenAddress,
  selectTradeBoxToTokenAddress,
  selectTradeBoxCollateralTokenAddress,
  selectTradeBoxTradeFlags,
], () => {
  // TODO: calculate swap routes.
  const findSwapPath = (usdIn: BN, opts: { byLiquidity?: boolean }) => {
    void opts;
    void usdIn;
    return undefined;
  };
  return {
    findSwapPath,
  };
});

// export const selectSwapAmounts = createSharedStatesSelector([
//   selectTradeBoxTradeFlags,
//   selectFromToken,
//   selectFromTokenInputAmount,
//   selectToToken,
//   selectToTokenInputAmount,
//   selectSwapRoutes,
//   selectTradeRatios,
//   selectFocusedInput,
// ], (
//   { isLimit },
//   fromToken,
//   fromTokenAmount,
//   toToken,
//   toTokenAmount,
//   { findSwapPath },
//   { markRatio, triggerRatio },
//   amountBy,
// ) => {
//   const fromTokenPrice = fromToken?.prices.minPrice;

//   if (!fromToken || !toToken || !fromTokenPrice) return;

//   if (amountBy === "from") {
//     return getSwapAmountsByFromValue({
//       tokenIn: fromToken,
//       tokenOut: toToken,
//       amountIn: fromTokenAmount,
//       triggerRatio: triggerRatio || markRatio,
//       isLimit,
//       findSwapPath,
//       // uiFeeFactor,
//     });
//   } else {
//     return getSwapAmountsByToValue({
//       tokenIn: fromToken,
//       tokenOut: toToken,
//       amountOut: toTokenAmount,
//       triggerRatio: triggerRatio || markRatio,
//       isLimit,
//       findSwapPath,
//       // uiFeeFactor,
//     });
//   }
// });

function getShouldUseMaxPrice(isIncrease: boolean, isLong: boolean) {
  return isIncrease ? isLong : !isLong;
}

function getMarkPrice({
  prices,
  isIncrease,
  isLong,
}: {
  prices: TokenPrices,
  isIncrease: boolean,
  isLong: boolean,
}) {
  const shouldUseMaxPrice = getShouldUseMaxPrice(isIncrease, isLong);
  return shouldUseMaxPrice ? prices.maxPrice : prices.minPrice;
}

function getTokensRatioByPrice(p: {
  fromToken: TokenData;
  toToken: TokenData;
  fromPrice: BN;
  toPrice: BN;
}): TokensRatio {
  const { fromToken, toToken, fromPrice, toPrice } = p;

  const [largestToken, smallestToken, largestPrice, smallestPrice] = fromPrice.gt(toPrice)
    ? [fromToken, toToken, fromPrice, toPrice]
    : [toToken, fromToken, toPrice, fromPrice];

  const ratio = largestPrice.mul(ONE_USD).div(smallestPrice);

  return { ratio, largestToken, smallestToken };
}
