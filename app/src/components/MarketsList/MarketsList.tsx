import { useMedia } from "react-use";
import icon_solana from "@/img/ic_solana_24.svg";

import { Trans, t } from "@lingui/macro";
import Tooltip from "@/components/Tooltip/Tooltip";
import { renderNetFeeHeaderTooltipContent } from "./NetFeeHeaderTooltipContent";
import TooltipWithPortal from "@/components/Tooltip/TooltipWithPortal";
import { IndexTokenStat } from "@/contexts/state";
import { USD_DECIMALS, expandDecimals, formatAmount, formatRatePercentage, formatUsd, getMarketIndexName, getMarketPoolName } from "./utils";
import StatsTooltipRow from "@/components/StatsTooltipRow/StatsTooltipRow";
import { NetFeeTooltip } from "./NetFeeTooltip";
import { BN } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import { MarketInfo } from "@/onchain/market";
import PageTitle from "../PageTitle/PageTitle";

import "./MarketsList.scss";
import { useDeployedMarkets } from "@/hooks";

const TOKEN_DECIMALS: number = 9;
const NORMAL_PRICE = new BN(135);
const PRICE = expandDecimals(NORMAL_PRICE, USD_DECIMALS);
const UNIT_PRICE = expandDecimals(NORMAL_PRICE, USD_DECIMALS - TOKEN_DECIMALS);

export function MarketsList() {
  const markets = useDeployedMarkets();
  const marketKeys = Object.keys(markets);
  const indexTokensStats: IndexTokenStat[] = marketKeys.length ? [{
    token: {
      symbol: "SOL",
      address: PublicKey.unique(),
      prices: {
        maxPrice: PRICE,
        minPrice: PRICE,
      },
    },
    price: new BN(1),
    totalPoolValue: new BN("1768607000000000000000000"),
    totalUtilization: new BN(7000),
    totalUsedLiquidity: new BN("1768607000000000000000000"),
    totalMaxLiquidity: new BN("1768607000000000000000000"),
    bestNetFeeLong: new BN("1000000000000000000000000"),
    bestNetFeeShort: new BN("-1000000000000000000000000"),
    marketsStats: Object.keys(markets).map(key => {
      const market = markets[key];
      return {
        marketInfo: {
          longToken: {
            symbol: "SOL",
            address: PublicKey.unique(),
            prices: {
              maxPrice: new BN(1),
              minPrice: new BN(1),
            },
          },
          shortToken: {
            symbol: "USDC",
            address: PublicKey.unique(),
            prices: {
              maxPrice: new BN(1),
              minPrice: new BN(1),
            },
          },
          indexToken: {
            symbol: "SOL",
            address: PublicKey.unique(),
            prices: {
              maxPrice: new BN(1),
              minPrice: new BN(1),
            },
          },
          ...market,
        } as MarketInfo,
        poolValueUsd: market.longPoolAmount.mul(UNIT_PRICE).add(market.shortPoolAmount.mul(expandDecimals(new BN(1), USD_DECIMALS - TOKEN_DECIMALS))),
        usedLiquidity: new BN(1),
        maxLiquidity: new BN(1),
        netFeeLong: new BN(1),
        netFeeShort: new BN(1),
        utilization: new BN(1),
      }
    }),
  }] : [];

  const isMobile = useMedia("(max-width: 1100px)");

  return (
    <>
      {!isMobile && <MarketsListDesktop indexTokensStats={indexTokensStats} />}
      {isMobile && <MarketsListMobile indexTokensStats={indexTokensStats} />}
    </>
  );
}

function MarketsListDesktop({ indexTokensStats }: { indexTokensStats: IndexTokenStat[] }) {
  return (
    <div className="token-table-wrapper App-card">
      <div className="App-card-title">
        <Trans>GM Pools</Trans> <img src={icon_solana} width="16" alt="Network Icon" />
      </div>
      <div className="App-card-divider"></div>
      <table className="token-table">
        <thead>
          <tr>
            <th>
              <Trans>MARKETS</Trans>
            </th>
            <th>
              <Trans>PRICE</Trans>
            </th>
            <th>
              <Trans comment="Total Value Locked">TVL</Trans>
            </th>
            <th>
              <Trans>LIQUIDITY</Trans>
            </th>
            <th>
              <Tooltip handle={<Trans>NET FEE / 1 H</Trans>} renderContent={renderNetFeeHeaderTooltipContent} />
            </th>
            <th>
              <Trans>UTILIZATION</Trans>
            </th>
          </tr>
        </thead>
        <tbody>
          {indexTokensStats.length ? (
            indexTokensStats.map((stats) => <MarketsListDesktopItem key={stats.token.address.toBase58()} stats={stats} />)
          ) : (
            // <MarketListSkeleton />
            <tr></tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

function MarketsListMobile({ indexTokensStats }: { indexTokensStats: IndexTokenStat[] }) {
  return (
    <>
      <PageTitle title={t`GM Pools`} />
      <div className="token-grid">
        {indexTokensStats.map((stats, index) => {
          const tooltipPositionNetFee = index < indexTokensStats.length / 2 ? "bottom-end" : "top-end";
          const netFeePerHourLong = stats.bestNetFeeLong;
          const netFeePerHourShort = stats.bestNetFeeShort;

          return (
            <div className="App-card" key={stats.token.symbol}>
              <div className="App-card-title">
                <div className="mobile-token-card">
                  <img
                    src={`src/img/ic_${stats.token.symbol.toLocaleLowerCase()}_40.svg`}
                    alt={stats.token.symbol}
                    width="20"
                  />
                  <div className="token-symbol-text">{stats.token.symbol}</div>
                  {/* <div>
                    <AssetDropdown assetSymbol={stats.token.symbol} />
                  </div> */}
                </div>
              </div>
              <div className="App-card-divider"></div>
              <div className="App-card-content">
                <div className="App-card-row">
                  <div className="label">
                    <Trans>Price</Trans>
                  </div>
                  <div>{formatUsd(stats.token.prices?.minPrice)}</div>
                </div>
                <div className="App-card-row">
                  <div className="label">
                    <Trans>TVL</Trans>
                  </div>
                  <div>
                    <Tooltip
                      handle={formatUsd(stats.totalPoolValue)}
                      position="bottom-end"
                      className="MarketList-mobile-tvl-tooltip"
                      renderContent={() => (
                        <>
                          {stats.marketsStats.map(({ marketInfo, poolValueUsd }) => (
                            <StatsTooltipRow
                              key={marketInfo.marketTokenAddress.toBase58()}
                              showDollar={false}
                              label={
                                <div className="items-top">
                                  <span className="text-white">{getMarketIndexName(marketInfo)}</span>
                                  <span className="subtext lh-1">[{getMarketPoolName(marketInfo)}]</span>
                                </div>
                              }
                              value={formatUsd(poolValueUsd)}
                            />
                          ))}
                        </>
                      )}
                    />
                  </div>
                </div>
                <div className="App-card-row">
                  <div className="label">
                    <Trans>Liquidity</Trans>
                  </div>
                  <div>
                    <Tooltip
                      handle={formatUsd(stats.totalMaxLiquidity)}
                      className="MarketList-mobile-tvl-tooltip"
                      renderContent={() => (
                        <>
                          {stats.marketsStats.map(({ marketInfo, maxLiquidity }) => (
                            <StatsTooltipRow
                              key={marketInfo.marketTokenAddress.toBase58()}
                              showDollar={false}
                              label={
                                <div className="items-top">
                                  <span className="text-white">{getMarketIndexName(marketInfo)}</span>
                                  <span className="subtext lh-1">[{getMarketPoolName(marketInfo)}]</span>
                                </div>
                              }
                              value={formatUsd(maxLiquidity)}
                            />
                          ))}
                        </>
                      )}
                    />
                  </div>
                </div>
                <div className="App-card-row">
                  <div className="label">
                    <Tooltip handle={<Trans>Net Fee / 1h</Trans>} renderContent={renderNetFeeHeaderTooltipContent} />
                  </div>
                  <div>
                    <TooltipWithPortal
                      portalClassName="MarketList-netfee-tooltip"
                      handle={`${formatRatePercentage(netFeePerHourLong)} / ${formatRatePercentage(
                        netFeePerHourShort
                      )}`}
                      position={tooltipPositionNetFee}
                      renderContent={() => <NetFeeTooltip marketStats={stats.marketsStats} />}
                    />
                  </div>
                </div>
                <div className="App-card-row">
                  <div className="label">
                    <Trans>Utilization</Trans>
                  </div>
                  <div>{formatAmount(stats.totalUtilization, 2, 2, false)}%</div>
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </>
  );
}

function MarketsListDesktopItem({ stats }: { stats: IndexTokenStat }) {
  const anyPool = stats.marketsStats[0];

  const netFeePerHourLong = stats.bestNetFeeLong;
  const netFeePerHourShort = stats.bestNetFeeShort;
  const marketIndexName = getMarketIndexName(anyPool.marketInfo);

  return (
    <tr key={stats.token.symbol}>
      <td>
        <div className="token-symbol-wrapper">
          <div className="items-center">
            <div className="App-card-title-info-icon">
              <img
                src={`src/img/ic_${stats.token.symbol.toLocaleLowerCase()}_40.svg`}
                alt={stats.token.symbol}
                width="40"
              />
            </div>
            <div className="App-card-title-info-text">
              <div className="App-card-info-title">{marketIndexName}</div>
            </div>
            {/* <div>
              <AssetDropdown token={stats.token} />
            </div> */}
          </div>
        </div>
      </td>
      <td>{formatUsd(stats.token.prices?.minPrice)}</td>
      <td>
        <Tooltip
          className="nowrap"
          handle={formatUsd(stats.totalPoolValue)}
          renderContent={() => (
            <>
              {stats.marketsStats.map(({ marketInfo, poolValueUsd }) => (
                <StatsTooltipRow
                  key={marketInfo.marketTokenAddress.toBase58()}
                  showDollar={false}
                  showColon
                  label={
                    <div className="items-top">
                      <span>{getMarketIndexName(marketInfo)}</span>
                      <span className="subtext lh-1">[{getMarketPoolName(marketInfo)}]</span>:
                    </div>
                  }
                  value={formatUsd(poolValueUsd)}
                />
              ))}
            </>
          )}
        />
      </td>
      <td>
        <Tooltip
          className="nowrap"
          handle={formatUsd(stats.totalMaxLiquidity)}
          renderContent={() => (
            <>
              {stats.marketsStats.map(({ marketInfo, maxLiquidity }) => (
                <StatsTooltipRow
                  key={marketInfo.marketTokenAddress.toBase58()}
                  showDollar={false}
                  showColon
                  label={
                    <div className="items-top">
                      <span>{getMarketIndexName(marketInfo)}</span>
                      <span className="subtext lh-1">[{getMarketPoolName(marketInfo)}]</span>:
                    </div>
                  }
                  value={formatUsd(maxLiquidity)}
                />
              ))}
            </>
          )}
        />
      </td>
      <td>
        <TooltipWithPortal
          portalClassName="MarketList-netfee-tooltip"
          handle={`${formatRatePercentage(netFeePerHourLong)} / ${formatRatePercentage(netFeePerHourShort)}`}
          maxAllowedWidth={510}
          position="bottom-end"
          renderContent={() => <NetFeeTooltip marketStats={stats.marketsStats} />}
        />
      </td>
      <td>{formatAmount(stats.totalUtilization, 2, 2)}%</td>
    </tr>
  );
}
