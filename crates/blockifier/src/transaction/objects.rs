use itertools::concat;
#[cfg(feature = "parity-scale-codec")]
use parity_scale_codec::{Decode, Encode};
use starknet_api::api_core::{ClassHash, ContractAddress, Nonce};
use starknet_api::hash::StarkFelt;
use starknet_api::transaction::{Fee, TransactionHash, TransactionSignature, TransactionVersion};

use crate::execution::entry_point::CallInfo;
use crate::stdlib::collections::{HashMap, HashSet};
use crate::stdlib::string::String;
use crate::stdlib::vec::Vec;
use crate::transaction::errors::TransactionExecutionError;

pub type TransactionExecutionResult<T> = Result<T, TransactionExecutionError>;

/// Contains the account information of the transaction (outermost call).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccountTransactionContext {
    pub transaction_hash: TransactionHash,
    pub max_fee: Fee,
    pub version: TransactionVersion,
    pub signature: TransactionSignature,
    pub nonce: Nonce,
    pub sender_address: ContractAddress,
}

impl AccountTransactionContext {
    pub fn is_v0(&self) -> bool {
        self.version == TransactionVersion(StarkFelt::from(0_u8))
    }
}

/// Contains the information gathered by the execution of a transaction.
#[derive(Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "parity-scale-codec", derive(Encode, Decode))]
pub struct TransactionExecutionInfo {
    /// Transaction validation call info; [None] for `L1Handler`.
    pub validate_call_info: Option<CallInfo>,
    /// Transaction execution call info; [None] for `Declare`.
    pub execute_call_info: Option<CallInfo>,
    /// Fee transfer call info; [None] for `L1Handler`.
    pub fee_transfer_call_info: Option<CallInfo>,
    /// The actual fee that was charged (in Wei).
    pub actual_fee: Fee,
    /// Actual execution resources the transaction is charged for,
    /// including L1 gas and additional OS resources estimation.
    pub actual_resources: ResourcesMapping,
    /// Error string for reverted transactions; [None] if transaction execution was successful.
    // TODO(Dori, 1/8/2023): If the `Eq` and `PartialEq` traits are removed, or implemented on all
    //   internal structs in this enum, this field should be `Option<TransactionExecutionError>`.
    pub revert_error: Option<String>,
}
#[cfg(feature = "scale-info")]
impl scale_info::TypeInfo for TransactionExecutionInfo {
    type Identity = Self;
    // The type info is saying that the ContractClassV0Inner must be seen as an
    // array of bytes.
    fn type_info() -> scale_info::Type {
        scale_info::Type::builder()
            .path(scale_info::Path::new("TransactionExecutionInfo", module_path!()))
            .composite(
                scale_info::build::Fields::unnamed()
                    .field(|f| f.ty::<[u8]>().type_name("TransactionExecutionInfo")),
            )
    }
}

impl TransactionExecutionInfo {
    pub fn non_optional_call_infos(&self) -> Vec<&CallInfo> {
        let call_infos = vec![
            self.validate_call_info.as_ref(),
            self.execute_call_info.as_ref(),
            self.fee_transfer_call_info.as_ref(),
        ];

        call_infos.into_iter().flatten().collect()
    }

    /// Returns the set of class hashes that were executed during this transaction execution.
    pub fn get_executed_class_hashes(&self) -> HashSet<ClassHash> {
        concat(
            self.non_optional_call_infos()
                .into_iter()
                .map(|call_info| call_info.get_executed_class_hashes()),
        )
    }

    pub fn is_reverted(&self) -> bool {
        self.revert_error.is_some()
    }
}

/// A mapping from a transaction execution resource to its actual usage.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct ResourcesMapping(pub HashMap<String, u64>);

#[cfg(feature = "parity-scale-codec")]
impl Encode for ResourcesMapping {
    fn size_hint(&self) -> usize {
        self.0.len() * core::mem::size_of::<u64>()
    }

    fn encode(&self) -> Vec<u8> {
        self.0.clone().into_iter().collect::<Vec<(String, u64)>>().encode()
    }
}

#[cfg(feature = "parity-scale-codec")]
impl Decode for ResourcesMapping {
    fn decode<I: parity_scale_codec::Input>(
        input: &mut I,
    ) -> Result<Self, parity_scale_codec::Error> {
        Ok(ResourcesMapping(HashMap::from_iter(<Vec<(String, u64)>>::decode(input)?)))
    }
}
