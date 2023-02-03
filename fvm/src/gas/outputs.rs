// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm_shared::econ::TokenAmount;

#[derive(Clone, Default)]
pub(crate) struct GasOutputs {
    pub base_fee_burn: TokenAmount,
    pub over_estimation_burn: TokenAmount,
    pub miner_penalty: TokenAmount,
    pub miner_tip: TokenAmount,
    pub refund: TokenAmount,

    // In whole gas units.
    pub gas_refund: u64,
    pub gas_burned: u64,
}

impl GasOutputs {
    pub fn compute(
        // In whole gas units.
        gas_used: u64,
        gas_limit: u64,
        base_fee: &TokenAmount,
        fee_cap: &TokenAmount,
        gas_premium: &TokenAmount,
    ) -> Self {
        let mut base_fee_to_pay = base_fee;

        let mut out = GasOutputs::default();

        if base_fee > fee_cap {
            base_fee_to_pay = fee_cap;
            out.miner_penalty = (base_fee - fee_cap) * gas_used
        }

        out.base_fee_burn = base_fee_to_pay * gas_used;

        let mut miner_tip = gas_premium.clone();
        if &(base_fee_to_pay + &miner_tip) > fee_cap {
            miner_tip = fee_cap - base_fee_to_pay;
        }
        out.miner_tip = &miner_tip * gas_limit;

        let (out_gas_refund, out_gas_burned) = compute_gas_overestimation_burn(gas_used, gas_limit);
        out.gas_refund = out_gas_refund;
        out.gas_burned = out_gas_burned;

        if out.gas_burned != 0 {
            out.over_estimation_burn = base_fee_to_pay * out.gas_burned;
            out.miner_penalty += (base_fee - base_fee_to_pay) * out.gas_burned;
        }
        let required_funds = fee_cap * gas_limit;
        let refund =
            required_funds - &out.base_fee_burn - &out.miner_tip - &out.over_estimation_burn;
        out.refund = refund;

        out
    }
}

fn compute_gas_overestimation_burn(gas_used: u64, gas_limit: u64) -> (u64, u64) {
    const GAS_OVERUSE_NUM: u128 = 11;
    const GAS_OVERUSE_DENOM: u128 = 10;

    if gas_used == 0 {
        return (0, gas_limit);
    }

    // Convert to u128 to prevent overflow on multiply.
    let gas_used = gas_used as u128;
    let gas_limit = gas_limit as u128;

    // This burns (N-10)% (clamped at 0% and 100%) of the remaining gas where N is the
    // overestimation percentage.
    let over = gas_limit
        .saturating_sub((GAS_OVERUSE_NUM * gas_used) / GAS_OVERUSE_DENOM)
        .min(gas_used);

    // We handle the case where the gas used exceeds the gas limit, just in case.
    let gas_remaining = gas_limit.saturating_sub(gas_used);

    // This computes the fraction of the "remaining" gas to burn and will never be greater than 100%
    // of the remaining gas.
    let gas_to_burn = (gas_remaining * over) / gas_used;

    // But... we use saturating sub, just in case.
    let refund = gas_remaining.saturating_sub(gas_to_burn);

    (refund as u64, gas_to_burn as u64)
}

// Adapted from lotus.
#[test]
fn overestimation_burn_test() {
    fn do_test(used: u64, limit: u64, refund: u64, toburn: u64) {
        let (computed_refund, computed_toburn) = compute_gas_overestimation_burn(used, limit);
        assert_eq!(refund, computed_refund, "refund");
        assert_eq!(toburn, computed_toburn, "burned");
    }

    do_test(100, 200, 10, 90);
    do_test(100, 150, 30, 20);
    do_test(1_000, 1_300, 240, 60);
    do_test(500, 700, 140, 60);
    do_test(200, 200, 0, 0);
    do_test(20_000, 21_000, 1_000, 0);
    do_test(0, 2_000, 0, 2_000);
    do_test(500, 651, 121, 30);
    do_test(500, 5_000, 0, 4_500);
    do_test(7_499_000_000, 7_500_000_000, 1_000_000, 0);
    do_test(7_500_000_000 / 2, 7_500_000_000, 375_000_000, 3_375_000_000);
    do_test(1, 7_500_000_000, 0, 7_499_999_999);
}

#[test]
fn gas_outputs_test() {
    #[allow(clippy::too_many_arguments)]
    fn do_test(
        used: u64,
        limit: u64,
        fee_cap: u64,
        premium: u64,
        base_fee_burn: u64,
        over_estimation_burn: u64,
        miner_penalty: u64,
        miner_tip: u64,
        refund: u64,
    ) {
        let base_fee = TokenAmount::from_atto(10);
        let output = GasOutputs::compute(
            used,
            limit,
            &base_fee,
            &TokenAmount::from_atto(fee_cap),
            &TokenAmount::from_atto(premium),
        );
        assert_eq!(
            TokenAmount::from_atto(base_fee_burn),
            output.base_fee_burn,
            "base_fee_burn"
        );
        assert_eq!(
            TokenAmount::from_atto(over_estimation_burn),
            output.over_estimation_burn,
            "over_estimation_burn"
        );
        assert_eq!(
            TokenAmount::from_atto(miner_penalty),
            output.miner_penalty,
            "miner_penalty"
        );
        assert_eq!(
            TokenAmount::from_atto(miner_tip),
            output.miner_tip,
            "miner_tip"
        );
        assert_eq!(TokenAmount::from_atto(refund), output.refund, "refund");
    }
    do_test(100, 110, 11, 1, 1_000, 0, 0, 110, 100);
    do_test(100, 130, 11, 1, 1_000, 60, 0, 130, 240);
    do_test(100, 110, 10, 1, 1_000, 0, 0, 0, 100);
    do_test(100, 110, 6, 1, 600, 0, 400, 0, 60);
}
