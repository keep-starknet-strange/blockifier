extern crate alloc;
pub use alloc::collections::btree_map::BTreeMap;

use cairo_felt::Felt252;
use cairo_lang_casm::hints::Hint;
use cairo_lang_casm_contract_class::{CasmContractClass, CasmContractEntryPoint};
use cairo_vm::serde::deserialize_program::{
    parse_program, parse_program_json, ApTracking, FlowTrackingData, HintParams, ProgramJson,
    ReferenceManager,
};
use cairo_vm::types::errors::program_errors::ProgramError;
use cairo_vm::types::program::Program;
use cairo_vm::types::relocatable::MaybeRelocatable;
use cairo_vm::vm::runners::builtin_runner::{HASH_BUILTIN_NAME, POSEIDON_BUILTIN_NAME};
use cairo_vm::vm::runners::cairo_runner::ExecutionResources as VmExecutionResources;
#[cfg(feature = "parity-scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "scale-info")]
use scale_info::{build::Fields, Path, Type, TypeInfo};
use serde::de::{self};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use starknet_api::api_core::EntryPointSelector;
use starknet_api::deprecated_contract_class::{
    ContractClass as DeprecatedContractClass, EntryPoint, EntryPointOffset, EntryPointType,
};

use crate::abi::abi_utils::selector_from_name;
use crate::abi::constants::{self, CONSTRUCTOR_ENTRY_POINT_NAME};
use crate::execution::errors::PreExecutionError;
use crate::execution::execution_utils::{felt_to_stark_felt, sn_api_to_cairo_vm_program};
use crate::stdlib::collections::HashMap;
use crate::stdlib::ops::Deref;
use crate::stdlib::string::{String, ToString};
use crate::stdlib::sync::Arc;
use crate::stdlib::vec::Vec;

/// Represents a runnable StarkNet contract class (meaning, the program is runnable by the VM).
/// We wrap the actual class in an Arc to avoid cloning the program when cloning the class.
// Note: when deserializing from a SN API class JSON string, the ABI field is ignored
// by serde, since it is not required for execution.
#[derive(Clone, Debug, Eq, PartialEq, derive_more::From, Serialize, Deserialize)]
#[cfg_attr(
    feature = "parity-scale-codec",
    derive(parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
#[cfg_attr(feature = "scale-info", derive(scale_info::TypeInfo))]
pub enum ContractClass {
    V0(ContractClassV0),
    V1(ContractClassV1),
}

impl ContractClass {
    pub fn constructor_selector(&self) -> Option<EntryPointSelector> {
        match self {
            ContractClass::V0(class) => class.constructor_selector(),
            ContractClass::V1(class) => class.constructor_selector(),
        }
    }

    pub fn estimate_casm_hash_computation_resources(&self) -> VmExecutionResources {
        match self {
            ContractClass::V0(class) => class.estimate_casm_hash_computation_resources(),
            ContractClass::V1(class) => class.estimate_casm_hash_computation_resources(),
        }
    }
}

#[cfg(feature = "parity-scale-codec")]
impl ContractClass {
    // This is the maximum size of a contract in starknet. https://docs.starknet.io/documentation/starknet_versions/limits_and_triggers/
    const MAX_CONTRACT_BYTE_SIZE: usize = 20971520;
}

#[cfg(feature = "parity-scale-codec")]
impl MaxEncodedLen for ContractClass {
    fn max_encoded_len() -> usize {
        Self::MAX_CONTRACT_BYTE_SIZE
    }
}

// V0.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Eq, PartialEq)]
#[cfg_attr(
    feature = "parity-scale-codec",
    derive(parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
#[cfg_attr(feature = "scale-info", derive(scale_info::TypeInfo))]
pub struct ContractClassV0(pub Arc<ContractClassV0Inner>);
impl Deref for ContractClassV0 {
    type Target = ContractClassV0Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ContractClassV0 {
    fn constructor_selector(&self) -> Option<EntryPointSelector> {
        Some(self.entry_points_by_type[&EntryPointType::Constructor].first()?.selector)
    }

    fn n_entry_points(&self) -> usize {
        self.entry_points_by_type.values().map(|vec| vec.len()).sum()
    }

    pub fn n_builtins(&self) -> usize {
        self.program.builtins_len()
    }

    pub fn bytecode_length(&self) -> usize {
        self.program.data_len()
    }

    fn estimate_casm_hash_computation_resources(&self) -> VmExecutionResources {
        let hashed_data_size = (constants::CAIRO0_ENTRY_POINT_STRUCT_SIZE * self.n_entry_points())
            + self.n_builtins()
            + self.bytecode_length()
            + 1; // Hinted class hash.
        // The hashed data size is approximately the number of hashes (invoked in hash chains).
        let n_steps = constants::N_STEPS_PER_PEDERSEN * hashed_data_size;

        VmExecutionResources {
            n_steps,
            n_memory_holes: 0,
            builtin_instance_counter: HashMap::from([(
                HASH_BUILTIN_NAME.to_string(),
                hashed_data_size,
            )]),
        }
    }

    pub fn try_from_json_string(raw_contract_class: &str) -> Result<ContractClassV0, ProgramError> {
        let contract_class: ContractClassV0Inner = serde_json::from_str(raw_contract_class)?;
        Ok(ContractClassV0(Arc::new(contract_class)))
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContractClassV0Inner {
    #[serde(with = "serde_program")]
    pub program: Program,
    pub entry_points_by_type: HashMap<EntryPointType, Vec<EntryPoint>>,
}

#[cfg(feature = "parity-scale-codec")]
impl Encode for ContractClassV0Inner {
    fn encode(&self) -> Vec<u8> {
        let val = self.clone();
        let entry_point_btree = val
            .entry_points_by_type
            .into_iter()
            .collect::<BTreeMap<EntryPointType, Vec<EntryPoint>>>();
        (val.program, entry_point_btree).encode()
    }
}

#[cfg(feature = "parity-scale-codec")]
impl Decode for ContractClassV0Inner {
    fn decode<I: parity_scale_codec::Input>(
        input: &mut I,
    ) -> Result<Self, parity_scale_codec::Error> {
        let (program, entry_point_btree) =
            <(Program, BTreeMap<EntryPointType, Vec<EntryPoint>>)>::decode(input)?;
        let entry_points_by_type =
            <HashMap<EntryPointType, Vec<EntryPoint>>>::from_iter(entry_point_btree);
        Ok(ContractClassV0Inner { program, entry_points_by_type })
    }
}

#[cfg(feature = "scale-info")]
impl TypeInfo for ContractClassV0Inner {
    type Identity = Self;
    // The type info is saying that the ContractClassV0Inner must be seen as an
    // array of bytes.
    fn type_info() -> Type {
        Type::builder().path(Path::new("ContractClassV0Inner", module_path!())).composite(
            Fields::unnamed().field(|f| f.ty::<[u8]>().type_name("ContractClassV0Inner")),
        )
    }
}

impl TryFrom<DeprecatedContractClass> for ContractClassV0 {
    type Error = ProgramError;

    fn try_from(class: DeprecatedContractClass) -> Result<Self, Self::Error> {
        Ok(Self(Arc::new(ContractClassV0Inner {
            program: sn_api_to_cairo_vm_program(class.program)?,
            entry_points_by_type: class.entry_points_by_type,
        })))
    }
}

// V1.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "parity-scale-codec",
    derive(parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
#[cfg_attr(feature = "scale-info", derive(scale_info::TypeInfo))]
pub struct ContractClassV1(pub Arc<ContractClassV1Inner>);
impl Deref for ContractClassV1 {
    type Target = ContractClassV1Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ContractClassV1 {
    fn constructor_selector(&self) -> Option<EntryPointSelector> {
        Some(self.0.entry_points_by_type[&EntryPointType::Constructor].first()?.selector)
    }

    pub fn bytecode_length(&self) -> usize {
        self.program.data_len()
    }

    pub fn get_entry_point(
        &self,
        call: &super::entry_point::CallEntryPoint,
    ) -> Result<EntryPointV1, PreExecutionError> {
        if call.entry_point_type == EntryPointType::Constructor
            && call.entry_point_selector != selector_from_name(CONSTRUCTOR_ENTRY_POINT_NAME)
        {
            return Err(PreExecutionError::InvalidConstructorEntryPointName);
        }

        let entry_points_of_same_type = &self.0.entry_points_by_type[&call.entry_point_type];
        let filtered_entry_points: Vec<_> = entry_points_of_same_type
            .iter()
            .filter(|ep| ep.selector == call.entry_point_selector)
            .collect();

        match &filtered_entry_points[..] {
            [] => Err(PreExecutionError::EntryPointNotFound(call.entry_point_selector)),
            [entry_point] => Ok((*entry_point).clone()),
            _ => Err(PreExecutionError::DuplicatedEntryPointSelector {
                selector: call.entry_point_selector,
                typ: call.entry_point_type,
            }),
        }
    }

    /// Returns the estimated VM resources required for computing Casm hash.
    /// This is an empiric measurement of several bytecode lengths, which constitutes as the
    /// dominant factor in it.
    fn estimate_casm_hash_computation_resources(&self) -> VmExecutionResources {
        let bytecode_length = self.bytecode_length() as f64;
        let n_steps = (503.0 + bytecode_length * 5.7) as usize;
        let n_poseidon_builtins = (10.9 + bytecode_length * 0.5) as usize;

        VmExecutionResources {
            n_steps,
            n_memory_holes: 0,
            builtin_instance_counter: HashMap::from([(
                POSEIDON_BUILTIN_NAME.to_string(),
                n_poseidon_builtins,
            )]),
        }
    }

    pub fn try_from_json_string(raw_contract_class: &str) -> Result<ContractClassV1, ProgramError> {
        let casm_contract_class: CasmContractClass = serde_json::from_str(raw_contract_class)?;
        let contract_class: ContractClassV1 = casm_contract_class.try_into()?;

        Ok(contract_class)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContractClassV1Inner {
    #[serde(with = "serde_program")]
    pub program: Program,
    pub entry_points_by_type: HashMap<EntryPointType, Vec<EntryPointV1>>,
    pub hints: HashMap<String, Hint>,
}

#[cfg(feature = "parity-scale-codec")]
impl Encode for ContractClassV1Inner {
    fn encode(&self) -> Vec<u8> {
        let val = self.clone();
        let entry_point_btree = val
            .entry_points_by_type
            .into_iter()
            .collect::<BTreeMap<EntryPointType, Vec<EntryPointV1>>>();
        let hints = val.hints.into_iter().collect::<BTreeMap<String, Hint>>();
        (val.program, entry_point_btree, hints).encode()
    }
}

#[cfg(feature = "parity-scale-codec")]
impl Decode for ContractClassV1Inner {
    fn decode<I: parity_scale_codec::Input>(
        input: &mut I,
    ) -> Result<Self, parity_scale_codec::Error> {
        let (program, entry_point_btree, hints) = <(
            Program,
            BTreeMap<EntryPointType, Vec<EntryPointV1>>,
            BTreeMap<String, Hint>,
        )>::decode(input)?;
        let entry_points_by_type =
            <HashMap<EntryPointType, Vec<EntryPointV1>>>::from_iter(entry_point_btree);
        let hints = <HashMap<String, Hint>>::from_iter(hints);
        Ok(ContractClassV1Inner { program, entry_points_by_type, hints })
    }
}

#[cfg(feature = "scale-info")]
impl TypeInfo for ContractClassV1Inner {
    type Identity = Self;
    // The type info is saying that the ContractClassV0Inner must be seen as an
    // array of bytes.
    fn type_info() -> Type {
        Type::builder().path(Path::new("ContractClassV1Inner", module_path!())).composite(
            Fields::unnamed().field(|f| f.ty::<[u8]>().type_name("ContractClassV1Inner")),
        )
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(
    feature = "parity-scale-codec",
    derive(parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
#[cfg_attr(feature = "scale-info", derive(scale_info::TypeInfo))]
pub struct EntryPointV1 {
    pub selector: EntryPointSelector,
    pub offset: EntryPointOffset,
    pub builtins: Vec<String>,
}

impl EntryPointV1 {
    pub fn pc(&self) -> usize {
        self.offset.0
    }
}

impl TryFrom<CasmContractClass> for ContractClassV1 {
    type Error = ProgramError;

    fn try_from(class: CasmContractClass) -> Result<Self, Self::Error> {
        let data: Vec<MaybeRelocatable> = class
            .bytecode
            .into_iter()
            .map(|x| MaybeRelocatable::from(Felt252::from(x.value)))
            .collect();

        let mut hints: HashMap<usize, Vec<HintParams>> = HashMap::new();
        for (i, hint_list) in class.hints.iter() {
            let hint_params: Result<Vec<HintParams>, ProgramError> =
                hint_list.iter().map(hint_to_hint_params).collect();
            hints.insert(*i, hint_params?);
        }

        // Collect a sting to hint map so that the hint processor can fetch the correct [Hint]
        // for each instruction.
        let mut string_to_hint: HashMap<String, Hint> = HashMap::new();
        for (_, hint_list) in class.hints.iter() {
            for hint in hint_list.iter() {
                string_to_hint.insert(serde_json::to_string(hint)?, hint.clone());
            }
        }

        let builtins = vec![]; // The builtins are initialize later.
        let main = None;
        let reference_manager = ReferenceManager { references: Vec::new() };
        let identifiers = HashMap::new();
        let error_message_attributes = vec![];
        let instruction_locations = None;

        let program = Program::new(
            builtins,
            data,
            main,
            hints,
            reference_manager,
            identifiers,
            error_message_attributes,
            instruction_locations,
        )?;

        let mut entry_points_by_type = HashMap::new();
        entry_points_by_type.insert(
            EntryPointType::Constructor,
            convert_entry_points_v1(class.entry_points_by_type.constructor)?,
        );
        entry_points_by_type.insert(
            EntryPointType::External,
            convert_entry_points_v1(class.entry_points_by_type.external)?,
        );
        entry_points_by_type.insert(
            EntryPointType::L1Handler,
            convert_entry_points_v1(class.entry_points_by_type.l1_handler)?,
        );

        Ok(Self(Arc::new(ContractClassV1Inner {
            program,
            entry_points_by_type,
            hints: string_to_hint,
        })))
    }
}

// V0 utilities.

mod serde_program {
    use super::*;

    /// Serializes the Program using the ProgramJson
    pub fn serialize<S: Serializer>(program: &Program, serializer: S) -> Result<S::Ok, S::Error> {
        let program = parse_program(program.clone());
        program.serialize(serializer)
    }

    /// Deserializes the Program using the ProgramJson
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Program, D::Error> {
        let prog = ProgramJson::deserialize(deserializer)?;
        parse_program_json(prog, None)
            .map_err(|e| de::Error::custom(format!("couldn't convert programjson to program {e:}")))
    }
}

pub use serde_program::{deserialize, serialize};

// V1 utilities.

// TODO(spapini): Share with cairo-lang-runner.
fn hint_to_hint_params(hint: &cairo_lang_casm::hints::Hint) -> Result<HintParams, ProgramError> {
    Ok(HintParams {
        code: serde_json::to_string(hint)?,
        accessible_scopes: vec![],
        flow_tracking_data: FlowTrackingData {
            ap_tracking: ApTracking::new(),
            reference_ids: HashMap::new(),
        },
    })
}

fn convert_entry_points_v1(
    external: Vec<CasmContractEntryPoint>,
) -> Result<Vec<EntryPointV1>, ProgramError> {
    external
        .into_iter()
        .map(|ep| -> Result<_, ProgramError> {
            Ok(EntryPointV1 {
                selector: EntryPointSelector(felt_to_stark_felt(
                    &Felt252::try_from(ep.selector).unwrap(),
                )),
                offset: EntryPointOffset(ep.offset),
                builtins: ep.builtins.into_iter().map(|builtin| builtin + "_builtin").collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{TEST_CONTRACT_CAIRO0_PATH, TEST_CONTRACT_CAIRO1_PATH};

    #[test]
    fn test_serialize_deserialize_contract_v0() {
        let contract = ContractClassV0::from_file(TEST_CONTRACT_CAIRO0_PATH);

        assert_eq!(
            contract,
            serde_json::from_slice(&serde_json::to_vec(&contract).unwrap()).unwrap()
        )
    }

    #[test]
    fn test_serialize_deserialize_contract_v1() {
        let contract = ContractClassV1::from_file(TEST_CONTRACT_CAIRO1_PATH);

        assert_eq!(
            contract,
            serde_json::from_slice(&serde_json::to_vec(&contract).unwrap()).unwrap()
        )
    }
}

#[cfg(test)]
#[cfg(feature = "parity-scale-codec")]
mod tests_scale_codec {
    use parity_scale_codec::{Decode, Encode};

    use crate::execution::contract_class::{ContractClassV0, ContractClassV1};
    use crate::test_utils::{TEST_CONTRACT_CAIRO0_PATH, TEST_CONTRACT_CAIRO1_PATH};

    #[test]
    fn test_encode_decode_contract_v0() {
        let contract = ContractClassV0::from_file(TEST_CONTRACT_CAIRO0_PATH);
        assert_eq!(contract, ContractClassV0::decode(&mut &contract.encode()[..]).unwrap())
    }

    #[test]
    fn test_encode_decode_contract_v1() {
        let contract = ContractClassV1::from_file(TEST_CONTRACT_CAIRO1_PATH);
        assert_eq!(contract, ContractClassV1::decode(&mut &contract.encode()[..]).unwrap())
    }
}
