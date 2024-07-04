import Footer from "@/components/Footer/Footer";
import "./Root.scss";
import PageTitle from "@/components/PageTitle/PageTitle";
import { Trans, t } from "@lingui/macro";

import "./Governance.scss";
import { CardRow } from "@/components/CardRow/CardRow";
import Tab from "@/components/Tab/Tab";
import Button from "@/components/Button/Button";
import { ChangeEvent, useCallback, useEffect, useMemo, useState } from "react";
import { useStoreProgram, useExchangeProgram } from "@/contexts/anchor";
import { BN, BorshInstructionCoder, utils } from "@coral-xyz/anchor";
import { getInstructionDataFromBase64 } from "@solana/spl-governance";
import { PublicKey } from "@solana/web3.js";
import { isNull, isUndefined } from "lodash";

enum Format {
  Governance = "Governance",
  Array = "Array",
  Hex = "Hex",
}

const FORMAT_LABELS = {
  [Format.Governance]: "Base64",
  [Format.Array]: "Array",
  [Format.Hex]: "Hex",
};

type InstructionArgs = Record<string, boolean | string | PublicKey | BN | null>;

interface Instruction {
  name: string,
  args: InstructionArgs,
  decodedBy?: string,
}

export function Governance() {
  const [format, setFormat] = useState(Format.Governance);
  const [data, setData] = useState("");
  const [instruction, setInstruction] = useState<Instruction | undefined>(undefined);
  const dataStore = useStoreProgram();
  const exchange = useExchangeProgram();

  const dataStoreCoder = useMemo(() => {
    return new BorshInstructionCoder(dataStore.idl);
  }, [dataStore.idl]);

  const exchangeCoder = useMemo(() => {
    return new BorshInstructionCoder(exchange.idl);
  }, [exchange.idl]);

  const decoder = useCallback((data: Buffer) => {
    const decodedByDataStore = dataStoreCoder.decode(data);
    if (decodedByDataStore) {
      return {
        instruction: decodedByDataStore,
        decodedBy: "DataStore",
      }
    }
    const decodedByExchange = exchangeCoder.decode(data);
    if (decodedByExchange) {
      return {
        instruction: decodedByExchange,
        decodedBy: "Exchange",
      }
    }
    return null;
  }, [dataStoreCoder, exchangeCoder]);

  const handleDataChange = useCallback((e: ChangeEvent<HTMLTextAreaElement>) => {
    setData(e.target.value);
  }, []);

  useEffect(() => {
    const decoded = decodeData(format, data);
    const decodedInstruction = decoded ? decoder(decoded) : null;
    console.debug("decoded:", decodedInstruction);
    if (!decodedInstruction) {
      setInstruction(undefined);
    } else {
      const { instruction, decodedBy } = decodedInstruction;
      setInstruction({
        name: instruction.name,
        args: instruction.data as InstructionArgs,
        decodedBy,
      })
    }
  }, [data, decoder, format]);

  return (
    <div className="App">
      <div className="App-content">
        <div className="default-container Governance-layout">
          <PageTitle
            title={t`Governance`}
            isTop
            subtitle={
              <div>
                <Trans>
                  Governance Utils
                </Trans>
              </div>
            }
          />
          <div className="Governance-content">
            <InstructionCard instruction={instruction} />
            <div className="Governance-box">
              <div className="App-box InstructionBox">
                <Tab
                  className="Exchange-swap-option-tabs"
                  options={Object.values(Format)}
                  optionLabels={FORMAT_LABELS}
                  option={format}
                  onChange={(format) => {
                    setFormat(format);
                  }}
                />
                <form onSubmit={(e) => {
                  e.preventDefault();
                  setData("");
                }}>
                  <div className="InstructionBox-form-layout">
                    <div className="Exchange-swap-section-bottom">
                      <textarea
                        className="w-full InstructionTextArea"
                        rows={8}
                        placeholder="Instruction data"
                        autoComplete="off"
                        autoCorrect="off"
                        spellCheck="false"
                        value={data}
                        onChange={handleDataChange}
                      />
                    </div>
                  </div>
                  <div className="Exchange-swap-button-container">
                    <Button
                      className="w-full"
                      variant="primary-action"
                      type="submit"
                    >
                      Clear
                    </Button>
                  </div>
                </form>
              </div>
            </div>
          </div>
        </div>
      </div>
      <Footer />
    </div>
  );
}

function InstructionCard({ instruction }: { instruction?: Instruction }) {
  const name = instruction?.name;
  const args = instruction?.args ?? {};
  const decodedBy = instruction?.decodedBy;

  return (
    <div className="App-card Instruction-card">
      <div className="App-card-content">
        <CardRow label={t`Program`} value={decodedBy ?? "*unknown*"} />
        <CardRow label={t`Instruction`} value={name ?? "*empty*"} />
        <div className="App-card-divider" />
        {
          Object.entries(args).map(([key, value]) => {
            const display = (isNull(value) || isUndefined(value)) ? "*null*" : value.toString();
            return (
              <CardRow key={key} label={key} value={display} />
            );
          })
        }
      </div>
    </div>
  );
}

const decodeData = (format: Format, data: string) => {
  if (format == Format.Array) {
    const byteStrings = data.replace(/[[\]\s']/g, '').split(',');
    const byteNumbers = byteStrings.map(byte => parseInt(byte, 10));
    return Buffer.from(byteNumbers);
  } else if (format == Format.Governance) {
    try {
      const ix = getInstructionDataFromBase64(data);
      return Buffer.from(ix.data);
    } catch (error) {
      return null;
    }
  } else if (format == Format.Hex) {
    return utils.bytes.hex.decode(data);
  } else {
    return null;
  }
};
