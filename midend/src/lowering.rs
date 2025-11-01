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
    loop_stack: Vec<LoopContext>,
}

impl ASTLowering {
    pub fn new() -> Self {
        Self {
            builder: IRBuilder::new(),
            current_function: None,
            value_map: HashMap::new(),
            loop_stack: Vec::new(),
        }
    }

    pub fn lower_module(&mut self, ast_module: &ASTModule) -> IRModule {
        let mut ir_module = IRModule::new(&ast_module.name);

        // Lower all functions
        for item in &ast_module.items {
            if let Item::Function(func) = item {
                let ir_func = self.lower_function(func);
                ir_module.add_function(ir_func);
            }
        }

        ir_module
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
        for (idx, param) in params.iter().enumerate() {
            self.value_map.insert(param.name.clone(), Value { id: idx });
        }

        // Lower function body
        self.current_function = Some(ir_func.clone());
        self.lower_block(&ast_func.body.statements, &mut ir_func);

        // Ensure function has a return
        if let Some(block) = ir_func.get_block_mut(entry_block) {
            if block.terminator.is_none() {
                block.set_terminator(Terminator::Return { value: None });
            }
        }

        ir_func
    }

    fn lower_block(&mut self, statements: &[Statement], ir_func: &mut IRFunction) {
        for stmt in statements {
            self.lower_statement(stmt, ir_func);
        }
    }

    fn lower_statement(&mut self, stmt: &Statement, ir_func: &mut IRFunction) {
        match &stmt.kind {
            StatementKind::Let(let_stmt) => {
                if let Some(ref value_expr) = let_stmt.value {
                    let value = self.lower_expression(value_expr, ir_func);
                    self.value_map.insert(let_stmt.name.clone(), value);
                }
            }
            StatementKind::Assignment(assign) => {
                let value = self.lower_expression(&assign.value, ir_func);
                self.value_map.insert(assign.target.clone(), value);
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
                self.loop_stack.push(LoopContext { header_block, exit_block });
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
                self.loop_stack.push(LoopContext { header_block, exit_block });
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
                self.loop_stack.push(LoopContext { header_block, exit_block });
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
                self.loop_stack.push(LoopContext { header_block: body_block, exit_block });
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
            ExpressionKind::StringLiteral(_) => {
                // TODO: String constant support
                // For now, just create a placeholder value
                let value = ir_func.next_value();
                value
            }
            ExpressionKind::BoolLiteral(b) => {
                self.builder.build_const_bool(ir_func, *b)
            }
            ExpressionKind::Identifier(name) => {
                self.value_map.get(name).copied().unwrap_or_else(|| {
                    // Unknown variable, create placeholder
                    ir_func.next_value()
                })
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
                    UnaryOperator::Not => {
                        self.builder.build_not(ir_func, operand_value)
                    }
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
                if let Some(Statement { kind: StatementKind::Expression(expr), .. }) = then_block.statements.last() {
                    then_value = Some(self.lower_expression(expr, ir_func));
                }
                let then_final_block = self.builder.get_current_block().unwrap_or(then_bb);
                self.builder.build_branch(ir_func, merge_bb);

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
                    if let Some(Statement { kind: StatementKind::Expression(expr), .. }) = else_body.statements.last() {
                        else_value = Some(self.lower_expression(expr, ir_func));
                    }
                }
                let else_final_block = self.builder.get_current_block().unwrap_or(else_bb);
                self.builder.build_branch(ir_func, merge_bb);

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
                if let Some(Statement { kind: StatementKind::Expression(expr), .. }) = then_block.statements.last() {
                    unless_value = Some(self.lower_expression(expr, ir_func));
                }
                let unless_then_final = self.builder.get_current_block().unwrap_or(unless_then_bb);
                self.builder.build_branch(ir_func, unless_merge_bb);

                // Else branch (executes when condition is true)
                self.builder.set_current_block(unless_else_bb);
                let mut unless_else_value = None;
                if let Some(else_body) = else_block {
                    self.lower_block(&else_body.statements, ir_func);
                    if let Some(Statement { kind: StatementKind::Expression(expr), .. }) = else_body.statements.last() {
                        unless_else_value = Some(self.lower_expression(expr, ir_func));
                    }
                }
                let unless_else_final = self.builder.get_current_block().unwrap_or(unless_else_bb);
                self.builder.build_branch(ir_func, unless_merge_bb);

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
        }
    }

    fn lower_type_annotation(&self, type_ann: &TypeAnnotation) -> IRType {
        // For now, simple mapping based on name
        if type_ann.segments.is_empty() {
            return IRType::Void;
        }

        match type_ann.segments[0].as_str() {
            "int" => IRType::Int,
            "float" => IRType::Float,
            "bool" => IRType::Bool,
            "string" => IRType::String,
            "char" => IRType::Char,
            _ => IRType::Void,
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
        }
    }
}

impl Default for ASTLowering {
    fn default() -> Self {
        Self::new()
    }
}
