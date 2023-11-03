use std::{fs, path::Path};

use runner::{execute_test_suite, run_evm, ResultWithTrace};

pub mod inspector;
pub mod runner;
use clap::Parser;
use serde_json::json;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// A hex string representing a contract runtime binary code
    #[clap(long)]
    #[clap(long, conflicts_with = "test_json")]
    bytecode: Option<String>,

    /// An optional hex encoded transaction data
    #[clap(long)]
    #[clap(long, conflicts_with = "test_json")]
    input: Option<String>,

    /// If provided, print stack traces to stdout
    #[clap(long)]
    pprint: bool,

    /// If provided, output as JSON to this file
    #[clap(long)]
    output: Option<String>,

    /// If provided, use the ethtest JSON file the input
    #[clap(long)]
    test_json: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    let code = cli.bytecode;
    let input = cli.input;

    let results = {
        if let Some(test_json) = cli.test_json {
            let path = Path::new(&test_json);
            execute_test_suite(&path).unwrap()
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
        if cli.pprint {
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

    if let Some(output_path) = &cli.output {
        let output_data = json!({"results": results});
        fs::write(output_path, output_data.to_string()).expect("Unable to write to file");
    }
}
