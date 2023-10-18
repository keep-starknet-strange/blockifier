use sp_arithmetic::fixed_point::{FixedPointNumber, FixedU128};
use sp_arithmetic::traits::Zero;
use starknet_api::transaction::Fee;

use crate::abi::constants;
use crate::block_context::BlockContext;
use crate::stdlib::collections::HashSet;
use crate::stdlib::string::String;
use crate::transaction::errors::TransactionExecutionError;
use crate::transaction::objects::{ResourcesMapping, TransactionExecutionResult};
#[cfg(test)]
#[path = "fee_test.rs"]
pub mod test;

pub fn extract_l1_gas_and_vm_usage(resources: &ResourcesMapping) -> (usize, ResourcesMapping) {
    let mut vm_resource_usage = resources.0.clone();
    let l1_gas_usage = vm_resource_usage
        .remove(constants::GAS_USAGE)
        .expect("`ResourcesMapping` does not have the key `l1_gas_usage`.");

    (l1_gas_usage, ResourcesMapping(vm_resource_usage))
}

/// Calculates the L1 gas consumed when submitting the underlying Cairo program to SHARP.
/// I.e., returns the heaviest Cairo resource weight (in terms of L1 gas), as the size of
/// a proof is determined similarly - by the (normalized) largest segment.
pub fn calculate_l1_gas_by_vm_usage(
    block_context: &BlockContext,
    vm_resource_usage: &ResourcesMapping,
) -> TransactionExecutionResult<FixedU128> {
    let vm_resource_fee_costs = &block_context.vm_resource_fee_cost;
    let vm_resource_names = HashSet::<&String>::from_iter(vm_resource_usage.0.keys());
    if !vm_resource_names.is_subset(&HashSet::from_iter(vm_resource_fee_costs.keys())) {
        return Err(TransactionExecutionError::CairoResourcesNotContainedInFeeCosts);
    };

    // Convert Cairo usage to L1 gas usage.
    vm_resource_fee_costs
        .iter()
        .map(|(key, resource_val)| {
            let key_resource_usage =
                vm_resource_usage.0.get(key).cloned().unwrap_or_default() as u128;
            let key_resource_usage = FixedU128::checked_from_integer(key_resource_usage)
                .ok_or(TransactionExecutionError::FixedPointConversion);

            key_resource_usage.map(|kru| kru.mul(*resource_val))
        })
        .try_fold(FixedU128::zero(), |accum, res| res.map(|v| v.max(accum)))
}

/// Calculates the fee that should be charged, given execution resources.
/// We add the l1_gas_usage (which may include, for example, the direct cost of L2-to-L1 messages)
/// to the gas consumed by Cairo VM resource and multiply by the L1 gas price.
pub fn calculate_tx_fee(
    resources: &ResourcesMapping,
    block_context: &BlockContext,
) -> TransactionExecutionResult<Fee> {
    let (l1_gas_usage, vm_resources) = extract_l1_gas_and_vm_usage(resources);
    let l1_gas_by_vm_usage = calculate_l1_gas_by_vm_usage(block_context, &vm_resources)?;
    let total_l1_gas_usage = FixedU128::checked_from_integer(l1_gas_usage as u128)
        .ok_or(TransactionExecutionError::FixedPointConversion)?
        + l1_gas_by_vm_usage;
    let total_l1_gas_usage = total_l1_gas_usage.ceil();

    Ok(Fee(total_l1_gas_usage.saturating_mul_int(block_context.gas_price)))
}
