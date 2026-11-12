<p align="center"><img src="apps/web/public/logo.png" width="88" alt="MOOT logo"></p>

<h1 align="center">MOOT</h1>
<p align="center"><b>Trade together. Earn together.</b></p>
<p align="center">A memecoin launchpad on NEAR where the trading fee pays the people who make the market, not only the platform.</p>

<p align="center">
  <a href="https://moot-delta.vercel.app"><b>Open the app</b></a> ·
  <a href="https://moot-control-room.vercel.app"><b>Control Room</b></a> ·
  <a href="https://testnet.nearblocks.io/address/moot-core-01.testnet"><b>Contract on-chain</b></a>
</p>

> **Status:** live on NEAR testnet. Unaudited. No real funds. Everything marked "built" below has been run on-chain. Everything marked "not built" is listed honestly at the bottom.

---

## The idea in 30 seconds

On most launchpads, the platform takes the fee and everyone else takes the risk.

MOOT splits the same 1% trading fee three ways, and a smart contract enforces it:

| Who | Share | Why |
|---|---|---|
| The token's creator | **50%** | Rewards the person who built the community around the coin |
| The trader | **20%** (the "MOOT TIP") | Gives every trader a reason to stay, not just flip |
| The treasury | **30%** | Funds the protocol |

Nobody has to trust us to pay out. The split lives in the contract, and every payout is recorded in public on-chain ledgers.

## What makes it different

- **Creators get no free allocation.** A creator launches a token with a fixed supply and receives none of it. The whole supply goes into the trading pool. That is designed to remove the most common rug-pull pattern.
- **Supply can never change.** The token contract has no mint function. The supply is set once at launch.
- **Liquidity is locked.** The creator's starting liquidity is locked for one year.
- **The fee is shared, not extracted.** Creators and traders earn from every trade. That is designed to keep both groups invested in a coin's success.
- **The admin has limits.** The admin can pause trading and tune fees within hard caps (fee at most 3%, launch fee at most $50). The admin has no way to mint tokens, freeze a token, or take anyone's funds.

## See it working (real testnet numbers)

These come from live trades on the deployed contract, shown in the Control Room:

| Check | Result |
|---|---|
| Solvency check (wNEAR held vs. wNEAR owed) | **Reconciles**: 1.641818 held, 1.641818 owed |
| Cumulative fee split over real trades | **50.0% creator / 19.9% trader / 30.0% treasury** |
| Flows run live | Token launch, buy, sell, tip withdrawal, creator fee withdrawal, creator-to-user tip |

Anyone can check these numbers directly against the contract. The Control Room reads every figure from the chain.

## How it works

1. **Launch.** A creator pays a small launch fee (about $3, paid in NEAR). The contract deploys a new fixed-supply token and puts the full supply into a pool.
2. **Trade.** Anyone can buy or sell against the pool. Prices come from a constant-product formula, the same model used by major decentralised exchanges. Each trade has a minimum-output floor and a deadline, so traders are protected from bad fills.
3. **Earn.** Each trade pays a 1% fee, split 50 / 20 / 30. Fees are credited only after the trade fully settles. If anything fails, the trade is reversed and the trader is refunded in full.
4. **Withdraw.** Creators and traders withdraw their earnings from the Dashboard. Creators can also tip a share of their earnings to any other account.

## Two apps, kept separate on purpose

| App | Who it's for | What it does |
|---|---|---|
| **MOOT** ([app](https://moot-delta.vercel.app)) | Everyone | Discover tokens, trade, launch, earn, withdraw, tip |
| **Control Room** ([app](https://moot-control-room.vercel.app)) | The protocol admin only | Solvency check, fee analytics, token overview, treasury withdrawals, protocol settings |

The Control Room only opens for the admin or treasury wallet. The contract itself rejects any admin call from other accounts, so the screen is a convenience and the contract is the real lock.

## Built and not built

**Built and run on testnet**
- Fixed-supply token factory
- Constant-product trading pool with slippage and deadline protection
- 50 / 20 / 30 fee split with on-chain ledgers and a reconciliation check
- Buy, sell, tip withdrawal, creator withdrawal, creator-to-user tipping
- Public app and a separate admin Control Room, both live
- 35 automated contract tests (`cargo test --workspace`)

**Built, not yet run live**
- The Launch form in the browser (the contract path was run; the form itself has not been exercised end to end)
- Treasury withdrawals from the Control Room (covered by tests only)

**Not built yet**
- Trade history, top-holder lists and shareable PNL cards (these need an event indexer)
- Releasing locked liquidity after the one-year lock ends (the lock is recorded; the release path is not built, so locked liquidity stays locked in this version)
- Price oracle (the NEAR/USD rate used for the launch fee is admin-set on testnet)
- Multisig admin, boosts, and MCP tooling
- Independent security audit

## Where it could go

If the prototype earns attention, the intended next steps are an indexer for history and holder stats, shareable PNL cards, a price oracle, a multisig admin, an audit, and a mainnet launch. None of that is promised or dated here. It is the order we would build in.

## Security

This is unaudited testnet software. It was built with these safeguards in mind:

- Fees are credited only after a swap settles, and are reversed on failure
- Every trade carries a minimum output and a deadline
- Withdrawals and tips need a full-access wallet key
- Callbacks are private, and malformed trade messages are refunded instead of failing silently
- Admin powers are capped and cannot touch user funds

More detail: [docs/SECURITY.md](docs/SECURITY.md)

## Under the hood

```
contracts/moot-ft      Fixed-supply token (Rust, NEP-141)
contracts/moot-core    Factory + trading pool + fee ledgers (Rust, near-sdk 5)
apps/web               Public app (Next.js)
apps/control-room      Admin app (Next.js)
docs/                  Architecture, security, deployment, demo script
```

- Core contract: [`moot-core-01.testnet`](https://testnet.nearblocks.io/address/moot-core-01.testnet)
- Example token: [`mtest.moot-core-01.testnet`](https://testnet.nearblocks.io/address/mtest.moot-core-01.testnet)
- Docs: [Architecture](docs/ARCHITECTURE.md) · [Security](docs/SECURITY.md) · [Deploy](docs/DEPLOY.md) · [Demo script](docs/DEMO.md)

### Run it yourself

```bash
# contracts
cargo test --workspace

# public app
cd apps/web && npm install && npm run dev

# control room
cd apps/control-room && npm install && npm run dev
```

You will need a NEAR testnet wallet. The apps hold no private keys; all signing happens in the user's wallet.

## Built by

Baron ([@Baronoffweb3](https://x.com/Baronoffweb3)), Web3 growth strategist and builder.

*MOOT is a testnet prototype. Nothing here is financial advice or an offer of any kind.*
