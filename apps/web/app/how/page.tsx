export default function How() {
  return (
    <div style={{ maxWidth: 720, margin: "0 auto", paddingTop: 30, lineHeight: 1.6 }}>
      <h1>How MOOT works</h1>
      <p className="mute">A testnet prototype. Everything below is enforced by a smart contract you can read on chain.</p>
      <h3>Launching</h3>
      <p>A creator pays a small launch fee (about $3 in NEAR) plus the on-chain storage for the new token. The token is created with a fixed supply that can never grow. The creator gets <b>none</b> of it. The whole supply goes into a pool against wNEAR when the creator adds starting liquidity, and that liquidity is locked for one year.</p>
      <h3>Trading</h3>
      <p>A constant-product pool prices every trade. Each trade pays a 1% fee, split <b>50% to the creator, 20% back to the trader as a MOOT TIP, and 30% to the treasury</b>. Fees are only credited after the trade has actually settled. If a transfer fails, the pool reverts and you are refunded.</p>
      <h3>Withdrawals</h3>
      <p>Creator fees and MOOT TIPs are on-chain balances. You withdraw them to your wallet whenever you like. Creators can also send part of their earnings to another user as a tip.</p>
      <h3>What the team cannot do</h3>
      <p>The admin can pause the protocol and adjust fees within hard caps (never above 3%). The admin cannot mint tokens, edit anyone's balance, or take pool liquidity.</p>
      <h3>Honest limits</h3>
      <p className="mute">Testnet only. The NEAR/USD rate is set by the admin, not an oracle. Trade history, top holders, PNL cards and boosts need an indexer and are not built yet. Withdrawing the locked liquidity after the lock ends is not built yet. This is unaudited software.</p>
    </div>
  );
}
