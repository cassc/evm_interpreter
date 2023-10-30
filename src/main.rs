use ethers_solc::Solc;
use revm::{
    interpreter::{Contract, DummyHost, Interpreter},
    primitives::{hex::FromHex, keccak256, BerlinSpec, Bytecode, Bytes, B256, U256},
    InMemoryDB,
};

use ethers::prelude::Abigen;
use eyre::Result;

use revm::primitives::alloy_primitives::address;

fn run() -> Result<()> {
    let mut args = std::env::args();
    args.next().unwrap(); // skip program name
    let contract_name = args.next().unwrap();
    let contract: String = args.next().unwrap();

    let contracts = Solc::default().compile_source(&contract)?;

    // contract code, jump table, etc.
    let contract = contracts.get(&contract, &contract_name).unwrap();
    let bytecode = &contract.bytecode().unwrap().0;

    let bytecode = Bytecode::new_raw(Bytes(bytecode.to_owned()));

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
    let contract = Contract::new(input, bytecode, hash, address, caller, value);
    let gas_limit = u64::MAX;
    let is_static = false;

    let mut evm = revm::new();
    evm.database(InMemoryDB::default());

    evm.env.tx.caller = caller;
    evm.env.tx.transact_to = address;
    evm.env.tx.data = bytecode;

    let mut host = DummyHost::new(evm.env.clone());
    let instruction_table = make_instruction_table::<DummyHost, BerlinSpec>();

    let mut interpreter = Interpreter::new(contract.into(), gas_limit, is_static);

    let r = interpreter.run(&mut host);

    println!("r: {:?}", r);

    Ok(())
}

fn make_instruction_table() -> _ {
    todo!()
}

fn main() {
    run().unwrap();
}
