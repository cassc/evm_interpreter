use revm::{
    interpreter::{Gas, Interpreter, Stack, OPCODE_JUMPMAP},
    Database, EVMData, Inspector,
};
use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
pub struct EvmGas {
    pub limit: u64,
    /// The total used gas (including memory expansion)
    pub used: u64,
    /// Used gas for memory expansion.
    pub memory: u64,
    pub refunded: i64,
}

impl From<&Gas> for EvmGas {
    fn from(gas: &Gas) -> Self {
        Self {
            limit: gas.limit(),
            used: gas.spend(),
            memory: gas.memory(),
            refunded: gas.refunded(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Trace {
    pub pc: usize,
    pub opcode: u8,
    pub stack: Stack,
    pub gas: EvmGas,
}

#[derive(Debug)]
pub struct TraceInspector<'a> {
    pub traces: &'a mut Vec<Trace>,
}

impl Trace {
    pub fn pprint(&self) {
        let readable_opcode = OPCODE_JUMPMAP.get(self.opcode as usize).unwrap().unwrap();
        println!(
            "➡️ PC: {:<5} OPCODE: 0x{:02x} {:<8} GAS_USED: {:<12} GAS_MEMORY: {:<8}",
            self.pc, self.opcode, readable_opcode, self.gas.used, self.gas.memory
        );
        println!("  STACK: {}", self.stack);
    }
}

impl<DB> Inspector<DB> for TraceInspector<'_>
where
    DB: Database,
{
    fn step(&mut self, interp: &mut Interpreter, _data: &mut EVMData<'_, DB>) {
        let pc = interp.program_counter();
        let opcode = interp.current_opcode();
        let stack = interp.stack.clone();
        let gas = interp.gas();
        self.traces.push(Trace {
            pc,
            opcode,
            stack,
            gas: gas.into(),
        });
    }
}
