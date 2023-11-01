use std::fs;

use runner::run_evm;

pub mod inspector;
pub mod runner;
use clap::Parser;
use serde_json::json;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// A hex string representing the binary input
    #[clap(long)]
    bytecode: String,

    /// An optional encoded argument as a hex string
    #[clap(long)]
    input: Option<String>,

    /// If provided, print to stdout
    #[clap(long)]
    pprint: bool,

    /// If provided, output as JSON to this file
    #[clap(long)]
    output: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    let code = cli.bytecode;
    let input = cli.input;

    match run_evm(code, input) {
        Ok((success, output, traces)) => {
            if cli.pprint {
                traces.iter().for_each(|trace| {
                    trace.pprint();
                });
            }

            println!("{} OUTPUT: {}", if success { "✅" } else { "❌" }, output);

            if let Some(output_path) = &cli.output {
                let output_data = json!({
                    "success": success,
                    "traces": traces,
                    "output": output,
                });
                fs::write(output_path, output_data.to_string()).expect("Unable to write to file");
            }
        }
        Err(e) => panic!("Error: {:?}", e),
    }
}
