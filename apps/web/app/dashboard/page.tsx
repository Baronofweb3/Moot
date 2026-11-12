"use client";
import Link from "next/link";
import { useState } from "react";
import { core, ft, nativeBalance } from "@/lib/near";
import { CORE, WNEAR, GAS, ONE_YOCTO, STORAGE_DEPOSIT } from "@/lib/config";
import { Call, useWallet } from "@/lib/wallet";
import { yoctoToNear, nearToYocto, unitsToTokens, spotPriceYocto } from "@/lib/units.mjs";
import { usePoll, Notice, Stat, Coin, Phase2 } from "@/components/ui";

export default function Dashboard() {
  const { accountId, signIn, send } = useWallet();
  const [msg, setMsg] = useState<{ k: "ok" | "err"; t: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [tipTo, setTipTo] = useState("");
  const [tipAmt, setTipAmt] = useState("");
  const [unwrapAmt, setUnwrapAmt] = useState("");

  const me = usePoll(async () => {
    if (!accountId) return null;
    const [creator, tip, wnear, native, tokens] = await Promise.all([
      core.creatorBalance(accountId), core.tipBalance(accountId), ft.wnearBalance(accountId).catch(() => "0"),
      nativeBalance(accountId).catch(() => 0n), core.list(0, 50),
    ]);
    const holdings = await Promise.all(tokens.map(async (k) => ({ k, bal: await ft.balance(k.token_id, accountId).catch(() => "0") })));
    return { creator, tip, wnear, native, tokens, holdings: holdings.filter((h) => BigInt(h.bal) > 0n) };
  }, [accountId], 15000);

  async function run(label: string, build: () => Promise<Call[]>) {
    setBusy(true); setMsg(null);
    try { await send(await build(), window.location.href); setMsg({ k: "ok", t: `${label} submitted. Balances refresh in a few seconds.` }); setTimeout(me.reload, 4000); }
    catch (e: any) { setMsg({ k: "err", t: e?.message || String(e) }); }
    finally { setBusy(false); }
  }
  const needWrapReg = async (): Promise<Call[]> => (await ft.storage(WNEAR, accountId!))
    ? [] : [{ receiverId: WNEAR, method: "storage_deposit", gas: GAS.WRAP, deposit: STORAGE_DEPOSIT, args: { account_id: accountId, registration_only: true } }];

  if (!accountId) return <div style={{ paddingTop: 50, textAlign: "center" }}><h1>Your dashboard</h1><p className="mute">Connect a wallet to see your earnings, holdings and launches.</p><button className="btn" onClick={signIn}>Connect wallet</button></div>;
  const d = me.data;
  const mine = d?.tokens.filter((k) => k.creator === accountId) ?? [];

  return (
    <div style={{ paddingTop: 26 }}>
      <h1 style={{ margin: "0 0 4px" }}>Dashboard</h1>
      <p className="mute" style={{ marginTop: 0 }}>{accountId}</p>
      {me.error && <Notice kind="err">Could not load: {me.error}</Notice>}
      {msg && <Notice kind={msg.k}>{msg.t}</Notice>}

      <div className="grid g3">
        <Stat label="MOOT TIP (trader rewards)" value={d ? `${yoctoToNear(d.tip, 6)} Ⓝ` : "…"} sub="20% of fees on your trades" />
        <Stat label="Creator fees" value={d ? `${yoctoToNear(d.creator, 6)} Ⓝ` : "…"} sub="50% of fees on your tokens" />
        <Stat label="wNEAR in wallet" value={d ? `${yoctoToNear(d.wnear, 5)} Ⓝ` : "…"} sub={d ? `${yoctoToNear(d.native.toString(), 3)} Ⓝ native` : ""} />
      </div>

      <section>
        <h2>Withdraw earnings</h2>
        <div className="grid g2">
          <div className="card"><h3>MOOT TIP</h3><p className="mute small">Paid out as wNEAR.</p>
            <button className="btn wide" disabled={busy || !d || BigInt(d.tip) === 0n}
              onClick={() => run("Tip withdrawal", async () => [...(await needWrapReg()), { receiverId: CORE, method: "withdraw_tips", gas: GAS.SIMPLE, deposit: ONE_YOCTO, args: { amount: null } }])}>Withdraw all tips</button></div>
          <div className="card"><h3>Creator fees</h3><p className="mute small">Paid out as wNEAR.</p>
            <button className="btn wide" disabled={busy || !d || BigInt(d.creator) === 0n}
              onClick={() => run("Creator withdrawal", async () => [...(await needWrapReg()), { receiverId: CORE, method: "withdraw_creator_fees", gas: GAS.SIMPLE, deposit: ONE_YOCTO, args: { amount: null } }])}>Withdraw all creator fees</button></div>
          <div className="card"><h3>Unwrap wNEAR → NEAR</h3>
            <input value={unwrapAmt} inputMode="decimal" placeholder="amount (blank = all)" onChange={(e) => setUnwrapAmt(e.target.value)} />
            <button className="btn wide" style={{ marginTop: 10 }} disabled={busy || !d || BigInt(d.wnear) === 0n}
              onClick={() => run("Unwrap", async () => { const amt = unwrapAmt.trim() ? nearToYocto(unwrapAmt) : BigInt(d!.wnear); return [{ receiverId: WNEAR, method: "near_withdraw", gas: GAS.WRAP, deposit: ONE_YOCTO, args: { amount: amt.toString() } }]; })}>Unwrap</button></div>
          <div className="card"><h3>Tip a user</h3><p className="mute small">Send part of your creator fees to another account's MOOT TIP balance.</p>
            <input value={tipTo} placeholder="account.testnet" onChange={(e) => setTipTo(e.target.value.trim())} />
            <input style={{ marginTop: 8 }} value={tipAmt} inputMode="decimal" placeholder="amount in NEAR" onChange={(e) => setTipAmt(e.target.value)} />
            <button className="btn wide" style={{ marginTop: 10 }} disabled={busy || !tipTo || !tipAmt}
              onClick={() => run("Tip", async () => [{ receiverId: CORE, method: "tip", gas: GAS.SIMPLE, deposit: ONE_YOCTO, args: { to: tipTo, amount: nearToYocto(tipAmt).toString() } }])}>Send tip</button></div>
        </div>
        <p className="mute small">Withdrawals and tips need a full-access wallet key and attach 1 yoctoNEAR, which the wallet will show you.</p>
      </section>

      <section>
        <h2>Tokens you launched</h2>
        {!mine.length ? <p className="mute">None yet. <Link href="/launch" style={{ textDecoration: "underline" }}>Launch one</Link>.</p> : (
          <div className="card"><table><thead><tr><th>Token</th><th>State</th><th>Volume</th><th>Fees earned</th></tr></thead><tbody>
            {mine.map((k) => <tr key={k.token_id}><td><Link href={`/token/${k.token_id}`}>{k.name} <span className="mute">${k.symbol}</span></Link></td><td><span className={`pill ${k.state}`}>{k.state}</span></td><td>{yoctoToNear(k.volume_wnear, 3)} Ⓝ</td><td>{yoctoToNear(k.creator_fees_earned, 6)} Ⓝ</td></tr>)}
          </tbody></table></div>)}
      </section>

      <section>
        <h2>Your holdings</h2>
        {!d?.holdings.length ? <p className="mute">No MOOT tokens held.</p> : (
          <div className="card"><table><thead><tr><th>Token</th><th>Balance</th><th>Value at pool price</th></tr></thead><tbody>
            {d.holdings.map(({ k, bal }) => <tr key={k.token_id}><td><Link href={`/token/${k.token_id}`}><span className="tok"><Coin symbol={k.symbol} />{k.name}</span></Link></td><td>{unitsToTokens(bal, 2)}</td><td>{yoctoToNear(((BigInt(bal) * spotPriceYocto(k.wnear_reserve, k.token_reserve)) / 10n ** 18n).toString(), 4)} Ⓝ</td></tr>)}
          </tbody></table></div>)}
      </section>

      <section>
        <h2>PNL cards & trade history <Phase2 /></h2>
        <p className="mute">Shareable PNL cards need the full trade history for your wallet, which requires an event indexer. Not built yet.</p>
      </section>
    </div>
  );
}
