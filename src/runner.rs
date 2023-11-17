use std::path::{Path, PathBuf};

use ethers::utils::hex;
use rand::{seq::SliceRandom, thread_rng};
use revm::{
    interpreter::{
        analysis::to_analysed, opcode::make_instruction_table, Contract, DummyHost,
        InstructionResult, Interpreter, SharedMemory, OPCODE_JUMPMAP,
    },
    primitives::{
        b256, calc_excess_blob_gas, keccak256, AccountInfo, Bytecode, Bytes, CreateScheme, Env,
        HashMap, ShanghaiSpec, SpecId, TransactTo, U256,
    },
    InMemoryDB,
};

use eyre::{ContextCompat, Result};

use revm::primitives::alloy_primitives::address;
use revme::cmd::statetest::models::{SpecName, TestSuite};
use serde::Serialize;
use walkdir::{DirEntry, WalkDir};

use crate::inspector::{Trace, TraceInspector};

#[derive(Debug, Serialize)]
pub struct ResultWithTrace {
    pub id: String,
    pub success: bool,
    pub output: Bytes,
    pub traces: Vec<Trace>,
}

/// Run EVM with an empty initial state plus runtime contract code and optinal input tx data
pub fn run_evm(runtime_code: String, input: Option<String>) -> Result<Vec<ResultWithTrace>> {
    let id = runtime_code[..6].into();
    let bytes = hex::decode(runtime_code)?;
    let bytes = Bytes::from(bytes.as_slice().to_vec());
    let runtime_bytecode = Bytecode::new_raw(bytes.clone());

    // contract address, randomly assigned here
    let address = address!("d8da6bf26964af9d7eed9e03e53415d37aa96045");
    // caller of the EVM, Caller is zero if it's a contract creation transaction
    let caller = address!("7484a096D45F3D28DDCbf3CC03142804B55da957");
    // value sent to the contract
    let value = U256::from(0);
    // hash of the bytecode
    let gas_limit = u64::MAX;

    let mut evm = revm::new();
    let db = {
        let mut db = InMemoryDB::default();
        let code_hash = keccak256(runtime_bytecode.bytes());
        let contract_account = AccountInfo {
            balance: U256::from(U256::MAX),
            code_hash,
            code: Some(runtime_bytecode),
            nonce: 0,
        };
        db.insert_account_info(address, contract_account);
        db.insert_account_info(caller, AccountInfo::default());
        db
    };

    // optional env configuration
    evm.env.cfg.spec_id = SpecId::LATEST;
    evm.env.cfg.chain_id = 1;

    evm.database(db);
    evm.env.tx.caller = caller;
    evm.env.tx.value = value;
    evm.env.tx.gas_limit = gas_limit;
    evm.env.tx.transact_to = TransactTo::Call(address);
    evm.env.tx.data = if let Some(input) = input {
        hex::decode(input)?.into()
    } else {
        Bytes::new()
    };

    println!("Input: {:?}", evm.env.tx.data);

    let mut traces = vec![];
    let inspector = TraceInspector {
        traces: &mut traces,
    };

    let res = evm.inspect_commit(inspector).expect("Execution failed");

    let output = {
        if let Some(o) = res.output() {
            o.to_owned()
        } else {
            Bytes::new()
        }
    };

    let success = res.is_success();

    let results = vec![ResultWithTrace {
        id,
        success,
        output,
        traces: traces.to_owned(),
    }];
    Ok(results)
}

/// Run binary directly on the EVM interpreter
pub fn run_interpreter(data: String) -> Result<()> {
    let bytes = hex::decode(data)?;
    let bytes = Bytes::from(bytes.as_slice().to_vec());
    let contract_bytecode = Bytecode::new_raw(bytes.clone());

    // contract address
    let address = address!("d8da6bf26964af9d7eed9e03e53415d37aa96045");
    // caller of the EVM, Caller is zero if it's a contract creation transaction
    let caller = address!("7484a096D45F3D28DDCbf3CC03142804B55da957");
    // value sent to the contract
    let value = U256::from(0);
    // hash of the bytecode

    let analyzed_code = to_analysed(contract_bytecode.clone());

    let hash_of_analyzed_code = keccak256(analyzed_code.bytes());
    let contract = Contract::new(
        Bytes::from(bytes.to_vec()), // contract data
        analyzed_code,
        hash_of_analyzed_code,
        address,
        caller,
        value,
    );
    let gas_limit = u64::MAX;
    let is_static = false;

    // Run the interpreter does not work.
    let env = Env::default();
    let mut host = DummyHost::new(env);
    let mut memory = SharedMemory::new();
    let mut interpreter = Interpreter::new(contract.into(), gas_limit, is_static, &mut memory);
    let mut instruction_table = make_instruction_table::<DummyHost, ShanghaiSpec>();
    // Pass tx data to interpreter
    let result = {
        while interpreter.instruction_result == InstructionResult::Continue {
            let opcode = interpreter.current_opcode();
            println!(
                "➡️ PC: {} OPCODE: 0x{:02x} {}",
                interpreter.program_counter(),
                opcode,
                OPCODE_JUMPMAP.get(opcode as usize).unwrap().unwrap(),
            );
            println!("STACK{}", interpreter.stack);
            interpreter.step::<_, _>(&mut instruction_table, &mut host);
        }
        interpreter.instruction_result
    };
    let return_value = interpreter.return_value();

    println!(
        "\nResult {}: {:?}",
        if result.is_ok() { "✅" } else { "❌" },
        result
    );
    println!("return len: {:?}", interpreter.return_len);
    println!("return_value: {:?}", return_value);

    Ok(())
}

/// Load a ethtest test suite json and execute
pub fn execute_test_suite(path: &Path, limit: usize) -> Result<Vec<ResultWithTrace>> {
    let mut results = vec![];
    if path.is_dir() {
        let mut json_files: Vec<PathBuf> = {
            WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
                .map(DirEntry::into_path)
                .collect()
        };

        let mut rng = thread_rng();
        json_files.shuffle(&mut rng);
        let json_files: Vec<PathBuf> = json_files.iter().take(limit).cloned().collect();

        for path in json_files {
            let mut r = execute_test_suite(&path, 1)?;
            results.append(&mut r);
        }
        return Ok(results);
    }

    println!("Processing {}", path.display());

    let s = std::fs::read_to_string(path)?;
    let suite: TestSuite = serde_json::from_str(&s)?;

    let map_caller_keys: HashMap<_, _> = [
        (
            b256!("45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8"),
            address!("a94f5374fce5edbc8e2a8697c15331677e6ebf0b"),
        ),
        (
            b256!("c85ef7d79691fe79573b1a7064c19c1a9819ebdbd1faaab1a8ec92344438aaf4"),
            address!("cd2a3d9f938e13cd947ec05abc7fe734df8dd826"),
        ),
        (
            b256!("044852b2a670ade5407e78fb2863c51de9fcb96542a07186fe3aeda6bb8a116d"),
            address!("82a978b3f5962a5b0957d9ee9eef472ee55b42f1"),
        ),
        (
            b256!("6a7eeac5f12b409d42028f66b0b2132535ee158cfda439e3bfdd4558e8f4bf6c"),
            address!("c9c5a15a403e41498b6f69f6f89dd9f5892d21f7"),
        ),
        (
            b256!("a95defe70ebea7804f9c3be42d20d24375e2a92b9d9666b832069c5f3cd423dd"),
            address!("3fb1cd2cd96c6d5c0b5eb3322d807b34482481d4"),
        ),
        (
            b256!("fe13266ff57000135fb9aa854bbfe455d8da85b21f626307bf3263a0c2a8e7fe"),
            address!("dcc5ba93a1ed7e045690d722f2bf460a51c61415"),
        ),
    ]
    .into();

    for (_name, unit) in suite.0 {
        // Create database and insert cache
        let mut cache_state = revm::CacheState::new(false);
        for (address, info) in unit.pre {
            let acc_info = revm::primitives::AccountInfo {
                balance: info.balance,
                code_hash: keccak256(&info.code),
                code: Some(Bytecode::new_raw(info.code)),
                nonce: info.nonce,
            };
            cache_state.insert_account_with_storage(address, acc_info, info.storage);
        }

        let mut env = Env::default();
        // for mainnet
        env.cfg.chain_id = 1;
        // env.cfg.spec_id is set down the road

        // block env
        env.block.number = unit.env.current_number;
        env.block.coinbase = unit.env.current_coinbase;
        env.block.timestamp = unit.env.current_timestamp;
        env.block.gas_limit = unit.env.current_gas_limit;
        env.block.basefee = unit.env.current_base_fee.unwrap_or_default();
        env.block.difficulty = unit.env.current_difficulty;
        // after the Merge prevrandao replaces mix_hash field in block and replaced difficulty opcode in EVM.
        env.block.prevrandao = Some(unit.env.current_difficulty.to_be_bytes().into());
        // EIP-4844
        if let (Some(parent_blob_gas_used), Some(parent_excess_blob_gas)) = (
            unit.env.parent_blob_gas_used,
            unit.env.parent_excess_blob_gas,
        ) {
            env.block
                .set_blob_excess_gas_and_price(calc_excess_blob_gas(
                    parent_blob_gas_used.to(),
                    parent_excess_blob_gas.to(),
                ));
        }

        // tx env
        let pk = unit.transaction.secret_key;
        env.tx.caller = map_caller_keys
            .get(&pk)
            .copied()
            .context("unknown caller private key")?;
        env.tx.gas_price = unit
            .transaction
            .gas_price
            .or(unit.transaction.max_fee_per_gas)
            .unwrap_or_default();
        env.tx.gas_priority_fee = unit.transaction.max_priority_fee_per_gas;
        // EIP-4844
        env.tx.blob_hashes = unit.transaction.blob_versioned_hashes;
        env.tx.max_fee_per_blob_gas = unit.transaction.max_fee_per_blob_gas;

        // post and execution
        for (spec_name, tests) in unit.post {
            if matches!(
                spec_name,
                SpecName::ByzantiumToConstantinopleAt5
                    | SpecName::Constantinople
                    | SpecName::Unknown
            ) {
                continue;
            }

            env.cfg.spec_id = spec_name.to_spec_id();

            for (index, test) in tests.into_iter().enumerate() {
                env.tx.gas_limit = unit.transaction.gas_limit[test.indexes.gas].saturating_to();

                env.tx.data = unit
                    .transaction
                    .data
                    .get(test.indexes.data)
                    .unwrap()
                    .clone();
                env.tx.value = unit.transaction.value[test.indexes.value];

                env.tx.access_list = unit
                    .transaction
                    .access_lists
                    .get(test.indexes.data)
                    .and_then(Option::as_deref)
                    .unwrap_or_default()
                    .iter()
                    .map(|item| {
                        (
                            item.address,
                            item.storage_keys
                                .iter()
                                .map(|key| U256::from_be_bytes(key.0))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect();

                let to = match unit.transaction.to {
                    Some(add) => TransactTo::Call(add),
                    None => TransactTo::Create(CreateScheme::Create),
                };
                env.tx.transact_to = to;

                let mut cache = cache_state.clone();
                cache.set_state_clear_flag(SpecId::enabled(
                    env.cfg.spec_id,
                    revm::primitives::SpecId::SPURIOUS_DRAGON,
                ));
                let mut state = revm::db::State::builder()
                    .with_cached_prestate(cache)
                    .with_bundle_update()
                    .build();
                let mut evm = revm::new();
                evm.database(&mut state);
                evm.env = env.clone();

                let mut traces = vec![];
                let inspector = TraceInspector {
                    traces: &mut traces,
                };

                let result = evm.inspect_commit(inspector);
                let mut success = false;
                let mut output = Bytes::new();
                if let Ok(result) = result {
                    success = result.is_success();
                    if let Some(o) = result.output() {
                        output = o.to_owned();
                    }
                }

                let traces = traces.to_owned();
                results.push(ResultWithTrace {
                    id: format!("{}_{:?}_{}", path.display(), spec_name, index),
                    success,
                    output,
                    traces,
                });

                // NOTE Test cases ignored
            }
        }
    }
    Ok(results)
}

/// Load a ethtest test suite json and execute
pub fn compare_test_suite(
    alt_evm_path: String,
    test_json_path: &Path,
    limit: usize,
) -> Result<Vec<ResultWithTrace>> {
    let mut results = vec![];
    if test_json_path.is_dir() {
        let mut json_files: Vec<PathBuf> = {
            WalkDir::new(test_json_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
                .map(DirEntry::into_path)
                .collect()
        };

        let mut rng = thread_rng();
        json_files.shuffle(&mut rng);
        let json_files: Vec<PathBuf> = json_files.iter().take(limit).cloned().collect();

        for path in json_files {
            let mut r = compare_test_suite(alt_evm_path.clone(), &path, 1)?;
            results.append(&mut r);
        }
        return Ok(results);
    }

    println!("Processing {}", test_json_path.display());

    let s = std::fs::read_to_string(test_json_path)?;
    let suite: TestSuite = serde_json::from_str(&s)?;

    let map_caller_keys: HashMap<_, _> = [
        (
            b256!("45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8"),
            address!("a94f5374fce5edbc8e2a8697c15331677e6ebf0b"),
        ),
        (
            b256!("c85ef7d79691fe79573b1a7064c19c1a9819ebdbd1faaab1a8ec92344438aaf4"),
            address!("cd2a3d9f938e13cd947ec05abc7fe734df8dd826"),
        ),
        (
            b256!("044852b2a670ade5407e78fb2863c51de9fcb96542a07186fe3aeda6bb8a116d"),
            address!("82a978b3f5962a5b0957d9ee9eef472ee55b42f1"),
        ),
        (
            b256!("6a7eeac5f12b409d42028f66b0b2132535ee158cfda439e3bfdd4558e8f4bf6c"),
            address!("c9c5a15a403e41498b6f69f6f89dd9f5892d21f7"),
        ),
        (
            b256!("a95defe70ebea7804f9c3be42d20d24375e2a92b9d9666b832069c5f3cd423dd"),
            address!("3fb1cd2cd96c6d5c0b5eb3322d807b34482481d4"),
        ),
        (
            b256!("fe13266ff57000135fb9aa854bbfe455d8da85b21f626307bf3263a0c2a8e7fe"),
            address!("dcc5ba93a1ed7e045690d722f2bf460a51c61415"),
        ),
    ]
    .into();

    for (_name, unit) in suite.0 {
        // Create database and insert cache
        let mut cache_state = revm::CacheState::new(false);
        for (address, info) in unit.pre {
            let acc_info = revm::primitives::AccountInfo {
                balance: info.balance,
                code_hash: keccak256(&info.code),
                code: Some(Bytecode::new_raw(info.code)),
                nonce: info.nonce,
            };
            cache_state.insert_account_with_storage(address, acc_info, info.storage);
        }

        let mut env = Env::default();
        // for mainnet
        env.cfg.chain_id = 1;
        // env.cfg.spec_id is set down the road

        // block env
        env.block.number = unit.env.current_number;
        env.block.coinbase = unit.env.current_coinbase;
        env.block.timestamp = unit.env.current_timestamp;
        env.block.gas_limit = unit.env.current_gas_limit;
        env.block.basefee = unit.env.current_base_fee.unwrap_or_default();
        env.block.difficulty = unit.env.current_difficulty;
        // after the Merge prevrandao replaces mix_hash field in block and replaced difficulty opcode in EVM.
        env.block.prevrandao = Some(unit.env.current_difficulty.to_be_bytes().into());
        // EIP-4844
        if let (Some(parent_blob_gas_used), Some(parent_excess_blob_gas)) = (
            unit.env.parent_blob_gas_used,
            unit.env.parent_excess_blob_gas,
        ) {
            env.block
                .set_blob_excess_gas_and_price(calc_excess_blob_gas(
                    parent_blob_gas_used.to(),
                    parent_excess_blob_gas.to(),
                ));
        }

        // tx env
        let pk = unit.transaction.secret_key;
        env.tx.caller = map_caller_keys
            .get(&pk)
            .copied()
            .context("unknown caller private key")?;
        env.tx.gas_price = unit
            .transaction
            .gas_price
            .or(unit.transaction.max_fee_per_gas)
            .unwrap_or_default();
        env.tx.gas_priority_fee = unit.transaction.max_priority_fee_per_gas;
        // EIP-4844
        env.tx.blob_hashes = unit.transaction.blob_versioned_hashes;
        env.tx.max_fee_per_blob_gas = unit.transaction.max_fee_per_blob_gas;

        // post and execution
        for (spec_name, tests) in unit.post {
            if matches!(
                spec_name,
                SpecName::ByzantiumToConstantinopleAt5
                    | SpecName::Constantinople
                    | SpecName::Unknown
            ) {
                continue;
            }

            env.cfg.spec_id = spec_name.to_spec_id();

            for (index, test) in tests.into_iter().enumerate() {
                env.tx.gas_limit = unit.transaction.gas_limit[test.indexes.gas].saturating_to();

                env.tx.data = unit
                    .transaction
                    .data
                    .get(test.indexes.data)
                    .unwrap()
                    .clone();
                env.tx.value = unit.transaction.value[test.indexes.value];

                env.tx.access_list = unit
                    .transaction
                    .access_lists
                    .get(test.indexes.data)
                    .and_then(Option::as_deref)
                    .unwrap_or_default()
                    .iter()
                    .map(|item| {
                        (
                            item.address,
                            item.storage_keys
                                .iter()
                                .map(|key| U256::from_be_bytes(key.0))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect();

                let to = match unit.transaction.to {
                    Some(add) => TransactTo::Call(add),
                    None => TransactTo::Create(CreateScheme::Create),
                };
                env.tx.transact_to = to;

                let mut cache = cache_state.clone();
                cache.set_state_clear_flag(SpecId::enabled(
                    env.cfg.spec_id,
                    revm::primitives::SpecId::SPURIOUS_DRAGON,
                ));
                let mut state = revm::db::State::builder()
                    .with_cached_prestate(cache)
                    .with_bundle_update()
                    .build();
                let mut evm = revm::new();
                evm.database(&mut state);
                evm.env = env.clone();

                let mut traces = vec![];
                let inspector = TraceInspector {
                    traces: &mut traces,
                };

                let result = evm.inspect_commit(inspector);
                let mut success = false;
                let mut output = Bytes::new();
                if let Ok(result) = result {
                    success = result.is_success();
                    if let Some(o) = result.output() {
                        output = o.to_owned();
                    }
                }

                let traces = traces.to_owned();
                results.push(ResultWithTrace {
                    id: format!("{}_{:?}_{}", test_json_path.display(), spec_name, index),
                    success,
                    output,
                    traces,
                });

                // NOTE Test cases ignored
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn test_interpreter() -> Result<()> {
        let data = "0x604260005260206000F3".into();
        run_interpreter(data)
    }

    #[test]
    pub fn test_evm() -> Result<()> {
        let runtime_code = "608060405234801561001057600080fd5b50600436106100415760003560e01c8063623845d81461004657806368be1b1e14610076578063cb12b48f14610094575b600080fd5b610060600480360381019061005b9190610138565b6100b2565b60405161006d919061017e565b60405180910390f35b61007e6100d1565b60405161008b919061017e565b60405180910390f35b61009c6100d7565b6040516100a991906101da565b60405180910390f35b60006001826100c19190610224565b6000819055506000549050919050565b60005481565b600160009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1681565b600080fd5b6000819050919050565b61011581610102565b811461012057600080fd5b50565b6000813590506101328161010c565b92915050565b60006020828403121561014e5761014d6100fd565b5b600061015c84828501610123565b91505092915050565b6000819050919050565b61017881610165565b82525050565b6000602082019050610193600083018461016f565b92915050565b600073ffffffffffffffffffffffffffffffffffffffff82169050919050565b60006101c482610199565b9050919050565b6101d4816101b9565b82525050565b60006020820190506101ef60008301846101cb565b92915050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601160045260246000fd5b600061022f82610165565b915061023a83610165565b9250828201905080821115610252576102516101f5565b5b9291505056fea2646970667358221220d2e53dc08c5d4c470a2efb27e2e6c22f84e94ef38557f093b0501a82c3d4c57564736f6c63430008120033".into() ;
        let input = Some("0x68be1b1e".into());
        let res = &run_evm(runtime_code, input)?[0];
        // use ethers::utils::hex::FromHex;
        // assert_eq!(Bytes::from_hex("0x03"), Ok(res.output)); // this will fail because we haven't set contract runtime code directly without initializing it's states through constructor call
        assert!(res.success);
        Ok(())
    }

    #[test]
    pub fn test_suite_single_json() -> Result<()> {
        let path = Path::new("dev-resources/ethtest/arith.json");
        let results = execute_test_suite(&path, 1)?;
        results.iter().for_each(|r| assert!(r.success));

        Ok(())
    }

    #[test]
    pub fn test_suite_folder() -> Result<()> {
        let path = Path::new("dev-resources/ethtest");
        let _results = execute_test_suite(&path, 10)?;
        Ok(())
    }
}
