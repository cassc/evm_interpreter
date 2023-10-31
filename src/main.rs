use ethers::utils::hex;
use ethers_solc::{artifacts::Source, CompilerInput, Solc};
use revm::{
    interpreter::{Contract, DummyHost, Interpreter},
    primitives::{keccak256, Bytecode, Bytes, ShanghaiSpec, TransactTo, U256},
    InMemoryDB,
};

use eyre::Result;

use revm::primitives::alloy_primitives::address;

fn run() -> Result<()> {
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

    let compile_result = solc.compile_source(&contract)?; // TODO abiv2 directive is broken
    if compile_result.has_error() {
        println!("Compile {} failed: {:?}", &contract, compile_result);
        return Ok(());
    }

    // contract code, jump table, etc.
    let contract = compile_result.get(&contract, &contract_name).unwrap();
    let bytes = &contract.bytecode().unwrap().0;

    let bytecode = Bytecode::new_raw(Bytes(bytes.to_owned()));

    // contract data
    let input = Bytes::new();

    // contract address
    let address = address!("d8da6bf26964af9d7eed9e03e53415d37aa96045");
    // caller of the EVM, Caller is zero if it's a contract creation transaction
    let caller = address!("7484a096D45F3D28DDCbf3CC03142804B55da957");
    // value sent to the contract
    let value = U256::from(0);
    // hash of the bytecode
    let hash = keccak256(bytecode.bytes());
    let contract = Contract::new(input, bytecode.clone(), hash, address, caller, value);
    let gas_limit = u64::MAX;
    let is_static = false;

    let mut evm = revm::new();
    evm.database(InMemoryDB::default());

    evm.env.tx.caller = caller;
    evm.env.tx.transact_to = TransactTo::Call(address);
    let _sig = "changeSomething(int256)"; // Todo add signautre calculation from rust
    evm.env.tx.data =
        hex::decode("0x27f12a5f0000000000000000000000000000000000000000000000000000000000000002")?
            .into();

    let mut host = DummyHost::new(evm.env.clone());

    let mut interpreter = Interpreter::new(contract.into(), gas_limit, is_static);
    println!("program counter: {:?}", interpreter.program_counter());
    let result = interpreter.run::<DummyHost, ShanghaiSpec>(&mut host);
    let return_value = interpreter.return_value();
    println!("result: {:?}", result);
    println!("return len: {:?}", interpreter.return_len);
    println!("return_value: {:?}", return_value);
    println!("program counter: {:?}", interpreter.program_counter());

    assert!(result.is_ok());

    Ok(())
}

fn main() {
    run().unwrap();
}
