use revm::{
    interpreter::{InstructionResult, Interpreter, Stack, OPCODE_JUMPMAP},
    Database, EVMData, Inspector,
};
use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
pub struct Trace {
    pub pc: usize,
    pub opcode: u8,
    pub stack: Stack,
}

#[derive(Debug)]
pub struct TraceInspector<'a> {
    pub traces: &'a mut Vec<Trace>,
}

impl Trace {
    pub fn pprint(&self) {
        let readable_opcode = OPCODE_JUMPMAP.get(self.opcode as usize).unwrap().unwrap();
        println!(
            "➡️ PC: {:<5} OPCODE: 0x{:02x} {}",
            self.pc, self.opcode, readable_opcode
        );
        println!("  STACK: {}", self.stack);
    }
}

impl<DB> Inspector<DB> for TraceInspector<'_>
where
    DB: Database,
{
    fn step(&mut self, interp: &mut Interpreter, _data: &mut EVMData<'_, DB>) -> InstructionResult {
        let pc = interp.program_counter();
        let opcode = interp.current_opcode();
        let stack = interp.stack.clone();
        self.traces.push(Trace { pc, opcode, stack });
        InstructionResult::Continue
    }

    fn step_end(
        &mut self,
        _interp: &mut Interpreter,
        _data: &mut EVMData<'_, DB>,
        _eval: InstructionResult,
    ) -> InstructionResult {
        InstructionResult::Continue
    }
}
