use std::collections::HashSet;

use spectra_compiler::{
    ast::{
        AssignmentStatement, Block, Expression, ExpressionKind, Function, FunctionParam, Item,
        LValue, LetStatement, Module, Statement, StatementKind, TypeAnnotation, TypeAnnotationKind,
    },
    error::{LexError, ParseError},
    lexer::Lexer,
    parser::Parser,
    span::{Location, Span},
};

#[derive(Debug)]
pub enum NavigationError {
    Lex(Vec<LexError>),
    Parse(Vec<ParseError>),
}

#[derive(Debug, Clone, Copy)]
pub enum SymbolKind {
    Function,
    Variable,
    Parameter,
}

impl SymbolKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Variable => "variable",
            SymbolKind::Parameter => "parameter",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub detail: String,
    pub definition_span: Span,
    pub reference_span: Span,
}

pub fn resolve_symbol(
    source: &str,
    position: Location,
) -> Result<Option<ResolvedSymbol>, NavigationError> {
    let tokens = Lexer::new(source)
        .tokenize()
        .map_err(NavigationError::Lex)?;

    let module = Parser::new(tokens, HashSet::new())
        .parse()
        .map_err(NavigationError::Parse)?;

    let mut resolver = SymbolResolver::new(source, position);
    Ok(resolver.resolve_module(&module))
}

struct SymbolResolver<'a> {
    source: &'a str,
    target: Location,
    result: Option<ResolvedSymbol>,
    scope_stack: Vec<Vec<SymbolDefinition>>,
}

impl<'a> SymbolResolver<'a> {
    fn new(source: &'a str, target: Location) -> Self {
        Self {
            source,
            target,
            result: None,
            scope_stack: vec![Vec::new()],
        }
    }

    fn resolve_module(&mut self, module: &'a Module) -> Option<ResolvedSymbol> {
        // Register top-level functions for lookup before traversing bodies.
        for item in &module.items {
            if let Item::Function(function) = item {
                if let Some(name_span) = self.function_name_span(function) {
                    let detail = format_function_signature(function);
                    if let Some(global_scope) = self.scope_stack.get_mut(0) {
                        global_scope.push(SymbolDefinition::new_function(
                            function.name.clone(),
                            name_span,
                            detail,
                        ));
                    }
                }
            }
        }

        for item in &module.items {
            self.visit_item(item);
            if self.result.is_some() {
                break;
            }
        }

        self.result.clone()
    }

    fn visit_item(&mut self, item: &'a Item) {
        if self.result.is_some() {
            return;
        }

        match item {
            Item::Function(function) => self.visit_function(function),
            Item::Import(_)
            | Item::Struct(_)
            | Item::Enum(_)
            | Item::Impl(_)
            | Item::Trait(_)
            | Item::TraitImpl(_) => {}
        }
    }

    fn visit_function(&mut self, function: &'a Function) {
        if self.result.is_some() {
            return;
        }

        let name_span = self.function_name_span(function).unwrap_or(function.span);
        let signature = format_function_signature(function);

        if location_in_span(self.target, &name_span) {
            self.result = Some(ResolvedSymbol {
                name: function.name.clone(),
                kind: SymbolKind::Function,
                detail: signature.clone(),
                definition_span: name_span,
                reference_span: name_span,
            });
            return;
        }

        self.enter_scope();

        for param in &function.params {
            let detail = format_parameter_detail(param);
            if location_in_span(self.target, &param.span) {
                self.result = Some(ResolvedSymbol {
                    name: param.name.clone(),
                    kind: SymbolKind::Parameter,
                    detail: detail.clone(),
                    definition_span: param.span,
                    reference_span: param.span,
                });
                self.push_definition(SymbolDefinition::new_parameter(
                    param.name.clone(),
                    param.span,
                    detail,
                ));
                self.exit_scope();
                return;
            }

            self.push_definition(SymbolDefinition::new_parameter(
                param.name.clone(),
                param.span,
                detail,
            ));
        }

        for statement in &function.body.statements {
            self.visit_statement(statement);
            if self.result.is_some() {
                break;
            }
        }

        self.exit_scope();
    }

    fn visit_statement(&mut self, statement: &'a Statement) {
        if self.result.is_some() {
            return;
        }

        match &statement.kind {
            StatementKind::Let(let_stmt) => self.visit_let(let_stmt),
            StatementKind::Assignment(assignment) => {
                self.visit_assignment(assignment);
            }
            StatementKind::Expression(expr) => self.visit_expression(expr),
            StatementKind::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.visit_expression(value);
                }
            }
            StatementKind::While(loop_stmt) => {
                self.visit_expression(&loop_stmt.condition);
                self.visit_block(&loop_stmt.body);
            }
            StatementKind::DoWhile(loop_stmt) => {
                self.visit_block(&loop_stmt.body);
                self.visit_expression(&loop_stmt.condition);
            }
            StatementKind::For(loop_stmt) => {
                self.visit_expression(&loop_stmt.iterable);
                self.enter_scope();
                self.visit_block_existing_scope(&loop_stmt.body);
                self.exit_scope();
            }
            StatementKind::Loop(loop_stmt) => {
                self.visit_block(&loop_stmt.body);
            }
            StatementKind::Switch(switch_stmt) => {
                self.visit_expression(&switch_stmt.value);
                for case in &switch_stmt.cases {
                    self.enter_scope();
                    self.visit_block(&case.body);
                    self.exit_scope();
                }
                if let Some(default_block) = &switch_stmt.default {
                    self.visit_block(default_block);
                }
            }
            StatementKind::Break | StatementKind::Continue => {}
        }
    }

    fn visit_assignment(&mut self, assignment: &'a AssignmentStatement) {
        if self.result.is_some() {
            return;
        }

        match &assignment.target {
            LValue::Identifier(name) => {
                self.handle_identifier(&assignment.target_span, name);
            }
            LValue::IndexAccess { array, index } => {
                self.visit_expression(array);
                self.visit_expression(index);
            }
        }

        self.visit_expression(&assignment.value);
    }

    fn visit_let(&mut self, let_stmt: &'a LetStatement) {
        let detail = format_variable_detail(let_stmt);

        if location_in_span(self.target, &let_stmt.span) {
            self.result = Some(ResolvedSymbol {
                name: let_stmt.name.clone(),
                kind: SymbolKind::Variable,
                detail: detail.clone(),
                definition_span: let_stmt.span,
                reference_span: let_stmt.span,
            });
        }

        if let Some(value) = &let_stmt.value {
            self.visit_expression(value);
        }

        self.push_definition(SymbolDefinition::new_variable(
            let_stmt.name.clone(),
            let_stmt.span,
            detail,
        ));
    }

    fn visit_block(&mut self, block: &'a Block) {
        self.enter_scope();
        self.visit_block_existing_scope(block);
        self.exit_scope();
    }

    fn visit_block_existing_scope(&mut self, block: &'a Block) {
        for statement in &block.statements {
            self.visit_statement(statement);
            if self.result.is_some() {
                break;
            }
        }
    }

    fn visit_expression(&mut self, expression: &'a Expression) {
        if self.result.is_some() {
            return;
        }

        match &expression.kind {
            ExpressionKind::Identifier(name) => {
                self.handle_identifier(&expression.span, name);
            }
            ExpressionKind::NumberLiteral(_)
            | ExpressionKind::StringLiteral(_)
            | ExpressionKind::BoolLiteral(_) => {}
            ExpressionKind::Binary { left, right, .. } => {
                self.visit_expression(left);
                self.visit_expression(right);
            }
            ExpressionKind::Unary { operand, .. } => {
                self.visit_expression(operand);
            }
            ExpressionKind::Call { callee, arguments } => {
                self.visit_expression(callee);
                for argument in arguments {
                    self.visit_expression(argument);
                }
            }
            ExpressionKind::Grouping(inner) => self.visit_expression(inner),
            ExpressionKind::ArrayLiteral { elements } => {
                for element in elements {
                    self.visit_expression(element);
                }
            }
            ExpressionKind::IndexAccess { array, index } => {
                self.visit_expression(array);
                self.visit_expression(index);
            }
            ExpressionKind::TupleLiteral { elements } => {
                for element in elements {
                    self.visit_expression(element);
                }
            }
            ExpressionKind::TupleAccess { tuple, .. } => {
                self.visit_expression(tuple);
            }
            ExpressionKind::StructLiteral { fields, .. } => {
                for (_, value) in fields {
                    self.visit_expression(value);
                }
            }
            ExpressionKind::FieldAccess { object, .. } => {
                self.visit_expression(object);
            }
            ExpressionKind::EnumVariant { data, .. } => {
                if let Some(values) = data {
                    for value in values {
                        self.visit_expression(value);
                    }
                }
            }
            ExpressionKind::Match { scrutinee, arms } => {
                self.visit_expression(scrutinee);
                for arm in arms {
                    self.enter_scope();
                    self.visit_expression(&arm.body);
                    self.exit_scope();
                }
            }
            ExpressionKind::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                self.visit_expression(condition);
                self.visit_block(then_block);
                for (elif_condition, elif_block) in elif_blocks {
                    self.visit_expression(elif_condition);
                    self.visit_block(elif_block);
                }
                if let Some(block) = else_block {
                    self.visit_block(block);
                }
            }
            ExpressionKind::Unless {
                condition,
                then_block,
                else_block,
            } => {
                self.visit_expression(condition);
                self.visit_block(then_block);
                if let Some(block) = else_block {
                    self.visit_block(block);
                }
            }
            ExpressionKind::MethodCall {
                object, arguments, ..
            } => {
                self.visit_expression(object);
                for argument in arguments {
                    self.visit_expression(argument);
                }
            }
        }
    }

    fn handle_identifier(&mut self, span: &Span, name: &str) {
        if self.result.is_some() {
            return;
        }

        if !location_in_span(self.target, span) {
            return;
        }

        if let Some(definition) = self.lookup_definition(name) {
            self.result = Some(ResolvedSymbol {
                name: definition.name.clone(),
                kind: definition.kind,
                detail: definition.detail.clone(),
                definition_span: definition.span,
                reference_span: *span,
            });
        }
    }

    fn lookup_definition(&self, name: &str) -> Option<SymbolDefinition> {
        for scope in self.scope_stack.iter().rev() {
            if let Some(definition) = scope
                .iter()
                .rev()
                .find(|definition| definition.name == name)
            {
                return Some(definition.clone());
            }
        }
        None
    }

    fn push_definition(&mut self, definition: SymbolDefinition) {
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.push(definition);
        }
    }

    fn enter_scope(&mut self) {
        self.scope_stack.push(Vec::new());
    }

    fn exit_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn function_name_span(&self, function: &Function) -> Option<Span> {
        find_identifier_span_in_source(self.source, function.span, &function.name)
    }
}

#[derive(Clone)]
struct SymbolDefinition {
    name: String,
    span: Span,
    detail: String,
    kind: SymbolKind,
}

impl SymbolDefinition {
    fn new_variable(name: String, span: Span, detail: String) -> Self {
        Self {
            name,
            span,
            detail,
            kind: SymbolKind::Variable,
        }
    }

    fn new_parameter(name: String, span: Span, detail: String) -> Self {
        Self {
            name,
            span,
            detail,
            kind: SymbolKind::Parameter,
        }
    }

    fn new_function(name: String, span: Span, detail: String) -> Self {
        Self {
            name,
            span,
            detail,
            kind: SymbolKind::Function,
        }
    }
}

fn location_in_span(location: Location, span: &Span) -> bool {
    if location.line < span.start_location.line {
        return false;
    }

    if location.line == span.start_location.line && location.column < span.start_location.column {
        return false;
    }

    if location.line > span.end_location.line {
        return false;
    }

    if location.line == span.end_location.line && location.column >= span.end_location.column {
        return false;
    }

    true
}

fn format_variable_detail(let_stmt: &LetStatement) -> String {
    let mut detail = format!("let {}", let_stmt.name);
    if let Some(ty) = &let_stmt.ty {
        detail.push_str(": ");
        detail.push_str(&type_annotation_to_string(ty));
    }
    detail
}

fn format_parameter_detail(param: &FunctionParam) -> String {
    let mut detail = format!("param {}", param.name);
    if let Some(ty) = &param.ty {
        detail.push_str(": ");
        detail.push_str(&type_annotation_to_string(ty));
    }
    detail
}

fn format_function_signature(function: &Function) -> String {
    let mut result = String::from("fn ");
    result.push_str(&function.name);

    if !function.type_params.is_empty() {
        result.push('<');
        for (index, param) in function.type_params.iter().enumerate() {
            if index > 0 {
                result.push_str(", ");
            }
            result.push_str(&param.name);
            if !param.bounds.is_empty() {
                result.push_str(": ");
                result.push_str(&param.bounds.join(" + "));
            }
        }
        result.push('>');
    }

    result.push('(');
    for (index, param) in function.params.iter().enumerate() {
        if index > 0 {
            result.push_str(", ");
        }
        result.push_str(&param.name);
        if let Some(ty) = &param.ty {
            result.push_str(": ");
            result.push_str(&type_annotation_to_string(ty));
        }
    }
    result.push(')');

    if let Some(return_type) = &function.return_type {
        result.push_str(" -> ");
        result.push_str(&type_annotation_to_string(return_type));
    }

    result
}

fn type_annotation_to_string(annotation: &TypeAnnotation) -> String {
    match &annotation.kind {
        TypeAnnotationKind::Simple { segments } => segments.join("::"),
        TypeAnnotationKind::Tuple { elements } => {
            let rendered: Vec<String> = elements.iter().map(type_annotation_to_string).collect();
            format!("({})", rendered.join(", "))
        }
    }
}

fn find_identifier_span_in_source(
    source: &str,
    outer_span: Span,
    identifier: &str,
) -> Option<Span> {
    if outer_span.start >= outer_span.end || identifier.is_empty() {
        return None;
    }

    let end = outer_span.end.min(source.len());
    let start = outer_span.start.min(end);
    if start >= end {
        return None;
    }

    let snippet = &source[start..end];
    let mut offset = 0usize;

    while offset < snippet.len() {
        if let Some(relative) = snippet[offset..].find(identifier) {
            let candidate_start = start + offset + relative;
            let candidate_end = candidate_start + identifier.len();

            if identifier_boundary(source, candidate_start, candidate_end) {
                let start_location = offset_to_location(source, candidate_start);
                let end_location = offset_to_location(source, candidate_end);
                return Some(Span::new(
                    candidate_start,
                    candidate_end,
                    start_location,
                    end_location,
                ));
            }

            offset += relative + identifier.len();
        } else {
            break;
        }
    }

    None
}

fn identifier_boundary(source: &str, start: usize, end: usize) -> bool {
    let is_ident = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';

    if start > 0 {
        if let Some(prev) = source[..start].chars().rev().next() {
            if is_ident(prev) {
                return false;
            }
        }
    }

    if end < source.len() {
        if let Some(next) = source[end..].chars().next() {
            if is_ident(next) {
                return false;
            }
        }
    }

    true
}

fn offset_to_location(source: &str, offset: usize) -> Location {
    let mut line = 1usize;
    let mut column = 1usize;

    for ch in source[..offset].chars() {
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    Location::new(line, column)
}
