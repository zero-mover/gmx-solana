import { useSharedStatesSelector } from "@/contexts/shared";
import Modal from "../Modal/Modal";
import "./ConfirmationBox.scss";
import { selectMarketAddress, selectTradeBoxCollateralTokenAddress, selectTradeBoxTradeFlags, selectIncreaseAmounts, selectIncreaseSwapParams } from "@/contexts/shared/selectors/trade-box-selectors";
import { useCallback, useMemo } from "react";
import { t } from "@lingui/macro";
import { useTradeStage, useSetTradeStage } from "@/contexts/shared/hooks";
import Button from "../Button/Button";
import LoadingDots from "../Common/LoadingDots/LoadingDots";
import { useExchange } from "@/contexts/anchor";
import { useTriggerInvocation } from "@/onchain/transaction";
import { invokeCreateIncreaseOrder } from "gmsol";
import { GMSOL_DEPLOYMENT } from "@/config/deployment";
import { useSWRConfig } from "swr";
import { filterBalances } from "@/onchain/token";
import { fitlerMarkets } from "@/onchain/market";
import { fitlerPositions } from "@/onchain/position";
import { toBigInt } from "@/utils/number";
import { translateAddress } from "@coral-xyz/anchor";

interface Props {
  onClose?: () => void,
}

export function ConfirmationBox({
  onClose
}: Props) {
  const { isMarket, isLimit, isSwap, isLong } = useSharedStatesSelector(selectTradeBoxTradeFlags);

  const title = useMemo(() => {
    if (isMarket) {
      if (isSwap) {
        return t`Confirm Swap`;
      }

      return isLong ? t`Confirm Long` : t`Confirm Short`;
    } else if (isLimit) {
      return t`Confirm Limit Order`;
    }
    return t`Confirm`
  }, [isLimit, isLong, isMarket, isSwap]);

  const submitButtonText = useMemo(() => {
    let text = "";
    if (isMarket) {
      if (isSwap) {
        text = t`Swap`;
      } else {
        text = isLong ? t`Long` : t`Short`;
      }
    } else if (isLimit) {
      text = t`Confirm Limit Order`;
    } else {
      text = t`Confirm`;
    }
    return text;
  }, [isLimit, isLong, isMarket, isSwap]);

  const stage = useTradeStage();
  const isVisible = useMemo(() => stage === "confirmation", [stage]);

  const setStage = useSetTradeStage();
  const handleClose = useCallback(() => {
    if (onClose) {
      onClose();
    }
    setStage("trade");
  }, [onClose, setStage]);

  const { trigger, isSending, error } = useTriggerCreateOrder();

  const handleSubmit = useCallback(() => {
    if (trigger) {
      void trigger().then(handleClose);
    }
  }, [handleClose, trigger]);

  return (
    <div className="Confirmation-box">
      <Modal
        isVisible={isVisible}
        setIsVisible={handleClose}
        label={title}
      >
        <div className="Confirmation-box-row">
          <Button
            variant="primary-action"
            className="w-full"
            type="submit"
            onClick={handleSubmit}
            disabled={isSending || Boolean(error)}
          >
            {!isSending ? error ? error : submitButtonText : <LoadingDots />}
          </Button>
        </div>
      </Modal>
    </div>
  );
}

function useTriggerCreateOrder() {
  const { isMarket, isLimit, isSwap, isLong, isIncrease } = useSharedStatesSelector(selectTradeBoxTradeFlags);
  const increaseAmounts = useSharedStatesSelector(selectIncreaseAmounts);
  const marketTokenAddress = useSharedStatesSelector(selectMarketAddress);
  const collateralTokenAddress = useSharedStatesSelector(selectTradeBoxCollateralTokenAddress);
  const increaseSwapParams = useSharedStatesSelector(selectIncreaseSwapParams);
  const exchange = useExchange();

  const { mutate } = useSWRConfig();
  const mutateStates = useCallback(() => {
    void mutate(filterBalances);
    void mutate(fitlerMarkets);
    void mutate(fitlerPositions);
  }, [mutate]);

  const invoker = useCallback(async () => {
    const payer = exchange.provider.publicKey;
    if (!payer) throw Error("Wallet is not connteced");
    if (!marketTokenAddress) throw Error("Missing market token address");
    if (!collateralTokenAddress) throw Error("Missing collateral token address");
    if (isMarket && isIncrease && increaseAmounts && increaseSwapParams) {
      const { initialCollateralDeltaAmount, sizeDeltaUsd } = increaseAmounts;
      const { initialCollateralToken, swapPath } = increaseSwapParams;
      const [signatrue, order] = await invokeCreateIncreaseOrder(exchange, {
        store: GMSOL_DEPLOYMENT!.store,
        payer,
        marketToken: translateAddress(marketTokenAddress),
        collateralToken: translateAddress(collateralTokenAddress),
        isLong,
        initialCollateralDeltaAmount: toBigInt(initialCollateralDeltaAmount),
        sizeDeltaUsd: toBigInt(sizeDeltaUsd),
        options: {
          swapPath,
          initialCollateralToken: initialCollateralToken.address,
        }
      }, { skipPreflight: false });
      console.log(`created increase order ${order.toBase58()} at tx ${signatrue}`);
      return signatrue;
    } else {
      throw Error("Unsupprted order type");
    }
  }, [exchange, marketTokenAddress, collateralTokenAddress, isMarket, isIncrease, increaseAmounts, increaseSwapParams, isLong]);

  const { trigger, isSending } = useTriggerInvocation<void>({
    key: "create-increase-order",
    onSentMessage: t`Creating market increase order...`,
    message: t`Market increase order created.`
  }, invoker, {
    onSuccess: mutateStates,
  });

  if (isMarket) {
    if (isSwap) {
      return {
        trigger: undefined,
        isSending: false,
        error: t`Swap orders are not supported for now.`
      }
    } else if (isIncrease) {
      return {
        trigger,
        isSending,
        error: null,
      }
    }
  } else if (isLimit) {
    return {
      trigger: undefined,
      isSending: false,
      error: t`Limit orders are not supported for now.`
    }
  }
  return {
    trigger: undefined,
    isSending: false,
    error: t`Unsupported order type.`,
  }
}
