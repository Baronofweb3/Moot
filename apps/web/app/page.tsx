"use client";
import Link from "next/link";
import { core } from "@/lib/near";
import { yoctoToNear, compactTokens, spotPriceYocto } from "@/lib/units.mjs";
import { usePoll, Stat, Coin, Notice } from "@/components/ui";

export default function Home() {
  const toks = usePoll(() => core.list(0, 50), [], 15000);
  const acc = usePoll(() => core.accounting(), [], 15000);
  const cfg = usePoll(() => core.config(), []);
  const list = (toks.data ?? []).filter((t) => t.state === "live");
  const t = acc.data?.totals;
  return (
    <>
      <div className="hero">
        <h1>Trade together.<br /><span>Earn together.</span></h1>
        <p>Launch a memecoin on NEAR in one step. No creator allocation, the whole supply goes into the pool, and every trade pays the creator and the trader, not just the platform.</p>
        <Link href="/launch" className="btn">Launch a token</Link>
      </div>

      <div className="grid g3">
        <Stat label="Tokens launched" value={t ? t.launches : "…"} />
        <Stat label="Trades" value={t ? t.trades : "…"} />
        <Stat label="Volume (wNEAR)" value={t ? yoctoToNear(t.volume_wnear, 3) : "…"} />
      </div>

      <section>
        <h2>Where every 1% trading fee goes</h2>
        <div className="card">
          <div className="split"><div><b>50%</b><span className="mute small">Creator</span></div><div><b>20%</b><span className="mute small">The trader (MOOT TIP)</span></div><div><b>30%</b><span className="mute small">Treasury</span></div></div>
          <p className="mute small" style={{ marginBottom: 0 }}>Enforced by the contract on every settled trade, recorded in on-chain ledgers you can read yourself. {cfg.data && `Current fee: ${(cfg.data.trading_fee_bps / 100).toFixed(2)}%.`}</p>
        </div>
      </section>

      <section>
        <h2>Live tokens</h2>
        {toks.error && <Notice kind="err">Could not reach the NEAR RPC: {toks.error}</Notice>}
        {toks.loading && <p className="mute">Loading from chain…</p>}
        {!toks.loading && !list.length && !toks.error && <Notice>No live tokens yet. Be the first to launch one.</Notice>}
        <div className="grid g2">
          {list.map((k) => (
            <Link key={k.token_id} href={`/token/${k.token_id}`} className="card">
              <div className="tok"><Coin symbol={k.symbol} />
                <div style={{ flex: 1 }}><h3>{k.name} <span className="mute">${k.symbol}</span></h3><div className="mute small">by {k.creator}</div></div>
                <span className="pill live">live</span></div>
              <div className="grid" style={{ gridTemplateColumns: "1fr 1fr 1fr", marginTop: 12 }}>
                <div><div className="mute small">Price</div><b>{yoctoToNear(spotPriceYocto(k.wnear_reserve, k.token_reserve), 12)} Ⓝ</b></div>
                <div><div className="mute small">Pool</div><b>{yoctoToNear(k.wnear_reserve, 2)} Ⓝ</b></div>
                <div><div className="mute small">Volume</div><b>{yoctoToNear(k.volume_wnear, 2)} Ⓝ</b></div>
              </div>
              <div className="mute small" style={{ marginTop: 10 }}>{compactTokens(k.token_reserve)} in pool · {k.buys} buys · {k.sells} sells</div>
            </Link>
          ))}
        </div>
      </section>
    </>
  );
}
