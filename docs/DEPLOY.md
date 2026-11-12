# Deploy

## Contracts (testnet)

1. Push to GitHub. The `contracts` workflow builds `moot_ft.wasm` and `moot_core.wasm` and uploads them as the `moot-wasm` artifact.
2. Create an account, deploy `moot_core.wasm` with the init call `new`:

```
near contract deploy <core>.testnet use-file moot_core.wasm with-init-call new json-args \
 '{"admin":"<admin>.testnet","treasury_account":"<treasury>.testnet","wnear":"wrap.testnet","near_usd_price_micro":"3000000","min_initial_liquidity":"500000000000000000000000"}' \
 prepaid-gas '100 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send
```

The core account needs roughly 5 NEAR to hold its own code.

## Apps (Vercel)

Import the GitHub repo **twice**, once per app:

| Project | Root directory |
|---|---|
| MOOT (public) | `apps/web` |
| MOOT Control Room | `apps/control-room` |

Framework preset: Next.js. Optional environment variables (both apps):

| Variable | Default |
|---|---|
| `NEXT_PUBLIC_MOOT_CORE` | `moot-core-01.testnet` |
| `NEXT_PUBLIC_WNEAR` | `wrap.testnet` |
| `NEXT_PUBLIC_RPC_URL` | `https://archival-rpc.testnet.fastnear.com` |

No secrets are needed. Neither app holds a private key; all signing happens in the user's wallet.
