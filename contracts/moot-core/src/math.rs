//! Integer-only maths for the AMM and fee split. No floating point anywhere.

use uint::construct_uint;

construct_uint! {
    /// 256-bit unsigned integer, used only for intermediate products.
    pub struct U256(4);
}

pub const BPS: u128 = 10_000;
/// Fixed fee split (basis points of the trading fee). Treasury also gets the rounding remainder.
pub const TREASURY_BPS: u128 = 3_000;
pub const CREATOR_BPS: u128 = 5_000;
pub const TRADER_BPS: u128 = 2_000;
const _: () = assert!(TREASURY_BPS + CREATOR_BPS + TRADER_BPS == BPS);

/// floor(a * b / c) with a 256-bit intermediate. Panics if c == 0 or the result overflows u128.
pub fn mul_div(a: u128, b: u128, c: u128) -> u128 {
    assert!(c != 0, "division by zero");
    let r = U256::from(a) * U256::from(b) / U256::from(c);
    assert!(r <= U256::from(u128::MAX), "overflow");
    r.as_u128()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeSplit {
    pub treasury: u128,
    pub creator: u128,
    pub trader: u128,
}

/// Splits a collected fee 30/50/20. Creator and trader are rounded down; the treasury gets the
/// remainder, so `treasury + creator + trader == fee` always.
pub fn split_fee(fee: u128) -> FeeSplit {
    let creator = mul_div(fee, CREATOR_BPS, BPS);
    let trader = mul_div(fee, TRADER_BPS, BPS);
    FeeSplit { treasury: fee - creator - trader, creator, trader }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyQuote {
    pub fee: u128,
    pub net_in: u128,
    pub tokens_out: u128,
}

/// Buy: the fee is `fee_bps` of the gross wNEAR paid; the remainder swaps through x*y=k.
/// `x` = wNEAR reserve, `y` = token reserve.
pub fn buy_quote(x: u128, y: u128, gross_in: u128, fee_bps: u128) -> Option<BuyQuote> {
    if x == 0 || y == 0 || gross_in == 0 {
        return None;
    }
    let fee = mul_div(gross_in, fee_bps, BPS);
    let net_in = gross_in - fee;
    if net_in == 0 {
        return None;
    }
    let tokens_out = mul_div(y, net_in, x.checked_add(net_in)?);
    if tokens_out == 0 || tokens_out >= y {
        return None;
    }
    Some(BuyQuote { fee, net_in, tokens_out })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellQuote {
    pub gross_out: u128,
    pub fee: u128,
    pub net_out: u128,
}

/// Sell: tokens swap through x*y=k for a gross wNEAR amount; the fee is `fee_bps` of that gross
/// output, taken before the seller is paid.
pub fn sell_quote(x: u128, y: u128, tokens_in: u128, fee_bps: u128) -> Option<SellQuote> {
    if x == 0 || y == 0 || tokens_in == 0 {
        return None;
    }
    let gross_out = mul_div(x, tokens_in, y.checked_add(tokens_in)?);
    if gross_out == 0 || gross_out >= x {
        return None;
    }
    let fee = mul_div(gross_out, fee_bps, BPS);
    let net_out = gross_out - fee;
    if net_out == 0 {
        return None;
    }
    Some(SellQuote { gross_out, fee, net_out })
}

/// Price impact of a trade in basis points, measured against the pre-trade spot price.
pub fn price_impact_bps(x: u128, y: u128, x_after: u128, y_after: u128) -> u128 {
    // spot = x / y. impact = |spot_after - spot| / spot = |x_after*y - x*y_after| / (x*y_after)
    let a = U256::from(x_after) * U256::from(y);
    let b = U256::from(x) * U256::from(y_after);
    if b.is_zero() {
        return 0;
    }
    let diff = if a > b { a - b } else { b - a };
    let r = diff * U256::from(BPS) / b;
    if r > U256::from(u128::MAX) { u128::MAX } else { r.as_u128() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_example_100_dollar_trade() {
        // $100 trade at 1% = $1 fee -> 0.30 / 0.50 / 0.20 (cents*100 to stay in integers)
        let s = split_fee(1_000_000);
        assert_eq!((s.treasury, s.creator, s.trader), (300_000, 500_000, 200_000));
    }

    #[test]
    fn split_always_sums_to_fee_and_remainder_goes_to_treasury() {
        for fee in [0u128, 1, 2, 3, 7, 9, 99, 101, 999_999_999, u128::MAX / 3] {
            let s = split_fee(fee);
            assert_eq!(s.treasury + s.creator + s.trader, fee, "fee {fee}");
            assert!(s.treasury >= mul_div(fee, TREASURY_BPS, BPS));
        }
        // 1 yocto: creator 0, trader 0, treasury keeps it all
        assert_eq!(split_fee(1), FeeSplit { treasury: 1, creator: 0, trader: 0 });
    }

    #[test]
    fn buy_fee_is_one_percent_of_gross_and_net_swaps() {
        let (x, y) = (1_000_000_000_000u128, 1_000_000_000_000u128);
        let q = buy_quote(x, y, 100_000_000, 100).unwrap();
        assert_eq!(q.fee, 1_000_000);
        assert_eq!(q.net_in, 99_000_000);
        assert_eq!(q.tokens_out, mul_div(y, 99_000_000, x + 99_000_000));
    }

    #[test]
    fn sell_fee_is_one_percent_of_gross_output() {
        let (x, y) = (1_000_000_000_000u128, 1_000_000_000_000u128);
        let q = sell_quote(x, y, 100_000_000, 100).unwrap();
        assert_eq!(q.gross_out, mul_div(x, 100_000_000, y + 100_000_000));
        assert_eq!(q.fee, q.gross_out / 100);
        assert_eq!(q.net_out, q.gross_out - q.fee);
    }

    #[test]
    fn k_never_decreases_on_buy_or_sell() {
        let cases = [(5_000_000_000_000_000_000_000_000u128, 1_000_000_000_000_000_000_000_000_000u128), (1_000, 1_000_000), (7, 13)];
        for (x, y) in cases {
            for amt in [1u128, 2, 10, 1_000, 123_456_789, x / 3 + 1] {
                if let Some(b) = buy_quote(x, y, amt, 100) {
                    let (x2, y2) = (x + b.net_in, y - b.tokens_out);
                    assert!(U256::from(x2) * U256::from(y2) >= U256::from(x) * U256::from(y));
                }
                if let Some(s) = sell_quote(x, y, amt.min(y), 100) {
                    let (x2, y2) = (x - s.gross_out, y + amt.min(y));
                    assert!(U256::from(x2) * U256::from(y2) >= U256::from(x) * U256::from(y));
                }
            }
        }
    }

    #[test]
    fn huge_supply_does_not_overflow() {
        // 1e12 whole tokens at 18 decimals = 1e30 ; 1e6 NEAR = 1e30 yocto
        let (x, y) = (1_000_000u128 * 10u128.pow(24), 1_000_000_000_000u128 * 10u128.pow(18));
        let q = buy_quote(x, y, 10u128.pow(24), 100).unwrap();
        assert!(q.tokens_out > 0 && q.tokens_out < y);
    }

    #[test]
    fn dust_and_empty_inputs_are_rejected() {
        assert!(buy_quote(0, 10, 5, 100).is_none());
        assert!(buy_quote(10, 10, 0, 100).is_none());
        assert!(sell_quote(10, 0, 5, 100).is_none());
        // output rounds to zero
        assert!(buy_quote(1_000_000_000, 10, 100, 100).is_none());
    }

    #[test]
    fn price_impact_grows_with_size() {
        let (x, y) = (1_000_000u128, 1_000_000u128);
        let small = buy_quote(x, y, 1_000, 100).unwrap();
        let big = buy_quote(x, y, 500_000, 100).unwrap();
        let i1 = price_impact_bps(x, y, x + small.net_in, y - small.tokens_out);
        let i2 = price_impact_bps(x, y, x + big.net_in, y - big.tokens_out);
        assert!(i2 > i1);
    }
}
