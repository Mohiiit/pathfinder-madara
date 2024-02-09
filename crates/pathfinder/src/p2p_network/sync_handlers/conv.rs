//! Workaround for the orphan rule - implement conversion fns for types ourside our crate.
use p2p_proto::common::{Address, Hash, Merkle, Patricia};
use p2p_proto::receipt::EthereumAddress;
use p2p_proto::receipt::{
    execution_resources::BuiltinCounter, DeclareTransactionReceipt,
    DeployAccountTransactionReceipt, DeployTransactionReceipt, ExecutionResources,
    InvokeTransactionReceipt, L1HandlerTransactionReceipt, MessageToL1, ReceiptCommon,
};
use p2p_proto::state::{ContractDiff, ContractStoredValue, StateDiff};
use p2p_proto::transaction::{AccountSignature, ResourceBounds};
use pathfinder_common::receipt::Receipt as CommonReceipt;
use pathfinder_common::transaction::DataAvailabilityMode;
use pathfinder_common::transaction::Transaction as CommonTransaction;
use pathfinder_common::{
    event::Event, state_update::ContractUpdate, transaction::ResourceBound,
    transaction::Transaction, BlockHeader, StateUpdate,
};
use pathfinder_common::{
    AccountDeploymentDataElem, PaymasterDataElem, StateCommitment, TransactionHash,
};
use pathfinder_crypto::Felt;
use std::time::{Duration, SystemTime};

pub trait ToProto<T> {
    fn to_proto(self) -> T;
}

impl ToProto<p2p_proto::block::BlockHeader> for BlockHeader {
    fn to_proto(self) -> p2p_proto::block::BlockHeader {
        const ZERO_MERKLE: Merkle = Merkle {
            n_leaves: 0,
            root: Hash(Felt::ZERO),
        };
        const ZERO_PATRICIA: Patricia = Patricia {
            height: 0,
            root: Hash(Felt::ZERO),
        };
        p2p_proto::block::BlockHeader {
            parent_hash: Hash(self.parent_hash.0),
            number: self.number.get(),
            time: SystemTime::UNIX_EPOCH // FIXME Dunno how to convert
                .checked_add(Duration::from_secs(self.timestamp.get()))
                .unwrap(),
            sequencer_address: Address(self.sequencer_address.0),
            // FIXME: calculate the merkles et al.
            state_diffs: ZERO_MERKLE,
            state: ZERO_PATRICIA,
            proof_fact: Hash(Felt::ZERO),
            transactions: ZERO_MERKLE,
            events: ZERO_MERKLE,
            receipts: ZERO_MERKLE,
            // FIXME extra fields added to make sync work
            hash: Hash(self.hash.0),
            gas_price: self.eth_l1_gas_price.0.to_be_bytes().into(),
            starknet_version: self.starknet_version.take_inner(),
            state_commitment: (self.state_commitment != StateCommitment::ZERO)
                .then_some(Hash(self.state_commitment.0)),
        }
    }
}

impl ToProto<p2p_proto::state::StateDiff> for StateUpdate {
    fn to_proto(self) -> p2p_proto::state::StateDiff {
        StateDiff {
            domain: 0, // FIXME there will initially be 2 trees, dunno which id is which
            contract_diffs: self
                .system_contract_updates
                .into_iter()
                .map(|(address, update)| {
                    let address = Address(address.0);
                    let values = update
                        .storage
                        .into_iter()
                        .map(|(storage_address, storage_value)| ContractStoredValue {
                            key: storage_address.0,
                            value: storage_value.0,
                        })
                        .collect();
                    ContractDiff {
                        address,
                        nonce: None,
                        class_hash: None,
                        values,
                    }
                })
                .chain(self.contract_updates.into_iter().map(|(address, update)| {
                    let address = Address(address.0);
                    let ContractUpdate {
                        storage,
                        class,
                        nonce,
                    } = update;
                    let values = storage
                        .into_iter()
                        .map(|(storage_address, storage_value)| ContractStoredValue {
                            key: storage_address.0,
                            value: storage_value.0,
                        })
                        .collect();
                    ContractDiff {
                        address,
                        nonce: nonce.map(|n| n.0),
                        class_hash: class.map(|c| c.class_hash().0),
                        values,
                    }
                }))
                .collect(),
        }
    }
}

impl ToProto<p2p_proto::transaction::Transaction> for Transaction {
    fn to_proto(self) -> p2p_proto::transaction::Transaction {
        use p2p_proto::transaction as proto;
        use pathfinder_common::transaction::TransactionVariant::*;

        match self.variant {
            DeclareV0(x) => proto::Transaction::DeclareV0(proto::DeclareV0 {
                sender: Address(x.sender_address.0),
                max_fee: x.max_fee.0,
                signature: AccountSignature {
                    parts: x.signature.into_iter().map(|s| s.0).collect(),
                },
                class_hash: Hash(x.class_hash.0),
            }),
            DeclareV1(x) => proto::Transaction::DeclareV1(proto::DeclareV1 {
                sender: Address(x.sender_address.0),
                max_fee: x.max_fee.0,
                signature: AccountSignature {
                    parts: x.signature.into_iter().map(|s| s.0).collect(),
                },
                class_hash: Hash(x.class_hash.0),
                nonce: x.nonce.0,
            }),
            DeclareV2(x) => proto::Transaction::DeclareV2(proto::DeclareV2 {
                sender: Address(x.sender_address.0),
                max_fee: x.max_fee.0,
                signature: AccountSignature {
                    parts: x.signature.into_iter().map(|s| s.0).collect(),
                },
                class_hash: Hash(x.class_hash.0),
                nonce: x.nonce.0,
                compiled_class_hash: x.compiled_class_hash.0,
            }),
            DeclareV3(x) => proto::Transaction::DeclareV3(proto::DeclareV3 {
                sender: Address(x.sender_address.0),
                signature: AccountSignature {
                    parts: x.signature.into_iter().map(|s| s.0).collect(),
                },
                class_hash: Hash(x.class_hash.0),
                nonce: x.nonce.0,
                compiled_class_hash: x.compiled_class_hash.0,
                resource_bounds: ResourceBounds {
                    l1_gas: x.resource_bounds.l1_gas.to_proto(),
                    l2_gas: x.resource_bounds.l2_gas.to_proto(),
                },
                tip: x.tip.0.into(),
                paymaster_data: Address(
                    x.paymaster_data
                        .first()
                        .unwrap_or(&PaymasterDataElem::ZERO)
                        .0,
                ), // TODO
                account_deployment_data: Address(
                    x.account_deployment_data
                        .first()
                        .unwrap_or(&AccountDeploymentDataElem::ZERO)
                        .0,
                ), // TODO
                nonce_domain: x.nonce_data_availability_mode.to_proto(),
                fee_domain: x.fee_data_availability_mode.to_proto(),
            }),
            Deploy(x) => proto::Transaction::Deploy(proto::Deploy {
                class_hash: Hash(x.class_hash.0),
                address_salt: x.contract_address_salt.0,
                calldata: x.constructor_calldata.into_iter().map(|c| c.0).collect(),
                address: Address(x.contract_address.0),
                // Only these two values are allowed in storage
                version: if x.version.is_zero() { 0 } else { 1 },
            }),
            DeployAccountV0V1(x) => proto::Transaction::DeployAccountV1(proto::DeployAccountV1 {
                max_fee: x.max_fee.0,
                signature: AccountSignature {
                    parts: x.signature.into_iter().map(|s| s.0).collect(),
                },
                class_hash: Hash(x.class_hash.0),
                nonce: x.nonce.0,
                address_salt: x.contract_address_salt.0,
                constructor_calldata: x.constructor_calldata.into_iter().map(|c| c.0).collect(),
            }),
            DeployAccountV3(x) => proto::Transaction::DeployAccountV3(proto::DeployAccountV3 {
                signature: AccountSignature {
                    parts: x.signature.into_iter().map(|s| s.0).collect(),
                },
                class_hash: Hash(x.class_hash.0),
                nonce: x.nonce.0,
                address_salt: x.contract_address_salt.0,
                calldata: x.constructor_calldata.into_iter().map(|c| c.0).collect(),
                resource_bounds: ResourceBounds {
                    l1_gas: x.resource_bounds.l1_gas.to_proto(),
                    l2_gas: x.resource_bounds.l2_gas.to_proto(),
                },
                tip: x.tip.0.into(),
                paymaster_data: Address(
                    x.paymaster_data
                        .first()
                        .unwrap_or(&PaymasterDataElem::ZERO)
                        .0,
                ), // TODO
                nonce_domain: x.nonce_data_availability_mode.to_proto(),
                fee_domain: x.fee_data_availability_mode.to_proto(),
            }),
            InvokeV0(x) => proto::Transaction::InvokeV0(proto::InvokeV0 {
                max_fee: x.max_fee.0,
                signature: AccountSignature {
                    parts: x.signature.into_iter().map(|s| s.0).collect(),
                },
                address: Address(x.sender_address.0),
                entry_point_selector: x.entry_point_selector.0,
                calldata: x.calldata.into_iter().map(|c| c.0).collect(),
            }),
            InvokeV1(x) => proto::Transaction::InvokeV1(proto::InvokeV1 {
                sender: Address(x.sender_address.0),
                max_fee: x.max_fee.0,
                signature: AccountSignature {
                    parts: x.signature.into_iter().map(|s| s.0).collect(),
                },
                nonce: x.nonce.0,
                calldata: x.calldata.into_iter().map(|c| c.0).collect(),
            }),
            InvokeV3(x) => proto::Transaction::InvokeV3(proto::InvokeV3 {
                sender: Address(x.sender_address.0),
                signature: AccountSignature {
                    parts: x.signature.into_iter().map(|s| s.0).collect(),
                },
                calldata: x.calldata.into_iter().map(|c| c.0).collect(),
                resource_bounds: ResourceBounds {
                    l1_gas: x.resource_bounds.l1_gas.to_proto(),
                    l2_gas: x.resource_bounds.l2_gas.to_proto(),
                },
                tip: x.tip.0.into(),
                paymaster_data: Address(
                    x.paymaster_data
                        .first()
                        .unwrap_or(&PaymasterDataElem::ZERO)
                        .0,
                ), // TODO
                account_deployment_data: Address(
                    x.account_deployment_data
                        .first()
                        .unwrap_or(&AccountDeploymentDataElem::ZERO)
                        .0,
                ), // TODO
                nonce_domain: x.nonce_data_availability_mode.to_proto(),
                fee_domain: x.fee_data_availability_mode.to_proto(),
                nonce: x.nonce.0,
            }),
            L1Handler(x) => proto::Transaction::L1HandlerV0(proto::L1HandlerV0 {
                nonce: x.nonce.0,
                address: Address(x.contract_address.0),
                entry_point_selector: x.entry_point_selector.0,
                calldata: x.calldata.into_iter().map(|c| c.0).collect(),
            }),
        }
    }
}

impl ToProto<p2p_proto::receipt::Receipt> for (CommonTransaction, CommonReceipt) {
    fn to_proto(self) -> p2p_proto::receipt::Receipt {
        use p2p_proto::receipt::Receipt::{Declare, Deploy, DeployAccount, Invoke, L1Handler};
        let revert_reason = self.1.revert_reason().unwrap_or_default();

        let common = ReceiptCommon {
            transaction_hash: Hash(self.1.transaction_hash.0),
            actual_fee: self.1.actual_fee.unwrap_or_default().0,
            messages_sent: self
                .1
                .l2_to_l1_messages
                .into_iter()
                .map(|m| MessageToL1 {
                    from_address: m.from_address.0,
                    payload: m.payload.into_iter().map(|p| p.0).collect(),
                    to_address: EthereumAddress(m.to_address.0),
                })
                .collect(),
            execution_resources: {
                let e = self.1.execution_resources.clone().unwrap_or_default();
                // Assumption: the values are small enough to fit into u32
                ExecutionResources {
                    builtins: BuiltinCounter {
                        bitwise: e
                            .builtin_instance_counter
                            .bitwise_builtin
                            .try_into()
                            .unwrap(),
                        ecdsa: e.builtin_instance_counter.ecdsa_builtin.try_into().unwrap(),
                        ec_op: e.builtin_instance_counter.ec_op_builtin.try_into().unwrap(),
                        pedersen: e
                            .builtin_instance_counter
                            .pedersen_builtin
                            .try_into()
                            .unwrap(),
                        range_check: e
                            .builtin_instance_counter
                            .range_check_builtin
                            .try_into()
                            .unwrap(),
                        poseidon: e
                            .builtin_instance_counter
                            .poseidon_builtin
                            .try_into()
                            .unwrap(),
                        keccak: e
                            .builtin_instance_counter
                            .keccak_builtin
                            .try_into()
                            .unwrap(),
                        output: e
                            .builtin_instance_counter
                            .output_builtin
                            .try_into()
                            .unwrap(),
                        segment_arena: e
                            .builtin_instance_counter
                            .segment_arena_builtin
                            .try_into()
                            .unwrap(),
                    },
                    steps: e.n_steps.try_into().unwrap(),
                    memory_holes: e.n_memory_holes.try_into().unwrap(),
                }
            },
            revert_reason,
        };

        use pathfinder_common::transaction::TransactionVariant;
        match self.0.variant {
            TransactionVariant::DeclareV0(_)
            | TransactionVariant::DeclareV1(_)
            | TransactionVariant::DeclareV2(_)
            | TransactionVariant::DeclareV3(_) => Declare(DeclareTransactionReceipt { common }),
            TransactionVariant::Deploy(x) => Deploy(DeployTransactionReceipt {
                common,
                contract_address: x.contract_address.0,
            }),
            TransactionVariant::DeployAccountV0V1(x) => {
                DeployAccount(DeployAccountTransactionReceipt {
                    common,
                    contract_address: x.contract_address.0,
                })
            }
            TransactionVariant::DeployAccountV3(x) => {
                DeployAccount(DeployAccountTransactionReceipt {
                    common,
                    contract_address: x.contract_address.0,
                })
            }
            TransactionVariant::InvokeV0(_)
            | TransactionVariant::InvokeV1(_)
            | TransactionVariant::InvokeV3(_) => Invoke(InvokeTransactionReceipt { common }),
            TransactionVariant::L1Handler(_) => L1Handler(L1HandlerTransactionReceipt {
                common,
                msg_hash: Hash(Felt::ZERO), // TODO what is this
            }),
        }
    }
}

impl ToProto<p2p_proto::event::Event> for (TransactionHash, Event) {
    fn to_proto(self) -> p2p_proto::event::Event {
        p2p_proto::event::Event {
            transaction_hash: Hash(self.0 .0),
            from_address: self.1.from_address.0,
            keys: self.1.keys.into_iter().map(|k| k.0).collect(),
            data: self.1.data.into_iter().map(|d| d.0).collect(),
        }
    }
}

impl ToProto<p2p_proto::transaction::ResourceLimits> for ResourceBound {
    fn to_proto(self) -> p2p_proto::transaction::ResourceLimits {
        p2p_proto::transaction::ResourceLimits {
            max_amount: self.max_amount.0.into(),
            max_price_per_unit: self.max_price_per_unit.0.into(),
        }
    }
}

impl ToProto<String> for DataAvailabilityMode {
    fn to_proto(self) -> String {
        match self {
            DataAvailabilityMode::L1 => "L1".to_owned(),
            DataAvailabilityMode::L2 => "L2".to_owned(),
        }
    }
}
