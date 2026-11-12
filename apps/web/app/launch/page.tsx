"use client";
import { useState } from "react";
import { core } from "@/lib/near";
import { CORE, GAS } from "@/lib/config";
import { useWallet } from "@/lib/wallet";
import { yoctoToNear, nearToYocto } from "@/lib/units.mjs";
import { usePoll, Notice, Row } from "@/components/ui";

export default function Launch() {
  const { accountId, signIn, send } = useWallet();
  const quote = usePoll(() => core.launchQuote(), [], 20000);
  const cfg = usePoll(() => core.config(), []);
  const [name, setName] = useState("");
  const [symbol, setSymbol] = useState("");
  const [desc, setDesc] = useState("");
  const [supply, setSupply] = useState("1000000000");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const symOk = /^[A-Z0-9]{2,10}$/.test(symbol);
  const supplyOk = /^\d+$/.test(supply) && BigInt(supply || "0") >= 1n && BigInt(supply || "0") <= 1_000_000_000_000n;
  const nameOk = name.trim().length > 0 && name.length <= 40;
  const valid = symOk && supplyOk && nameOk && desc.length <= 280;
  const q = quote.data;

  async function submit() {
    if (!q) return;
    setBusy(true); setErr(null);
    try {
      const deposit = (BigInt(q.required_deposit_yocto) + nearToYocto("0.05")).toString(); // small buffer, unused part is refunded
      const tokenId = `${symbol.toLowerCase()}.${CORE}`;
      await send(
        [{ receiverId: CORE, method: "create_token", gas: GAS.CREATE, deposit,
           args: { name: name.trim(), symbol, icon: null, description: desc.trim(), total_supply_whole: supply } }],
        `${window.location.origin}/token/${tokenId}`
      );
      window.location.href = `/token/${tokenId}`;
    } catch (e: any) { setErr(e?.message || String(e)); setBusy(false); }
  }

  return (
    <div style={{ maxWidth: 560, margin: "0 auto", paddingTop: 30 }}>
      <h1 style={{ margin: "0 0 6px" }}>Launch a token</h1>
      <p className="mute" style={{ marginTop: 0 }}>Two steps: create the token (this page), then add your starting liquidity on the token page. Your token only becomes tradable after step two.</p>
      {cfg.data?.paused && <Notice kind="err">Launches are paused by the protocol admin.</Notice>}
      <div className="card">
        <label>Token name</label><input value={name} maxLength={40} onChange={(e) => setName(e.target.value)} placeholder="Moot Dog" />
        <label>Symbol (2–10 letters or numbers, capitals)</label>
        <input value={symbol} maxLength={10} onChange={(e) => setSymbol(e.target.value.toUpperCase().replace(/[^A-Z0-9]/g, ""))} placeholder="MDOG" />
        <label>Description (optional)</label><textarea rows={3} maxLength={280} value={desc} onChange={(e) => setDesc(e.target.value)} />
        <label>Total supply (whole tokens, fixed forever)</label>
        <input value={supply} inputMode="numeric" onChange={(e) => setSupply(e.target.value.replace(/\D/g, ""))} />
        <Notice>The entire supply goes into the liquidity pool. <b>You receive zero tokens.</b> No mint function exists after launch.</Notice>
      </div>

      <div className="card" style={{ marginTop: 14 }}>
        <h3>What you pay now</h3>
        {q ? (<>
          <Row k={`Launch fee ($${cfg.data ? Number(cfg.data.launch_fee_usd_micro) / 1e6 : "3"} in NEAR)`} v={`${yoctoToNear(q.launch_fee_yocto, 3)} Ⓝ`} />
          <Row k="Token contract storage (on-chain rent)" v={`${yoctoToNear(q.deploy_cost_yocto, 3)} Ⓝ`} />
          <Row k="Record reserve" v={`${yoctoToNear(q.record_reserve_yocto, 3)} Ⓝ`} />
          <Row k="Total (any unused part is refunded)" v={`${yoctoToNear(q.required_deposit_yocto, 3)} Ⓝ`} />
          {!q.price_is_fresh && <Notice kind="err">The NEAR/USD rate is stale. Launches are blocked until the admin refreshes it.</Notice>}
        </>) : <p className="mute">{quote.error ? `Could not load quote: ${quote.error}` : "Loading quote…"}</p>}
        <p className="mute small">Step two also needs your starting liquidity in wNEAR (minimum {cfg.data ? yoctoToNear(cfg.data.min_initial_liquidity, 2) : "…"} Ⓝ). That liquidity is locked for 1 year.</p>
      </div>

      {err && <Notice kind="err">{err}</Notice>}
      <div style={{ marginTop: 14 }}>
        {accountId ? (
          <button className="btn wide" disabled={!valid || busy || !q?.price_is_fresh || cfg.data?.paused} onClick={submit}>{busy ? "Waiting for wallet…" : "Create token"}</button>
        ) : <button className="btn wide" onClick={signIn}>Connect wallet to launch</button>}
      </div>
    </div>
  );
}
