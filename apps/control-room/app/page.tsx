"use client";
import { useState } from "react";
import { core, ft } from "@/lib/near";
import { CORE, WNEAR, GAS, ONE_YOCTO, STORAGE_DEPOSIT, EXPLORER } from "@/lib/config";
import { Call, useWallet } from "@/lib/wallet";
import { yoctoToNear, nearToYocto, parseUnits, nsToDate, compactTokens, formatUnits } from "@/lib/units.mjs";
import { usePoll, Notice, Stat, Row, Phase2 } from "@/components/ui";

type Tab = "overview" | "tokens" | "treasury" | "settings" | "team" | "boosts";
const TABS: [Tab, string][] = [["overview", "Overview & analytics"], ["tokens", "Tokens"], ["treasury", "Treasury"], ["settings", "Protocol settings"], ["team", "Team & audit"], ["boosts", "Boosts & MCP"]];

export default function ControlRoom() {
  const { accountId, signIn, signOut, ready } = useWallet();
  const roles = usePoll(async () => ({ admin: await core.admin(), treasury: await core.treasuryAccount() }), []);
  const [tab, setTab] = useState<Tab>("overview");
  const isAdmin = !!accountId && accountId === roles.data?.admin;
  const isTreasury = !!accountId && accountId === roles.data?.treasury;
  const allowed = isAdmin || isTreasury;

  return (
    <>
      <header className="top"><div className="wrap">
        <div className="brand"><img src="/logo.png" alt="" />MOOT <em>CONTROL ROOM</em></div>
        <div style={{ flex: 1 }} />
        {accountId ? <button className="btn ghost" onClick={() => signOut()}>{accountId}</button> : <button className="btn" disabled={!ready} onClick={signIn}>Sign in with wallet</button>}
      </div></header>
      <main className="wrap">
        {!accountId && <div style={{ paddingTop: 60, textAlign: "center" }}><h1>Restricted</h1><p className="mute">Sign in with the protocol admin or treasury wallet. This app is separate from the public MOOT app.</p></div>}
        {accountId && roles.loading && <p className="mute" style={{ paddingTop: 40 }}>Checking permissions on chain…</p>}
        {accountId && roles.data && !allowed && (
          <div style={{ paddingTop: 50 }}><Notice kind="err">{accountId} is not the admin or treasury account of {CORE}. Access denied. (Admin: {roles.data.admin} · Treasury: {roles.data.treasury})</Notice></div>
        )}
        {allowed && roles.data && (<>
          <div className="tabs">{TABS.map(([k, l]) => <button key={k} className={tab === k ? "on" : ""} onClick={() => setTab(k)}>{l}</button>)}</div>
          <p className="mute small" style={{ marginTop: -8 }}>Signed in as <b>{isAdmin ? "admin" : ""}{isAdmin && isTreasury ? " + " : ""}{isTreasury ? "treasury" : ""}</b>. The UI is a convenience; the contract itself rejects any call from other accounts.</p>
          {tab === "overview" && <Overview />}
          {tab === "tokens" && <Tokens />}
          {tab === "treasury" && <Treasury isTreasury={isTreasury} treasury={roles.data.treasury} />}
          {tab === "settings" && <Settings isAdmin={isAdmin} />}
          {tab === "team" && <Planned title="Team & audit log" items={["Multiple operator accounts with roles (viewer, operator, admin)", "Searchable audit log of every admin action (events exist on chain; the viewer needs the indexer)", "Multisig for admin and treasury (use a NEAR multisig account in production)"]} />}
          {tab === "boosts" && <Planned title="Boosts, MCP tools & reputation" items={["Token boosts and promoted placements", "Creator reputation signals", "MCP tools so AI agents can launch, quote and trade on MOOT"]} />}
        </>)}
        <footer>MOOT Control Room · testnet · every number is read from {CORE}</footer>
      </main>
    </>
  );
}

function Planned({ title, items }: { title: string; items: string[] }) {
  return <div className="card"><h3>{title} <Phase2 /></h3><p className="mute small">Not built. Planned for a later phase:</p><ul className="mute">{items.map((i) => <li key={i}>{i}</li>)}</ul></div>;
}

function Overview() {
  const acc = usePoll(() => core.accounting(), [], 10000);
  const cfg = usePoll(() => core.config(), [], 20000);
  const a = acc.data, t = a?.totals;
  const fees = t ? [BigInt(t.creator_fees_credited), BigInt(t.tips_credited), BigInt(t.treasury_fees_credited)] : [0n, 0n, 0n];
  const tot = fees[0] + fees[1] + fees[2];
  const w = (x: bigint) => (tot === 0n ? 0 : Number((x * 1000n) / tot) / 10);
  return (<>
    {acc.error && <Notice kind="err">{acc.error}</Notice>}
    <div className="card" style={{ marginBottom: 14 }}>
      <h3>Solvency check <span className={`pill ${a?.reconciles ? "good" : "bad"}`}>{a ? (a.reconciles ? "reconciles" : "MISMATCH") : "…"}</span></h3>
      <Row k="wNEAR held (in − out)" v={a ? `${yoctoToNear(a.wnear_net_held, 6)} Ⓝ` : "…"} />
      <Row k="wNEAR owed (pools + creator + tips + treasury)" v={a ? `${yoctoToNear(a.wnear_liabilities, 6)} Ⓝ` : "…"} />
      <p className="mute small" style={{ marginBottom: 0 }}>Counters are maintained by the contract on every settled flow. If they ever disagree, pause the protocol and investigate.</p>
    </div>
    <div className="grid g3">
      <Stat label="Tokens launched" value={t?.launches ?? "…"} />
      <Stat label="Trades settled" value={t?.trades ?? "…"} />
      <Stat label="Volume" value={t ? `${yoctoToNear(t.volume_wnear, 4)} Ⓝ` : "…"} />
      <Stat label="Trading fees collected" value={t ? `${yoctoToNear(t.fees_collected, 6)} Ⓝ` : "…"} />
      <Stat label="Launch fees (native)" value={t ? `${yoctoToNear(t.launch_fees_native, 4)} Ⓝ` : "…"} />
      <Stat label="Protocol status" value={cfg.data ? (cfg.data.paused ? "PAUSED" : "Live") : "…"} />
    </div>
    <div className="card" style={{ marginTop: 14 }}>
      <h3>Fee distribution (cumulative)</h3>
      <div className="bar"><i style={{ width: w(fees[1]) + "%", background: "#3ddc97" }} /><i style={{ width: w(fees[0]) + "%", background: "#5766ff" }} /><i style={{ width: w(fees[2]) + "%", background: "#ffc857" }} /></div>
      <div className="grid g3" style={{ marginTop: 12 }}>
        <Row k="Creators (target 50%)" v={t ? `${yoctoToNear(t.creator_fees_credited, 6)} Ⓝ · ${w(fees[0]).toFixed(1)}%` : "…"} />
        <Row k="Traders (target 20%)" v={t ? `${yoctoToNear(t.tips_credited, 6)} Ⓝ · ${w(fees[1]).toFixed(1)}%` : "…"} />
        <Row k="Treasury (target 30%)" v={t ? `${yoctoToNear(t.treasury_fees_credited, 6)} Ⓝ · ${w(fees[2]).toFixed(1)}%` : "…"} />
      </div>
    </div>
    {cfg.data && <div className="card" style={{ marginTop: 14 }}><h3>Current configuration</h3>
      <Row k="Trading fee" v={`${cfg.data.trading_fee_bps / 100}%`} />
      <Row k="Launch fee" v={`$${Number(cfg.data.launch_fee_usd_micro) / 1e6}`} />
      <Row k="NEAR/USD rate (admin-set, testnet)" v={`$${Number(cfg.data.near_usd_price_micro) / 1e6}`} />
      <Row k="Rate updated" v={nsToDate(cfg.data.price_updated_ns).toUTCString()} />
      <Row k="Min. initial liquidity" v={`${yoctoToNear(cfg.data.min_initial_liquidity, 3)} Ⓝ`} />
      <Row k="LP lock for new launches" v={`${Number(BigInt(cfg.data.lp_lock_ns) / 86_400_000_000_000n)} days`} />
    </div>}
  </>);
}

function Tokens() {
  const toks = usePoll(() => core.list(0, 50), [], 15000);
  return (<div className="card">
    {toks.error && <Notice kind="err">{toks.error}</Notice>}
    <table><thead><tr><th>Token</th><th>Creator</th><th>State</th><th>Pool</th><th>Volume</th><th>Trades</th><th>LP unlock</th></tr></thead><tbody>
      {(toks.data ?? []).map((k) => <tr key={k.token_id}>
        <td><a href={`${EXPLORER}/address/${k.token_id}`} target="_blank" rel="noreferrer">{k.name} <span className="mute">${k.symbol}</span></a></td>
        <td className="mute small">{k.creator}</td><td><span className={`pill ${k.state === "live" ? "live" : ""}`}>{k.state}</span></td>
        <td>{yoctoToNear(k.wnear_reserve, 3)} Ⓝ</td><td>{yoctoToNear(k.volume_wnear, 3)} Ⓝ</td><td>{Number(k.buys) + Number(k.sells)}</td>
        <td className="small">{k.lp_unlock_at ? nsToDate(k.lp_unlock_at).toISOString().slice(0, 10) : "—"}</td></tr>)}
    </tbody></table>
    {!toks.loading && !(toks.data ?? []).length && <p className="mute">No tokens yet.</p>}
    <p className="mute small">There is deliberately no delist, freeze or confiscate action here. The contract has none.</p>
  </div>);
}

function Treasury({ isTreasury, treasury }: { isTreasury: boolean; treasury: string }) {
  const { send } = useWallet();
  const acc = usePoll(() => core.accounting(), [], 10000);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ k: "ok" | "err"; t: string } | null>(null);
  const t = acc.data?.totals;
  async function run(calls: () => Promise<Call[]>) {
    setBusy(true); setMsg(null);
    try { await send(await calls(), window.location.href); setMsg({ k: "ok", t: "Submitted. Balances refresh shortly." }); setTimeout(acc.reload, 4000); }
    catch (e: any) { setMsg({ k: "err", t: e?.message || String(e) }); } finally { setBusy(false); }
  }
  return (<>
    {msg && <Notice kind={msg.k}>{msg.t}</Notice>}
    <div className="grid g2">
      <div className="card"><h3>Treasury wNEAR (30% of trading fees)</h3><div className="stat"><div className="v" style={{ fontSize: 26, fontWeight: 800 }}>{t ? yoctoToNear(t.treasury_wnear, 6) : "…"} Ⓝ</div></div>
        <p className="mute small">Withdraws to {treasury} only.</p>
        <button className="btn wide" disabled={!isTreasury || busy || !t || BigInt(t.treasury_wnear) === 0n}
          onClick={() => run(async () => [
            ...((await ft.storage(WNEAR, treasury)) ? [] : [{ receiverId: WNEAR, method: "storage_deposit", gas: GAS.WRAP, deposit: STORAGE_DEPOSIT, args: { account_id: treasury, registration_only: true } } as Call]),
            { receiverId: CORE, method: "withdraw_treasury_wnear", gas: GAS.SIMPLE, deposit: ONE_YOCTO, args: { amount: null } }])}>Withdraw all wNEAR</button></div>
      <div className="card"><h3>Treasury NEAR (launch fees)</h3><div className="stat"><div className="v" style={{ fontSize: 26, fontWeight: 800 }}>{t ? yoctoToNear(t.treasury_native, 6) : "…"} Ⓝ</div></div>
        <p className="mute small">Withdraws to {treasury} only.</p>
        <button className="btn wide" disabled={!isTreasury || busy || !t || BigInt(t.treasury_native) === 0n}
          onClick={() => run(async () => [{ receiverId: CORE, method: "withdraw_treasury_native", gas: GAS.SIMPLE, deposit: ONE_YOCTO, args: { amount: null } }])}>Withdraw all NEAR</button></div>
    </div>
    {!isTreasury && <Notice kind="warn">Only the treasury account ({treasury}) can withdraw. You are signed in as admin. In production the treasury should be a multisig.</Notice>}
  </>);
}

function Settings({ isAdmin }: { isAdmin: boolean }) {
  const { send } = useWallet();
  const cfg = usePoll(() => core.config(), []);
  const [msg, setMsg] = useState<{ k: "ok" | "err"; t: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [v, setV] = useState({ fee: "", launch: "", price: "", minliq: "", lock: "", treasury: "" });
  async function call(method: string, build: () => object) {
    setBusy(true); setMsg(null);
    try { await send([{ receiverId: CORE, method, gas: GAS.SIMPLE, args: build() }], window.location.href); setMsg({ k: "ok", t: `${method} submitted.` }); setTimeout(cfg.reload, 4000); }
    catch (e: any) { setMsg({ k: "err", t: e?.message || String(e) }); } finally { setBusy(false); }
  }
  const paused = cfg.data?.paused;
  const field = (label: string, key: keyof typeof v, ph: string, onSave: () => void, note?: string) => (
    <div className="card"><h3>{label}</h3>{note && <p className="mute small" style={{ margin: "0 0 8px" }}>{note}</p>}
      <input value={v[key]} placeholder={ph} onChange={(e) => setV({ ...v, [key]: e.target.value })} />
      <button className="btn wide" style={{ marginTop: 10 }} disabled={!isAdmin || busy || !v[key]} onClick={onSave}>Save</button></div>);
  return (<>
    {!isAdmin && <Notice kind="warn">Read-only. Settings can only be changed by the admin account.</Notice>}
    {msg && <Notice kind={msg.k}>{msg.t}</Notice>}
    <div className="card" style={{ marginBottom: 14 }}><h3>Emergency pause</h3>
      <p className="mute small">Pausing blocks new launches and refunds any incoming trade in full. It never touches user balances, and withdrawals keep working.</p>
      <button className={`btn wide ${paused ? "" : "danger"}`} disabled={!isAdmin || busy || !cfg.data} onClick={() => call("set_paused", () => ({ paused: !paused }))}>{paused ? "Resume protocol" : "Pause protocol"}</button></div>
    <div className="grid g2">
      {field("Trading fee (basis points, max 300)", "fee", `now ${cfg.data?.trading_fee_bps ?? "…"}`, () => call("set_trading_fee_bps", () => ({ bps: Number(v.fee) })), "100 = 1%. The 30/50/20 split is fixed in the contract.")}
      {field("Launch fee (USD)", "launch", `now $${cfg.data ? Number(cfg.data.launch_fee_usd_micro) / 1e6 : "…"}`, () => call("set_launch_fee_usd_micro", () => ({ micro: parseUnits(v.launch, 6).toString() })), "Hard cap $50.")}
      {field("NEAR price (USD) — testnet only", "price", `now $${cfg.data ? Number(cfg.data.near_usd_price_micro) / 1e6 : "…"}`, () => call("set_near_usd_price", () => ({ micro_usd_per_near: parseUnits(v.price, 6).toString() })), "Manual rate. Launches are blocked if it is older than 7 days. Replace with an oracle before mainnet.")}
      {field("Min. initial liquidity (NEAR)", "minliq", `now ${cfg.data ? yoctoToNear(cfg.data.min_initial_liquidity, 3) : "…"}`, () => call("set_min_initial_liquidity", () => ({ amount: nearToYocto(v.minliq).toString() })))}
      {field("LP lock for new launches (days)", "lock", `now ${cfg.data ? Number(BigInt(cfg.data.lp_lock_ns) / 86_400_000_000_000n) : "…"}`, () => call("set_lp_lock_ns", () => ({ ns: (BigInt(Math.floor(Number(v.lock))) * 86_400_000_000_000n).toString() })), "Minimum 1 day. Existing locks keep their own unlock time.")}
      {field("Treasury account", "treasury", "account.near", () => call("set_treasury_account", () => ({ account: v.treasury.trim() })), "Where treasury withdrawals are allowed to go. Double-check this value.")}
    </div></>);
}
