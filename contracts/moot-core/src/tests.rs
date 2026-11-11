use super::*;
use near_sdk::test_utils::{get_logs, VMContextBuilder};
use near_sdk::{testing_env, PromiseResult, RuntimeFeesConfig};

const N: u128 = NEAR;

fn acc(s: &str) -> AccountId {
    s.parse().unwrap()
}
fn ctx(pred: &str, deposit: u128) -> VMContextBuilder {
    let mut b = VMContextBuilder::new();
    b.current_account_id(acc("moot.testnet"))
        .predecessor_account_id(acc(pred))
        .attached_deposit(NearToken::from_yoctonear(deposit))
        .block_timestamp(1_000_000_000_000);
    b
}
fn with_result(pred: &str, ok: bool) {
    let b = ctx(pred, 0);
    let r = if ok { PromiseResult::Successful(vec![]) } else { PromiseResult::Failed };
    testing_env!(b.build(), near_sdk::test_vm_config(), RuntimeFeesConfig::test(), Default::default(), vec![r]);
}
fn new_core() -> MootCore {
    testing_env!(ctx("admin.testnet", 0).build());
    // $3 at $3/NEAR => 1 NEAR launch fee
    MootCore::new(acc("admin.testnet"), acc("treasury.testnet"), acc("wrap.testnet"), U128(3_000_000), U128(N))
}
const SUPPLY_WHOLE: u128 = 1_000_000_000;
fn launch(c: &mut MootCore) -> AccountId {
    let q = c.launch_quote();
    testing_env!(ctx("alice.testnet", q.required_deposit_yocto.0).build());
    let _ = c.create_token("Moot Dog".into(), "MDOG".into(), None, "d".into(), U128(SUPPLY_WHOLE));
    let tid = acc("mdog.moot.testnet");
    with_result("moot.testnet", true);
    assert!(c.on_token_created(tid.clone(), acc("alice.testnet"), q.launch_fee_yocto, q.required_deposit_yocto, Ok(())));
    tid
}
fn live(c: &mut MootCore) -> AccountId {
    let tid = launch(c);
    testing_env!(ctx("wrap.testnet", 0).build());
    let _ = c.ft_on_transfer(acc("alice.testnet"), U128(100 * N), format!(r#"{{"action":"seed","token":"{}"}}"#, tid));
    tid
}
fn buy(c: &mut MootCore, tid: &AccountId, who: &str, amt: u128) -> BuyQuoteView {
    let q = c.quote_buy(tid.clone(), U128(amt)).unwrap();
    testing_env!(ctx("wrap.testnet", 0).build());
    let _ = c.ft_on_transfer(
        acc(who),
        U128(amt),
        format!(r#"{{"action":"buy","token":"{}","min_out":"0"}}"#, tid),
    );
    with_result("moot.testnet", true);
    c.on_buy_settled(tid.clone(), acc(who), U128(amt), q.net_in, q.fee, q.tokens_out, Ok(()));
    q
}

#[test]
fn launch_fee_is_three_usd_in_near() {
    let c = new_core();
    assert_eq!(c.launch_quote().launch_fee_yocto.0, N); // $3 / $3 per NEAR
}

#[test]
fn launch_puts_whole_supply_in_pool_side_and_creator_gets_none() {
    let mut c = new_core();
    let tid = launch(&mut c);
    let t = c.get_token(tid).unwrap();
    assert_eq!(t.total_supply.0, SUPPLY_WHOLE * WHOLE);
    assert_eq!(t.token_reserve.0, t.total_supply.0);
    assert_eq!(t.state, TokenState::Created);
    assert_eq!(c.get_accounting().totals.launch_fees_native.0, N);
}

#[test]
fn duplicate_symbol_rejected() {
    let mut c = new_core();
    launch(&mut c);
    let q = c.launch_quote();
    testing_env!(ctx("bob.testnet", q.required_deposit_yocto.0).build());
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        c.create_token("X".into(), "MDOG".into(), None, "".into(), U128(10));
    }));
    assert!(r.is_err());
}

#[test]
#[should_panic(expected = "attach at least")]
fn underpaying_launch_rejected() {
    let mut c = new_core();
    testing_env!(ctx("alice.testnet", N / 2).build());
    c.create_token("A".into(), "AB".into(), None, "".into(), U128(10));
}

#[test]
#[should_panic(expected = "stale")]
fn stale_price_blocks_launch() {
    let mut c = new_core();
    let mut b = ctx("alice.testnet", 100 * N);
    b.block_timestamp(1_000_000_000_000 + 8 * 86_400 * 1_000_000_000);
    testing_env!(b.build());
    c.create_token("A".into(), "AB".into(), None, "".into(), U128(10));
}

#[test]
fn failed_deploy_removes_record_and_allows_retry() {
    let mut c = new_core();
    let q = c.launch_quote();
    testing_env!(ctx("alice.testnet", q.required_deposit_yocto.0).build());
    let _ = c.create_token("A".into(), "AB".into(), None, "".into(), U128(10));
    with_result("moot.testnet", false);
    assert!(!c.on_token_created(acc("ab.moot.testnet"), acc("alice.testnet"), q.launch_fee_yocto, q.required_deposit_yocto, Err(PromiseError::Failed)));
    assert!(c.get_token(acc("ab.moot.testnet")).is_none());
    assert_eq!(c.get_accounting().totals.launch_fees_native.0, 0);
}

#[test]
fn only_creator_can_seed_and_min_enforced() {
    let mut c = new_core();
    let tid = launch(&mut c);
    let msg = format!(r#"{{"action":"seed","token":"{}"}}"#, tid);
    testing_env!(ctx("wrap.testnet", 0).build());
    // wrong sender: full refund
    let r = c.ft_on_transfer(acc("mallory.testnet"), U128(100 * N), msg.clone());
    assert!(matches!(r, PromiseOrValue::Value(v) if v.0 == 100 * N));
    // too small
    let r = c.ft_on_transfer(acc("alice.testnet"), U128(N / 2), msg.clone());
    assert!(matches!(r, PromiseOrValue::Value(v) if v.0 == N / 2));
    // wrong caller (not wNEAR)
    testing_env!(ctx("evil.testnet", 0).build());
    let r = c.ft_on_transfer(acc("alice.testnet"), U128(100 * N), msg);
    assert!(matches!(r, PromiseOrValue::Value(_)));
    assert_eq!(c.get_token(tid).unwrap().state, TokenState::Created);
}

#[test]
fn seeding_makes_token_live_and_locks_lp_one_year() {
    let mut c = new_core();
    let tid = live(&mut c);
    let t = c.get_token(tid).unwrap();
    assert_eq!(t.state, TokenState::Live);
    assert_eq!(t.wnear_reserve.0, 100 * N);
    assert_eq!(t.lp_unlock_at.unwrap().0 - t.lp_locked_at.unwrap().0, DEFAULT_LP_LOCK_NS);
    assert!(c.get_accounting().reconciles);
}

#[test]
fn spec_example_100_dollar_trade_splits_30_50_20() {
    let mut c = new_core();
    let tid = live(&mut c);
    // 100 NEAR at 1% fee = 1 NEAR fee => 0.30 / 0.50 / 0.20
    let q = buy(&mut c, &tid, "bob.testnet", 100 * N);
    assert_eq!(q.fee.0, N);
    assert_eq!(q.fee_to_treasury.0, N * 30 / 100);
    assert_eq!(q.fee_to_creator.0, N * 50 / 100);
    assert_eq!(q.fee_to_trader.0, N * 20 / 100);
    assert_eq!(c.get_creator_balance(acc("alice.testnet")).0, N / 2);
    assert_eq!(c.get_tip_balance(acc("bob.testnet")).0, N / 5);
    assert_eq!(c.get_accounting().totals.treasury_wnear.0, N * 30 / 100);
}

#[test]
fn total_supply_never_changes_and_accounting_reconciles_after_trades() {
    let mut c = new_core();
    let tid = live(&mut c);
    buy(&mut c, &tid, "bob.testnet", 10 * N);
    buy(&mut c, &tid, "carol.testnet", 3 * N);
    let t = c.get_token(tid).unwrap();
    assert_eq!(t.total_supply.0, SUPPLY_WHOLE * WHOLE);
    assert!(t.token_reserve.0 < t.total_supply.0);
    let a = c.get_accounting();
    assert!(a.reconciles, "{:?} vs {:?}", a.wnear_net_held.0, a.wnear_liabilities.0);
    assert_eq!(a.totals.fees_collected.0,
        a.totals.creator_fees_credited.0 + a.totals.tips_credited.0 + a.totals.treasury_fees_credited.0);
}

#[test]
fn slippage_and_deadline_refund_in_full() {
    let mut c = new_core();
    let tid = live(&mut c);
    testing_env!(ctx("wrap.testnet", 0).build());
    let r = c.ft_on_transfer(acc("bob.testnet"), U128(N),
        format!(r#"{{"action":"buy","token":"{}","min_out":"{}"}}"#, tid, u128::MAX));
    assert!(matches!(r, PromiseOrValue::Value(v) if v.0 == N));
    let r = c.ft_on_transfer(acc("bob.testnet"), U128(N),
        format!(r#"{{"action":"buy","token":"{}","min_out":"0","deadline_ns":"1"}}"#, tid));
    assert!(matches!(r, PromiseOrValue::Value(v) if v.0 == N));
    assert_eq!(c.get_token(tid).unwrap().wnear_reserve.0, 100 * N);
}

#[test]
fn failed_buy_transfer_reverts_reserves_and_credits_no_fees() {
    let mut c = new_core();
    let tid = live(&mut c);
    let q = c.quote_buy(tid.clone(), U128(10 * N)).unwrap();
    testing_env!(ctx("wrap.testnet", 0).build());
    let _ = c.ft_on_transfer(acc("bob.testnet"), U128(10 * N),
        format!(r#"{{"action":"buy","token":"{}","min_out":"0"}}"#, tid));
    with_result("moot.testnet", false);
    let refund = c.on_buy_settled(tid.clone(), acc("bob.testnet"), U128(10 * N), q.net_in, q.fee, q.tokens_out, Err(PromiseError::Failed));
    assert_eq!(refund.0, 10 * N);
    let t = c.get_token(tid).unwrap();
    assert_eq!(t.wnear_reserve.0, 100 * N);
    assert_eq!(t.token_reserve.0, t.total_supply.0);
    assert_eq!(c.get_creator_balance(acc("alice.testnet")).0, 0);
    assert!(c.get_accounting().reconciles);
}

#[test]
fn sell_splits_fee_and_reconciles() {
    let mut c = new_core();
    let tid = live(&mut c);
    let bought = buy(&mut c, &tid, "bob.testnet", 10 * N).tokens_out.0;
    let q = c.quote_sell(tid.clone(), U128(bought)).unwrap();
    testing_env!(ctx(tid.as_str(), 0).build());
    let _ = c.ft_on_transfer(acc("bob.testnet"), U128(bought), r#"{"action":"sell","min_out":"0"}"#.into());
    with_result("moot.testnet", true);
    c.on_sell_settled(tid.clone(), acc("bob.testnet"), U128(bought), q.gross_out, q.fee, q.net_out, Ok(()));
    assert!(c.get_accounting().reconciles);
    assert_eq!(q.fee.0, q.fee_to_creator.0 + q.fee_to_trader.0 + q.fee_to_treasury.0);
    assert!(c.get_tip_balance(acc("bob.testnet")).0 > 0);
}

#[test]
fn failed_sell_transfer_restores_pool_and_returns_tokens() {
    let mut c = new_core();
    let tid = live(&mut c);
    let bought = buy(&mut c, &tid, "bob.testnet", 10 * N).tokens_out.0;
    let before = c.get_token(tid.clone()).unwrap();
    let q = c.quote_sell(tid.clone(), U128(bought)).unwrap();
    testing_env!(ctx(tid.as_str(), 0).build());
    let _ = c.ft_on_transfer(acc("bob.testnet"), U128(bought), r#"{"action":"sell","min_out":"0"}"#.into());
    with_result("moot.testnet", false);
    let back = c.on_sell_settled(tid.clone(), acc("bob.testnet"), U128(bought), q.gross_out, q.fee, q.net_out, Err(PromiseError::Failed));
    assert_eq!(back.0, bought);
    let after = c.get_token(tid).unwrap();
    assert_eq!(after.wnear_reserve.0, before.wnear_reserve.0);
    assert_eq!(after.token_reserve.0, before.token_reserve.0);
    assert!(c.get_accounting().reconciles);
}

#[test]
fn sell_from_unknown_token_contract_is_refunded() {
    let mut c = new_core();
    live(&mut c);
    testing_env!(ctx("fake.testnet", 0).build());
    let r = c.ft_on_transfer(acc("bob.testnet"), U128(5), r#"{"action":"sell","min_out":"0"}"#.into());
    assert!(matches!(r, PromiseOrValue::Value(v) if v.0 == 5));
}

#[test]
fn creator_withdraw_needs_one_yocto_and_balance_then_restores_on_failure() {
    let mut c = new_core();
    let tid = live(&mut c);
    buy(&mut c, &tid, "bob.testnet", 100 * N);
    testing_env!(ctx("alice.testnet", 1).build());
    let _ = c.withdraw_creator_fees(None);
    assert_eq!(c.get_creator_balance(acc("alice.testnet")).0, 0);
    with_result("moot.testnet", false);
    assert!(!c.on_withdraw_done("creator".into(), acc("alice.testnet"), U128(N / 2), Err(PromiseError::Failed)));
    assert_eq!(c.get_creator_balance(acc("alice.testnet")).0, N / 2);
    assert!(c.get_accounting().reconciles);
}

#[test]
#[should_panic(expected = "nothing to withdraw")]
fn cannot_withdraw_more_than_balance() {
    let mut c = new_core();
    testing_env!(ctx("alice.testnet", 1).build());
    c.withdraw_creator_fees(Some(U128(1)));
}

#[test]
#[should_panic(expected = "treasury account only")]
fn only_treasury_can_withdraw_treasury() {
    let mut c = new_core();
    testing_env!(ctx("admin.testnet", 1).build());
    c.withdraw_treasury_wnear(None);
}

#[test]
fn tip_moves_creator_fees_to_recipient_tip_balance() {
    let mut c = new_core();
    let tid = live(&mut c);
    buy(&mut c, &tid, "bob.testnet", 100 * N);
    testing_env!(ctx("alice.testnet", 1).build());
    c.tip(acc("dave.testnet"), U128(N / 4));
    assert_eq!(c.get_tip_balance(acc("dave.testnet")).0, N / 4);
    assert_eq!(c.get_creator_balance(acc("alice.testnet")).0, N / 4);
    assert!(c.get_accounting().reconciles);
}

#[test]
#[should_panic(expected = "not enough creator fees")]
fn cannot_tip_more_than_balance() {
    let mut c = new_core();
    testing_env!(ctx("alice.testnet", 1).build());
    c.tip(acc("dave.testnet"), U128(1));
}

#[test]
#[should_panic(expected = "admin only")]
fn non_admin_cannot_change_config() {
    let mut c = new_core();
    testing_env!(ctx("mallory.testnet", 0).build());
    c.set_trading_fee_bps(0);
}

#[test]
#[should_panic(expected = "hard cap")]
fn admin_cannot_exceed_fee_cap() {
    let mut c = new_core();
    testing_env!(ctx("admin.testnet", 0).build());
    c.set_trading_fee_bps(301);
}

#[test]
fn pause_refunds_trades() {
    let mut c = new_core();
    let tid = live(&mut c);
    testing_env!(ctx("admin.testnet", 0).build());
    c.set_paused(true);
    testing_env!(ctx("wrap.testnet", 0).build());
    let r = c.ft_on_transfer(acc("bob.testnet"), U128(N),
        format!(r#"{{"action":"buy","token":"{}","min_out":"0"}}"#, tid));
    assert!(matches!(r, PromiseOrValue::Value(v) if v.0 == N));
}

#[test]
fn events_are_nep297() {
    let mut c = new_core();
    live(&mut c);
    let logs = get_logs();
    assert!(logs.iter().any(|l| l.starts_with("EVENT_JSON:") && l.contains("\"standard\":\"moot\"")));
}
