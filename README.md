# Evm Interpreter


## Requirements

- Rust
- Cargo

## Download

Prebuilt binaries are available for Ubuntu 22.04 at [release page](https://github.com/cassc/evm_interpreter/releases).

## Installation

```bash
cargo install --path .
```

## Usage

### Execute a transaction

```bash
❯ evm-interpreter execute -h
Usage: evm-interpreter execute [OPTIONS]

Options:
      --bytecode <BYTECODE>    A hex string representing a contract runtime binary code
      --input <INPUT>          An optional hex encoded transaction data
      --pprint                 If provided, print stack traces to stdout
      --output <OUTPUT>        If provided, output as JSON to this file
      --test-json <TEST_JSON>  If provided, use the ethtest JSON file as the input
      --limit <LIMIT>          Maximum number of test files to run, valid when using with --test-json [default: 10]
  -h, --help                   Print help

```

### Compare with CuEVM

```bash
❯ evm-interpreter compare --executable path_to_cuevm_interpreter --test-json dev-resources/ethtest/GeneralStateTests/VMTests/vmArithmeticTest/arith.json
```


## Examples

### Call contract with no input (zero length transaction)

``` bash
❯ evm-interpreter execute --bytecode 604260005260206000F3 --pprint
Input: Bytes(0x)
Output: Bytes(0x0000000000000000000000000000000000000000000000000000000000000042)
➡️ PC: 0     OPCODE: 0x60 PUSH1
  STACK: []
➡️ PC: 2     OPCODE: 0x60 PUSH1
  STACK: [66]
➡️ PC: 4     OPCODE: 0x52 MSTORE
  STACK: [66, 0]
➡️ PC: 5     OPCODE: 0x60 PUSH1
  STACK: []
➡️ PC: 7     OPCODE: 0x60 PUSH1
  STACK: [32]
➡️ PC: 9     OPCODE: 0xf3 RETURN
  STACK: [32, 0]
✅ OUTPUT: 0x0000000000000000000000000000000000000000000000000000000000000042
```

### Call contract with input

``` bash
❯ evm-interpreter --bytecode 608060405234801561001057600080fd5b506004361061004c5760003560e01c806327f12a5f1461005157806368be1b1e14610081578063cb12b48f1461009f578063e1c7392a146100bd575b600080fd5b61006b60048036038101906100669190610221565b6100c7565b6040516100789190610267565b60405180910390f35b6100896100e6565b6040516100969190610267565b60405180910390f35b6100a76100ec565b6040516100b491906102c3565b60405180910390f35b6100c5610112565b005b60006001826100d6919061030d565b6000819055506000549050919050565b60005481565b600160009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1681565b600073ffffffffffffffffffffffffffffffffffffffff16600160009054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16146101a3576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161019a906103c0565b60405180910390fd5b30600160006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff160217905550565b600080fd5b6000819050919050565b6101fe816101eb565b811461020957600080fd5b50565b60008135905061021b816101f5565b92915050565b600060208284031215610237576102366101e6565b5b60006102458482850161020c565b91505092915050565b6000819050919050565b6102618161024e565b82525050565b600060208201905061027c6000830184610258565b92915050565b600073ffffffffffffffffffffffffffffffffffffffff82169050919050565b60006102ad82610282565b9050919050565b6102bd816102a2565b82525050565b60006020820190506102d860008301846102b4565b92915050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052601160045260246000fd5b60006103188261024e565b91506103238361024e565b9250827fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff03821115610358576103576102de565b5b828201905092915050565b600082825260208201905092915050565b7f796f752063616e206f6e6c7920696e6974206f6e636500000000000000000000600082015250565b60006103aa601683610363565b91506103b582610374565b602082019050919050565b600060208201905081810360008301526103d98161039d565b905091905056fea2646970667358221220e8c90a7bd7886645284457ca078f5253948dfbab01d5e32fe6754103a272a72764736f6c634300080f0033 --input 0x27f12a5f0000000000000000000000000000000000000000000000000000000000000021 --pprint --output output.json

Input: Bytes(0x27f12a5f0000000000000000000000000000000000000000000000000000000000000021)
➡️ PC: 0     OPCODE: 0x60 PUSH1
  STACK: []
➡️ PC: 2     OPCODE: 0x60 PUSH1
  STACK: [128]
➡️ PC: 4     OPCODE: 0x52 MSTORE
  STACK: [128, 64]
➡️ PC: 5     OPCODE: 0x34 CALLVALUE
  STACK: []
➡️ PC: 6     OPCODE: 0x80 DUP1
  STACK: [0]
➡️ PC: 7     OPCODE: 0x15 ISZERO
  STACK: [0, 0]
...
...
➡️ PC: 126   OPCODE: 0x03 SUB
  STACK: [670116447, 128, 128, 160]
➡️ PC: 127   OPCODE: 0x90 SWAP1
  STACK: [670116447, 128, 32]
➡️ PC: 128   OPCODE: 0xf3 RETURN
  STACK: [670116447, 32, 128]
ID: 608060 ✅ OUTPUT: 0x0000000000000000000000000000000000000000000000000000000000000022
```

### Run ethtest json file(s)

``` bash
❯ evm-interpreter compare -h
Compares the output of two EVM interpreters

Usage: evm-interpreter compare [OPTIONS] --executable <EXECUTABLE> --test-json <TEST_JSON>

Options:
      --executable <EXECUTABLE>  Path to another EVM interpreter executable
      --test-json <TEST_JSON>    A path which contains ethtest JSON files
      --pprint                   If provided, print stack traces to stdout
      --limit <LIMIT>            Maximum number of test files to run, valid when using with --test-json [default: 10]
      --failfast                 Fail on the first mismatch
  -h, --help                     Print help
```

Randomly select 10 testcases to run

``` bash
❯ evm-interpreter compare --executable path-to-CuEVM-cpu_interpreter --test-json dev-resources/ethtest/GeneralStateTests/VMTests/vmArithmeticTest/
```

## Caveats

When using the `--bytecode` option, because the contract binary is hardcoded to a randomly selected contract address,
there is no storage initialization, all fields in contract will initially be empty
(zero value).

> When using the `--test-json` option, the initial state will be set using the `pre` field in the ethtest json file. For example, https://github.com/cassc/evm_interpreter/blob/main/dev-resources/ethtest/GeneralStateTests/stRandom/randomStatetest0.json#L99-L121
