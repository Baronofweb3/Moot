import test from "node:test";
import assert from "node:assert/strict";
import { parseUnits, formatUnits, nearToYocto, yoctoToNear, applySlippage, spotPriceYocto, compactTokens } from "./units.mjs";

test("parse and format NEAR", () => {
  assert.equal(nearToYocto("1"), 10n ** 24n);
  assert.equal(nearToYocto("0.5"), 5n * 10n ** 23n);
  assert.equal(nearToYocto("0.000000000000000000000001"), 1n);
  assert.equal(yoctoToNear(1495n * 10n ** 21n), "1.495");
  assert.equal(yoctoToNear(10n ** 24n), "1");
});
test("rejects bad input and too many decimals", () => {
  assert.throws(() => parseUnits("", 24));
  assert.throws(() => parseUnits("1.2.3", 24));
  assert.throws(() => parseUnits("abc", 24));
  assert.throws(() => parseUnits("1.1234567", 6));
  assert.throws(() => parseUnits("-1", 24));
});
test("format truncates, never rounds up", () => {
  assert.equal(formatUnits(1999999n, 6, 2), "1.99");
  assert.equal(formatUnits(0n, 6), "0");
  assert.equal(formatUnits(1234567890n, 6, 4), "1,234.5678");
});
test("slippage", () => {
  assert.equal(applySlippage(10000n, 100), 9900n);
  assert.equal(applySlippage(10000n, 0), 10000n);
  assert.equal(applySlippage(10000n, 99999), 0n);
});
test("spot price matches seed ratio", () => {
  // 1 NEAR vs 1e9 whole tokens -> 1e-9 NEAR per token = 1e15 yocto
  assert.equal(spotPriceYocto(10n ** 24n, 10n ** 9n * 10n ** 18n), 10n ** 15n);
});
test("compact tokens", () => {
  assert.equal(compactTokens(10n ** 9n * 10n ** 18n), "1.00B");
});
