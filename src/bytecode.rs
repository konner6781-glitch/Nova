use crate::tokens::SourceLocation;
use crate::value::Value;

/// Every instruction that the Nova virtual machine can execute.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    // ========================================================
    // CONSTANTS AND VALUES
    // ========================================================
    LoadConstant(usize),
    LoadNone,

    // ========================================================
    // VARIABLES
    // ========================================================
    DefineVariable(String),
    LoadVariable(String),
    StoreVariable(String),
    DeleteVariable(String),

    // ========================================================
    // ARITHMETIC
    // ========================================================
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,
    Negate,

    // ========================================================
    // COMPARISON
    // ========================================================
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,

    // ========================================================
    // LOGICAL OPERATIONS
    // ========================================================
    And,
    Or,
    Not,

    // ========================================================
    // CONTROL FLOW
    // ========================================================
    Jump(usize),
    JumpIfFalse(usize),
    JumpIfTrue(usize),

    // ========================================================
    // LOOPS
    // ========================================================
    LoopStart,
    LoopSetLimit,
    LoopSetDelay,
    LoopLimitCheck(usize),
    LoopCheck(usize),
    LoopEnd(usize),
    Break(usize),

    // ========================================================
    // FUNCTIONS
    // ========================================================
    DefineFunction {
        name: String,
        parameters: Vec<String>,
        entry_point: usize,
        end_point: usize,
    },

    CallFunction {
        name: String,
        argument_count: usize,
    },

    Return {
        value_count: usize,
    },

    // ========================================================
    // COLLECTIONS
    // ========================================================
    BuildList(usize),
    BuildDictionary(usize),
    Unpack(usize),
    IndexGet,
    IndexSet {
        name: String,
        index_count: usize,
    },
    IndexCompoundSet {
        name: String,
        index_count: usize,
        operator: IndexCompoundOperator,
    },

    // ========================================================
    // STACK OPERATIONS
    // ========================================================
    Pop,
    Duplicate,
    Swap,

    // ========================================================
    // BUILT-IN OPERATIONS
    // ========================================================
    Print {
        argument_count: usize,
    },

    // ========================================================
    // IMPORTS
    // ========================================================
    ImportModule(String),
    ImportItem {
        module: String,
        item: String,
    },

    // ========================================================
    // PROGRAM CONTROL
    // ========================================================
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexCompoundOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

// ============================================================
// BYTECODE CHUNK
// ============================================================

/// Stores the compiled instructions and constants of a Nova program.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    /// Instructions executed by the virtual machine.
    pub instructions: Vec<Instruction>,

    /// Constant values referenced by instructions.
    pub constants: Vec<Value>,

    /// Source locations for instructions, used to annotate runtime errors.
    pub locations: Vec<Option<SourceLocation>>,

    current_location: Option<SourceLocation>,
}

impl Chunk {
    /// Creates an empty bytecode chunk.
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            constants: Vec::new(),
            locations: Vec::new(),
            current_location: None,
        }
    }

    /// Adds an instruction and returns its position.
    pub fn write(&mut self, instruction: Instruction) -> usize {
        self.write_at(instruction, self.current_location)
    }

    pub(crate) fn set_current_location(&mut self, location: Option<SourceLocation>) {
        self.current_location = location;
    }

    pub(crate) fn current_location(&self) -> Option<SourceLocation> {
        self.current_location
    }

    /// Adds an instruction and its optional source location.
    pub fn write_at(
        &mut self,
        instruction: Instruction,
        location: Option<SourceLocation>,
    ) -> usize {
        let position = self.instructions.len();

        self.instructions.push(instruction);
        self.locations.push(location);

        position
    }

    /// Adds a constant and returns its index.
    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);

        self.constants.len() - 1
    }

    /// Retrieves an instruction by position.
    pub fn get_instruction(&self, position: usize) -> Option<&Instruction> {
        self.instructions.get(position)
    }

    /// Retrieves a constant by index.
    pub fn get_constant(&self, index: usize) -> Option<&Value> {
        self.constants.get(index)
    }

    /// Returns the source location associated with an instruction, if known.
    pub fn source_location(&self, position: usize) -> Option<SourceLocation> {
        self.locations.get(position).copied().flatten()
    }

    /// Returns the number of instructions.
    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }

    /// Returns the number of constants.
    pub fn constant_count(&self) -> usize {
        self.constants.len()
    }

    /// Checks whether the chunk is empty.
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// Replaces an instruction at a given position.
    pub fn patch(&mut self, position: usize, instruction: Instruction) -> Result<(), String> {
        if position >= self.instructions.len() {
            return Err(format!(
                "Cannot patch instruction at position {}.",
                position
            ));
        }

        self.instructions[position] = instruction;

        Ok(())
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}
