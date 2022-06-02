use std::convert::TryFrom;

use fvm_shared::bigint::BigInt;
use fvm_shared::econ::TokenAmount;

#[derive(Clone, Default)]
pub(crate) struct GasOutputs {
    pub base_fee_burn: TokenAmount,
    pub over_estimation_burn: TokenAmount,
    pub miner_penalty: TokenAmount,
    pub miner_tip: TokenAmount,
    pub refund: TokenAmount,

    // In whole gas units.
    pub gas_refund: i64,
    pub gas_burned: i64,
}

impl GasOutputs {
    pub fn compute(
        // In whole gas units.
        gas_used: i64,
        gas_limit: i64,
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

fn compute_gas_overestimation_burn(gas_used: i64, gas_limit: i64) -> (i64, i64) {
    const GAS_OVERUSE_NUM: i64 = 11;
    const GAS_OVERUSE_DENOM: i64 = 10;

    if gas_used == 0 {
        return (0, gas_limit);
    }

    let mut over = gas_limit - (GAS_OVERUSE_NUM * gas_used) / GAS_OVERUSE_DENOM;
    if over < 0 {
        return (gas_limit - gas_used, 0);
    }

    if over > gas_used {
        over = gas_used;
    }

    let mut gas_to_burn: BigInt = (gas_limit - gas_used).into();
    gas_to_burn *= over;
    gas_to_burn /= gas_used;

    let gas_to_burn = i64::try_from(gas_to_burn).unwrap();
    (gas_limit - gas_used - gas_to_burn, gas_to_burn)
}
