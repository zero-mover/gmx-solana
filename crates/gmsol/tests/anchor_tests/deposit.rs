use gmsol::exchange::ExchangeOps;

use crate::anchor_tests::setup::{self, Deployment};

#[tokio::test]
async fn single_token_pool_deposit() -> eyre::Result<()> {
    let deployment = setup::current_deployment().await?;
    let _guard = deployment.use_accounts().await?;

    deployment
        .mint_or_transfer_to("WSOL", "user_0", 1_000_000_000)
        .await?;

    let client = deployment.user_client(Deployment::DEFAULT_USER)?.unwrap();
    let _keeper = deployment.user_client(Deployment::DEFAULT_KEEPER).unwrap();
    let store = &deployment.store;
    let market_token = deployment.market_token("SOL", "WSOL", "WSOL").unwrap();

    let (rpc, deposit) = client
        .create_deposit(store, market_token)
        .long_token(1_000_000, None, None)
        .short_token(1_000_000, None, None)
        .execution_fee(200_000)
        .build_with_address()
        .await?;
    let signature = rpc.send().await?;
    tracing::info!(%deposit, %signature, "create deposit");

    Ok(())
}

#[tokio::test]
async fn create_deposit_2() -> eyre::Result<()> {
    let deployment = setup::current_deployment().await?;
    let _guard = deployment.use_accounts().await?;

    deployment
        .mint_or_transfer_to("fBTC", "user_0", 1_000_000_000)
        .await?;

    Ok(())
}
