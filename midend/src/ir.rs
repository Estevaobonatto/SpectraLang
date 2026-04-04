// Spectra Intermediate Representation (IR)
// SSA-based representation with explicit control flow

pub mod pretty;

/// IR Module - top level container
#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub functions: Vec<Function>,
    pub globals: Vec<Global>,
    /// vtable definitions for dyn Trait dispatch
    pub vtables: Vec<VTableDef>,
}

/// A vtable that maps a concrete type's methods for a trait.
/// Emitted as a read-only data section of function-pointer slots.
#[derive(Debug, Clone)]
pub struct VTableDef {
    /// Symbol name: `__vtable_TypeName_TraitName`
    pub name: String,
    /// Ordered function names (IR function names) for each slot.
    pub methods: Vec<String>,
}

/// Global variable
#[derive(Debug, Clone)]
pub struct Global {
    pub id: usize,
    pub name: String,
    pub ty: Type,
    pub is_mutable: bool,
    pub initializer: Option<Constant>,
}

/// Function in IR
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Type,
    pub blocks: Vec<BasicBlock>,
    pub next_value_id: usize,
    pub next_block_id: usize,
}

/// Function parameter
#[derive(Debug, Clone)]
pub struct Parameter {
    pub id: usize,
    pub name: String,
    pub ty: Type,
}

/// Basic block in SSA form
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: usize,
    pub label: String,
    pub instructions: Vec<Instruction>,
    pub terminator: Option<Terminator>,
}

/// SSA Instruction
#[derive(Debug, Clone)]
pub struct Instruction {
    pub id: usize,
    pub kind: InstructionKind,
}

#[derive(Debug, Clone)]
pub enum InstructionKind {
    // Arithmetic
    Add {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    Sub {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    Mul {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    Div {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    Rem {
        result: Value,
        lhs: Value,
        rhs: Value,
    },

    // Comparisons
    Eq {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    Ne {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    Lt {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    Le {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    Gt {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    Ge {
        result: Value,
        lhs: Value,
        rhs: Value,
    },

    // Logical
    And {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    Or {
        result: Value,
        lhs: Value,
        rhs: Value,
    },
    Not {
        result: Value,
        operand: Value,
    },

    // Memory
    Alloca {
        result: Value,
        ty: Type,
    },
    Load {
        result: Value,
        ptr: Value,
        ty: Type,
    },
    Store {
        ptr: Value,
        value: Value,
    },
    GetElementPtr {
        result: Value,
        ptr: Value,
        index: Value,
        element_type: Type,
    },

    // Function calls
    Call {
        result: Option<Value>,
        function: String,
        args: Vec<Value>,
    },
    // Host function invocation
    HostCall {
        result: Option<Value>,
        host: String,
        args: Vec<Value>,
    },
    /// Get the address of a named function as an opaque i64 pointer (for closures/HOF).
    FuncAddr {
        result: Value,
        function: String,
    },
    /// Indirect call through a function pointer (closures passed as arguments).
    CallIndirect {
        result: Option<Value>,
        fn_ptr: Value,
        args: Vec<Value>,
        /// Parameter types of the callee signature (used to build SigRef in the backend).
        signature_params: Vec<Type>,
        /// Return type of the callee signature.
        signature_return: Box<Type>,
    },

    // PHI node for SSA
    Phi {
        result: Value,
        incoming: Vec<(Value, usize)>,
    },

    // Copy/Move
    Copy {
        result: Value,
        source: Value,
    },

    // Constants (for literal values)
    ConstInt {
        result: Value,
        value: i64,
    },
    ConstFloat {
        result: Value,
        value: f64,
    },
    ConstBool {
        result: Value,
        value: bool,
    },
    /// Numeric type conversion: int↔float, int↔char
    Cast {
        result: Value,
        operand: Value,
        from_ty: Type,
        to_ty: Type,
    },
    /// Build a fat pointer (data_ptr, vtable_ptr) for `T as dyn Trait`.
    MakeDynFatPtr {
        result: Value,
        data_ptr: Value,
        vtable_ptr: Value,
    },
    /// Load the data pointer from a fat pointer (dyn Trait object).
    LoadDynDataPtr {
        result: Value,
        fat_ptr: Value,
    },
    /// Load the vtable pointer from a fat pointer (dyn Trait object).
    LoadDynVtablePtr {
        result: Value,
        fat_ptr: Value,
    },
    /// Load a function pointer from a vtable at a given slot index.
    LoadVtableSlot {
        result: Value,
        vtable_ptr: Value,
        slot_index: usize,
    },
}

/// Block terminator (control flow)
#[derive(Debug, Clone)]
pub enum Terminator {
    Return {
        value: Option<Value>,
    },
    Branch {
        target: usize,
    },
    CondBranch {
        condition: Value,
        true_block: usize,
        false_block: usize,
    },
    Switch {
        value: Value,
        cases: Vec<(i64, usize)>,
        default: usize,
    },
    Unreachable,
}

/// SSA Value
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Value {
    pub id: usize,
}

/// IR Type system
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Void,
    Int,
    Float,
    Bool,
    String,
    Char,
    Pointer(Box<Type>),
    Array {
        element_type: Box<Type>,
        size: usize,
    },
    Tuple {
        elements: Vec<Type>,
    },
    Struct {
        name: String,
        fields: Vec<(String, Type)>,
    },
    Enum {
        name: String,
        variants: Vec<(String, Option<Vec<Type>>)>, // (name, data_types)
    },
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
    },
    /// Fat pointer for dyn Trait objects: (data_ptr: i64, vtable_ptr: i64).
    DynTrait {
        trait_name: String,
    },
}

/// Constant values
#[derive(Debug, Clone)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Char(char),
    Null,
}

impl Module {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            functions: Vec::new(),
            globals: Vec::new(),
            vtables: Vec::new(),
        }
    }

    pub fn add_function(&mut self, function: Function) {
        self.functions.push(function);
    }

    pub fn get_function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|f| f.name == name)
    }
}

impl Function {
    pub fn new(name: impl Into<String>, params: Vec<Parameter>, return_type: Type) -> Self {
        let param_count = params.len();
        Self {
            name: name.into(),
            params,
            return_type,
            blocks: Vec::new(),
            next_value_id: param_count, // Start after parameters
            next_block_id: 0,
        }
    }

    pub fn add_block(&mut self, label: impl Into<String>) -> usize {
        let id = self.next_block_id;
        self.next_block_id += 1;

        self.blocks.push(BasicBlock {
            id,
            label: label.into(),
            instructions: Vec::new(),
            terminator: None,
        });

        id
    }

    pub fn get_block(&self, id: usize) -> Option<&BasicBlock> {
        self.blocks.iter().find(|b| b.id == id)
    }

    pub fn get_block_mut(&mut self, id: usize) -> Option<&mut BasicBlock> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }

    pub fn next_value(&mut self) -> Value {
        let id = self.next_value_id;
        self.next_value_id += 1;
        Value { id }
    }
}

impl BasicBlock {
    pub fn add_instruction(&mut self, kind: InstructionKind) -> usize {
        let id = self.instructions.len();
        self.instructions.push(Instruction { id, kind });
        id
    }

    pub fn set_terminator(&mut self, terminator: Terminator) {
        self.terminator = Some(terminator);
    }
}

impl Type {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float)
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Type::Int)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Type::Bool)
    }
}
