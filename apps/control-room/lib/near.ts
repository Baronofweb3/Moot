import { CORE, RPC_URLS, WNEAR } from "./config";

export async function rpc<T>(method: string, params: unknown): Promise<T> {
  let lastErr: unknown;
  for (const url of RPC_URLS) {
    try {
      const r = await fetch(url, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: "moot", method, params }),
      });
      const j = await r.json();
      if (j.error) throw new Error(j.error.data || j.error.message || "RPC error");
      return j.result as T;
    } catch (e) {
      lastErr = e;
    }
  }
  throw lastErr instanceof Error ? lastErr : new Error("RPC unavailable");
}

export async function view<T>(contractId: string, methodName: string, args: object = {}): Promise<T> {
  const res = await rpc<{ result: number[]; error?: string }>("query", {
    request_type: "call_function",
    finality: "optimistic",
    account_id: contractId,
    method_name: methodName,
    args_base64: btoa(JSON.stringify(args)),
  });
  if (res.error) throw new Error(res.error);
  const text = new TextDecoder().decode(new Uint8Array(res.result));
  return (text ? JSON.parse(text) : null) as T;
}

export async function nativeBalance(accountId: string): Promise<bigint> {
  const r = await rpc<{ amount: string }>("query", { request_type: "view_account", finality: "optimistic", account_id: accountId });
  return BigInt(r.amount);
}

export type TokenState = "deploying" | "created" | "live";
export interface TokenView {
  token_id: string; creator: string; name: string; symbol: string; description: string; decimals: number;
  total_supply: string; state: TokenState; token_reserve: string; wnear_reserve: string; initial_wnear: string;
  created_at: string; live_at: string; lp_owner: string | null; lp_shares: string | null;
  lp_locked_at: string | null; lp_unlock_at: string | null; creator_fees_earned: string;
  volume_wnear: string; buys: string; sells: string;
}
export interface BuyQuote { fee: string; net_in: string; tokens_out: string; price_impact_bps: string; fee_to_creator: string; fee_to_trader: string; fee_to_treasury: string }
export interface SellQuote { gross_out: string; fee: string; net_out: string; price_impact_bps: string; fee_to_creator: string; fee_to_trader: string; fee_to_treasury: string }
export interface Config { trading_fee_bps: number; launch_fee_usd_micro: string; near_usd_price_micro: string; price_updated_ns: string; price_max_age_ns: string; min_initial_liquidity: string; lp_lock_ns: string; paused: boolean }
export interface LaunchQuote { launch_fee_yocto: string; deploy_cost_yocto: string; record_reserve_yocto: string; required_deposit_yocto: string; price_is_fresh: boolean }
export interface Totals {
  launches: string; trades: string; volume_wnear: string; fees_collected: string; creator_fees_credited: string; tips_credited: string;
  treasury_fees_credited: string; launch_fees_native: string; pool_wnear: string; tips_outstanding: string; creator_outstanding: string;
  treasury_wnear: string; treasury_native: string; wnear_in: string; wnear_out: string;
}
export interface Accounting { totals: Totals; wnear_net_held: string; wnear_liabilities: string; reconciles: boolean }

export const core = {
  config: () => view<Config>(CORE, "get_config"),
  admin: () => view<string>(CORE, "get_admin"),
  treasuryAccount: () => view<string>(CORE, "get_treasury_account"),
  launchQuote: () => view<LaunchQuote>(CORE, "launch_quote"),
  token: (id: string) => view<TokenView | null>(CORE, "get_token", { token_id: id }),
  list: (from = 0, limit = 50) => view<TokenView[]>(CORE, "list_tokens", { from_index: String(from), limit: String(limit) }),
  count: () => view<string>(CORE, "token_count"),
  quoteBuy: (id: string, wnear: string) => view<BuyQuote | null>(CORE, "quote_buy", { token_id: id, wnear_in: wnear }),
  quoteSell: (id: string, tokens: string) => view<SellQuote | null>(CORE, "quote_sell", { token_id: id, tokens_in: tokens }),
  tipBalance: (a: string) => view<string>(CORE, "get_tip_balance", { account_id: a }),
  creatorBalance: (a: string) => view<string>(CORE, "get_creator_balance", { account_id: a }),
  accounting: () => view<Accounting>(CORE, "get_accounting"),
};

export const ft = {
  balance: (contract: string, a: string) => view<string>(contract, "ft_balance_of", { account_id: a }),
  storage: (contract: string, a: string) => view<{ total: string; available: string } | null>(contract, "storage_balance_of", { account_id: a }),
  wnearBalance: (a: string) => view<string>(WNEAR, "ft_balance_of", { account_id: a }),
};
