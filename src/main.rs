use cuevm_test_suite::CuEvmTestSuite;
use eyre::Result;
use inspector::TraceInspector;
use revm::primitives::{
    calc_excess_blob_gas, keccak256, Bytecode, Bytes, CreateScheme, Env, TransactTo, U256,
};
use runner::{execute_test_suite, run_evm, ResultWithTrace};
use std::{fs, path::Path, process::Command};
use tempfile::Builder;

pub mod cuevm_test_suite;
pub mod inspector;
pub mod runner;
use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Executes the EVM interpreter with provided options
    Execute {
        /// A hex string representing a contract runtime binary code
        #[clap(long, conflicts_with = "test_json")]
        bytecode: Option<String>,

        /// An optional hex encoded transaction data
        #[clap(long, conflicts_with = "test_json")]
        input: Option<String>,

        /// If provided, print stack traces to stdout
        #[clap(long)]
        pprint: bool,

        /// If provided, output as JSON to this file
        #[clap(long)]
        output: Option<String>,

        /// If provided, use the ethtest JSON file as the input
        #[clap(long)]
        test_json: Option<String>,

        /// Maximum number of test files to run, valid when using with --test-json
        #[clap(long, default_value_t = 10)]
        limit: usize,
    },
    /// Compares the output of two EVM interpreters
    Compare {
        /// Path to another EVM interpreter executable
        #[clap(long)]
        executable: String,

        /// A path which contains ethtest JSON files
        #[clap(long)]
        test_json: String,

        /// If provided, print stack traces to stdout
        #[clap(long)]
        pprint: bool,

        /// Maximum number of test files to run, valid when using with --test-json
        #[clap(long, default_value_t = 10)]
        limit: usize,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Execute {
            bytecode,
            input,
            pprint,
            output,
            test_json,
            limit,
        } => {
            // Logic for the 'execute' subcommand
            execute_evm(
                bytecode.clone(),
                input.clone(),
                *pprint,
                output.clone(),
                test_json.clone(),
                *limit,
            );
        }
        Commands::Compare {
            executable,
            test_json,
            pprint,
            limit,
        } => {
            // Logic for the 'compare' subcommand
            compare_interpreters(executable.into(), test_json.clone(), *pprint, *limit)
                .expect("Comparison failed");
        }
    }
}

fn execute_evm(
    bytecode: Option<String>,
    input: Option<String>,
    pprint: bool,
    output: Option<String>,
    test_json: Option<String>,
    limit: usize,
) {
    let code = bytecode;

    let results = {
        if let Some(test_json) = test_json {
            let path = Path::new(&test_json);
            execute_test_suite(path, limit).unwrap()
        } else {
            let code = code.expect("Contract code should be provided");
            run_evm(code, input).unwrap()
        }
    };

    for ResultWithTrace {
        id,
        success,
        output,
        traces,
    } in results.iter()
    {
        if pprint {
            traces.iter().for_each(|trace| {
                trace.pprint();
            });
        }

        println!(
            "ID: {} {} OUTPUT: {}",
            id,
            if *success { "✅" } else { "❌" },
            output
        );
        println!();
    }

    if let Some(output_path) = output {
        let output_data = json!({"results": results});
        fs::write(output_path, output_data.to_string()).expect("Unable to write to file");
    }
}

fn compare_interpreters(
    cuevm_executable: String,
    test_json: String,
    pprint: bool,
    limit: usize,
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

    let _ = Command::new(cuevm_executable)
        .args(["--input", &test_json])
        .args(["--output", &output_json])
        .output()?;

    println!("Using output from CuEVM {}", output_json);

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
            cache_state.insert_account_with_storage(address, acc_info, info.storage);
        }

        let mut env = Env::default();
        env.cfg.chain_id = 1;
        env.block.number = unit.env.current_number;
        env.block.coinbase = unit.env.current_coinbase;
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

        // tx env

        // Test in post
        for (index, test) in unit.post.into_iter().enumerate() {
            env.tx.gas_limit = test.msg.gas_limit.saturating_to();
            env.tx.caller = test.msg.sender;
            env.tx.gas_price = test.msg.gas_price.unwrap_or_default(); // Note some ethtest has max_fee_per_gas

            env.tx.data = test.msg.data.clone();
            env.tx.value = test.msg.value;

            let to = match test.msg.to {
                Some(add) => TransactTo::Call(add),
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

            traces.iter().enumerate().skip(1).for_each(|(idx, t)| {
                let revm_stack = t.stack.data().clone();
                let cuevm_stack = test.traces[idx - 1].stack.data.clone();
                compare_stack(idx, revm_stack, cuevm_stack).unwrap();
            });
        }
    }

    Ok(())
}

fn compare_stack(idx: usize, expected: Vec<U256>, actual: Vec<Bytes>) -> Result<()> {
    if expected.len() != actual.len() {
        eprintln!("Expected: {:?}", expected);
        eprintln!("Actual: {:?}", actual);
        Err(eyre::eyre!("Stack length mismatch at index {}", idx))?
    }
    let actual: Vec<U256> = actual
        .into_iter()
        .map(|x| {
            let mut padded_array = [0u8; 32];
            let xs = x.into_iter().collect::<Vec<u8>>();
            let len = xs.len();
            padded_array[32 - len..].copy_from_slice(&xs);
            U256::from_be_bytes(padded_array)
        })
        .collect();
    if actual != expected {
        eprintln!("Expected: {:?}", expected);
        eprintln!("Actual: {:?}", actual);
        Err(eyre::eyre!("Stack mismatch at index {}", idx))?
    }
    Ok(())
}
