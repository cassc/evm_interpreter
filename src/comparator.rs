use eyre::Result;
use revm::primitives::{
    calc_excess_blob_gas, keccak256, Bytecode, Bytes, CreateScheme, Env, TransactTo, U256,
};
use std::{path::Path, process::Command};
use tempfile::Builder;

use crate::{cuevm_test_suite::CuEvmTestSuite, inspector::TraceInspector};

/// Compare the output of CuEVM with the output of revm. Panics if there is a
/// mismatch.
pub fn execute_and_compare(
    cuevm_executable: String,
    test_json: String,
    pprint: bool,
) -> Result<()> {
    let output_json = {
        let temp_dir = std::env::temp_dir();
        let file = Builder::new()
            .prefix("mytempfile_")
            .suffix(".json")
            .tempfile_in(temp_dir)?;
        file.into_temp_path()
            .canonicalize()?
            .as_path()
            .to_str()
            .unwrap()
            .to_string()
    };

    let out = Command::new(cuevm_executable)
        .args(["--input", &test_json])
        .args(["--output", &output_json])
        .output()?;

    if output_json.is_empty() {
        Err(eyre::eyre!(
            "Output json is empty, cuevm returns stdout: {:?}",
            out.stdout
        ))?;
    }

    println!("Input: {} CuEVM Output: {}", test_json, output_json);

    let path = Path::new(&output_json);
    let s = std::fs::read_to_string(path)?;
    let suite: CuEvmTestSuite = serde_json::from_str(&s)?;

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
            cache_state.insert_account_with_storage(address.into(), acc_info, info.storage);
        }

        let mut env = Env::default();
        env.cfg.chain_id = 1;
        env.block.number = unit.env.current_number;
        env.block.coinbase = unit.env.current_coinbase.into();
        env.block.timestamp = unit.env.current_timestamp;
        env.block.gas_limit = unit.env.current_gas_limit;
        env.block.basefee = unit.env.current_base_fee.unwrap_or_default();
        env.block.difficulty = unit.env.current_difficulty;
        env.block.prevrandao = Some(unit.env.current_difficulty.to_be_bytes().into());
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

        // Run test in post generated by cuevm
        for (index, test) in unit.post.into_iter().enumerate() {
            env.tx.gas_limit = test.msg.gas_limit.saturating_to();
            env.tx.caller = test.msg.sender.into();
            env.tx.gas_price = test.msg.gas_price.unwrap_or_default(); // Note some ethtest has max_fee_per_gas

            env.tx.data = test.msg.data.clone();
            env.tx.value = test.msg.value;

            let to = match test.msg.to {
                Some(add) => TransactTo::Call(add.into()),
                None => TransactTo::Create(CreateScheme::Create),
            };
            env.tx.transact_to = to;

            let cache = cache_state.clone();
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

            if pprint {
                traces.iter().for_each(|trace| {
                    trace.pprint();
                });
                println!(
                    "ID: {} {} OUTPUT: {}",
                    index,
                    if success { "✅" } else { "❌" },
                    output
                );
            }

            if traces.len() != test.traces.len() {
                eprintln!(
                    "WARN: {} Trace length mismatch, stack comparison result might be wrong. Length expected {}, actual: {}",
                    index,
                    traces.len(),
                    test.traces.len(),
                );
            }

            let traces_iter = traces.iter().enumerate().skip(1);

            for (idx, t) in traces_iter {
                let revm_stack = t.stack.data().clone();
                if idx >= test.traces.len() {
                    break;
                }
                let cuevm_stack = test.traces[idx - 1].stack.data.clone();
                compare_stack(&test_json, idx, revm_stack, cuevm_stack)?;
            }
        }
    }

    Ok(())
}

fn compare_stack(
    test_json: &str,
    idx: usize,
    expected: Vec<U256>,
    actual: Vec<Bytes>,
) -> Result<()> {
    macro_rules! err {
        () => {
            eprintln!("Expected: {:?}", &expected);
            eprintln!("Actual: {:?}", &actual);
            Err(eyre::eyre!(
                "Stack length mismatch at index {} from {}",
                idx,
                test_json
            ))?
        };
    }
    if expected.len() != actual.len() {
        err!();
    }
    let actual: Vec<U256> = actual
        .iter()
        .map(|x| {
            let mut padded_array = [0u8; 32];
            let xs = x.iter().cloned().collect::<Vec<u8>>();
            let len = xs.len();
            padded_array[32 - len..].copy_from_slice(&xs);
            U256::from_be_bytes(padded_array)
        })
        .collect();
    if actual != expected {
        err!();
    }
    Ok(())
}
