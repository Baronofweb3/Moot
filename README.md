<p align="center"><img src="apps/web/public/logo.png" width="96" alt="MOOT"></p>

# MOOT

**Trade together. Earn together.**

MOOT is a memecoin launchpad on NEAR where the fee pays the people who make the market, not just the platform. A creator launches a token with a fixed supply and keeps none of it. Every trade pays a 1% fee, split **50% to the creator, 20% back to the trader (the "MOOT TIP"), and 30% to the treasury**. The split is enforced by a smart contract and recorded in on-chain ledgers.

> Status: a working **NEAR testnet prototype** built for a hackathon. Unaudited. No real funds. See [what is and isn't verified](#what-is-and-isnt-verified).

## Quick links

| | |
|---|---|
| Core contract (testnet) | [`moot-core-01.testnet`](https://testnet.nearblocks.io/address/moot-core-01.testnet) |
| Test token | [`mtest.moot-core-01.testnet`](https://testnet.nearblocks.io/address/mtest.moot-core-01.testnet) |
| Public app | `apps/web` (Next.js) |
| Control Room (separate app) | `apps/control-room` (Next.js) |
| Architecture | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| Security notes | [docs/SECURITY.md](docs/SECURITY.md) |
| Deploy guide | [docs/DEPLOY.md](docs/DEPLOY.md) |
| Demo script | [docs/DEMO.md](docs/DEMO.md) |

## How it works

1. **Launch.** The creator pays about $3 (in NEAR) plus the on-chain storage for the new token. The core contract deploys a fresh fixed-supply NEP-141 token and mints the whole supply once, to itself. The creator receives zero tokens.
2. **Open the pool.** The creator adds starting liquidity in wNEAR. The pool opens for trading at once and the creator's liquidity is locked for one year.
3. **Trade.** A constant-product pool prices every trade. The 1% fee is split 50/20/30 and credited only after the trade settles. If a transfer fails, reserves revert and the trader is refunded.
4. **Earn.** Creator fees and MOOT TIPs are on-chain balances. Anyone can withdraw theirs, and creators can tip part of their earnings to other users.

### What the contract enforces

| Rule | Where |
|---|---|
| Supply is fixed at launch, no mint function exists | `moot-ft` |
| Creator allocation is zero, the whole supply backs the pool | `moot-core` |
| Fee split 30/50/20, rounding remainder to treasury | `math.rs` |
| Fee credited only after a settled swap, reverted on failure | `moot-core` |
| Slippage floor and deadline on every trade | `moot-core` |
| Withdrawals and tips need 1 yoctoNEAR (full-access key) | `moot-core` |
| Admin can pause and tune fees within hard caps (max 3%), nothing else | `moot-core` |
| On-chain reconciliation counters (`get_accounting`) | `moot-core` |

## What is and isn't verified

**Run live on NEAR testnet (real transactions):** core deployed and initialised, a token launched through the factory, pool seeded, a buy settled, the 30/50/20 fee split credited exactly, accounting reconciles.

**Covered by unit tests only (35 tests, `cargo test --workspace`), not yet run live:** sells, creator and trader withdrawals, treasury withdrawals, tipping, failed-transfer reverts, pause behaviour, admin caps.

**Apps:** both build, type-check and render. Wallet flows (connect, launch, seed, buy, sell, withdraw, tip) are written against the NEAR wallet selector but have **not** been exercised end to end in a browser yet.

**Not built:** event indexer and Postgres, trade history, top-holder lists, PNL cards, withdrawing locked liquidity after the lock ends, Privy login, boosts, MCP tools, multisig, price oracle (the NEAR/USD rate is admin-set on testnet), team roles in the Control Room.

## Repository

```
contracts/moot-ft      Fixed-supply NEP-141 token (Rust)
contracts/moot-core    Factory + AMM + fee ledgers (Rust, near-sdk 5.30)
apps/web               Public MOOT app
apps/control-room      Separate admin app (wallet-gated)
docs/                  Architecture, security, deploy, demo
```

### Build and test

```bash
# Contracts (CI builds wasm: .github/workflows/contracts.yml)
cargo test --workspace

# Apps
cd apps/web && npm install && npm test && npm run dev
cd apps/control-room && npm install && npm run dev
```

The token wasm is embedded in the core contract, so CI builds `moot-ft` first, copies it to `contracts/moot-core/res/moot_ft.wasm`, then builds `moot-core`.
