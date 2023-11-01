use ethers::utils::hex;
use ethers_solc::{artifacts::Source, Solc};
use revm::{
    interpreter::{
        analysis::to_analysed, Contract, DummyHost, InstructionResult, Interpreter, OPCODE_JUMPMAP,
    },
    primitives::{
        keccak256, AccountInfo, Bytecode, Bytes, Env, ResultAndState, ShanghaiSpec, TransactTo,
        U256,
    },
    InMemoryDB,
};

use eyre::Result;

use revm::primitives::alloy_primitives::address;

/// Run `changeSomething(int256)` in the contract `A` and print the stack trace.
/// This function properly sets the stats, however it does not print the
/// stacktrace.
fn _run_evm() -> Result<()> {
    let mut args = std::env::args();
    args.next().unwrap(); // skip program name
    let contract_name = args.next().unwrap_or("A".to_string());
    let contract: String = args
        .next()
        .unwrap_or("dev-resources/sample.sol".to_string());

    let content = std::fs::read_to_string(&contract)?;
    let source = Source::new(content);
    let version = Solc::detect_version(&source)?;

    let solc = Solc::blocking_install(&version)?;

    let compile_result = solc.compile_source(&contract)?; // NOTE compilation for contracts with abiv2 directive is broken
    if compile_result.has_error() {
        println!("Compile {} failed: {:?}", &contract, compile_result);
        return Ok(());
    }

    let contract = compile_result.get(&contract, &contract_name).unwrap();
    // let bytes = &contract.bytecode().unwrap().0; // deployment binary
    let bytes = &contract.bin_runtime.unwrap().as_bytes().unwrap().0; // runtime binary

    let runtime_bytecode = Bytecode::new_raw(Bytes(bytes.to_owned()));

    // // Alternatively load the runtime bin from hex string
    // println!("runtime bytes: {}", hex::encode(bytes));
    // let bytes = hex::decode("run-time-bin-as-hex").unwrap();
    // let bytes = Bytes::from(bytes.as_slice().to_vec());
    // let bytecode = Bytecode::new_raw(bytes);

    // contract address, randomly assigned here
    let address = address!("d8da6bf26964af9d7eed9e03e53415d37aa96045");
    // caller of the EVM, Caller is zero if it's a contract creation transaction
    let caller = address!("7484a096D45F3D28DDCbf3CC03142804B55da957");
    // value sent to the contract
    let value = U256::from(0);
    // hash of the bytecode

    let analyzed_code = to_analysed(runtime_bytecode.clone());

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

    let mut evm = revm::new();
    let db = {
        let mut db = InMemoryDB::default();
        let contract_account = AccountInfo {
            balance: U256::from(0),
            code_hash: contract.hash,
            code: Some(contract.bytecode.clone().unlock()),
            nonce: 0,
        };
        db.insert_account_info(address, contract_account);
        db.insert_account_info(caller, AccountInfo::default());
        db
    };

    evm.database(db);
    evm.env.tx.caller = caller;
    evm.env.tx.value = value;
    evm.env.tx.gas_limit = gas_limit;
    evm.env.tx.transact_to = TransactTo::Call(address);
    let _sig = "changeSomething(int256)"; // Todo add signautre calculation from rust
    evm.env.tx.data =
        hex::decode("27f12a5f0000000000000000000000000000000000000000000000000000000000000017")?
            .into();

    let res = evm.transact();
    assert!(res.is_ok());

    if let Ok(ResultAndState { result, state: _ }) = res {
        println!("result: {:?}", result);
    }

    Ok(())
}

/// Run binary directly on EVM interpreter
fn run_interpreter(data: String) -> Result<()> {
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
    let mut interpreter = Interpreter::new(contract.into(), gas_limit, is_static);
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
            interpreter.step::<DummyHost, ShanghaiSpec>(&mut host);
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

fn main() {
    let mut args = std::env::args();
    args.next().unwrap(); // skip program name
    let data = args.next().expect("Missing input");
    run_interpreter(data).unwrap();
}
