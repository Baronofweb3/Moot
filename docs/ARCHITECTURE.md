# Architecture

## Components

- **moot-ft**: a NEP-141 token with a fixed supply. `new` mints the entire supply once to a single holder, the core contract. `storage_unregister` cannot burn, and `ft_resolve_transfer` rejects burns, so the supply can never shrink or grow.
- **moot-core**: the factory, the AMM and the fee ledgers in one contract. `create_token` creates a sub-account `{symbol}.{core}`, deploys the embedded token wasm and initialises it in one batch. The result is checked in `on_token_created`.
- **apps/web**: reads the chain over JSON-RPC and signs through NEAR wallet selector.
- **apps/control-room**: a separate app. It only shows controls after the connected account matches the contract's `admin` or `treasury_account`. The contract enforces the same rule, the UI check is a convenience.

## Trade flow

```
wNEAR.ft_transfer_call(core, amount, msg=buy)
  -> core.ft_on_transfer           quote, check slippage + deadline, update reserves (optimistic)
  -> token.ft_transfer(buyer)      send tokens out
  -> core.on_buy_settled           success: credit 30/50/20 fee ledgers
                                   failure: revert reserves, refund wNEAR in full
```

Sells mirror this: the token contract calls `ft_on_transfer`, the core pays out wNEAR, and `on_sell_settled` credits or reverts.

## Money invariants

`get_accounting()` returns running counters. The contract holds exactly `pool + creator fees + MOOT TIPs + treasury` in wNEAR, and `reconciles` is true when `wnear_in − wnear_out` equals that sum. Launch fees are tracked separately as native NEAR.

## Fee math

Integer only, U256 intermediate products. Creator and trader shares are floored, the treasury takes the remainder, so the three always sum to the fee. Buys charge the fee on the input, sells on the output.

## Known gaps

- Locked liquidity cannot be withdrawn after the lock ends (the lock is recorded, the release path is not built).
- Users must register on a token and on wNEAR before trading; the app adds those steps to the first trade.
- No indexer: lists and counters come from contract views. Trade history and holder lists need an indexer.
- Pool price is the pool's spot price only. There is no oracle.
