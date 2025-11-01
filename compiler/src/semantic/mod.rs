use crate::{
    ast::{
        Block, Expression, ExpressionKind, Function, Item, Module, Statement, StatementKind, Type,
    },
    error::SemanticError,
    span::Span,
};
use std::collections::HashMap;

pub fn analyze_modules(modules: &[&Module]) -> Result<(), Vec<SemanticError>> {
    let mut errors = Vec::new();

    for module in modules {
        let mut analyzer = SemanticAnalyzer::new();
        analyzer.analyze_module(module);
        errors.extend(analyzer.errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Debug, Clone)]
struct SymbolInfo {
    #[allow(dead_code)]
    span: Span,
    ty: Type,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    params: Vec<Type>,
    return_type: Type,
}

pub struct SemanticAnalyzer {
    errors: Vec<SemanticError>,
    // Symbol table: maps variable/function names to their type info
    symbols: Vec<HashMap<String, SymbolInfo>>,
    // Function table: maps function names to their signatures
    functions: HashMap<String, FunctionSignature>,
    // Track if we're inside a loop (for break/continue validation)
    loop_depth: usize,
    // Track if we're inside a function (for return validation)
    current_function: Option<String>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            symbols: vec![HashMap::new()], // Start with global scope
            functions: HashMap::new(),
            loop_depth: 0,
            current_function: None,
        }
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.errors.push(SemanticError::new(message, span));
    }

    fn push_scope(&mut self) {
        self.symbols.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.symbols.pop();
    }

    fn type_annotation_to_type(&self, type_ann: &Option<crate::ast::TypeAnnotation>) -> Type {
        use crate::ast::TypeAnnotationKind;
        
        match type_ann {
            Some(ann) => match &ann.kind {
                TypeAnnotationKind::Simple { segments } if segments.len() == 1 => {
                    match segments[0].as_str() {
                        "int" => Type::Int,
                        "float" => Type::Float,
                        "bool" => Type::Bool,
                        "string" => Type::String,
                        "char" => Type::Char,
                        _ => Type::Unknown,
                    }
                }
                TypeAnnotationKind::Tuple { elements } => {
                    let element_types: Vec<Type> = elements
                        .iter()
                        .map(|elem_ann| self.type_annotation_to_type(&Some(elem_ann.clone())))
                        .collect();
                    Type::Tuple { elements: element_types }
                }
                _ => Type::Unknown,
            },
            None => Type::Unknown,
        }
    }

    fn declare_symbol(&mut self, name: String, span: Span, ty: Type) -> bool {
        // Check if already declared in current scope
        if let Some(current_scope) = self.symbols.last_mut() {
            if current_scope.contains_key(&name) {
                return false; // Already declared
            }
            current_scope.insert(name, SymbolInfo { span, ty });
            true
        } else {
            false
        }
    }

    fn lookup_symbol(&self, name: &str) -> Option<&SymbolInfo> {
        // Search from innermost to outermost scope
        for scope in self.symbols.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }

    pub fn analyze_module(&mut self, module: &Module) -> Vec<SemanticError> {
        // First pass: collect all function declarations
        for item in &module.items {
            if let Item::Function(func) = item {
                if self.functions.contains_key(&func.name) {
                    self.error(
                        format!("Function '{}' is already defined", func.name),
                        func.span,
                    );
                } else {
                    // Extract parameter types
                    let params: Vec<Type> = func
                        .params
                        .iter()
                        .map(|p| self.type_annotation_to_type(&p.ty))
                        .collect();

                    // Extract return type
                    let return_type = self.type_annotation_to_type(&func.return_type);

                    let signature = FunctionSignature {
                        params,
                        return_type: return_type.clone(),
                    };

                    self.functions.insert(func.name.clone(), signature);
                    self.declare_symbol(func.name.clone(), func.span, return_type);
                }
            }
        }

        // Second pass: analyze function bodies
        for item in &module.items {
            self.analyze_item(item);
        }

        // Return collected errors
        std::mem::take(&mut self.errors)
    }

    fn analyze_item(&mut self, item: &Item) {
        match item {
            Item::Import(_) => {
                // Import analysis would go here
            }
            Item::Function(func) => {
                self.analyze_function(func);
            }
            Item::Struct(_struct) => {
                // TODO: Struct validation (unique field names, etc.)
            }
        }
    }

    fn analyze_function(&mut self, func: &Function) {
        self.current_function = Some(func.name.clone());
        self.push_scope();

        // Declare parameters in function scope
        for param in &func.params {
            let param_type = self.type_annotation_to_type(&param.ty);
            if !self.declare_symbol(param.name.clone(), param.span, param_type) {
                self.error(
                    format!("Parameter '{}' is already declared", param.name),
                    param.span,
                );
            }
        }

        // Analyze function body
        self.analyze_block(&func.body);

        self.pop_scope();
        self.current_function = None;
    }

    fn analyze_block(&mut self, block: &Block) {
        self.push_scope();

        for statement in &block.statements {
            self.analyze_statement(statement);
        }

        self.pop_scope();
    }

    fn analyze_statement(&mut self, statement: &Statement) {
        match &statement.kind {
            StatementKind::Let(let_stmt) => {
                // Infer type from value expression or annotation
                let inferred_type = if let Some(ref value) = let_stmt.value {
                    self.infer_expression_type(value)
                } else {
                    self.type_annotation_to_type(&let_stmt.ty)
                };

                // Check if value expression is valid (if present)
                if let Some(ref value) = let_stmt.value {
                    self.analyze_expression(value);
                }

                // Declare the variable with its type
                if !self.declare_symbol(let_stmt.name.clone(), let_stmt.span, inferred_type) {
                    self.error(
                        format!(
                            "Variable '{}' is already declared in this scope",
                            let_stmt.name
                        ),
                        let_stmt.span,
                    );
                }
            }
            StatementKind::Assignment(assign_stmt) => {
                // Analyze the target (lvalue)
                match &assign_stmt.target {
                    crate::ast::LValue::Identifier(name) => {
                        // Check if variable exists
                        if self.lookup_symbol(name).is_none() {
                            self.error(
                                format!("Variable '{}' is not defined", name),
                                assign_stmt.target_span,
                            );
                        }
                    }
                    crate::ast::LValue::IndexAccess { array, index } => {
                        // Analyze array and index expressions
                        self.analyze_expression(array);
                        self.analyze_expression(index);
                        
                        // Check that index is an integer
                        let index_type = self.infer_expression_type(index);
                        if !matches!(index_type, Type::Int | Type::Unknown) {
                            self.error(
                                format!("Array index must be an integer, found {:?}", index_type),
                                assign_stmt.target_span,
                            );
                        }
                    }
                }
                
                // Analyze the value expression
                self.analyze_expression(&assign_stmt.value);

                // TODO: Check type compatibility between target and assigned value
            }
            StatementKind::Return(ret_stmt) => {
                if self.current_function.is_none() {
                    self.error("Return statement outside of function", ret_stmt.span);
                }

                if let Some(ref value) = ret_stmt.value {
                    self.analyze_expression(value);
                }
            }
            StatementKind::Expression(expr) => {
                self.analyze_expression(expr);
            }
            StatementKind::While(while_loop) => {
                self.analyze_expression(&while_loop.condition);
                self.loop_depth += 1;
                self.analyze_block(&while_loop.body);
                self.loop_depth -= 1;
            }
            StatementKind::DoWhile(do_while_loop) => {
                self.loop_depth += 1;
                self.analyze_block(&do_while_loop.body);
                self.loop_depth -= 1;
                self.analyze_expression(&do_while_loop.condition);
            }
            StatementKind::For(for_loop) => {
                self.push_scope();

                // Analyze iterable expression
                self.analyze_expression(&for_loop.iterable);

                // Declare iterator variable (type is inferred from iterable)
                let iterator_type = Type::Unknown; // TODO: inferir do tipo do iterável
                if !self.declare_symbol(for_loop.iterator.clone(), for_loop.span, iterator_type) {
                    self.error(
                        format!(
                            "Iterator variable '{}' conflicts with existing declaration",
                            for_loop.iterator
                        ),
                        for_loop.span,
                    );
                }

                // Analyze loop body
                self.loop_depth += 1;
                self.analyze_block(&for_loop.body);
                self.loop_depth -= 1;

                self.pop_scope();
            }
            StatementKind::Loop(loop_stmt) => {
                self.loop_depth += 1;
                self.analyze_block(&loop_stmt.body);
                self.loop_depth -= 1;
            }
            StatementKind::Switch(switch_stmt) => {
                // Analyze the value being switched on
                self.analyze_expression(&switch_stmt.value);

                // Analyze each case
                for case in &switch_stmt.cases {
                    self.analyze_expression(&case.pattern);
                    self.analyze_block(&case.body);
                }

                // Analyze default case if present
                if let Some(ref default_block) = switch_stmt.default {
                    self.analyze_block(default_block);
                }
            }
            StatementKind::Break => {
                if self.loop_depth == 0 {
                    self.error("Break statement outside of loop", statement.span);
                }
            }
            StatementKind::Continue => {
                if self.loop_depth == 0 {
                    self.error("Continue statement outside of loop", statement.span);
                }
            }
        }
    }

    fn infer_expression_type(&mut self, expr: &Expression) -> Type {
        match &expr.kind {
            ExpressionKind::NumberLiteral(num) => {
                if num.contains('.') {
                    Type::Float
                } else {
                    Type::Int
                }
            }
            ExpressionKind::StringLiteral(_) => Type::String,
            ExpressionKind::BoolLiteral(_) => Type::Bool,
            ExpressionKind::Identifier(name) => {
                if let Some(info) = self.lookup_symbol(name) {
                    info.ty.clone()
                } else if let Some(sig) = self.functions.get(name) {
                    sig.return_type.clone()
                } else {
                    Type::Unknown
                }
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => {
                let left_type = self.infer_expression_type(left);
                let right_type = self.infer_expression_type(right);

                use crate::ast::BinaryOperator;
                match operator {
                    BinaryOperator::Add => {
                        // If either operand is string, result is string (concatenation)
                        if matches!(left_type, Type::String) || matches!(right_type, Type::String) {
                            Type::String
                        } else {
                            left_type
                        }
                    }
                    BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo => left_type,
                    BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::Greater
                    | BinaryOperator::LessEqual
                    | BinaryOperator::GreaterEqual
                    | BinaryOperator::And
                    | BinaryOperator::Or => Type::Bool,
                }
            }
            ExpressionKind::Unary {
                operator: _,
                operand,
            } => self.infer_expression_type(operand),
            ExpressionKind::Call {
                callee,
                arguments: _,
            } => {
                if let ExpressionKind::Identifier(name) = &callee.kind {
                    if let Some(sig) = self.functions.get(name) {
                        return sig.return_type.clone();
                    }
                }
                Type::Unknown
            }
            ExpressionKind::If { .. } => Type::Unknown, // TODO: inferir tipo comum dos ramos
            ExpressionKind::Unless { .. } => Type::Unknown, // TODO: inferir tipo comum dos ramos
            ExpressionKind::Grouping(inner) => self.infer_expression_type(inner),
            ExpressionKind::ArrayLiteral { elements } => {
                if elements.is_empty() {
                    // Array vazio, tipo desconhecido
                    Type::Array {
                        element_type: Box::new(Type::Unknown),
                        size: Some(0),
                    }
                } else {
                    // Inferir tipo do primeiro elemento
                    let elem_type = self.infer_expression_type(&elements[0]);
                    Type::Array {
                        element_type: Box::new(elem_type),
                        size: Some(elements.len()),
                    }
                }
            }
            ExpressionKind::IndexAccess { array, index: _ } => {
                let array_type = self.infer_expression_type(array);
                match array_type {
                    Type::Array { element_type, .. } => *element_type,
                    _ => Type::Unknown,
                }
            }
            ExpressionKind::TupleLiteral { elements } => {
                if elements.is_empty() {
                    // Empty tuple - unit type
                    Type::Tuple { elements: vec![] }
                } else {
                    // Infer type of each element
                    let element_types: Vec<Type> = elements
                        .iter()
                        .map(|e| self.infer_expression_type(e))
                        .collect();
                    Type::Tuple { elements: element_types }
                }
            }
            ExpressionKind::TupleAccess { tuple, index } => {
                let tuple_type = self.infer_expression_type(tuple);
                match tuple_type {
                    Type::Tuple { elements } => {
                        if *index < elements.len() {
                            elements[*index].clone()
                        } else {
                            Type::Unknown
                        }
                    }
                    _ => Type::Unknown,
                }
            }
            ExpressionKind::StructLiteral { name, fields: _ } => {
                // TODO: Verificar se struct existe e retornar seu tipo
                Type::Struct { name: name.clone() }
            }
            ExpressionKind::FieldAccess { object: _, field: _ } => {
                // TODO: Inferir tipo do campo baseado no tipo do objeto
                Type::Unknown
            }
        }
    }

    fn analyze_expression(&mut self, expr: &Expression) {
        match &expr.kind {
            ExpressionKind::Identifier(name) => {
                // Check if identifier is declared
                if self.lookup_symbol(name).is_none() && !self.functions.contains_key(name) {
                    self.error(
                        format!("Undefined variable or function '{}'", name),
                        expr.span,
                    );
                }
            }
            ExpressionKind::NumberLiteral(_)
            | ExpressionKind::StringLiteral(_)
            | ExpressionKind::BoolLiteral(_) => {
                // Literals are always valid
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => {
                self.analyze_expression(left);
                self.analyze_expression(right);

                // Type check binary operations
                let left_type = self.infer_expression_type(left);
                let right_type = self.infer_expression_type(right);

                use crate::ast::BinaryOperator;
                match operator {
                    BinaryOperator::Add => {
                        // Add supports both numeric types and string concatenation
                        let is_string_concat = matches!(left_type, Type::String) || matches!(right_type, Type::String);
                        
                        if is_string_concat {
                            // String concatenation - both operands must be strings
                            if !matches!(left_type, Type::String | Type::Unknown) {
                                self.error(
                                    format!("Cannot concatenate non-string type {:?} with string", left_type),
                                    left.span,
                                );
                            }
                            if !matches!(right_type, Type::String | Type::Unknown) {
                                self.error(
                                    format!("Cannot concatenate string with non-string type {:?}", right_type),
                                    right.span,
                                );
                            }
                        } else {
                            // Numeric addition
                            if !matches!(left_type, Type::Int | Type::Float | Type::Unknown) {
                                self.error(
                                    format!("Left operand of arithmetic operation must be numeric, found {:?}", left_type),
                                    left.span,
                                );
                            }
                            if !matches!(right_type, Type::Int | Type::Float | Type::Unknown) {
                                self.error(
                                    format!("Right operand of arithmetic operation must be numeric, found {:?}", right_type),
                                    right.span,
                                );
                            }
                        }
                    }
                    BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo => {
                        // Arithmetic operations require numeric types
                        if !matches!(left_type, Type::Int | Type::Float | Type::Unknown) {
                            self.error(
                                format!("Left operand of arithmetic operation must be numeric, found {:?}", left_type),
                                left.span,
                            );
                        }
                        if !matches!(right_type, Type::Int | Type::Float | Type::Unknown) {
                            self.error(
                                format!("Right operand of arithmetic operation must be numeric, found {:?}", right_type),
                                right.span,
                            );
                        }
                        // Check if types match
                        if left_type != Type::Unknown
                            && right_type != Type::Unknown
                            && left_type != right_type
                        {
                            self.error(
                                format!(
                                    "Type mismatch in arithmetic operation: {:?} and {:?}",
                                    left_type, right_type
                                ),
                                expr.span,
                            );
                        }
                    }
                    BinaryOperator::Equal | BinaryOperator::NotEqual => {
                        // Equality can compare any types, but they should match
                        if left_type != Type::Unknown
                            && right_type != Type::Unknown
                            && left_type != right_type
                        {
                            self.error(
                                format!(
                                    "Type mismatch in equality comparison: {:?} and {:?}",
                                    left_type, right_type
                                ),
                                expr.span,
                            );
                        }
                    }
                    BinaryOperator::Less
                    | BinaryOperator::Greater
                    | BinaryOperator::LessEqual
                    | BinaryOperator::GreaterEqual => {
                        // Comparison requires numeric types
                        if !matches!(left_type, Type::Int | Type::Float | Type::Unknown) {
                            self.error(
                                format!(
                                    "Left operand of comparison must be numeric, found {:?}",
                                    left_type
                                ),
                                left.span,
                            );
                        }
                        if !matches!(right_type, Type::Int | Type::Float | Type::Unknown) {
                            self.error(
                                format!(
                                    "Right operand of comparison must be numeric, found {:?}",
                                    right_type
                                ),
                                right.span,
                            );
                        }
                    }
                    BinaryOperator::And | BinaryOperator::Or => {
                        // Logical operations require boolean types
                        if !matches!(left_type, Type::Bool | Type::Unknown) {
                            self.error(
                                format!(
                                    "Left operand of logical operation must be boolean, found {:?}",
                                    left_type
                                ),
                                left.span,
                            );
                        }
                        if !matches!(right_type, Type::Bool | Type::Unknown) {
                            self.error(
                                format!("Right operand of logical operation must be boolean, found {:?}", right_type),
                                right.span,
                            );
                        }
                    }
                }
            }
            ExpressionKind::Unary { operand, .. } => {
                self.analyze_expression(operand);
            }
            ExpressionKind::Call { callee, arguments } => {
                // Check if function exists and validate argument types
                if let ExpressionKind::Identifier(name) = &callee.kind {
                    if let Some(signature) = self.functions.get(name).cloned() {
                        // Validate number of arguments
                        if arguments.len() != signature.params.len() {
                            self.error(
                                format!(
                                    "Function '{}' expects {} arguments, but {} were provided",
                                    name,
                                    signature.params.len(),
                                    arguments.len()
                                ),
                                expr.span,
                            );
                        } else {
                            // Validate argument types
                            for (i, (arg, expected_type)) in
                                arguments.iter().zip(&signature.params).enumerate()
                            {
                                let arg_type = self.infer_expression_type(arg);
                                if arg_type != Type::Unknown
                                    && *expected_type != Type::Unknown
                                    && arg_type != *expected_type
                                {
                                    self.error(
                                        format!(
                                            "Argument {} of function '{}' has type {:?}, expected {:?}",
                                            i + 1,
                                            name,
                                            arg_type,
                                            expected_type
                                        ),
                                        arg.span,
                                    );
                                }
                            }
                        }
                    } else if self.lookup_symbol(name).is_none() {
                        self.error(format!("Undefined function '{}'", name), callee.span);
                    }
                } else {
                    self.analyze_expression(callee);
                }

                // Analyze arguments
                for arg in arguments {
                    self.analyze_expression(arg);
                }
            }
            ExpressionKind::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                self.analyze_expression(condition);
                self.analyze_block(then_block);

                for (elif_cond, elif_body) in elif_blocks {
                    self.analyze_expression(elif_cond);
                    self.analyze_block(elif_body);
                }

                if let Some(ref else_body) = else_block {
                    self.analyze_block(else_body);
                }
            }
            ExpressionKind::Unless {
                condition,
                then_block,
                else_block,
            } => {
                self.analyze_expression(condition);
                self.analyze_block(then_block);

                if let Some(ref else_body) = else_block {
                    self.analyze_block(else_body);
                }
            }
            ExpressionKind::Grouping(inner) => {
                self.analyze_expression(inner);
            }
            ExpressionKind::ArrayLiteral { elements } => {
                // Analyze all elements
                for element in elements {
                    self.analyze_expression(element);
                }
                
                // Check that all elements have the same type
                if !elements.is_empty() {
                    let first_type = self.infer_expression_type(&elements[0]);
                    for (i, element) in elements.iter().enumerate().skip(1) {
                        let elem_type = self.infer_expression_type(element);
                        if first_type != Type::Unknown && elem_type != Type::Unknown && first_type != elem_type {
                            self.error(
                                format!(
                                    "Array element {} has type {:?}, expected {:?}",
                                    i, elem_type, first_type
                                ),
                                element.span,
                            );
                        }
                    }
                }
            }
            ExpressionKind::IndexAccess { array, index } => {
                self.analyze_expression(array);
                self.analyze_expression(index);
                
                // Check that index is an integer
                let index_type = self.infer_expression_type(index);
                if !matches!(index_type, Type::Int | Type::Unknown) {
                    self.error(
                        format!("Array index must be an integer, found {:?}", index_type),
                        index.span,
                    );
                }
                
                // Check that array is actually an array
                let array_type = self.infer_expression_type(array);
                if !matches!(array_type, Type::Array { .. } | Type::Unknown) {
                    self.error(
                        format!("Cannot index into non-array type {:?}", array_type),
                        array.span,
                    );
                }
            }
            ExpressionKind::TupleLiteral { elements } => {
                // Analyze all elements
                for element in elements {
                    self.analyze_expression(element);
                }
            }
            ExpressionKind::TupleAccess { tuple, index } => {
                self.analyze_expression(tuple);
                
                // Check that tuple is actually a tuple
                let tuple_type = self.infer_expression_type(tuple);
                match tuple_type {
                    Type::Tuple { elements } => {
                        if *index >= elements.len() {
                            self.error(
                                format!(
                                    "Tuple index {} out of bounds (tuple has {} elements)",
                                    index,
                                    elements.len()
                                ),
                                tuple.span,
                            );
                        }
                    }
                    Type::Unknown => {
                        // Can't validate, but don't error
                    }
                    _ => {
                        self.error(
                            format!("Cannot access tuple element on non-tuple type {:?}", tuple_type),
                            tuple.span,
                        );
                    }
                }
            }
            ExpressionKind::StructLiteral { name: _, fields } => {
                // TODO: Validar struct existe e campos são corretos
                for (_field_name, field_value) in fields {
                    self.analyze_expression(field_value);
                }
            }
            ExpressionKind::FieldAccess { object, field: _ } => {
                // TODO: Validar campo existe no struct
                self.analyze_expression(object);
            }
        }
    }
}
