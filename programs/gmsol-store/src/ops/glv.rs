use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Mint, TokenAccount},
    token_interface,
};
use gmsol_model::price::Prices;
use typed_builder::TypedBuilder;

use crate::{
    constants,
    states::{
        common::action::ActionExt, glv::GlvWithdrawal, market::revertible::Revertible, Glv,
        GlvDeposit, HasMarketMeta, Market, NonceBytes, Oracle, Store, ValidateOracleTime,
    },
    utils::internal::TransferUtils,
    CoreError, CoreResult, ModelError,
};

use super::market::{Execute, RevertibleLiquidityMarketOperation};

/// Create GLV Deposit Params.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateGlvDepositParams {
    /// Execution fee in lamports
    pub execution_lamports: u64,
    /// The length of the swap path for long token.
    pub long_token_swap_length: u8,
    /// The length of the swap path for short token.
    pub short_token_swap_length: u8,
    /// Initial long token amount to deposit.
    pub initial_long_token_amount: u64,
    /// Initial short otken amount to deposit.
    pub initial_short_token_amount: u64,
    /// Market token amount.
    pub market_token_amount: u64,
    /// Minimum acceptable maount of market tokens to be minted.
    pub min_market_token_amount: u64,
    /// Minimum acceptable amount of glv tokens to receive.
    pub min_glv_token_amount: u64,
}

/// Operation for creating GLV deposit.
#[derive(TypedBuilder)]
pub(crate) struct CreateGlvDepositOperation<'a, 'info> {
    glv_deposit: AccountLoader<'info, GlvDeposit>,
    market: AccountLoader<'info, Market>,
    store: AccountLoader<'info, Store>,
    owner: AccountInfo<'info>,
    nonce: &'a NonceBytes,
    bump: u8,
    initial_long_token: Option<&'a Account<'info, TokenAccount>>,
    initial_short_token: Option<&'a Account<'info, TokenAccount>>,
    market_token: &'a Account<'info, TokenAccount>,
    glv_token: &'a InterfaceAccount<'info, token_interface::TokenAccount>,
    params: &'a CreateGlvDepositParams,
    swap_paths: &'info [AccountInfo<'info>],
}

impl<'a, 'info> CreateGlvDepositOperation<'a, 'info> {
    /// Execute.
    ///
    /// # CHECK
    /// - The address of `glv_deposit` must be derived from the given `store`,
    ///   `owner`, `nonce` and `bump`.
    /// - `glv` must be owned by the given `store`.
    /// - `glv` must contain the given `market`.
    /// - All the token accounts must be owned by `glv_deposit`.
    /// - The mint of `market_token` account must be the market token of the
    ///   given `market`.
    /// - The mint of `glv_token` account must be the glv token of the given `glv`.
    ///
    /// # Errors
    /// - `market` must be initialized, enabled and owned by the given `store`.
    /// - `initial_long_token_amount`, `initial_short_token_amount` and `market_token_amount`
    ///   must not all be zero.
    /// - When the token amount is not zero, the corresponding token account must be provided and
    ///   have enough amount of tokens.
    /// - `execution_lamports` must greater than `MIN_EXECUTION_LAMPORTS` and there must be enough lamports
    ///   in the `glv_deposit` account.
    /// - `glv_deposit` must be uninitialized.
    /// - `swap_paths` must be valid.
    pub(crate) fn unchecked_execute(self) -> Result<()> {
        let (long_token, short_token) = self.validate_market_and_get_tokens()?;

        self.validate_params_excluding_swap()?;

        let id = self
            .market
            .load_mut()?
            .indexer_mut()
            .next_glv_deposit_id()?;

        let mut glv_deposit = self.glv_deposit.load_init()?;

        glv_deposit.header.init(
            id,
            self.store.key(),
            self.market.key(),
            self.owner.key(),
            *self.nonce,
            self.bump,
            self.params.execution_lamports,
        )?;

        // Init tokens and token accounts.
        let primary_token_in = if let Some(account) = self.initial_long_token {
            glv_deposit.tokens.initial_long_token.init(account);
            account.mint
        } else {
            long_token
        };

        let secondary_token_in = if let Some(account) = self.initial_short_token {
            glv_deposit.tokens.initial_short_token.init(account);
            account.mint
        } else {
            short_token
        };
        glv_deposit.tokens.market_token.init(self.market_token);
        glv_deposit
            .tokens
            .glv_token
            .init_with_interface(self.glv_token);

        // Init params.
        glv_deposit.params.deposit.initial_long_token_amount =
            self.params.initial_long_token_amount;
        glv_deposit.params.deposit.initial_short_token_amount =
            self.params.initial_short_token_amount;
        glv_deposit.params.deposit.min_market_token_amount = self.params.min_market_token_amount;

        glv_deposit.params.market_token_amount = self.params.market_token_amount;
        glv_deposit.params.min_glv_token_amount = self.params.min_glv_token_amount;

        // Init swap path.
        glv_deposit.swap.validate_and_init(
            &*self.market.load()?,
            self.params.long_token_swap_length,
            self.params.short_token_swap_length,
            self.swap_paths,
            &self.store.key(),
            (&primary_token_in, &secondary_token_in),
            (&long_token, &short_token),
        )?;

        Ok(())
    }

    fn validate_market_and_get_tokens(&self) -> Result<(Pubkey, Pubkey)> {
        let market = self.market.load()?;
        let meta = market.validated_meta(&self.store.key())?;
        Ok((meta.long_token_mint, meta.short_token_mint))
    }

    fn validate_params_excluding_swap(&self) -> Result<()> {
        let params = self.params;
        require!(
            params.initial_long_token_amount != 0
                || params.initial_short_token_amount != 0
                || params.market_token_amount != 0,
            CoreError::EmptyDeposit
        );

        if params.initial_long_token_amount != 0 {
            let Some(account) = self.initial_long_token.as_ref() else {
                return err!(CoreError::TokenAccountNotProvided);
            };
            require_gte!(
                account.amount,
                params.initial_long_token_amount,
                CoreError::NotEnoughTokenAmount
            );
        }

        if params.initial_short_token_amount != 0 {
            let Some(account) = self.initial_short_token.as_ref() else {
                return err!(CoreError::TokenAccountNotProvided);
            };
            require_gte!(
                account.amount,
                params.initial_short_token_amount,
                CoreError::NotEnoughTokenAmount
            );
        }

        // If the two token accounts are actually the same, then we should check for the sum.
        let same_initial_token_amount = self.initial_long_token.as_ref().and_then(|long| {
            self.initial_short_token
                .as_ref()
                .and_then(|short| (long.key() == short.key()).then(|| long.amount))
        });
        if let Some(amount) = same_initial_token_amount {
            let total_amount = params
                .initial_long_token_amount
                .checked_add(params.initial_short_token_amount)
                .ok_or(error!(CoreError::TokenAmountExceedsLimit))?;
            require_gte!(amount, total_amount, CoreError::NotEnoughTokenAmount);
        }

        ActionExt::validate_balance(&self.glv_deposit, params.execution_lamports)?;

        Ok(())
    }
}

/// Operation for executing a GLV deposit.
#[derive(TypedBuilder)]
pub(crate) struct ExecuteGlvDepositOperation<'a, 'info> {
    glv_deposit: AccountLoader<'info, GlvDeposit>,
    token_program: AccountInfo<'info>,
    glv_token_program: AccountInfo<'info>,
    throw_on_execution_error: bool,
    store: AccountLoader<'info, Store>,
    glv: AccountLoader<'info, Glv>,
    glv_token_mint: &'a mut InterfaceAccount<'info, token_interface::Mint>,
    glv_token_receiver: AccountInfo<'info>,
    market: AccountLoader<'info, Market>,
    market_token_mint: &'a mut Account<'info, Mint>,
    market_token_source: AccountInfo<'info>,
    market_token_vault: AccountInfo<'info>,
    markets: &'info [AccountInfo<'info>],
    market_tokens: &'info [AccountInfo<'info>],
    market_token_vaults: &'info [AccountInfo<'info>],
    oracle: &'a Oracle,
    remaining_accounts: &'info [AccountInfo<'info>],
}

impl<'a, 'info> ExecuteGlvDepositOperation<'a, 'info> {
    /// Execute.
    ///
    /// # CHECK
    /// - The `glv_deposit` must be owned by the `store`.
    /// - The `glv` must be owned by the `store`, and be the GLV account of the `glv_deposit`.
    /// - The `market_token_mint` must be the market token of the `market`.
    /// - The `market_token_vault` must be the vault of GLV for the `market_token`.
    /// - The lengths of `markets` and `market_tokens` must be the same as the market tokens list of the `glv`.
    /// - The order of `markets` and `market_tokens` must be the same of the market tokens list of the `glv`.
    /// - The required prices of tokens must have been validated and stored in the `oracle`.
    ///
    /// # Errors
    /// - The `market` must be owned by the `store` and be the current market of the `glv_deposit`.
    /// - The swap markets provided by `remaining_accounts` must be valid.
    pub(crate) fn unchecked_execute(self) -> Result<bool> {
        let throw_on_execution_error = self.throw_on_execution_error;
        match self.validate_oracle() {
            Ok(()) => {}
            Err(CoreError::OracleTimestampsAreLargerThanRequired) if !throw_on_execution_error => {
                msg!(
                    "GLV Deposit expired at {}",
                    self.oracle_updated_before()
                        .ok()
                        .flatten()
                        .expect("must have an expiration time"),
                );
            }
            Err(err) => {
                return Err(error!(err));
            }
        }
        match self.perform_glv_deposit() {
            Ok(()) => Ok(true),
            Err(err) if !throw_on_execution_error => {
                msg!("Execute GLV deposit error: {}", err);
                Ok(false)
            }
            Err(err) => Err(err),
        }
    }

    fn validate_oracle(&self) -> CoreResult<()> {
        self.oracle.validate_time(self)
    }

    fn validate_market_and_glv_deposit(&self) -> Result<()> {
        let market = self.market.load()?;
        market.validate(&self.store.key())?;

        let glv = self.glv.load()?;

        self.glv_deposit.load()?.unchecked_validate_for_execution(
            &self.market_token_mint.to_account_info(),
            &market,
            &self.glv_token_mint.to_account_info(),
            &glv,
        )?;

        Ok(())
    }

    #[inline(never)]
    fn perform_glv_deposit(self) -> Result<()> {
        use gmsol_model::utils::usd_to_market_token_amount;

        self.validate_market_and_glv_deposit()?;

        let glv_amount = {
            let deposit = self.glv_deposit.load()?;
            let mut market_token_amount = deposit.params.market_token_amount;

            let mut market = RevertibleLiquidityMarketOperation::new(
                &self.store,
                self.oracle,
                &self.market,
                self.market_token_mint,
                self.token_program.clone(),
                Some(&deposit.swap),
                self.remaining_accounts,
            )?;

            let mut op = market.op()?;

            if deposit.is_market_deposit_required() {
                let executed = op.unchecked_deposit(
                    &self.market_token_vault,
                    &deposit.params.deposit,
                    (
                        deposit.tokens.initial_long_token.token(),
                        deposit.tokens.initial_short_token.token(),
                    ),
                )?;

                market_token_amount = market_token_amount
                    .checked_add(executed.output)
                    .ok_or(error!(CoreError::TokenAmountOverflow))?;

                op = executed.with_output(());
            }

            // Calculate GLV token amount to mint.
            let glv_amount = {
                let glv_supply = self.glv_token_mint.supply;
                let glv_value = unchecked_get_glv_value(
                    self.oracle,
                    &op,
                    self.markets,
                    self.market_tokens,
                    self.market_token_vaults,
                )?;

                let (received_value, market_pool_value, market_token_supply) = {
                    let mut prices = self.oracle.market_prices(op.market())?;
                    get_glv_value_for_market(
                        self.oracle,
                        &mut prices,
                        op.market(),
                        u128::from(market_token_amount),
                        false,
                    )?
                };

                // Validate market token balance.
                {
                    let current_balance =
                        anchor_spl::token::accessor::amount(&self.market_token_vault)?;
                    let new_balance = current_balance
                        .checked_add(market_token_amount)
                        .ok_or(error!(CoreError::TokenAmountOverflow))?;
                    self.glv.load()?.validate_market_token_balance(
                        &op.market().market_meta().market_token_mint,
                        new_balance,
                        &market_pool_value,
                        &market_token_supply,
                    )?;
                }

                let glv_amount = usd_to_market_token_amount(
                    received_value,
                    glv_value,
                    u128::from(glv_supply),
                    constants::MARKET_USD_TO_AMOUNT_DIVISOR,
                )
                .ok_or(error!(CoreError::FailedToCalculateGlvAmountToMint))?;
                u64::try_from(glv_amount).map_err(|_| error!(CoreError::TokenAmountOverflow))?
            };

            op.commit();

            glv_amount
        };

        // Invertible operations after the commitment.
        {
            // Complete the market tokens trasfer.
            self.transfer_market_tokens_in();

            // Mint GLV token to the receiver.
            self.mint_glv_tokens(glv_amount);
        }

        Ok(())
    }

    /// Mint GLV tokens to target account.
    ///
    /// # Panic
    /// This is an invertible operation that will panic on error.
    fn mint_glv_tokens(&self, glv_amount: u64) {
        if glv_amount != 0 {
            TransferUtils::new(
                self.glv_token_program.clone(),
                &self.store,
                self.glv_token_mint.to_account_info(),
            )
            .mint_to(&self.glv_token_receiver, glv_amount)
            .expect("failed to mint glv tokens");
        }
    }

    /// Transfer market tokens to vault.
    ///
    /// # Panic
    /// This is an invertible operation that will panic on error.
    fn transfer_market_tokens_in(&self) {
        use anchor_spl::token_interface::{transfer_checked, TransferChecked};

        let deposit = self.glv_deposit.load().expect("must have been checked");
        let signer = deposit.signer();

        let amount = deposit.params.market_token_amount;
        if amount != 0 {
            let token = &*self.market_token_mint;
            let from = &self.market_token_source;
            let to = &self.market_token_vault;
            let ctx = CpiContext::new(
                self.token_program.to_account_info(),
                TransferChecked {
                    from: from.to_account_info(),
                    mint: token.to_account_info(),
                    to: to.to_account_info(),
                    authority: self.glv_deposit.to_account_info(),
                },
            );
            transfer_checked(
                ctx.with_signer(&[&signer.as_seeds()]),
                amount,
                token.decimals,
            )
            .expect("failed to transfer market tokens");
        }
    }
}

impl<'a, 'info> ValidateOracleTime for ExecuteGlvDepositOperation<'a, 'info> {
    fn oracle_updated_after(&self) -> CoreResult<Option<i64>> {
        Ok(Some(
            self.glv_deposit
                .load()
                .map_err(|_| CoreError::LoadAccountError)?
                .header
                .updated_at,
        ))
    }

    fn oracle_updated_before(&self) -> CoreResult<Option<i64>> {
        let ts = self
            .store
            .load()
            .map_err(|_| CoreError::LoadAccountError)?
            .request_expiration_at(
                self.glv_deposit
                    .load()
                    .map_err(|_| CoreError::LoadAccountError)?
                    .header
                    .updated_at,
            )?;
        Ok(Some(ts))
    }

    fn oracle_updated_after_slot(&self) -> CoreResult<Option<u64>> {
        Ok(Some(
            self.glv_deposit
                .load()
                .map_err(|_| CoreError::LoadAccountError)?
                .header
                .updated_at_slot,
        ))
    }
}

/// Get total GLV value.
///
/// # CHECK
/// TODO: basically one must make sure that `glv_markets`,
/// `glv_market_tokens` and `glv_market_token_valuts` are aligned.
///
/// # Errors
///
fn unchecked_get_glv_value<'info>(
    oracle: &Oracle,
    op: &Execute<'_, 'info>,
    glv_markets: &'info [AccountInfo<'info>],
    glv_market_tokens: &'info [AccountInfo<'info>],
    glv_market_token_vaults: &'info [AccountInfo<'info>],
) -> Result<u128> {
    use crate::states::market::AsLiquidityMarket;
    use anchor_spl::token::accessor;

    let mut value = 0u128;

    let current_market = op.market();
    let swap_markets = op.swap_markets();

    let mut prices = oracle.market_prices(current_market)?;

    for ((market, market_token), vault) in glv_markets
        .iter()
        .zip(glv_market_tokens)
        .zip(glv_market_token_vaults)
    {
        let key = market_token.key();

        // Get the current balance of market tokens in the GLV vault.
        let balance = u128::from(accessor::amount(vault)?);

        let value_for_market = if key == current_market.key() {
            let market = current_market;
            // Note that we should use the balance prior to the operation.
            get_glv_value_for_market(oracle, &mut prices, market, balance, true)?.0
        } else if let Some(market) = swap_markets.get(&key) {
            let mint = Account::<Mint>::try_from(market_token)?;
            let market = AsLiquidityMarket::new(market, &mint);
            get_glv_value_for_market(oracle, &mut prices, &market, balance, true)?.0
        } else {
            let market = AccountLoader::<Market>::try_from(market)?;
            let mint = Account::<Mint>::try_from(market_token)?;
            let market = market.load()?;
            let market = market.as_liquidity_market(&mint);
            get_glv_value_for_market(oracle, &mut prices, &market, balance, true)?.0
        };

        value = value
            .checked_add(value_for_market)
            .ok_or(error!(CoreError::ValueOverflow))?;
    }

    Ok(value)
}

fn get_glv_value_for_market<M>(
    oracle: &Oracle,
    prices: &mut Prices<u128>,
    market: &M,
    balance: u128,
    maximize: bool,
) -> Result<(u128, u128, u128)>
where
    M: gmsol_model::LiquidityMarket<{ constants::MARKET_DECIMALS }, Num = u128>,
    M: HasMarketMeta,
{
    use gmsol_model::{utils, LiquidityMarketExt, PnlFactorKind};

    let value = market
        .pool_value(prices, PnlFactorKind::MaxAfterDeposit, maximize)
        .map_err(ModelError::from)?;

    let supply = market.total_supply();

    if balance == 0 {
        return Ok((0, value, supply));
    }

    {
        let index_token_mint = market.market_meta().index_token_mint;
        prices.index_token_price = oracle.get_primary_price(&index_token_mint)?;
    }

    let glv_value = utils::market_token_amount_to_usd(&balance, &value, &supply)
        .ok_or(error!(CoreError::FailedToCalculateGlvValueForMarket))?;

    Ok((glv_value, value, supply))
}

/// Create GLV Withdrawal Params.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CreateGlvWithdrawalParams {
    /// Execution fee in lamports
    pub execution_lamports: u64,
    /// The length of the swap path for long token.
    pub long_token_swap_length: u8,
    /// The length of the swap path for short token.
    pub short_token_swap_length: u8,
    /// The amount of glv tokens to burn.
    pub glv_token_amount: u64,
    /// Minimum acceptable final long token to receive.
    pub min_final_long_token_amount: u64,
    /// Minimum acceptable final short token to receive.
    pub min_final_short_token_amount: u64,
}

/// Operation for creating GLV withdrawal.
#[derive(TypedBuilder)]
pub(crate) struct CreateGlvWithdrawalOperation<'a, 'info> {
    glv_withdrawal: AccountLoader<'info, GlvWithdrawal>,
    market: AccountLoader<'info, Market>,
    store: AccountLoader<'info, Store>,
    owner: &'a AccountInfo<'info>,
    nonce: &'a NonceBytes,
    bump: u8,
    final_long_token: &'a Account<'info, TokenAccount>,
    final_short_token: &'a Account<'info, TokenAccount>,
    market_token: &'a Account<'info, TokenAccount>,
    glv_token: &'a InterfaceAccount<'info, token_interface::TokenAccount>,
    params: &'a CreateGlvWithdrawalParams,
    swap_paths: &'info [AccountInfo<'info>],
}

impl<'a, 'info> CreateGlvWithdrawalOperation<'a, 'info> {
    /// Execute.
    ///
    /// # CHECK
    ///
    /// # Errors
    ///
    pub(crate) fn unchecked_execute(self) -> Result<()> {
        todo!()
    }
}
