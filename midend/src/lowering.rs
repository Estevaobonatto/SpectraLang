// AST to IR lowering pass
// Converts semantic AST to SSA-based IR

use crate::builder::IRBuilder;
use crate::ir::{
    Function as IRFunction, Module as IRModule, Parameter, Terminator, Type as IRType, Value,
};
use spectra_compiler::ast::{
    BinaryOperator, Expression, ExpressionKind, Function as ASTFunction, Item, Module as ASTModule,
    Statement, StatementKind, Type as ASTType, TypeAnnotation,
};
use std::collections::HashMap;

/// Loop context for break/continue handling
#[derive(Clone)]
struct LoopContext {
    header_block: usize,
    exit_block: usize,
}

pub struct ASTLowering {
    builder: IRBuilder,
    current_function: Option<IRFunction>,
    value_map: HashMap<String, Value>,
    /// Maps variable names to their allocated memory locations (for mutable variables)
    alloca_map: HashMap<String, Value>,
    /// Maps array names to their base pointers (stack addresses)
    array_map: HashMap<String, Value>,
    /// Maps struct names to their field definitions
    struct_definitions: HashMap<String, Vec<(String, IRType)>>,
    /// Maps struct variable names to (pointer, struct_name) for field access
    struct_var_map: HashMap<String, (Value, String)>,
    /// Maps enum names to their variant definitions: (variant_name, tag, data_types)
    enum_definitions: HashMap<String, Vec<(String, usize, Option<Vec<IRType>>)>>,
    loop_stack: Vec<LoopContext>,
}

impl ASTLowering {
    pub fn new() -> Self {
        Self {
            builder: IRBuilder::new(),
            current_function: None,
            value_map: HashMap::new(),
            alloca_map: HashMap::new(),
            array_map: HashMap::new(),
            struct_definitions: HashMap::new(),
            struct_var_map: HashMap::new(),
            enum_definitions: HashMap::new(),
            loop_stack: Vec::new(),
        }
    }

    pub fn lower_module(&mut self, ast_module: &ASTModule) -> IRModule {
        let mut ir_module = IRModule::new(&ast_module.name);

        // First pass: collect struct and enum definitions
        for item in &ast_module.items {
            if let Item::Struct(struct_def) = item {
                let fields: Vec<(String, IRType)> = struct_def
                    .fields
                    .iter()
                    .map(|field| {
                        let field_type = self.lower_type_annotation(&field.ty);
                        (field.name.clone(), field_type)
                    })
                    .collect();
                self.struct_definitions.insert(struct_def.name.clone(), fields);
            } else if let Item::Enum(enum_def) = item {
                let variants: Vec<(String, usize, Option<Vec<IRType>>)> = enum_def
                    .variants
                    .iter()
                    .enumerate()
                    .map(|(tag, variant)| {
                        let data_types = variant.data.as_ref().map(|types| {
                            types.iter().map(|ty| self.lower_type_annotation(ty)).collect()
                        });
                        (variant.name.clone(), tag, data_types)
                    })
                    .collect();
                self.enum_definitions.insert(enum_def.name.clone(), variants);
            }
        }

        // Second pass: lower functions
        for item in &ast_module.items {
            if let Item::Function(func) = item {
                let ir_func = self.lower_function(func);
                ir_module.add_function(ir_func);
            }
        }

        ir_module
    }

    /// Infere o tipo IR de uma expressão AST (análise simplificada)
    fn infer_expr_ir_type(&self, expr: &Expression) -> IRType {
        match &expr.kind {
            ExpressionKind::NumberLiteral(s) => {
                // Se tem ponto, é float, senão int
                if s.contains('.') {
                    IRType::Float
                } else {
                    IRType::Int
                }
            }
            ExpressionKind::StringLiteral(_) => IRType::String,
            ExpressionKind::BoolLiteral(_) => IRType::Bool,
            ExpressionKind::ArrayLiteral { elements } => {
                if elements.is_empty() {
                    IRType::Array {
                        element_type: Box::new(IRType::Int),
                        size: 0,
                    }
                } else {
                    let elem_type = self.infer_expr_ir_type(&elements[0]);
                    IRType::Array {
                        element_type: Box::new(elem_type),
                        size: elements.len(),
                    }
                }
            }
            ExpressionKind::TupleLiteral { elements } => {
                let element_types: Vec<IRType> = elements
                    .iter()
                    .map(|e| self.infer_expr_ir_type(e))
                    .collect();
                IRType::Tuple {
                    elements: element_types,
                }
            }
            _ => IRType::Int, // Fallback
        }
    }

    fn lower_function(&mut self, ast_func: &ASTFunction) -> IRFunction {
        // Convert parameters
        let params: Vec<Parameter> = ast_func
            .params
            .iter()
            .enumerate()
            .map(|(idx, param)| Parameter {
                id: idx,
                name: param.name.clone(),
                ty: param
                    .ty
                    .as_ref()
                    .map(|t| self.lower_type_annotation(t))
                    .unwrap_or(IRType::Void),
            })
            .collect();

        // Create function
        let return_type = ast_func
            .return_type
            .as_ref()
            .map(|t| self.lower_type_annotation(t))
            .unwrap_or(IRType::Void);

        let mut ir_func = IRFunction::new(&ast_func.name, params.clone(), return_type);

        // Create entry block
        let entry_block = ir_func.add_block("entry");
        self.builder.set_current_block(entry_block);

        // Map parameters to values
        self.value_map.clear();
        self.alloca_map.clear();
        self.array_map.clear();
        self.struct_var_map.clear();
        for (idx, param) in params.iter().enumerate() {
            self.value_map.insert(param.name.clone(), Value { id: idx });
        }

        // Analyze which variables are assigned to (need memory allocation)
        let assigned_vars = self.find_assigned_variables(&ast_func.body.statements);

        // Allocate memory for mutable variables
        for var_name in &assigned_vars {
            let alloca_value = self.builder.build_alloca(&mut ir_func, IRType::Int);
            self.alloca_map.insert(var_name.clone(), alloca_value);
        }

        // Lower function body
        self.current_function = Some(ir_func.clone());
        
        // Check if last statement is an expression (implicit return)
        let mut implicit_return_value = None;
        if let Some(last_stmt) = ast_func.body.statements.last() {
            if let StatementKind::Expression(expr) = &last_stmt.kind {
                // Lower all statements except the last
                if ast_func.body.statements.len() > 1 {
                    for stmt in &ast_func.body.statements[..ast_func.body.statements.len() - 1] {
                        self.lower_statement(stmt, &mut ir_func);
                    }
                }
                // Lower last expression and capture its value
                implicit_return_value = Some(self.lower_expression(expr, &mut ir_func));
            } else {
                // No implicit return, lower all statements
                self.lower_block(&ast_func.body.statements, &mut ir_func);
            }
        } else {
            // Empty body
            self.lower_block(&ast_func.body.statements, &mut ir_func);
        }

        // Ensure function has a return in the current block
        // (After lowering all statements, we should be in the final block)
        if let Some(current_block_id) = self.builder.get_current_block() {
            if let Some(block) = ir_func.get_block_mut(current_block_id) {
                if block.terminator.is_none() {
                    block.set_terminator(Terminator::Return {
                        value: implicit_return_value,
                    });
                }
            }
        }

        ir_func
    }

    fn lower_block(&mut self, statements: &[Statement], ir_func: &mut IRFunction) {
        for stmt in statements {
            self.lower_statement(stmt, ir_func);
        }
    }

    /// Analyzes which variables are assigned to in a block
    fn find_assigned_variables(
        &self,
        statements: &[Statement],
    ) -> std::collections::HashSet<String> {
        use std::collections::HashSet;
        let mut assigned = HashSet::new();

        for stmt in statements {
            match &stmt.kind {
                StatementKind::Assignment(assign) => {
                    // Extract variable name from LValue
                    // For now, only track simple identifiers (not array elements)
                    if let spectra_compiler::ast::LValue::Identifier(name) = &assign.target {
                        assigned.insert(name.clone());
                    }
                }
                StatementKind::While(while_stmt) => {
                    // Recursively check loop body
                    assigned.extend(self.find_assigned_variables(&while_stmt.body.statements));
                }
                StatementKind::DoWhile(do_while) => {
                    assigned.extend(self.find_assigned_variables(&do_while.body.statements));
                }
                StatementKind::For(for_stmt) => {
                    assigned.extend(self.find_assigned_variables(&for_stmt.body.statements));
                }
                StatementKind::Loop(loop_stmt) => {
                    assigned.extend(self.find_assigned_variables(&loop_stmt.body.statements));
                }
                StatementKind::Switch(switch) => {
                    for case in &switch.cases {
                        assigned.extend(self.find_assigned_variables(&case.body.statements));
                    }
                    if let Some(default) = &switch.default {
                        assigned.extend(self.find_assigned_variables(&default.statements));
                    }
                }
                StatementKind::Expression(expr) => {
                    // Check if expression contains assignments in blocks
                    if let ExpressionKind::If {
                        then_block,
                        elif_blocks,
                        else_block,
                        ..
                    } = &expr.kind
                    {
                        assigned.extend(self.find_assigned_variables(&then_block.statements));
                        for (_, block) in elif_blocks {
                            assigned.extend(self.find_assigned_variables(&block.statements));
                        }
                        if let Some(else_b) = else_block {
                            assigned.extend(self.find_assigned_variables(&else_b.statements));
                        }
                    }
                }
                _ => {}
            }
        }

        assigned
    }

    fn lower_statement(&mut self, stmt: &Statement, ir_func: &mut IRFunction) {
        match &stmt.kind {
            StatementKind::Let(let_stmt) => {
                if let Some(ref value_expr) = let_stmt.value {
                    let value = self.lower_expression(value_expr, ir_func);
                    
                    // Check if this is an array literal - store pointer in array_map
                    if matches!(value_expr.kind, ExpressionKind::ArrayLiteral { .. }) {
                        self.array_map.insert(let_stmt.name.clone(), value);
                    }
                    // Check if this is a struct literal - store pointer + name in struct_var_map
                    else if let ExpressionKind::StructLiteral { name, .. } = &value_expr.kind {
                        self.struct_var_map.insert(let_stmt.name.clone(), (value, name.clone()));
                    }
                    // If variable will be assigned later, store to allocated memory
                    else if let Some(&alloca_ptr) = self.alloca_map.get(&let_stmt.name) {
                        self.builder.build_store(ir_func, alloca_ptr, value);
                    } else {
                        // Otherwise, just map the SSA value
                        self.value_map.insert(let_stmt.name.clone(), value);
                    }
                }
            }
            StatementKind::Assignment(assign) => {
                let value = self.lower_expression(&assign.value, ir_func);
                
                match &assign.target {
                    spectra_compiler::ast::LValue::Identifier(name) => {
                        // Assignment to simple variable (uses memory)
                        if let Some(&alloca_ptr) = self.alloca_map.get(name) {
                            self.builder.build_store(ir_func, alloca_ptr, value);
                        } else {
                            // Fallback: update value_map (shouldn't happen if analysis is correct)
                            self.value_map.insert(name.clone(), value);
                        }
                    }
                    spectra_compiler::ast::LValue::IndexAccess { array, index } => {
                        // Assignment to array element
                        let array_ptr = self.lower_expression(array, ir_func);
                        let index_value = self.lower_expression(index, ir_func);
                        
                        // Calcular endereço do elemento
                        let elem_type = IRType::Int; // Assumir int por enquanto
                        let elem_ptr = self.builder.build_getelementptr(
                            ir_func,
                            array_ptr,
                            index_value,
                            elem_type,
                        );
                        
                        // Store valor no elemento
                        self.builder.build_store(ir_func, elem_ptr, value);
                    }
                }
            }
            StatementKind::Return(ret) => {
                let value = ret
                    .value
                    .as_ref()
                    .map(|expr| self.lower_expression(expr, ir_func));
                self.builder.build_return(ir_func, value);
            }
            StatementKind::Expression(expr) => {
                self.lower_expression(expr, ir_func);
            }
            StatementKind::While(while_stmt) => {
                let header_block = ir_func.add_block("while.header");
                let body_block = ir_func.add_block("while.body");
                let exit_block = ir_func.add_block("while.exit");

                // Branch to header
                self.builder.build_branch(ir_func, header_block);
                self.builder.set_current_block(header_block);

                // Evaluate condition
                let condition = self.lower_expression(&while_stmt.condition, ir_func);
                self.builder
                    .build_cond_branch(ir_func, condition, body_block, exit_block);

                // Body (push loop context for break/continue)
                self.loop_stack.push(LoopContext {
                    header_block,
                    exit_block,
                });
                self.builder.set_current_block(body_block);
                self.lower_block(&while_stmt.body.statements, ir_func);
                self.builder.build_branch(ir_func, header_block);
                self.loop_stack.pop();

                // Exit
                self.builder.set_current_block(exit_block);
            }
            StatementKind::DoWhile(do_while) => {
                let body_block = ir_func.add_block("do_while.body");
                let header_block = ir_func.add_block("do_while.header");
                let exit_block = ir_func.add_block("do_while.exit");

                // Branch to body first
                self.builder.build_branch(ir_func, body_block);

                // Body (push loop context for break/continue)
                self.loop_stack.push(LoopContext {
                    header_block,
                    exit_block,
                });
                self.builder.set_current_block(body_block);
                self.lower_block(&do_while.body.statements, ir_func);
                self.builder.build_branch(ir_func, header_block);
                self.loop_stack.pop();

                // Header/condition
                self.builder.set_current_block(header_block);
                let condition = self.lower_expression(&do_while.condition, ir_func);
                self.builder
                    .build_cond_branch(ir_func, condition, body_block, exit_block);

                // Exit
                self.builder.set_current_block(exit_block);
            }
            StatementKind::For(for_stmt) => {
                // For now, implement simple for-in/for-of lowering
                // TODO: Proper iterator support
                let header_block = ir_func.add_block("for.header");
                let body_block = ir_func.add_block("for.body");
                let exit_block = ir_func.add_block("for.exit");

                // Evaluate iterable
                let _iterable = self.lower_expression(&for_stmt.iterable, ir_func);

                // Branch to header
                self.builder.build_branch(ir_func, header_block);
                self.builder.set_current_block(header_block);

                // TODO: Check if iterator has next element
                let condition = ir_func.next_value(); // Placeholder
                self.builder
                    .build_cond_branch(ir_func, condition, body_block, exit_block);

                // Body (push loop context for break/continue)
                self.loop_stack.push(LoopContext {
                    header_block,
                    exit_block,
                });
                self.builder.set_current_block(body_block);
                // TODO: Bind iterator variable
                self.lower_block(&for_stmt.body.statements, ir_func);
                self.builder.build_branch(ir_func, header_block);
                self.loop_stack.pop();

                // Exit
                self.builder.set_current_block(exit_block);
            }
            StatementKind::Loop(loop_stmt) => {
                let body_block = ir_func.add_block("loop.body");
                let exit_block = ir_func.add_block("loop.exit");

                // Branch to body
                self.builder.build_branch(ir_func, body_block);

                // Body (infinite loop - needs break to exit)
                // Use body_block as header since it's the loop entry point
                self.loop_stack.push(LoopContext {
                    header_block: body_block,
                    exit_block,
                });
                self.builder.set_current_block(body_block);
                self.lower_block(&loop_stmt.body.statements, ir_func);
                self.builder.build_branch(ir_func, body_block);
                self.loop_stack.pop();

                // Exit (unreachable unless break is used)
                self.builder.set_current_block(exit_block);
            }
            StatementKind::Switch(switch) => {
                let scrutinee = self.lower_expression(&switch.value, ir_func);

                // Create blocks for each case and default/exit
                let exit_block = ir_func.add_block("switch.exit");
                let mut cases = Vec::new();
                let mut case_blocks = Vec::new();

                for (idx, case) in switch.cases.iter().enumerate() {
                    let case_block = ir_func.add_block(&format!("switch.case.{}", idx));
                    case_blocks.push((case_block, case));

                    // Extract constant value from pattern
                    // TODO: Proper constant evaluation
                    let pattern_int = if let ExpressionKind::NumberLiteral(n) = &case.pattern.kind {
                        n.parse::<i64>().unwrap_or(0)
                    } else {
                        0 // Default value for non-integer patterns
                    };
                    cases.push((pattern_int, case_block));
                }

                // Build switch terminator
                let default = if switch.default.is_some() {
                    ir_func.add_block("switch.default")
                } else {
                    exit_block
                };

                if let Some(current_block) = self.builder.get_current_block() {
                    if let Some(block) = ir_func.get_block_mut(current_block) {
                        block.set_terminator(Terminator::Switch {
                            value: scrutinee,
                            cases,
                            default,
                        });
                    }
                }

                // Lower each case body
                for (case_block, case) in case_blocks {
                    self.builder.set_current_block(case_block);
                    self.lower_block(&case.body.statements, ir_func);
                    self.builder.build_branch(ir_func, exit_block);
                }

                // Lower default if present
                if let Some(ref default_block) = switch.default {
                    self.builder.set_current_block(default);
                    self.lower_block(&default_block.statements, ir_func);
                    self.builder.build_branch(ir_func, exit_block);
                }

                // Exit
                self.builder.set_current_block(exit_block);
            }
            StatementKind::Break => {
                // Branch to the exit block of the innermost loop
                if let Some(loop_ctx) = self.loop_stack.last() {
                    self.builder.build_branch(ir_func, loop_ctx.exit_block);
                } else {
                    // Break outside of loop - error, but generate unreachable
                    if let Some(current_block) = self.builder.get_current_block() {
                        if let Some(block) = ir_func.get_block_mut(current_block) {
                            block.set_terminator(Terminator::Unreachable);
                        }
                    }
                }
            }
            StatementKind::Continue => {
                // Branch to the header block of the innermost loop
                if let Some(loop_ctx) = self.loop_stack.last() {
                    self.builder.build_branch(ir_func, loop_ctx.header_block);
                } else {
                    // Continue outside of loop - error, but generate unreachable
                    if let Some(current_block) = self.builder.get_current_block() {
                        if let Some(block) = ir_func.get_block_mut(current_block) {
                            block.set_terminator(Terminator::Unreachable);
                        }
                    }
                }
            }
        }
    }

    fn lower_expression(&mut self, expr: &Expression, ir_func: &mut IRFunction) -> Value {
        match &expr.kind {
            ExpressionKind::NumberLiteral(n) => {
                // Try to parse as integer first, then float
                if let Ok(int_val) = n.parse::<i64>() {
                    self.builder.build_const_int(ir_func, int_val)
                } else if let Ok(float_val) = n.parse::<f64>() {
                    self.builder.build_const_float(ir_func, float_val)
                } else {
                    // Fallback to 0 if parsing fails
                    self.builder.build_const_int(ir_func, 0)
                }
            }
            ExpressionKind::StringLiteral(_s) => {
                // TODO: String constant support com globais
                // Por ora, criar uma constante int como placeholder
                // Isso permite que strings funcionem em tuples
                self.builder.build_const_int(ir_func, 0)
            }
            ExpressionKind::BoolLiteral(b) => self.builder.build_const_bool(ir_func, *b),
            ExpressionKind::Identifier(name) => {
                // Check if this is an array - return pointer directly
                if let Some(&array_ptr) = self.array_map.get(name) {
                    array_ptr
                }
                // Check if variable is in memory (mutable)
                else if let Some(&alloca_ptr) = self.alloca_map.get(name) {
                    // Load from memory
                    self.builder.build_load(ir_func, alloca_ptr)
                } else if let Some(&value) = self.value_map.get(name) {
                    // Use SSA value directly
                    value
                } else {
                    // Unknown variable, create placeholder
                    ir_func.next_value()
                }
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => {
                let lhs = self.lower_expression(left, ir_func);
                let rhs = self.lower_expression(right, ir_func);

                match operator {
                    BinaryOperator::Add => self.builder.build_add(ir_func, lhs, rhs),
                    BinaryOperator::Subtract => self.builder.build_sub(ir_func, lhs, rhs),
                    BinaryOperator::Multiply => self.builder.build_mul(ir_func, lhs, rhs),
                    BinaryOperator::Divide => self.builder.build_div(ir_func, lhs, rhs),
                    BinaryOperator::Modulo => self.builder.build_rem(ir_func, lhs, rhs),
                    BinaryOperator::Equal => self.builder.build_eq(ir_func, lhs, rhs),
                    BinaryOperator::NotEqual => self.builder.build_ne(ir_func, lhs, rhs),
                    BinaryOperator::Less => self.builder.build_lt(ir_func, lhs, rhs),
                    BinaryOperator::LessEqual => self.builder.build_le(ir_func, lhs, rhs),
                    BinaryOperator::Greater => self.builder.build_gt(ir_func, lhs, rhs),
                    BinaryOperator::GreaterEqual => self.builder.build_ge(ir_func, lhs, rhs),
                    BinaryOperator::And => self.builder.build_and(ir_func, lhs, rhs),
                    BinaryOperator::Or => self.builder.build_or(ir_func, lhs, rhs),
                }
            }
            ExpressionKind::Unary { operator, operand } => {
                use spectra_compiler::ast::UnaryOperator;
                let operand_value = self.lower_expression(operand, ir_func);

                match operator {
                    UnaryOperator::Negate => {
                        // Negate: 0 - operand
                        let zero = self.builder.build_const_int(ir_func, 0);
                        self.builder.build_sub(ir_func, zero, operand_value)
                    }
                    UnaryOperator::Not => self.builder.build_not(ir_func, operand_value),
                }
            }
            ExpressionKind::Call { callee, arguments } => {
                let arg_values: Vec<Value> = arguments
                    .iter()
                    .map(|arg| self.lower_expression(arg, ir_func))
                    .collect();

                // Extract function name from callee
                let function_name = if let ExpressionKind::Identifier(name) = &callee.kind {
                    name.clone()
                } else {
                    "unknown".to_string()
                };

                self.builder
                    .build_call(ir_func, function_name, arg_values, true)
                    .unwrap_or_else(|| ir_func.next_value())
            }
            ExpressionKind::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                let then_bb = ir_func.add_block("if.then");
                let else_bb = ir_func.add_block("if.else");
                let merge_bb = ir_func.add_block("if.merge");

                // Evaluate condition
                let cond_value = self.lower_expression(condition, ir_func);
                self.builder
                    .build_cond_branch(ir_func, cond_value, then_bb, else_bb);

                // Then branch
                self.builder.set_current_block(then_bb);
                let mut then_value = None;
                self.lower_block(&then_block.statements, ir_func);
                // If last statement is an expression, use it as value
                if let Some(Statement {
                    kind: StatementKind::Expression(expr),
                    ..
                }) = then_block.statements.last()
                {
                    then_value = Some(self.lower_expression(expr, ir_func));
                }
                let then_final_block = self.builder.get_current_block().unwrap_or(then_bb);

                // Only add branch if block doesn't have terminator
                if let Some(block) = ir_func.get_block_mut(then_final_block) {
                    if block.terminator.is_none() {
                        self.builder.build_branch(ir_func, merge_bb);
                    }
                }

                // Else/elif branches
                self.builder.set_current_block(else_bb);
                let mut else_value = None;

                // Handle elif branches
                if !elif_blocks.is_empty() {
                    // TODO: Proper elif chain with multiple blocks
                    // For now, treat as nested if
                }

                // Else branch
                if let Some(else_body) = else_block {
                    self.lower_block(&else_body.statements, ir_func);
                    if let Some(Statement {
                        kind: StatementKind::Expression(expr),
                        ..
                    }) = else_body.statements.last()
                    {
                        else_value = Some(self.lower_expression(expr, ir_func));
                    }
                }
                let else_final_block = self.builder.get_current_block().unwrap_or(else_bb);

                // Check if else block has terminator
                let else_has_terminator = if let Some(block) = ir_func.get_block(else_final_block) {
                    block.terminator.is_some()
                } else {
                    false
                };

                // Only add branch if block doesn't have terminator
                if !else_has_terminator {
                    if let Some(block) = ir_func.get_block_mut(else_final_block) {
                        if block.terminator.is_none() {
                            self.builder.build_branch(ir_func, merge_bb);
                        }
                    }
                }

                // Check if then block has terminator
                let then_has_terminator = if let Some(block) = ir_func.get_block(then_final_block) {
                    block.terminator.is_some()
                } else {
                    false
                };

                // Only use merge block if at least one branch reaches it
                if !then_has_terminator || !else_has_terminator {
                    // Merge block with PHI node
                    self.builder.set_current_block(merge_bb);

                    // If both branches produce values, create PHI node
                    if let (Some(then_val), Some(else_val)) = (then_value, else_value) {
                        self.builder.build_phi(
                            ir_func,
                            vec![(then_val, then_final_block), (else_val, else_final_block)],
                        )
                    } else {
                        // No value produced (void)
                        ir_func.next_value()
                    }
                } else {
                    // Both branches have terminators (returns), no merge needed
                    // Don't set current block to merge - leave it undefined
                    // This will make the function return check skip adding a return
                    ir_func.next_value()
                }
            }
            ExpressionKind::Unless {
                condition,
                then_block,
                else_block,
            } => {
                // Unless is equivalent to: if (!condition) { then_block } else { else_block }
                let unless_then_bb = ir_func.add_block("unless.then");
                let unless_else_bb = ir_func.add_block("unless.else");
                let unless_merge_bb = ir_func.add_block("unless.merge");

                // Evaluate and negate condition
                let cond_value = self.lower_expression(condition, ir_func);
                let negated_cond = self.builder.build_not(ir_func, cond_value);

                self.builder.build_cond_branch(
                    ir_func,
                    negated_cond,
                    unless_then_bb,
                    unless_else_bb,
                );

                // Unless body (executes when condition is false)
                self.builder.set_current_block(unless_then_bb);
                let mut unless_value = None;
                self.lower_block(&then_block.statements, ir_func);
                if let Some(Statement {
                    kind: StatementKind::Expression(expr),
                    ..
                }) = then_block.statements.last()
                {
                    unless_value = Some(self.lower_expression(expr, ir_func));
                }
                let unless_then_final = self.builder.get_current_block().unwrap_or(unless_then_bb);

                // Only add branch if block doesn't have terminator
                if let Some(block) = ir_func.get_block_mut(unless_then_final) {
                    if block.terminator.is_none() {
                        self.builder.build_branch(ir_func, unless_merge_bb);
                    }
                }

                // Else branch (executes when condition is true)
                self.builder.set_current_block(unless_else_bb);
                let mut unless_else_value = None;
                if let Some(else_body) = else_block {
                    self.lower_block(&else_body.statements, ir_func);
                    if let Some(Statement {
                        kind: StatementKind::Expression(expr),
                        ..
                    }) = else_body.statements.last()
                    {
                        unless_else_value = Some(self.lower_expression(expr, ir_func));
                    }
                }
                let unless_else_final = self.builder.get_current_block().unwrap_or(unless_else_bb);

                // Only add branch if block doesn't have terminator
                if let Some(block) = ir_func.get_block_mut(unless_else_final) {
                    if block.terminator.is_none() {
                        self.builder.build_branch(ir_func, unless_merge_bb);
                    }
                }

                // Merge
                self.builder.set_current_block(unless_merge_bb);
                if let (Some(then_val), Some(else_val)) = (unless_value, unless_else_value) {
                    self.builder.build_phi(
                        ir_func,
                        vec![(then_val, unless_then_final), (else_val, unless_else_final)],
                    )
                } else {
                    ir_func.next_value()
                }
            }
            ExpressionKind::Grouping(inner) => self.lower_expression(inner, ir_func),
            ExpressionKind::ArrayLiteral { elements } => {
                // Alocar memória para o array
                let size = elements.len();
                if size == 0 {
                    // Array vazio - retornar um valor placeholder
                    return ir_func.next_value();
                }
                
                // Determinar o tipo do elemento (assume todos são do mesmo tipo)
                // Por simplicidade, vamos usar Int como padrão
                let elem_type = IRType::Int;
                
                // Alocar espaço para o array no stack (tipo Array com tamanho)
                let array_type = IRType::Array {
                    element_type: Box::new(elem_type.clone()),
                    size,
                };
                let array_ptr = self.builder.build_alloca(ir_func, array_type);
                
                // Inicializar cada elemento
                for (i, elem_expr) in elements.iter().enumerate() {
                    let elem_value = self.lower_expression(elem_expr, ir_func);
                    let index_value = self.builder.build_const_int(ir_func, i as i64);
                    let elem_ptr = self.builder.build_getelementptr(
                        ir_func,
                        array_ptr,
                        index_value,
                        elem_type.clone(),
                    );
                    self.builder.build_store(ir_func, elem_ptr, elem_value);
                }
                
                // Retornar o ponteiro para o array
                array_ptr
            }
            ExpressionKind::IndexAccess { array, index } => {
                // Avaliar a expressão do array
                let array_ptr = self.lower_expression(array, ir_func);
                
                // Avaliar o índice
                let index_value = self.lower_expression(index, ir_func);
                
                // Calcular o endereço do elemento
                // Por simplicidade, assumir tipo Int
                let elem_type = IRType::Int;
                let elem_ptr = self.builder.build_getelementptr(
                    ir_func,
                    array_ptr,
                    index_value,
                    elem_type,
                );
                
                // Carregar o valor do elemento
                self.builder.build_load(ir_func, elem_ptr)
            }
            ExpressionKind::TupleLiteral { elements } => {
                // Alocar memória para a tuple
                let size = elements.len();
                if size == 0 {
                    // Tuple vazia - retornar um valor placeholder
                    return ir_func.next_value();
                }
                
                // Determinar os tipos dos elementos usando inferência
                let elem_types: Vec<IRType> = elements
                    .iter()
                    .map(|e| self.infer_expr_ir_type(e))
                    .collect();
                
                // Alocar espaço para a tuple no stack
                let tuple_type = IRType::Tuple {
                    elements: elem_types.clone(),
                };
                let tuple_ptr = self.builder.build_alloca(ir_func, tuple_type);
                
                // Inicializar cada elemento
                for (i, elem_expr) in elements.iter().enumerate() {
                    let elem_value = self.lower_expression(elem_expr, ir_func);
                    let index_value = self.builder.build_const_int(ir_func, i as i64);
                    let elem_ptr = self.builder.build_getelementptr(
                        ir_func,
                        tuple_ptr,
                        index_value,
                        elem_types[i].clone(),
                    );
                    self.builder.build_store(ir_func, elem_ptr, elem_value);
                }
                
                // Retornar o ponteiro para a tuple
                tuple_ptr
            }
            ExpressionKind::TupleAccess { tuple, index } => {
                // Avaliar a expressão da tuple
                let tuple_ptr = self.lower_expression(tuple, ir_func);
                
                // Calcular o endereço do elemento usando o índice constante
                let index_value = self.builder.build_const_int(ir_func, *index as i64);
                
                // Inferir o tipo do elemento da tuple
                let elem_type = if let ExpressionKind::TupleLiteral { elements } = &tuple.kind {
                    // Se é um literal, inferir diretamente
                    if *index < elements.len() {
                        self.infer_expr_ir_type(&elements[*index])
                    } else {
                        IRType::Int
                    }
                } else {
                    // Caso contrário, inferir o tipo da tuple inteira e extrair o elemento
                    match self.infer_expr_ir_type(tuple) {
                        IRType::Tuple { elements } if *index < elements.len() => {
                            elements[*index].clone()
                        }
                        _ => IRType::Int, // Fallback
                    }
                };
                
                let elem_ptr = self.builder.build_getelementptr(
                    ir_func,
                    tuple_ptr,
                    index_value,
                    elem_type,
                );
                
                // Carregar o valor do elemento
                self.builder.build_load(ir_func, elem_ptr)
            }
            ExpressionKind::StructLiteral { name, fields } => {
                // Buscar definição do struct
                let struct_fields = self.struct_definitions.get(name).cloned();
                
                if let Some(field_defs) = struct_fields {
                    // Criar tipo struct
                    let struct_type = IRType::Struct {
                        name: name.clone(),
                        fields: field_defs.clone(),
                    };
                    
                    // Alocar espaço para o struct no stack
                    let struct_ptr = self.builder.build_alloca(ir_func, struct_type);
                    
                    // Inicializar cada campo
                    for (field_idx, (field_name, field_expr)) in fields.iter().enumerate() {
                        // Lower da expressão do campo
                        let field_value = self.lower_expression(field_expr, ir_func);
                        
                        // Obter tipo do campo
                        let field_type = field_defs.iter()
                            .find(|(name, _)| name == field_name)
                            .map(|(_, ty)| ty.clone())
                            .unwrap_or(IRType::Int);
                        
                        // GEP para o campo
                        let index_value = self.builder.build_const_int(ir_func, field_idx as i64);
                        let field_ptr = self.builder.build_getelementptr(
                            ir_func,
                            struct_ptr,
                            index_value,
                            field_type,
                        );
                        
                        // Store do valor
                        self.builder.build_store(ir_func, field_ptr, field_value);
                    }
                    
                    // Retornar ponteiro para o struct
                    struct_ptr
                } else {
                    // Struct não encontrado, retornar placeholder
                    ir_func.next_value()
                }
            }
            ExpressionKind::FieldAccess { object, field } => {
                // Se o objeto é um identificador, buscar no struct_var_map
                if let ExpressionKind::Identifier(name) = &object.kind {
                    if let Some((struct_ptr, struct_name)) = self.struct_var_map.get(name) {
                        // Buscar definição do struct
                        if let Some(field_defs) = self.struct_definitions.get(struct_name) {
                            // Encontrar índice do campo
                            if let Some((field_idx, (_, field_type))) = field_defs
                                .iter()
                                .enumerate()
                                .find(|(_, (fname, _))| fname == field)
                            {
                                // GEP para o campo
                                let index_value = self.builder.build_const_int(ir_func, field_idx as i64);
                                let field_ptr = self.builder.build_getelementptr(
                                    ir_func,
                                    *struct_ptr,
                                    index_value,
                                    field_type.clone(),
                                );
                                
                                // Load do campo
                                return self.builder.build_load(ir_func, field_ptr);
                            }
                        }
                    }
                }
                // Se o objeto é um StructLiteral inline, processar diretamente
                else if let ExpressionKind::StructLiteral { name, .. } = &object.kind {
                    let object_ptr = self.lower_expression(object, ir_func);
                    if let Some(field_defs) = self.struct_definitions.get(name) {
                        // Encontrar índice do campo
                        if let Some((field_idx, (_, field_type))) = field_defs
                            .iter()
                            .enumerate()
                            .find(|(_, (fname, _))| fname == field)
                        {
                            // GEP para o campo
                            let index_value = self.builder.build_const_int(ir_func, field_idx as i64);
                            let field_ptr = self.builder.build_getelementptr(
                                ir_func,
                                object_ptr,
                                index_value,
                                field_type.clone(),
                            );
                            
                            // Load do campo
                            return self.builder.build_load(ir_func, field_ptr);
                        }
                    }
                }
                
                // Se não conseguimos determinar, retornar placeholder
                ir_func.next_value()
            }
            ExpressionKind::EnumVariant { enum_name, variant_name, data } => {
                // Buscar definição do enum (clonar para evitar borrow issues)
                let variants_opt = self.enum_definitions.get(enum_name).cloned();
                
                if let Some(variants) = variants_opt {
                    // Encontrar o variant
                    if let Some((_, tag, variant_data_types)) = variants
                        .iter()
                        .find(|(name, _, _)| name == variant_name)
                    {
                        // Se é unit variant, retornar apenas o tag
                        if variant_data_types.is_none() {
                            return self.builder.build_const_int(ir_func, *tag as i64);
                        }
                        
                        // Se é tuple variant, criar tupla (tag, data...)
                        if let Some(data_exprs) = data {
                            let mut elements = Vec::new();
                            
                            // Primeiro elemento: tag
                            elements.push(self.builder.build_const_int(ir_func, *tag as i64));
                            
                            // Demais elementos: dados do variant
                            for data_expr in data_exprs {
                                elements.push(self.lower_expression(data_expr, ir_func));
                            }
                            
                            // Criar tipos da tupla
                            let mut element_types = vec![IRType::Int];
                            if let Some(data_types) = variant_data_types {
                                element_types.extend(data_types.clone());
                            }
                            
                            let tuple_type = IRType::Tuple {
                                elements: element_types.clone(),
                            };
                            
                            // Alocar tupla no stack
                            let tuple_ptr = self.builder.build_alloca(ir_func, tuple_type.clone());
                            
                            // Store cada elemento
                            for (idx, elem_value) in elements.iter().enumerate() {
                                let index_value = self.builder.build_const_int(ir_func, idx as i64);
                                let elem_ptr = self.builder.build_getelementptr(
                                    ir_func,
                                    tuple_ptr,
                                    index_value,
                                    element_types[idx].clone(),
                                );
                                self.builder.build_store(ir_func, elem_ptr, *elem_value);
                            }
                            
                            return tuple_ptr;
                        }
                        
                        // Variant com dados mas sem argumentos fornecidos - erro
                        return self.builder.build_const_int(ir_func, *tag as i64);
                    }
                }
                
                // Enum ou variant não encontrado
                ir_func.next_value()
            }
        }
    }

    fn lower_type_annotation(&self, type_ann: &TypeAnnotation) -> IRType {
        use spectra_compiler::ast::TypeAnnotationKind;
        
        match &type_ann.kind {
            TypeAnnotationKind::Simple { segments } => {
                if segments.is_empty() {
                    return IRType::Void;
                }
                
                match segments[0].as_str() {
                    "int" => IRType::Int,
                    "float" => IRType::Float,
                    "bool" => IRType::Bool,
                    "string" => IRType::String,
                    "char" => IRType::Char,
                    _ => IRType::Void,
                }
            }
            TypeAnnotationKind::Tuple { elements } => {
                let ir_elements: Vec<IRType> = elements
                    .iter()
                    .map(|elem_ann| self.lower_type_annotation(elem_ann))
                    .collect();
                IRType::Tuple { elements: ir_elements }
            }
        }
    }

    #[allow(dead_code)]
    fn lower_type(&self, ast_type: &ASTType) -> IRType {
        match ast_type {
            ASTType::Int => IRType::Int,
            ASTType::Float => IRType::Float,
            ASTType::Bool => IRType::Bool,
            ASTType::String => IRType::String,
            ASTType::Char => IRType::Char,
            ASTType::Unit => IRType::Void,
            ASTType::Unknown => IRType::Void,
            ASTType::Array { element_type, .. } => {
                // Arrays são representados como ponteiros no IR
                IRType::Pointer(Box::new(self.lower_type(element_type)))
            }
            ASTType::Tuple { elements } => {
                // Converter cada tipo do elemento
                let ir_elements: Vec<IRType> = elements
                    .iter()
                    .map(|elem_type| self.lower_type(elem_type))
                    .collect();
                IRType::Tuple { elements: ir_elements }
            }
            ASTType::Struct { name: _ } => {
                // Structs são representados como ponteiros
                IRType::Pointer(Box::new(IRType::Void))
            }
            ASTType::Enum { name } => {
                // Enums são representados como tagged unions
                // Para simplificar, vamos representar como uma tupla ou int
                // dependendo se tem dados ou não
                if let Some(variants) = self.enum_definitions.get(name) {
                    // Se todos os variants são unit, usar int
                    let all_unit = variants.iter().all(|(_, _, data)| data.is_none());
                    if all_unit {
                        IRType::Int
                    } else {
                        // Se algum tem dados, precisa de tupla dinâmica
                        // Por simplificação, usar ponteiro genérico
                        IRType::Pointer(Box::new(IRType::Void))
                    }
                } else {
                    // Enum não encontrado, usar int como fallback
                    IRType::Int
                }
            }
        }
    }
}

impl Default for ASTLowering {
    fn default() -> Self {
        Self::new()
    }
}
