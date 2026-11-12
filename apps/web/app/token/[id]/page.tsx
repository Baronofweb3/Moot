"use client";
import { use, useEffect, useState } from "react";
import { core, ft, BuyQuote, SellQuote } from "@/lib/near";
import { CORE, WNEAR, GAS, ONE_YOCTO, STORAGE_DEPOSIT, EXPLORER } from "@/lib/config";
import { Call, useWallet } from "@/lib/wallet";
import { applySlippage, nearToYocto, yoctoToNear, unitsToTokens, tokensToUnits, spotPriceYocto, nsToDate, compactTokens } from "@/lib/units.mjs";
import { usePoll, useDebounced, Notice, Row, Coin, Phase2 } from "@/components/ui";

const deadlineNs = () => (BigInt(Date.now() + 10 * 60_000) * 1_000_000n).toString();

export default function TokenPage({ params }: { params: Promise<{ id: string }> }) {
  const { id: rawId } = use(params);
  const id = decodeURIComponent(rawId);
  const { accountId, signIn, send } = useWallet();
  const tok = usePoll(() => core.token(id), [id], 8000);
  const cfg = usePoll(() => core.config(), []);
  const t = tok.data;

  const mine = usePoll(async () => {
    if (!accountId) return null;
    const [bal, wbal] = await Promise.all([ft.balance(id, accountId).catch(() => "0"), ft.wnearBalance(accountId).catch(() => "0")]);
    return { bal, wbal };
  }, [id, accountId], 10000);

  if (tok.loading) return <p className="mute" style={{ paddingTop: 40 }}>Loading from chain…</p>;
  if (tok.error) return <Notice kind="err">Could not load token: {tok.error}</Notice>;
  if (!t) return <Notice kind="warn">Token not found yet. If you just created it, wait a few seconds and refresh.</Notice>;

  const price = spotPriceYocto(t.wnear_reserve, t.token_reserve);
  const supply = BigInt(t.total_supply);
  const bal = BigInt(mine.data?.bal ?? "0");
  const pct = supply > 0n ? Number((bal * 1_000_000n) / supply) / 10_000 : 0;
  const isCreator = accountId === t.creator;

  return (
    <div style={{ paddingTop: 26 }}>
      <div className="tok" style={{ marginBottom: 16 }}>
        <Coin symbol={t.symbol} />
        <div style={{ flex: 1 }}>
          <h1 style={{ margin: 0, fontSize: 26 }}>{t.name} <span className="mute">${t.symbol}</span> <span className={`pill ${t.state}`}>{t.state}</span></h1>
          <div className="mute small">Created by {t.creator} · <a href={`${EXPLORER}/address/${t.token_id}`} target="_blank" rel="noreferrer" style={{ textDecoration: "underline" }}>{t.token_id}</a></div>
        </div>
      </div>
      {t.description && <p className="mute" style={{ marginTop: 0 }}>{t.description}</p>}

      <div className="grid g2">
        <div>
          <div className="card">
            <h3>Pool</h3>
            <Row k="Price" v={t.state === "live" ? `${yoctoToNear(price, 12)} Ⓝ` : "—"} />
            <Row k="wNEAR in pool" v={`${yoctoToNear(t.wnear_reserve, 4)} Ⓝ`} />
            <Row k="Tokens in pool" v={`${compactTokens(t.token_reserve)} of ${compactTokens(t.total_supply)}`} />
            <Row k="Volume" v={`${yoctoToNear(t.volume_wnear, 4)} Ⓝ`} />
            <Row k="Buys / sells" v={`${t.buys} / ${t.sells}`} />
            <Row k="Creator fees earned" v={`${yoctoToNear(t.creator_fees_earned, 5)} Ⓝ`} />
          </div>
          <div className="card" style={{ marginTop: 14 }}>
            <h3>Supply & liquidity lock</h3>
            <Row k="Total supply (fixed)" v={unitsToTokens(t.total_supply, 0)} />
            <Row k="Creator allocation" v="0" />
            {t.lp_unlock_at ? (<>
              <Row k="Liquidity owner" v={t.lp_owner} />
              <Row k="Initial liquidity" v={`${yoctoToNear(t.initial_wnear, 3)} Ⓝ`} />
              <Row k="Locked until" v={nsToDate(t.lp_unlock_at).toUTCString()} />
            </>) : <p className="mute small">Liquidity not added yet.</p>}
            <Row k={<>Top holders <Phase2 /></> as any} v="needs indexer" />
          </div>
          {accountId && (
            <div className="card" style={{ marginTop: 14 }}>
              <h3>You</h3>
              <Row k="Your balance" v={`${unitsToTokens(bal.toString(), 2)} ${t.symbol}`} />
              <Row k="Share of supply" v={`${pct.toFixed(4)}%`} />
              <Row k="Value at pool price" v={`${yoctoToNear((bal * price) / 10n ** 18n, 4)} Ⓝ`} />
              <Row k="Your wNEAR" v={`${yoctoToNear(mine.data?.wbal ?? "0", 4)} Ⓝ`} />
            </div>
          )}
        </div>

        <div>
          {t.state === "created" && (isCreator
            ? <SeedPanel tokenId={t.token_id} min={cfg.data?.min_initial_liquidity} onDone={() => tok.reload()} />
            : <Notice kind="warn">Waiting for the creator to add starting liquidity. Trading opens as soon as they do.</Notice>)}
          {t.state === "live" && <Trade t={t} bal={bal} bps={cfg.data?.trading_fee_bps ?? 100} onDone={() => { tok.reload(); mine.reload(); }} />}
          {!accountId && t.state === "live" && <div style={{ marginTop: 12 }}><button className="btn wide" onClick={signIn}>Connect wallet to trade</button></div>}
          <div className="card" style={{ marginTop: 14 }}>
            <h3>Trade history & PNL card <Phase2 /></h3>
            <p className="mute small" style={{ margin: 0 }}>Per-trade history and shareable PNL cards need an event indexer, which is not built yet. Every trade is still visible on chain via the explorer link above.</p>
          </div>
        </div>
      </div>
    </div>
  );
}

function SeedPanel({ tokenId, min, onDone }: { tokenId: string; min?: string; onDone: () => void }) {
  const { accountId, send, signIn } = useWallet();
  const [amt, setAmt] = useState("1");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  let yocto = 0n, bad: string | null = null;
  try { yocto = nearToYocto(amt); if (min && yocto < BigInt(min)) bad = `Minimum is ${yoctoToNear(min, 2)} Ⓝ`; } catch (e: any) { bad = e.message; }

  async function go() {
    if (!accountId) return;
    setBusy(true); setErr(null);
    try {
      const calls: Call[] = [];
      const [coreReg, meReg] = await Promise.all([ft.storage(WNEAR, CORE), ft.storage(WNEAR, accountId)]);
      if (!coreReg) calls.push({ receiverId: WNEAR, method: "storage_deposit", gas: GAS.WRAP, deposit: STORAGE_DEPOSIT, args: { account_id: CORE, registration_only: true } });
      if (!meReg) calls.push({ receiverId: WNEAR, method: "storage_deposit", gas: GAS.WRAP, deposit: STORAGE_DEPOSIT, args: { account_id: accountId, registration_only: true } });
      calls.push({ receiverId: WNEAR, method: "near_deposit", gas: GAS.WRAP, deposit: yocto.toString(), args: {} });
      calls.push({ receiverId: WNEAR, method: "ft_transfer_call", gas: GAS.TRANSFER_CALL, deposit: ONE_YOCTO,
        args: { receiver_id: CORE, amount: yocto.toString(), msg: JSON.stringify({ action: "seed", token: tokenId }) } });
      await send(calls, window.location.href);
      onDone();
    } catch (e: any) { setErr(e?.message || String(e)); } finally { setBusy(false); }
  }
  return (
    <div className="card">
      <h3>Step 2 · Add starting liquidity</h3>
      <p className="mute small">Your NEAR is wrapped to wNEAR and paired with the whole token supply. The pool opens for trading immediately and your liquidity is locked for 1 year. You keep no tokens.</p>
      <label>Starting liquidity (NEAR)</label>
      <input value={amt} inputMode="decimal" onChange={(e) => setAmt(e.target.value)} />
      {bad && <Notice kind="warn">{bad}</Notice>}
      {err && <Notice kind="err">{err}</Notice>}
      <div style={{ marginTop: 12 }}>{accountId ? <button className="btn wide" disabled={busy || !!bad} onClick={go}>{busy ? "Waiting for wallet…" : "Add liquidity & open trading"}</button> : <button className="btn wide" onClick={signIn}>Connect wallet</button>}</div>
    </div>
  );
}

function Trade({ t, bal, bps, onDone }: { t: any; bal: bigint; bps: number; onDone: () => void }) {
  const { accountId, send } = useWallet();
  const [side, setSide] = useState<"buy" | "sell">("buy");
  const [amt, setAmt] = useState("");
  const [slip, setSlip] = useState(100);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);
  const d = useDebounced(amt);

  let units = 0n, parseErr: string | null = null;
  try { if (d.trim()) units = side === "buy" ? nearToYocto(d) : tokensToUnits(d); } catch (e: any) { parseErr = e.message; }

  const quote = usePoll<BuyQuote | SellQuote | null>(async () => {
    if (units <= 0n) return null;
    return side === "buy" ? core.quoteBuy(t.token_id, units.toString()) : core.quoteSell(t.token_id, units.toString());
  }, [side, units.toString(), t.wnear_reserve, t.token_reserve]);
  const q = quote.data as any;
  const minOut = q ? applySlippage(side === "buy" ? q.tokens_out : q.net_out, slip) : 0n;
  const impact = q ? Number(q.price_impact_bps) / 100 : 0;

  async function go() {
    if (!accountId || !q) return;
    setBusy(true); setErr(null); setOk(null);
    try {
      const calls: Call[] = [];
      if (side === "buy") {
        const [tReg, wReg] = await Promise.all([ft.storage(t.token_id, accountId), ft.storage(WNEAR, accountId)]);
        if (!tReg) calls.push({ receiverId: t.token_id, method: "storage_deposit", gas: GAS.WRAP, deposit: STORAGE_DEPOSIT, args: { account_id: accountId, registration_only: true } });
        if (!wReg) calls.push({ receiverId: WNEAR, method: "storage_deposit", gas: GAS.WRAP, deposit: STORAGE_DEPOSIT, args: { account_id: accountId, registration_only: true } });
        calls.push({ receiverId: WNEAR, method: "near_deposit", gas: GAS.WRAP, deposit: units.toString(), args: {} });
        calls.push({ receiverId: WNEAR, method: "ft_transfer_call", gas: GAS.TRANSFER_CALL, deposit: ONE_YOCTO,
          args: { receiver_id: CORE, amount: units.toString(), msg: JSON.stringify({ action: "buy", token: t.token_id, min_out: minOut.toString(), deadline_ns: deadlineNs() }) } });
      } else {
        const wReg = await ft.storage(WNEAR, accountId);
        if (!wReg) calls.push({ receiverId: WNEAR, method: "storage_deposit", gas: GAS.WRAP, deposit: STORAGE_DEPOSIT, args: { account_id: accountId, registration_only: true } });
        calls.push({ receiverId: t.token_id, method: "ft_transfer_call", gas: GAS.TRANSFER_CALL, deposit: ONE_YOCTO,
          args: { receiver_id: CORE, amount: units.toString(), msg: JSON.stringify({ action: "sell", min_out: minOut.toString(), deadline_ns: deadlineNs() }) } });
      }
      await send(calls, window.location.href);
      setOk("Submitted. The pool updates in a few seconds."); setAmt(""); onDone();
    } catch (e: any) { setErr(e?.message || String(e)); } finally { setBusy(false); }
  }

  const tooBig = side === "sell" && units > bal;
  return (
    <div className="card">
      <div className="tabs">
        <button className={`b ${side === "buy" ? "on" : ""}`} onClick={() => { setSide("buy"); setAmt(""); }}>Buy</button>
        <button className={`s ${side === "sell" ? "on" : ""}`} onClick={() => { setSide("sell"); setAmt(""); }}>Sell</button>
      </div>
      <label>{side === "buy" ? "You pay (NEAR)" : `You sell (${t.symbol})`}</label>
      <input value={amt} inputMode="decimal" placeholder="0.0" onChange={(e) => setAmt(e.target.value)} />
      {side === "sell" && (
        <div style={{ display: "flex", gap: 6, marginTop: 8 }}>
          {[25, 50, 100].map((p) => <button key={p} className="btn ghost" style={{ flex: 1, padding: 8 }} onClick={() => setAmt(unitsToTokens(((bal * BigInt(p)) / 100n).toString(), 6).replace(/,/g, ""))}>{p === 100 ? "Max" : p + "%"}</button>)}
        </div>
      )}
      {parseErr && <Notice kind="warn">{parseErr}</Notice>}
      {tooBig && <Notice kind="warn">You only hold {unitsToTokens(bal.toString(), 2)} {t.symbol}.</Notice>}
      {q && (
        <div style={{ marginTop: 12 }}>
          <Row k={side === "buy" ? "You receive (est.)" : "You receive (est.)"} v={side === "buy" ? `${unitsToTokens(q.tokens_out, 2)} ${t.symbol}` : `${yoctoToNear(q.net_out, 5)} wNEAR`} />
          <Row k={`Minimum after ${(slip / 100).toFixed(1)}% slippage`} v={side === "buy" ? `${unitsToTokens(minOut.toString(), 2)} ${t.symbol}` : `${yoctoToNear(minOut.toString(), 5)} wNEAR`} />
          <Row k="Price impact" v={`${impact.toFixed(2)}%`} />
          <Row k={`Fee (${(bps / 100).toFixed(2)}%)`} v={`${yoctoToNear(q.fee, 6)} Ⓝ`} />
          <div className="split">
            <div><b>{yoctoToNear(q.fee_to_creator, 6)}</b><span className="mute small">Creator</span></div>
            <div><b>{yoctoToNear(q.fee_to_trader, 6)}</b><span className="mute small">You (MOOT TIP)</span></div>
            <div><b>{yoctoToNear(q.fee_to_treasury, 6)}</b><span className="mute small">Treasury</span></div>
          </div>
          {impact > 5 && <Notice kind="warn">High price impact. This trade moves the price a lot.</Notice>}
        </div>
      )}
      {quote.error && <Notice kind="err">Quote failed: {quote.error}</Notice>}
      {units > 0n && !q && !quote.loading && !quote.error && <Notice kind="warn">Amount too small or pool can't fill it.</Notice>}
      <div style={{ display: "flex", gap: 6, marginTop: 12, alignItems: "center" }}>
        <span className="mute small">Slippage</span>
        {[50, 100, 300].map((s) => <button key={s} className={`btn ghost`} style={{ padding: "6px 10px", opacity: slip === s ? 1 : 0.55 }} onClick={() => setSlip(s)}>{s / 100}%</button>)}
      </div>
      {err && <Notice kind="err">{err}</Notice>}
      {ok && <Notice kind="ok">{ok}</Notice>}
      <div style={{ marginTop: 12 }}>
        <button className={`btn wide ${side}`} disabled={!accountId || !q || busy || tooBig} onClick={go}>{busy ? "Waiting for wallet…" : side === "buy" ? `Buy ${t.symbol}` : `Sell ${t.symbol}`}</button>
      </div>
      <p className="mute small" style={{ marginBottom: 0 }}>First trade also registers your account on the token and wNEAR (a one-time ~0.001 Ⓝ refundable storage cost).</p>
    </div>
  );
}
