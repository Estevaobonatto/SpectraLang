use crate::ast::{
    Enum, Function, FunctionParam, ImplBlock, Item, Method, Module, Parameter, Struct,
    TraitImpl, Type, TypeAnnotation, TypeAnnotationKind,
};
use crate::error::CompilerError;
use crate::lint::{lint_module, LintDiagnostic};
use crate::parser::workspace::ModuleLoader;
use crate::pipeline::CompilationOptions;
use crate::semantic::{
    builtin_modules::register_builtin_modules, module_registry::ModuleRegistry, SemanticAnalyzer,
    SymbolInfo,
};
use crate::span::Span;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct DefinitionInfo {
    pub span: Span,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedSymbol {
    pub span: Span,
    pub info: SymbolInfo,
    pub definition: Option<DefinitionInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct DocumentAnalysis {
    pub module: Option<Module>,
    pub diagnostics: Vec<CompilerError>,
    pub warnings: Vec<LintDiagnostic>,
    pub symbols: HashMap<Span, SymbolInfo>,
    pub definitions: HashMap<Span, DefinitionInfo>,
}

impl DocumentAnalysis {
    pub fn symbol_at(&self, line: usize, column: usize) -> Option<ResolvedSymbol> {
        let mut best_match: Option<(Span, &SymbolInfo)> = None;

        for (span, info) in &self.symbols {
            if !span_contains(*span, line, column) {
                continue;
            }

            match best_match {
                Some((current, _)) if span_len(current) <= span_len(*span) => {}
                _ => best_match = Some((*span, info)),
            }
        }

        let (span, info) = best_match?;
        let definition = info
            .def_span
            .and_then(|definition_span| self.definitions.get(&definition_span).cloned())
            .or_else(|| {
                info.def_span.map(|definition_span| DefinitionInfo {
                    span: definition_span,
                    label: type_to_string(&info.ty),
                })
            });

        Some(ResolvedSymbol {
            span,
            info: info.clone(),
            definition,
        })
    }
}

pub fn analyze_document(
    source: &str,
    filename: &str,
    options: &CompilationOptions,
    package_name: Option<String>,
) -> DocumentAnalysis {
    let mut loader = ModuleLoader::new();
    let parse_outcome = loader.parse_module(filename, source, &options.experimental_features);

    let mut analysis = DocumentAnalysis::default();

    let mut module = match parse_outcome {
        Ok(parsed) => parsed.module,
        Err(error) => {
            analysis.diagnostics = match error {
                crate::parser::workspace::ModuleParseError::Lexical(errors) => errors
                    .into_iter()
                    .map(CompilerError::Lexical)
                    .collect(),
                crate::parser::workspace::ModuleParseError::Parse(errors) => errors
                    .into_iter()
                    .map(CompilerError::Parse)
                    .collect(),
            };
            return analysis;
        }
    };

    let registry = {
        let mut reg = ModuleRegistry::new();
        register_builtin_modules(&mut reg);
        Arc::new(RwLock::new(reg))
    };

    let mut semantic = SemanticAnalyzer::new_with_registry(registry, package_name);
    let semantic_errors = semantic.analyze_module(&mut module);
    analysis.symbols = semantic.symbol_resolutions.clone();
    analysis.definitions = build_definition_index(&module);

    if !semantic_errors.is_empty() {
        analysis.diagnostics = semantic_errors
            .into_iter()
            .map(CompilerError::Semantic)
            .collect();
        analysis.module = Some(module);
        return analysis;
    }

    analysis.warnings = lint_module(&module, &options.lint);
    analysis.module = Some(module);
    analysis
}

pub fn type_to_string(ty: &Type) -> String {
    match ty {
        Type::Int => "int".to_string(),
        Type::Float => "float".to_string(),
        Type::Bool => "bool".to_string(),
        Type::String => "string".to_string(),
        Type::Char => "char".to_string(),
        Type::Unit => "unit".to_string(),
        Type::Unknown => "unknown".to_string(),
        Type::Array { element_type, size } => match size {
            Some(size) => format!("[{}; {}]", type_to_string(element_type), size),
            None => format!("[{}]", type_to_string(element_type)),
        },
        Type::Tuple { elements } => format!(
            "({})",
            elements
                .iter()
                .map(type_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Type::Struct { name } => name.clone(),
        Type::Enum { name } => name.clone(),
        Type::TypeParameter { name } => name.clone(),
        Type::SelfType => "Self".to_string(),
        Type::Fn {
            params,
            return_type,
        } => format!(
            "fn({}) -> {}",
            params
                .iter()
                .map(type_to_string)
                .collect::<Vec<_>>()
                .join(", "),
            type_to_string(return_type)
        ),
    }
}

fn span_contains(span: Span, line: usize, column: usize) -> bool {
    let starts_before = line > span.start_location.line
        || (line == span.start_location.line && column >= span.start_location.column);
    let ends_after = line < span.end_location.line
        || (line == span.end_location.line && column < span.end_location.column);
    starts_before && ends_after
}

fn span_len(span: Span) -> usize {
    span.end.saturating_sub(span.start)
}

fn build_definition_index(module: &Module) -> HashMap<Span, DefinitionInfo> {
    let mut definitions = HashMap::new();

    for item in &module.items {
        match item {
            Item::Function(function) => {
                definitions.insert(
                    function.span,
                    DefinitionInfo {
                        span: function.span,
                        label: format_function(function),
                    },
                );
                for param in &function.params {
                    definitions.insert(
                        param.span,
                        DefinitionInfo {
                            span: param.span,
                            label: format_function_param(param),
                        },
                    );
                }
            }
            Item::Struct(struct_def) => {
                index_struct(&mut definitions, struct_def);
            }
            Item::Enum(enum_def) => {
                index_enum(&mut definitions, enum_def);
            }
            Item::Impl(impl_block) => {
                index_impl(&mut definitions, impl_block, None);
            }
            Item::Trait(trait_decl) => {
                definitions.insert(
                    trait_decl.span,
                    DefinitionInfo {
                        span: trait_decl.span,
                        label: format!("trait {}", trait_decl.name),
                    },
                );
            }
            Item::TraitImpl(trait_impl) => {
                definitions.insert(
                    trait_impl.span,
                    DefinitionInfo {
                        span: trait_impl.span,
                        label: format!("impl {} for {}", trait_impl.trait_name, trait_impl.type_name),
                    },
                );
                index_trait_impl(&mut definitions, trait_impl);
            }
            Item::Import(_) => {}
        }
    }

    definitions
}

fn index_struct(definitions: &mut HashMap<Span, DefinitionInfo>, struct_def: &Struct) {
    definitions.insert(
        struct_def.span,
        DefinitionInfo {
            span: struct_def.span,
            label: format!("struct {}", struct_def.name),
        },
    );

    for field in &struct_def.fields {
        definitions.insert(
            field.span,
            DefinitionInfo {
                span: field.span,
                label: format!("field {}: {}", field.name, format_type_annotation(&field.ty)),
            },
        );
    }
}

fn index_enum(definitions: &mut HashMap<Span, DefinitionInfo>, enum_def: &Enum) {
    definitions.insert(
        enum_def.span,
        DefinitionInfo {
            span: enum_def.span,
            label: format!("enum {}", enum_def.name),
        },
    );

    for variant in &enum_def.variants {
        definitions.insert(
            variant.span,
            DefinitionInfo {
                span: variant.span,
                label: format!("variant {}::{}", enum_def.name, variant.name),
            },
        );
    }
}

fn index_impl(
    definitions: &mut HashMap<Span, DefinitionInfo>,
    impl_block: &ImplBlock,
    trait_name: Option<&str>,
) {
    definitions.insert(
        impl_block.span,
        DefinitionInfo {
            span: impl_block.span,
            label: match trait_name.or(impl_block.trait_name.as_deref()) {
                Some(trait_name) => format!("impl {} for {}", trait_name, impl_block.type_name),
                None => format!("impl {}", impl_block.type_name),
            },
        },
    );

    for method in &impl_block.methods {
        definitions.insert(
            method.span,
            DefinitionInfo {
                span: method.span,
                label: format_method(&impl_block.type_name, method),
            },
        );
        for param in &method.params {
            definitions.insert(
                param.span,
                DefinitionInfo {
                    span: param.span,
                    label: format_parameter(param),
                },
            );
        }
    }
}

fn index_trait_impl(definitions: &mut HashMap<Span, DefinitionInfo>, trait_impl: &TraitImpl) {
    for method in &trait_impl.methods {
        definitions.insert(
            method.span,
            DefinitionInfo {
                span: method.span,
                label: format_method(&trait_impl.type_name, method),
            },
        );
        for param in &method.params {
            definitions.insert(
                param.span,
                DefinitionInfo {
                    span: param.span,
                    label: format_parameter(param),
                },
            );
        }
    }
}

fn format_function(function: &Function) -> String {
    let params = function
        .params
        .iter()
        .map(format_function_param)
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = function
        .return_type
        .as_ref()
        .map(format_type_annotation)
        .unwrap_or_else(|| "unit".to_string());
    format!("fn {}({}) -> {}", function.name, params, return_type)
}

fn format_method(type_name: &str, method: &Method) -> String {
    let params = method
        .params
        .iter()
        .map(format_parameter)
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = method
        .return_type
        .as_ref()
        .map(format_type_annotation)
        .unwrap_or_else(|| "unit".to_string());
    format!("fn {}::{}({}) -> {}", type_name, method.name, params, return_type)
}

fn format_function_param(param: &FunctionParam) -> String {
    match &param.ty {
        Some(ty) => format!("{}: {}", param.name, format_type_annotation(ty)),
        None => param.name.clone(),
    }
}

fn format_parameter(param: &Parameter) -> String {
    if param.is_self {
        if param.is_reference {
            if param.is_mutable {
                "&mut self".to_string()
            } else {
                "&self".to_string()
            }
        } else {
            "self".to_string()
        }
    } else {
        match &param.type_annotation {
            Some(ty) => format!("{}: {}", param.name, format_type_annotation(ty)),
            None => param.name.clone(),
        }
    }
}

fn format_type_annotation(annotation: &TypeAnnotation) -> String {
    match &annotation.kind {
        TypeAnnotationKind::Simple { segments } => segments.join("::"),
        TypeAnnotationKind::Tuple { elements } => format!(
            "({})",
            elements
                .iter()
                .map(format_type_annotation)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeAnnotationKind::Function {
            params,
            return_type,
        } => format!(
            "fn({}) -> {}",
            params
                .iter()
                .map(format_type_annotation)
                .collect::<Vec<_>>()
                .join(", "),
            format_type_annotation(return_type)
        ),
        TypeAnnotationKind::Generic { name, type_args } => format!(
            "{}<{}>",
            name,
            type_args
                .iter()
                .map(format_type_annotation)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}