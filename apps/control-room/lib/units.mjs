// Integer-only unit helpers. Never use floats for money.
export const YOCTO_DECIMALS = 24;
export const TOKEN_DECIMALS = 18;

/** "1.5" -> 1500000000000000000000000n (for 24 decimals). Throws on bad input. */
export function parseUnits(input, decimals) {
  const s = String(input).trim();
  if (!/^\d*\.?\d*$/.test(s) || s === "" || s === ".") throw new Error("Enter a valid number");
  const [whole = "0", frac = ""] = s.split(".");
  if (frac.length > decimals) throw new Error(`Max ${decimals} decimal places`);
  return BigInt(whole || "0") * 10n ** BigInt(decimals) + BigInt((frac + "0".repeat(decimals)).slice(0, decimals) || "0");
}

/** bigint -> "1.2345" trimmed to maxFrac places (truncates, never rounds up). */
export function formatUnits(value, decimals, maxFrac = 4) {
  const v = BigInt(value);
  const neg = v < 0n;
  const abs = neg ? -v : v;
  const base = 10n ** BigInt(decimals);
  const whole = abs / base;
  let frac = (abs % base).toString().padStart(decimals, "0").slice(0, maxFrac).replace(/0+$/, "");
  const w = whole.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  return (neg ? "-" : "") + w + (frac ? "." + frac : "");
}

export const nearToYocto = (s) => parseUnits(s, YOCTO_DECIMALS);
export const yoctoToNear = (v, maxFrac = 4) => formatUnits(v, YOCTO_DECIMALS, maxFrac);
export const tokensToUnits = (s) => parseUnits(s, TOKEN_DECIMALS);
export const unitsToTokens = (v, maxFrac = 2) => formatUnits(v, TOKEN_DECIMALS, maxFrac);

/** Minimum acceptable output after slippage, in basis points. */
export function applySlippage(amount, slippageBps) {
  const a = BigInt(amount);
  const bps = BigInt(Math.max(0, Math.min(10000, Math.floor(slippageBps))));
  return (a * (10000n - bps)) / 10000n;
}

/** Compact display for big token numbers: 331103678.9 -> "331.10M". */
export function compactTokens(v) {
  const whole = BigInt(v) / 10n ** BigInt(TOKEN_DECIMALS);
  const n = Number(whole);
  if (n >= 1e12) return (n / 1e12).toFixed(2) + "T";
  if (n >= 1e9) return (n / 1e9).toFixed(2) + "B";
  if (n >= 1e6) return (n / 1e6).toFixed(2) + "M";
  if (n >= 1e3) return (n / 1e3).toFixed(2) + "K";
  return n.toString();
}

/** Spot price: NEAR per whole token, as a bigint of yocto per whole token. */
export function spotPriceYocto(wnearReserve, tokenReserve) {
  const t = BigInt(tokenReserve);
  if (t === 0n) return 0n;
  return (BigInt(wnearReserve) * 10n ** BigInt(TOKEN_DECIMALS)) / t;
}

/** ns timestamp (string/bigint) -> JS Date */
export const nsToDate = (ns) => new Date(Number(BigInt(ns) / 1_000_000n));
