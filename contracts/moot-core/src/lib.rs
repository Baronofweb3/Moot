//! MOOT core: token factory + constant-product AMM + on-chain fee ledger.
//!
//! Money flow (all wNEAR, integer yocto):
//!  * `create_token`  (payable NEAR) deploys a fixed-supply NEP-141 token and charges the launch fee.
//!  * `ft_on_transfer` is the single entry for wNEAR seeding / buys and for token sells.
//!  * Every swap updates reserves first, then settles the outgoing transfer; the callback either
//!    credits the 30/50/20 fee ledger (success) or reverts reserves and refunds the sender (failure).
//!  * Fee ledgers (creator fees, MOOT TIP, treasury) are internal balances, withdrawn on demand.
//!
//! There is no function that mints tokens, edits a user's balance, or moves user funds to the team.

pub mod math;

use math::{buy_quote, price_impact_bps, sell_quote, split_fee, BPS};
use near_sdk::json_types::{U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::serde_json::{self, json};
use near_sdk::store::{LookupMap, Vector};
use near_sdk::{
    assert_one_yocto, env, ext_contract, log, near, require, AccountId, BorshStorageKey, Gas,
    NearToken, PanicOnDefault, Promise, PromiseError, PromiseOrValue,
};

#[cfg(not(test))]
static FT_WASM: &[u8] = include_bytes!("../res/moot_ft.wasm");
#[cfg(test)]
static FT_WASM: &[u8] = &[0u8; 100_000];

pub const TOKEN_DECIMALS: u8 = 18;
const WHOLE: u128 = 1_000_000_000_000_000_000; // 10^18
const MAX_SUPPLY_WHOLE: u128 = 1_000_000_000_000; // 1e12 whole tokens
const NEAR: u128 = 1_000_000_000_000_000_000_000_000;
/// Headroom funded into each token account on top of its code storage (state, holder entries).
const TOKEN_STATE_HEADROOM: u128 = NEAR / 10 * 15 / 10; // 0.15 NEAR
/// Kept by this contract to pay for the token record it stores.
const CORE_RECORD_RESERVE: u128 = NEAR / 50; // 0.02 NEAR
const MAX_ICON_BYTES: usize = 4096;
const MAX_FEE_BPS: u128 = 300;
const MAX_LAUNCH_FEE_USD_MICRO: u128 = 50_000_000; // $50
const MIN_LP_LOCK_NS: u64 = 86_400 * 1_000_000_000; // one day
pub const DEFAULT_LP_LOCK_NS: u64 = 365 * 86_400 * 1_000_000_000; // one year
const GAS_CALLBACK: Gas = Gas::from_tgas(20);
const GAS_FT_TRANSFER: Gas = Gas::from_tgas(15);
const GAS_FT_INIT: Gas = Gas::from_tgas(30);

#[ext_contract(ext_ft)]
pub trait FungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
}

#[derive(BorshStorageKey)]
#[near]
enum StorageKey {
    Tokens,
    TokenIds,
    Tips,
    CreatorFees,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TokenState {
    /// Contract deployment in flight.
    Deploying,
    /// Deployed and holding its full supply, waiting for the creator's wNEAR liquidity. Not tradable.
    Created,
    /// Pool seeded and lock recorded. Tradable.
    Live,
}

#[near(serializers = [borsh])]
#[derive(Clone)]
pub struct LpLock {
    pub owner: AccountId,
    pub shares: u128,
    pub locked_at: u64,
    pub unlock_at: u64,
}

#[near(serializers = [borsh])]
#[derive(Clone)]
pub struct TokenInfo {
    pub creator: AccountId,
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub total_supply: u128,
    pub state: TokenState,
    pub token_reserve: u128,
    pub wnear_reserve: u128,
    pub initial_wnear: u128,
    pub created_at: u64,
    pub live_at: u64,
    pub lp: Option<LpLock>,
    pub creator_fees_earned: u128,
    pub volume_wnear: u128,
    pub buys: u64,
    pub sells: u64,
}

#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct Config {
    /// Trading fee in basis points of gross trade value (default 100 = 1%).
    pub trading_fee_bps: u16,
    /// Launch fee in micro-USD (3_000_000 = $3).
    pub launch_fee_usd_micro: U128,
    /// NEAR price in micro-USD. TESTNET ONLY: set by admin. Production must use an oracle.
    pub near_usd_price_micro: U128,
    pub price_updated_ns: U64,
    pub price_max_age_ns: U64,
    /// Minimum creator wNEAR liquidity to make a token Live.
    pub min_initial_liquidity: U128,
    /// Lock applied to NEW launches. Existing locks keep their own unlock time.
    pub lp_lock_ns: U64,
    pub paused: bool,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Default)]
pub struct Totals {
    pub launches: U64,
    pub trades: U64,
    pub volume_wnear: U128,
    pub fees_collected: U128,
    pub creator_fees_credited: U128,
    pub tips_credited: U128,
    pub treasury_fees_credited: U128,
    pub launch_fees_native: U128,
    /// Liabilities that must always reconcile with wNEAR held: pools + ledgers.
    pub pool_wnear: U128,
    pub tips_outstanding: U128,
    pub creator_outstanding: U128,
    pub treasury_wnear: U128,
    pub treasury_native: U128,
    /// wNEAR that has entered / left this contract through settled flows.
    pub wnear_in: U128,
    pub wnear_out: U128,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct MootCore {
    admin: AccountId,
    treasury_account: AccountId,
    wnear: AccountId,
    cfg: Config,
    tokens: LookupMap<AccountId, TokenInfo>,
    token_ids: Vector<AccountId>,
    /// Trader rewards (MOOT TIP), in wNEAR.
    tips: LookupMap<AccountId, u128>,
    creator_fees: LookupMap<AccountId, u128>,
    t: Totals,
}

#[near(serializers = [json])]
#[serde(tag = "action", rename_all = "snake_case")]
enum Msg {
    Seed { token: AccountId },
    Buy { token: AccountId, min_out: U128, deadline_ns: Option<U64> },
    Sell { min_out: U128, deadline_ns: Option<U64> },
}

#[near(serializers = [json])]
pub struct TokenView {
    pub token_id: AccountId,
    pub creator: AccountId,
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub decimals: u8,
    pub total_supply: U128,
    pub state: TokenState,
    pub token_reserve: U128,
    pub wnear_reserve: U128,
    pub initial_wnear: U128,
    pub created_at: U64,
    pub live_at: U64,
    pub lp_owner: Option<AccountId>,
    pub lp_shares: Option<U128>,
    pub lp_locked_at: Option<U64>,
    pub lp_unlock_at: Option<U64>,
    pub creator_fees_earned: U128,
    pub volume_wnear: U128,
    pub buys: U64,
    pub sells: U64,
}

#[near(serializers = [json])]
pub struct BuyQuoteView {
    pub fee: U128,
    pub net_in: U128,
    pub tokens_out: U128,
    pub price_impact_bps: U128,
    pub fee_to_creator: U128,
    pub fee_to_trader: U128,
    pub fee_to_treasury: U128,
}

#[near(serializers = [json])]
pub struct SellQuoteView {
    pub gross_out: U128,
    pub fee: U128,
    pub net_out: U128,
    pub price_impact_bps: U128,
    pub fee_to_creator: U128,
    pub fee_to_trader: U128,
    pub fee_to_treasury: U128,
}

#[near(serializers = [json])]
pub struct LaunchQuote {
    pub launch_fee_yocto: U128,
    pub deploy_cost_yocto: U128,
    pub record_reserve_yocto: U128,
    pub required_deposit_yocto: U128,
    pub price_is_fresh: bool,
}

#[near(serializers = [json])]
pub struct Accounting {
    pub totals: Totals,
    /// wnear_in - wnear_out
    pub wnear_net_held: U128,
    /// pool_wnear + tips_outstanding + creator_outstanding + treasury_wnear
    pub wnear_liabilities: U128,
    pub reconciles: bool,
}

fn emit(event: &str, data: serde_json::Value) {
    log!("EVENT_JSON:{}", json!({ "standard": "moot", "version": "1.0.0", "event": event, "data": [data] }));
}

fn add(a: u128, b: u128) -> u128 {
    a.checked_add(b).expect("overflow")
}
fn sub(a: u128, b: u128) -> u128 {
    a.checked_sub(b).expect("underflow")
}
fn ssub(a: u128, b: u128) -> u128 {
    a.saturating_sub(b)
}

fn valid_symbol(s: &str) -> bool {
    (2..=10).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

#[near]
impl MootCore {
    #[init]
    pub fn new(
        admin: AccountId,
        treasury_account: AccountId,
        wnear: AccountId,
        near_usd_price_micro: U128,
        min_initial_liquidity: U128,
    ) -> Self {
        require!(near_usd_price_micro.0 > 0, "price must be positive");
        Self {
            admin,
            treasury_account,
            wnear,
            cfg: Config {
                trading_fee_bps: 100,
                launch_fee_usd_micro: U128(3_000_000),
                near_usd_price_micro,
                price_updated_ns: U64(env::block_timestamp()),
                price_max_age_ns: U64(7 * 86_400 * 1_000_000_000),
                min_initial_liquidity,
                lp_lock_ns: U64(DEFAULT_LP_LOCK_NS),
                paused: false,
            },
            tokens: LookupMap::new(StorageKey::Tokens),
            token_ids: Vector::new(StorageKey::TokenIds),
            tips: LookupMap::new(StorageKey::Tips),
            creator_fees: LookupMap::new(StorageKey::CreatorFees),
            t: Totals::default(),
        }
    }

    // ------------------------------------------------------------------ admin
    // Admin can tune future parameters and pause. It cannot touch user balances, mint, or
    // move liquidity. In production `admin` and `treasury_account` must be multisig accounts.

    pub fn set_paused(&mut self, paused: bool) {
        self.assert_admin();
        self.cfg.paused = paused;
        emit("paused", json!({ "paused": paused }));
    }

    pub fn set_trading_fee_bps(&mut self, bps: u16) {
        self.assert_admin();
        require!((bps as u128) <= MAX_FEE_BPS, "fee above hard cap of 3%");
        self.cfg.trading_fee_bps = bps;
        emit("config", json!({ "trading_fee_bps": bps }));
    }

    pub fn set_launch_fee_usd_micro(&mut self, micro: U128) {
        self.assert_admin();
        require!(micro.0 <= MAX_LAUNCH_FEE_USD_MICRO, "launch fee above hard cap of $50");
        self.cfg.launch_fee_usd_micro = micro;
        emit("config", json!({ "launch_fee_usd_micro": micro }));
    }

    pub fn set_min_initial_liquidity(&mut self, amount: U128) {
        self.assert_admin();
        require!(amount.0 > 0, "must be positive");
        self.cfg.min_initial_liquidity = amount;
        emit("config", json!({ "min_initial_liquidity": amount }));
    }

    pub fn set_lp_lock_ns(&mut self, ns: U64) {
        self.assert_admin();
        require!(ns.0 >= MIN_LP_LOCK_NS, "lock must be at least one day");
        self.cfg.lp_lock_ns = ns;
        emit("config", json!({ "lp_lock_ns": ns }));
    }

    /// TESTNET-ONLY exchange rate. Stale rates are rejected at launch time.
    pub fn set_near_usd_price(&mut self, micro_usd_per_near: U128) {
        self.assert_admin();
        require!(micro_usd_per_near.0 > 0, "price must be positive");
        self.cfg.near_usd_price_micro = micro_usd_per_near;
        self.cfg.price_updated_ns = U64(env::block_timestamp());
        emit("config", json!({ "near_usd_price_micro": micro_usd_per_near }));
    }

    pub fn set_treasury_account(&mut self, account: AccountId) {
        self.assert_admin();
        self.treasury_account = account.clone();
        emit("config", json!({ "treasury_account": account }));
    }

    // ------------------------------------------------------------------ launch

    /// Step 1 of a launch. Attach at least `launch_quote().required_deposit_yocto`.
    /// Deploys the token contract, mints the whole fixed supply to this contract, charges the
    /// launch fee. The token is NOT tradable until the creator seeds liquidity (step 2).
    #[payable]
    pub fn create_token(
        &mut self,
        name: String,
        symbol: String,
        icon: Option<String>,
        description: String,
        total_supply_whole: U128,
    ) -> Promise {
        require!(!self.cfg.paused, "launches are paused");
        require!(self.price_is_fresh(), "exchange rate is stale");
        require!(valid_symbol(&symbol), "symbol must be 2-10 chars, A-Z and 0-9");
        require!(!name.trim().is_empty() && name.len() <= 40, "name must be 1-40 chars");
        require!(description.len() <= 280, "description too long");
        if let Some(i) = &icon {
            require!(i.len() <= MAX_ICON_BYTES, "icon too large");
        }
        require!(
            total_supply_whole.0 >= 1 && total_supply_whole.0 <= MAX_SUPPLY_WHOLE,
            "supply must be between 1 and 1,000,000,000,000 tokens"
        );
        let total_supply = total_supply_whole.0.checked_mul(WHOLE).expect("overflow");

        let creator = env::predecessor_account_id();
        let token_id: AccountId = format!("{}.{}", symbol.to_lowercase(), env::current_account_id())
            .parse()
            .unwrap_or_else(|_| env::panic_str("invalid token account"));
        require!(self.tokens.get(&token_id).is_none(), "symbol already launched");

        let q = self.launch_quote_inner();
        let attached = env::attached_deposit().as_yoctonear();
        require!(attached >= q.required, "attach at least the required deposit");

        self.tokens.insert(
            token_id.clone(),
            TokenInfo {
                creator: creator.clone(),
                name: name.clone(),
                symbol: symbol.clone(),
                description,
                total_supply,
                state: TokenState::Deploying,
                token_reserve: 0,
                wnear_reserve: 0,
                initial_wnear: 0,
                created_at: env::block_timestamp(),
                live_at: 0,
                lp: None,
                creator_fees_earned: 0,
                volume_wnear: 0,
                buys: 0,
                sells: 0,
            },
        );

        let metadata = json!({
            "spec": "ft-1.0.0", "name": name, "symbol": symbol, "icon": icon,
            "reference": null, "reference_hash": null, "decimals": TOKEN_DECIMALS
        });
        let init_args = serde_json::to_vec(&json!({
            "holder_id": env::current_account_id(),
            "total_supply": U128(total_supply),
            "metadata": metadata,
        }))
        .unwrap();

        Promise::new(token_id.clone())
            .create_account()
            .transfer(NearToken::from_yoctonear(q.deploy_cost + TOKEN_STATE_HEADROOM))
            .deploy_contract(FT_WASM.to_vec())
            .function_call("new".to_string(), init_args, NearToken::from_yoctonear(0), GAS_FT_INIT)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_CALLBACK)
                    .on_token_created(token_id, creator, U128(q.launch_fee), U128(attached)),
            )
    }

    #[private]
    pub fn on_token_created(
        &mut self,
        token_id: AccountId,
        creator: AccountId,
        launch_fee: U128,
        attached: U128,
        #[callback_result] result: Result<(), PromiseError>,
    ) -> bool {
        let mut tok = self.tokens.get(&token_id).expect("token record missing").clone();
        require!(tok.state == TokenState::Deploying, "not deploying");
        if result.is_err() {
            self.tokens.remove(&token_id);
            emit("token_create_failed", json!({ "token_id": token_id, "creator": creator }));
            // The whole attached deposit goes back to the creator.
            let _ = Promise::new(creator).transfer(NearToken::from_yoctonear(attached.0));
            return false;
        }
        tok.state = TokenState::Created;
        tok.token_reserve = tok.total_supply;
        let supply = tok.total_supply;
        self.tokens.insert(token_id.clone(), tok);
        self.token_ids.push(token_id.clone());
        self.t.launches = U64(self.t.launches.0 + 1);
        self.t.launch_fees_native = U128(add(self.t.launch_fees_native.0, launch_fee.0));
        self.t.treasury_native = U128(add(self.t.treasury_native.0, launch_fee.0));

        // Refund anything above what was needed.
        let q = self.launch_quote_inner();
        let used = add(q.deploy_cost + TOKEN_STATE_HEADROOM, add(launch_fee.0, CORE_RECORD_RESERVE));
        if attached.0 > used {
            let _ = Promise::new(creator.clone()).transfer(NearToken::from_yoctonear(attached.0 - used));
        }
        emit(
            "token_created",
            json!({
                "token_id": token_id, "creator": creator, "total_supply": U128(supply),
                "launch_fee_native": launch_fee
            }),
        );
        true
    }

    // ------------------------------------------------------------------ wNEAR / token receiver

    /// Called by wNEAR (seed, buy) or by a MOOT token (sell). Returns the UNUSED amount.
    /// Anything invalid is refunded in full instead of panicking.
    pub fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, msg: String) -> PromiseOrValue<U128> {
        let caller = env::predecessor_account_id();
        match self.route_transfer(caller, sender_id, amount.0, &msg) {
            Ok(p) => p,
            Err(reason) => {
                emit("transfer_refused", json!({ "reason": reason, "amount": amount }));
                PromiseOrValue::Value(amount)
            }
        }
    }

    // ------------------------------------------------------------------ settlement callbacks

    #[private]
    pub fn on_buy_settled(
        &mut self,
        token_id: AccountId,
        buyer: AccountId,
        gross: U128,
        net: U128,
        fee: U128,
        tokens_out: U128,
        #[callback_result] result: Result<(), PromiseError>,
    ) -> U128 {
        let mut tok = self.tokens.get(&token_id).expect("token record missing").clone();
        if result.is_ok() {
            tok.buys += 1;
            tok.volume_wnear = add(tok.volume_wnear, gross.0);
            self.credit_fee(&mut tok, &buyer, fee.0);
            self.tokens.insert(token_id.clone(), tok);
            self.t.trades = U64(self.t.trades.0 + 1);
            self.t.volume_wnear = U128(add(self.t.volume_wnear.0, gross.0));
            emit(
                "swap",
                json!({ "token_id": token_id, "side": "buy", "trader": buyer, "wnear": gross,
                        "tokens": tokens_out, "fee": fee, "status": "settled" }),
            );
            U128(0)
        } else {
            // Undo the optimistic reserve change; the buyer's wNEAR is refunded in full.
            tok.wnear_reserve = ssub(tok.wnear_reserve, net.0);
            tok.token_reserve = add(tok.token_reserve, tokens_out.0);
            self.tokens.insert(token_id.clone(), tok);
            self.t.pool_wnear = U128(ssub(self.t.pool_wnear.0, net.0));
            self.t.wnear_in = U128(ssub(self.t.wnear_in.0, gross.0));
            emit(
                "swap",
                json!({ "token_id": token_id, "side": "buy", "trader": buyer, "wnear": gross,
                        "status": "failed_refunded" }),
            );
            gross
        }
    }

    #[private]
    pub fn on_sell_settled(
        &mut self,
        token_id: AccountId,
        seller: AccountId,
        tokens_in: U128,
        gross_out: U128,
        fee: U128,
        net_out: U128,
        #[callback_result] result: Result<(), PromiseError>,
    ) -> U128 {
        let mut tok = self.tokens.get(&token_id).expect("token record missing").clone();
        if result.is_ok() {
            tok.sells += 1;
            tok.volume_wnear = add(tok.volume_wnear, gross_out.0);
            self.credit_fee(&mut tok, &seller, fee.0);
            self.tokens.insert(token_id.clone(), tok);
            self.t.trades = U64(self.t.trades.0 + 1);
            self.t.volume_wnear = U128(add(self.t.volume_wnear.0, gross_out.0));
            emit(
                "swap",
                json!({ "token_id": token_id, "side": "sell", "trader": seller, "wnear": net_out,
                        "gross": gross_out, "tokens": tokens_in, "fee": fee, "status": "settled" }),
            );
            U128(0)
        } else {
            tok.token_reserve = ssub(tok.token_reserve, tokens_in.0);
            tok.wnear_reserve = add(tok.wnear_reserve, gross_out.0);
            self.tokens.insert(token_id.clone(), tok);
            self.t.pool_wnear = U128(add(self.t.pool_wnear.0, gross_out.0));
            self.t.wnear_out = U128(ssub(self.t.wnear_out.0, net_out.0));
            emit(
                "swap",
                json!({ "token_id": token_id, "side": "sell", "trader": seller, "tokens": tokens_in,
                        "status": "failed_refunded" }),
            );
            tokens_in
        }
    }

    // ------------------------------------------------------------------ withdrawals

    /// Withdraw MOOT TIP (trader rewards) to the caller's wallet. Needs a full-access key.
    #[payable]
    pub fn withdraw_tips(&mut self, amount: Option<U128>) -> Promise {
        assert_one_yocto();
        let who = env::predecessor_account_id();
        let bal = self.tips.get(&who).copied().unwrap_or(0);
        let amt = amount.map(|a| a.0).unwrap_or(bal);
        require!(amt > 0 && amt <= bal, "nothing to withdraw");
        self.tips.insert(who.clone(), bal - amt);
        self.t.tips_outstanding = U128(sub(self.t.tips_outstanding.0, amt));
        self.t.wnear_out = U128(add(self.t.wnear_out.0, amt));
        self.send_wnear(&who, amt).then(
            Self::ext(env::current_account_id())
                .with_static_gas(GAS_CALLBACK)
                .on_withdraw_done("tip".to_string(), who, U128(amt)),
        )
    }

    /// Withdraw creator fees (the 50% share) to the caller's wallet. Needs a full-access key.
    #[payable]
    pub fn withdraw_creator_fees(&mut self, amount: Option<U128>) -> Promise {
        assert_one_yocto();
        let who = env::predecessor_account_id();
        let bal = self.creator_fees.get(&who).copied().unwrap_or(0);
        let amt = amount.map(|a| a.0).unwrap_or(bal);
        require!(amt > 0 && amt <= bal, "nothing to withdraw");
        self.creator_fees.insert(who.clone(), bal - amt);
        self.t.creator_outstanding = U128(sub(self.t.creator_outstanding.0, amt));
        self.t.wnear_out = U128(add(self.t.wnear_out.0, amt));
        self.send_wnear(&who, amt).then(
            Self::ext(env::current_account_id())
                .with_static_gas(GAS_CALLBACK)
                .on_withdraw_done("creator".to_string(), who, U128(amt)),
        )
    }

    /// Treasury wNEAR (30% of trading fees) to the configured treasury account only.
    #[payable]
    pub fn withdraw_treasury_wnear(&mut self, amount: Option<U128>) -> Promise {
        assert_one_yocto();
        require!(env::predecessor_account_id() == self.treasury_account, "treasury account only");
        let bal = self.t.treasury_wnear.0;
        let amt = amount.map(|a| a.0).unwrap_or(bal);
        require!(amt > 0 && amt <= bal, "nothing to withdraw");
        self.t.treasury_wnear = U128(bal - amt);
        self.t.wnear_out = U128(add(self.t.wnear_out.0, amt));
        let to = self.treasury_account.clone();
        self.send_wnear(&to, amt).then(
            Self::ext(env::current_account_id())
                .with_static_gas(GAS_CALLBACK)
                .on_withdraw_done("treasury".to_string(), to, U128(amt)),
        )
    }

    /// Treasury native NEAR (launch fees) to the configured treasury account only.
    #[payable]
    pub fn withdraw_treasury_native(&mut self, amount: Option<U128>) -> Promise {
        assert_one_yocto();
        require!(env::predecessor_account_id() == self.treasury_account, "treasury account only");
        let bal = self.t.treasury_native.0;
        let amt = amount.map(|a| a.0).unwrap_or(bal);
        require!(amt > 0 && amt <= bal, "nothing to withdraw");
        self.t.treasury_native = U128(bal - amt);
        let to = self.treasury_account.clone();
        Promise::new(to.clone()).transfer(NearToken::from_yoctonear(amt)).then(
            Self::ext(env::current_account_id())
                .with_static_gas(GAS_CALLBACK)
                .on_withdraw_done("treasury_native".to_string(), to, U128(amt)),
        )
    }

    #[private]
    pub fn on_withdraw_done(
        &mut self,
        kind: String,
        to: AccountId,
        amount: U128,
        #[callback_result] result: Result<(), PromiseError>,
    ) -> bool {
        if result.is_ok() {
            emit("withdrawal", json!({ "kind": kind, "to": to, "amount": amount, "status": "settled" }));
            return true;
        }
        // Transfer failed: put the money back where it came from.
        match kind.as_str() {
            "tip" => {
                let b = self.tips.get(&to).copied().unwrap_or(0);
                self.tips.insert(to.clone(), add(b, amount.0));
                self.t.tips_outstanding = U128(add(self.t.tips_outstanding.0, amount.0));
                self.t.wnear_out = U128(ssub(self.t.wnear_out.0, amount.0));
            }
            "creator" => {
                let b = self.creator_fees.get(&to).copied().unwrap_or(0);
                self.creator_fees.insert(to.clone(), add(b, amount.0));
                self.t.creator_outstanding = U128(add(self.t.creator_outstanding.0, amount.0));
                self.t.wnear_out = U128(ssub(self.t.wnear_out.0, amount.0));
            }
            "treasury" => {
                self.t.treasury_wnear = U128(add(self.t.treasury_wnear.0, amount.0));
                self.t.wnear_out = U128(ssub(self.t.wnear_out.0, amount.0));
            }
            "treasury_native" => {
                self.t.treasury_native = U128(add(self.t.treasury_native.0, amount.0));
            }
            _ => {}
        }
        emit("withdrawal", json!({ "kind": kind, "to": to, "amount": amount, "status": "failed_restored" }));
        false
    }

    /// Move part of the caller's creator-fee balance into another account's MOOT TIP balance.
    /// The recipient is a NEAR account id; the app resolves and confirms the @username first.
    #[payable]
    pub fn tip(&mut self, to: AccountId, amount: U128) {
        assert_one_yocto();
        let from = env::predecessor_account_id();
        require!(from != to, "cannot tip yourself");
        require!(amount.0 > 0, "amount must be positive");
        let bal = self.creator_fees.get(&from).copied().unwrap_or(0);
        require!(amount.0 <= bal, "not enough creator fees");
        self.creator_fees.insert(from.clone(), bal - amount.0);
        let tb = self.tips.get(&to).copied().unwrap_or(0);
        self.tips.insert(to.clone(), add(tb, amount.0));
        self.t.creator_outstanding = U128(sub(self.t.creator_outstanding.0, amount.0));
        self.t.tips_outstanding = U128(add(self.t.tips_outstanding.0, amount.0));
        emit("tip", json!({ "from": from, "to": to, "amount": amount }));
    }

    // ------------------------------------------------------------------ views

    pub fn get_config(&self) -> Config {
        self.cfg.clone()
    }
    pub fn get_admin(&self) -> AccountId {
        self.admin.clone()
    }
    pub fn get_treasury_account(&self) -> AccountId {
        self.treasury_account.clone()
    }
    pub fn get_wnear(&self) -> AccountId {
        self.wnear.clone()
    }

    pub fn launch_quote(&self) -> LaunchQuote {
        let q = self.launch_quote_inner();
        LaunchQuote {
            launch_fee_yocto: U128(q.launch_fee),
            deploy_cost_yocto: U128(q.deploy_cost + TOKEN_STATE_HEADROOM),
            record_reserve_yocto: U128(CORE_RECORD_RESERVE),
            required_deposit_yocto: U128(q.required),
            price_is_fresh: self.price_is_fresh(),
        }
    }

    pub fn get_token(&self, token_id: AccountId) -> Option<TokenView> {
        self.tokens.get(&token_id).map(|t| view(&token_id, t))
    }

    pub fn list_tokens(&self, from_index: Option<U64>, limit: Option<U64>) -> Vec<TokenView> {
        let from = from_index.map(|x| x.0).unwrap_or(0) as u32;
        let limit = limit.map(|x| x.0).unwrap_or(20).min(50) as u32;
        let mut out = Vec::new();
        let n = self.token_ids.len();
        let mut i = from;
        while i < n && (out.len() as u32) < limit {
            let id = self.token_ids.get(i).unwrap().clone();
            if let Some(t) = self.tokens.get(&id) {
                out.push(view(&id, t));
            }
            i += 1;
        }
        out
    }

    pub fn token_count(&self) -> U64 {
        U64(self.token_ids.len() as u64)
    }

    pub fn quote_buy(&self, token_id: AccountId, wnear_in: U128) -> Option<BuyQuoteView> {
        let t = self.tokens.get(&token_id)?;
        if t.state != TokenState::Live {
            return None;
        }
        let q = buy_quote(t.wnear_reserve, t.token_reserve, wnear_in.0, self.cfg.trading_fee_bps as u128)?;
        let s = split_fee(q.fee);
        let imp = price_impact_bps(t.wnear_reserve, t.token_reserve, t.wnear_reserve + q.net_in, t.token_reserve - q.tokens_out);
        Some(BuyQuoteView {
            fee: U128(q.fee),
            net_in: U128(q.net_in),
            tokens_out: U128(q.tokens_out),
            price_impact_bps: U128(imp),
            fee_to_creator: U128(s.creator),
            fee_to_trader: U128(s.trader),
            fee_to_treasury: U128(s.treasury),
        })
    }

    pub fn quote_sell(&self, token_id: AccountId, tokens_in: U128) -> Option<SellQuoteView> {
        let t = self.tokens.get(&token_id)?;
        if t.state != TokenState::Live {
            return None;
        }
        let q = sell_quote(t.wnear_reserve, t.token_reserve, tokens_in.0, self.cfg.trading_fee_bps as u128)?;
        let s = split_fee(q.fee);
        let imp = price_impact_bps(t.wnear_reserve, t.token_reserve, t.wnear_reserve - q.gross_out, t.token_reserve + tokens_in.0);
        Some(SellQuoteView {
            gross_out: U128(q.gross_out),
            fee: U128(q.fee),
            net_out: U128(q.net_out),
            price_impact_bps: U128(imp),
            fee_to_creator: U128(s.creator),
            fee_to_trader: U128(s.trader),
            fee_to_treasury: U128(s.treasury),
        })
    }

    pub fn get_tip_balance(&self, account_id: AccountId) -> U128 {
        U128(self.tips.get(&account_id).copied().unwrap_or(0))
    }
    pub fn get_creator_balance(&self, account_id: AccountId) -> U128 {
        U128(self.creator_fees.get(&account_id).copied().unwrap_or(0))
    }

    pub fn get_accounting(&self) -> Accounting {
        let net = self.t.wnear_in.0 as i128 - self.t.wnear_out.0 as i128;
        let liab = self.t.pool_wnear.0 + self.t.tips_outstanding.0 + self.t.creator_outstanding.0 + self.t.treasury_wnear.0;
        Accounting {
            totals: self.t.clone(),
            wnear_net_held: U128(net.max(0) as u128),
            wnear_liabilities: U128(liab),
            reconciles: net >= 0 && net as u128 == liab,
        }
    }
}

// ---------------------------------------------------------------------- internals

struct LaunchQuoteInner {
    launch_fee: u128,
    deploy_cost: u128,
    required: u128,
}

impl MootCore {
    fn assert_admin(&self) {
        require!(env::predecessor_account_id() == self.admin, "admin only");
    }

    fn price_is_fresh(&self) -> bool {
        env::block_timestamp() <= self.cfg.price_updated_ns.0.saturating_add(self.cfg.price_max_age_ns.0)
    }

    fn launch_quote_inner(&self) -> LaunchQuoteInner {
        let launch_fee = math::mul_div(self.cfg.launch_fee_usd_micro.0, NEAR, self.cfg.near_usd_price_micro.0);
        let deploy_cost = (FT_WASM.len() as u128 + 600) * env::storage_byte_cost().as_yoctonear();
        let required = add(add(launch_fee, deploy_cost + TOKEN_STATE_HEADROOM), CORE_RECORD_RESERVE);
        LaunchQuoteInner { launch_fee, deploy_cost, required }
    }

    fn send_wnear(&self, to: &AccountId, amount: u128) -> Promise {
        ext_ft::ext(self.wnear.clone())
            .with_attached_deposit(NearToken::from_yoctonear(1))
            .with_static_gas(GAS_FT_TRANSFER)
            .ft_transfer(to.clone(), U128(amount), Some("MOOT".to_string()))
    }

    /// Credits the 30/50/20 split of an already-collected fee. Called only after a swap settles.
    fn credit_fee(&mut self, tok: &mut TokenInfo, trader: &AccountId, fee: u128) {
        let s = split_fee(fee);
        let cb = self.creator_fees.get(&tok.creator).copied().unwrap_or(0);
        self.creator_fees.insert(tok.creator.clone(), add(cb, s.creator));
        let tb = self.tips.get(trader).copied().unwrap_or(0);
        self.tips.insert(trader.clone(), add(tb, s.trader));
        tok.creator_fees_earned = add(tok.creator_fees_earned, s.creator);
        self.t.treasury_wnear = U128(add(self.t.treasury_wnear.0, s.treasury));
        self.t.creator_outstanding = U128(add(self.t.creator_outstanding.0, s.creator));
        self.t.tips_outstanding = U128(add(self.t.tips_outstanding.0, s.trader));
        self.t.fees_collected = U128(add(self.t.fees_collected.0, fee));
        self.t.creator_fees_credited = U128(add(self.t.creator_fees_credited.0, s.creator));
        self.t.tips_credited = U128(add(self.t.tips_credited.0, s.trader));
        self.t.treasury_fees_credited = U128(add(self.t.treasury_fees_credited.0, s.treasury));
        emit(
            "fee_allocated",
            json!({ "token_id": tok.symbol, "creator": tok.creator, "trader": trader, "fee": U128(fee),
                    "treasury": U128(s.treasury), "creator_share": U128(s.creator), "trader_share": U128(s.trader) }),
        );
    }

    fn route_transfer(
        &mut self,
        caller: AccountId,
        sender: AccountId,
        amount: u128,
        msg: &str,
    ) -> Result<PromiseOrValue<U128>, &'static str> {
        if self.cfg.paused {
            return Err("trading is paused");
        }
        let parsed: Msg = serde_json::from_str(msg).map_err(|_| "bad message")?;
        let now = env::block_timestamp();
        match parsed {
            Msg::Seed { token } => {
                if caller != self.wnear {
                    return Err("seed must come from wNEAR");
                }
                let mut tok = self.tokens.get(&token).ok_or("unknown token")?.clone();
                if tok.state != TokenState::Created {
                    return Err("token is not waiting for liquidity");
                }
                if sender != tok.creator {
                    return Err("only the creator can seed liquidity");
                }
                if amount < self.cfg.min_initial_liquidity.0 {
                    return Err("below minimum initial liquidity");
                }
                tok.wnear_reserve = amount;
                tok.token_reserve = tok.total_supply;
                tok.initial_wnear = amount;
                tok.live_at = now;
                tok.state = TokenState::Live;
                tok.lp = Some(LpLock {
                    owner: tok.creator.clone(),
                    shares: amount,
                    locked_at: now,
                    unlock_at: now.saturating_add(self.cfg.lp_lock_ns.0),
                });
                let (creator, supply, unlock) = (tok.creator.clone(), tok.total_supply, now.saturating_add(self.cfg.lp_lock_ns.0));
                self.tokens.insert(token.clone(), tok);
                self.t.pool_wnear = U128(add(self.t.pool_wnear.0, amount));
                self.t.wnear_in = U128(add(self.t.wnear_in.0, amount));
                emit(
                    "token_live",
                    json!({ "token_id": token, "creator": creator, "wnear_liquidity": U128(amount),
                            "token_liquidity": U128(supply), "lp_unlock_at": U64(unlock) }),
                );
                Ok(PromiseOrValue::Value(U128(0)))
            }
            Msg::Buy { token, min_out, deadline_ns } => {
                if caller != self.wnear {
                    return Err("buy must come from wNEAR");
                }
                if let Some(d) = deadline_ns {
                    if now > d.0 {
                        return Err("deadline passed");
                    }
                }
                let mut tok = self.tokens.get(&token).ok_or("unknown token")?.clone();
                if tok.state != TokenState::Live {
                    return Err("token is not live");
                }
                let q = buy_quote(tok.wnear_reserve, tok.token_reserve, amount, self.cfg.trading_fee_bps as u128)
                    .ok_or("trade too small or pool empty")?;
                if q.tokens_out < min_out.0 {
                    return Err("slippage: below minimum output");
                }
                tok.wnear_reserve = add(tok.wnear_reserve, q.net_in);
                tok.token_reserve = sub(tok.token_reserve, q.tokens_out);
                self.tokens.insert(token.clone(), tok);
                self.t.pool_wnear = U128(add(self.t.pool_wnear.0, q.net_in));
                self.t.wnear_in = U128(add(self.t.wnear_in.0, amount));
                let p = ext_ft::ext(token.clone())
                    .with_attached_deposit(NearToken::from_yoctonear(1))
                    .with_static_gas(GAS_FT_TRANSFER)
                    .ft_transfer(sender.clone(), U128(q.tokens_out), Some("MOOT buy".to_string()))
                    .then(
                        Self::ext(env::current_account_id())
                            .with_static_gas(GAS_CALLBACK)
                            .on_buy_settled(token, sender, U128(amount), U128(q.net_in), U128(q.fee), U128(q.tokens_out)),
                    );
                Ok(PromiseOrValue::Promise(p))
            }
            Msg::Sell { min_out, deadline_ns } => {
                // For a sell the caller is the MOOT token contract itself.
                if let Some(d) = deadline_ns {
                    if now > d.0 {
                        return Err("deadline passed");
                    }
                }
                let token = caller;
                let mut tok = self.tokens.get(&token).ok_or("unknown token")?.clone();
                if tok.state != TokenState::Live {
                    return Err("token is not live");
                }
                let q = sell_quote(tok.wnear_reserve, tok.token_reserve, amount, self.cfg.trading_fee_bps as u128)
                    .ok_or("trade too small or pool empty")?;
                if q.net_out < min_out.0 {
                    return Err("slippage: below minimum output");
                }
                tok.token_reserve = add(tok.token_reserve, amount);
                tok.wnear_reserve = sub(tok.wnear_reserve, q.gross_out);
                self.tokens.insert(token.clone(), tok);
                self.t.pool_wnear = U128(sub(self.t.pool_wnear.0, q.gross_out));
                self.t.wnear_out = U128(add(self.t.wnear_out.0, q.net_out));
                let p = self.send_wnear(&sender, q.net_out).then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_CALLBACK)
                        .on_sell_settled(token, sender, U128(amount), U128(q.gross_out), U128(q.fee), U128(q.net_out)),
                );
                Ok(PromiseOrValue::Promise(p))
            }
        }
    }
}

fn view(id: &AccountId, t: &TokenInfo) -> TokenView {
    TokenView {
        token_id: id.clone(),
        creator: t.creator.clone(),
        name: t.name.clone(),
        symbol: t.symbol.clone(),
        description: t.description.clone(),
        decimals: TOKEN_DECIMALS,
        total_supply: U128(t.total_supply),
        state: t.state,
        token_reserve: U128(t.token_reserve),
        wnear_reserve: U128(t.wnear_reserve),
        initial_wnear: U128(t.initial_wnear),
        created_at: U64(t.created_at),
        live_at: U64(t.live_at),
        lp_owner: t.lp.as_ref().map(|l| l.owner.clone()),
        lp_shares: t.lp.as_ref().map(|l| U128(l.shares)),
        lp_locked_at: t.lp.as_ref().map(|l| U64(l.locked_at)),
        lp_unlock_at: t.lp.as_ref().map(|l| U64(l.unlock_at)),
        creator_fees_earned: U128(t.creator_fees_earned),
        volume_wnear: U128(t.volume_wnear),
        buys: U64(t.buys),
        sells: U64(t.sells),
    }
}

#[cfg(test)]
mod tests;
