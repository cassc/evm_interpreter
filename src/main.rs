use comparator::execute_and_compare;
use rand::{seq::SliceRandom, thread_rng};
use runner::{execute_test_suite, run_evm, ResultWithTrace};
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::{DirEntry, WalkDir};

pub mod comparator;
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

        /// Fail on the first mismatch
        #[clap(long)]
        failfast: bool,
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
            failfast,
        } => {
            // Logic for the 'compare' subcommand
            compare_evms(
                executable.into(),
                test_json.clone(),
                *pprint,
                *limit,
                *failfast,
            );
        }
    }
}

fn compare_evms(
    cuevm_executable: String,
    test_json: String,
    pprint: bool,
    limit: usize,
    failfast: bool,
) {
    let path = Path::new(&test_json);
    let results = {
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
            json_files.iter().take(limit).cloned().collect()
        } else {
            vec![Path::new(&test_json).to_path_buf()]
        }
    };

    results.into_iter().for_each(|json_file| {
        let r = execute_and_compare(
            cuevm_executable.clone(),
            json_file
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            pprint,
        );

        println!(
            "Comparing {} Passed? {}",
            json_file.display(),
            if let Err(e) = &r {
                format!("❌ {:?}", e)
            } else {
                "✅".to_string()
            }
        );

        if failfast && r.is_err() {
            panic!("Failfast is enabled, exiting");
        }
    });
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
