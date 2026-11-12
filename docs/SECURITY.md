# Security notes

This is unaudited testnet software. Do not use it with real funds.

## What the admin can and cannot do

Can: pause trading and launches, change the trading fee (hard cap 3%), launch fee (hard cap $50), minimum initial liquidity, the lock period for **new** launches, the treasury account, and the testnet NEAR/USD rate.

Cannot: mint tokens, change anyone's balance, take pool liquidity, change the 30/50/20 split, or move creator fees and MOOT TIPs. Pausing refunds incoming trades and does not block withdrawals.

## Design choices

- Fees are credited only after a swap settles, and reverted otherwise.
- Every trade carries a minimum output and a deadline.
- Anything that moves money out needs 1 yoctoNEAR, which forces a full-access key.
- Invalid or hostile `ft_on_transfer` messages are refunded, not panicked on.
- Callbacks are `#[private]`.

## Before mainnet

1. Independent audit.
2. Use multisig accounts for `admin` and `treasury_account`.
3. Replace the admin-set NEAR/USD rate with an oracle.
4. Build the liquidity release path and test it.
5. Run the failure paths (failed transfers, unregistered receivers) on testnet, not just in unit tests.
