use serde_json::Value;
use spectra_compiler::{
    analyze_document, CompilationOptions, CompilerError, DocumentAnalysis, LintDiagnostic, Span,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

const COMMAND_RUN_DIAGNOSTICS: &str = "spectra.diagnostics.run";
const COMMAND_LINT_WORKSPACE: &str = "spectra.lintWorkspace";
const KEYWORDS: &[&str] = &[
    "fn", "let", "return", "if", "else", "match", "while", "for", "loop", "break",
    "continue", "struct", "enum", "impl", "trait", "import", "pub", "internal",
    "true", "false", "self",
];

#[derive(Debug, Clone)]
struct ServerConfig {
    cli_path: String,
    lint_on_save: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            cli_path: "spectra".to_string(),
            lint_on_save: true,
        }
    }
}

#[derive(Debug, Clone)]
struct DocumentState {
    text: String,
    analysis: DocumentAnalysis,
}

#[derive(Debug, Default)]
struct BackendState {
    documents: RwLock<HashMap<Url, DocumentState>>,
    workspace_folders: RwLock<Vec<PathBuf>>,
    config: RwLock<ServerConfig>,
}

#[derive(Debug)]
struct Backend {
    client: Client,
    state: BackendState,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(folders) = params.workspace_folders {
            let mut workspace_folders = self.state.workspace_folders.write().await;
            *workspace_folders = folders
                .into_iter()
                .filter_map(|folder| folder.uri.to_file_path().ok())
                .collect();
        }

        if let Some(options) = params.initialization_options {
            self.update_config(options).await;
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        COMMAND_RUN_DIAGNOSTICS.to_string(),
                        COMMAND_LINT_WORKSPACE.to_string(),
                    ],
                    ..Default::default()
                }),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "spectra-lsp".to_string(),
                version: Some("0.1.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "spectra-lsp inicializado")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        self.update_config(params.settings).await;
    }

    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        let mut folders = self.state.workspace_folders.write().await;
        for removed in params.event.removed {
            if let Ok(path) = removed.uri.to_file_path() {
                folders.retain(|folder| folder != &path);
            }
        }
        for added in params.event.added {
            if let Ok(path) = added.uri.to_file_path() {
                if !folders.iter().any(|folder| folder == &path) {
                    folders.push(path);
                }
            }
        }
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.analyze_and_store(
            params.text_document.uri,
            params.text_document.text,
            false,
        )
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            self.analyze_and_store(params.text_document.uri, change.text, false)
                .await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let include_lints = self.state.config.read().await.lint_on_save;
        if let Some(text) = params.text {
            self.analyze_and_store(params.text_document.uri, text, include_lints)
                .await;
        } else {
            self.reanalyze_document(&params.text_document.uri, include_lints).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.state.documents.write().await.remove(&params.text_document.uri);
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let text_position = params.text_document_position_params;
        let Some(document) = self.state.documents.read().await.get(&text_position.text_document.uri).cloned() else {
            return Ok(None);
        };

        let line = text_position.position.line as usize + 1;
        let column = text_position.position.character as usize + 1;
        let Some(symbol) = document.analysis.symbol_at(line, column) else {
            return Ok(None);
        };

        let definition_label = symbol
            .definition
            .as_ref()
            .map(|definition| definition.label.clone())
            .unwrap_or_else(|| spectra_compiler::language_service::type_to_string(&symbol.info.ty));
        let scope = if symbol.info.is_local { "local" } else { "global" };
        let type_label = spectra_compiler::language_service::type_to_string(&symbol.info.ty);
        let contents = HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!(
                "```spectra\n{}\n```\n\nTipo: `{}`\n\nEscopo: {}",
                definition_label, type_label, scope
            ),
        });

        Ok(Some(Hover {
            contents,
            range: Some(span_to_range(symbol.span)),
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let text_position = params.text_document_position_params;
        let Some(document) = self.state.documents.read().await.get(&text_position.text_document.uri).cloned() else {
            return Ok(None);
        };

        let line = text_position.position.line as usize + 1;
        let column = text_position.position.character as usize + 1;
        let Some(symbol) = document.analysis.symbol_at(line, column) else {
            return Ok(None);
        };
        let Some(definition) = symbol.definition else {
            return Ok(None);
        };

        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri: text_position.text_document.uri,
            range: span_to_range(definition.span),
        })))
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let Some(document) = self.state.documents.read().await.get(&params.text_document.uri).cloned() else {
            return Ok(None);
        };

        match self.format_document(&params.text_document.uri, &document.text).await {
            Ok(Some(formatted)) if formatted != document.text => {
                let edit = TextEdit {
                    range: full_document_range(&document.text),
                    new_text: formatted,
                };
                Ok(Some(vec![edit]))
            }
            Ok(_) => Ok(None),
            Err(message) => {
                self.client.log_message(MessageType::ERROR, message).await;
                Ok(None)
            }
        }
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let text_position = params.text_document_position;
        let Some(document) = self.state.documents.read().await.get(&text_position.text_document.uri).cloned() else {
            return Ok(None);
        };

        let line = text_position.position.line as usize + 1;
        let column = text_position.position.character as usize + 1;
        let Some(symbol) = document.analysis.symbol_at(line, column) else {
            return Ok(None);
        };
        let Some(definition_span) = symbol.info.def_span else {
            return Ok(None);
        };

        let include_declaration = params.context.include_declaration;
        let mut locations = Vec::new();

        for (span, info) in &document.analysis.symbols {
            if info.def_span != Some(definition_span) {
                continue;
            }

            if !include_declaration && *span == definition_span {
                continue;
            }

            locations.push(Location {
                uri: text_position.text_document.uri.clone(),
                range: span_to_range(*span),
            });
        }

        if include_declaration && !locations.iter().any(|location| location.range == span_to_range(definition_span)) {
            locations.push(Location {
                uri: text_position.text_document.uri,
                range: span_to_range(definition_span),
            });
        }

        Ok(Some(locations))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let Some(document) = self.state.documents.read().await.get(&params.text_document.uri).cloned() else {
            return Ok(None);
        };
        let Some(module) = &document.analysis.module else {
            return Ok(None);
        };

        Ok(Some(DocumentSymbolResponse::Nested(document_symbols(module))))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let documents = self.state.documents.read().await;
        let document = documents.get(&uri);

        let mut items: Vec<CompletionItem> = KEYWORDS
            .iter()
            .map(|keyword| CompletionItem {
                label: (*keyword).to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            })
            .collect();

        if let Some(document) = document {
            if let Some(module) = &document.analysis.module {
                for item in &module.items {
                    items.extend(item_to_completion_items(item));
                }
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<Value>> {
        match params.command.as_str() {
            COMMAND_RUN_DIAGNOSTICS => {
                if let Some(Value::String(uri_text)) = params.arguments.first() {
                    if let Ok(uri) = Url::parse(uri_text) {
                        self.reanalyze_document(&uri, true).await;
                    }
                }
            }
            COMMAND_LINT_WORKSPACE => {
                let folders: Vec<PathBuf> = params
                    .arguments
                    .iter()
                    .filter_map(|argument| argument.as_str())
                    .map(PathBuf::from)
                    .collect();
                self.run_workspace_lint(&folders).await;
            }
            _ => {}
        }

        Ok(None)
    }
}

impl Backend {
    async fn update_config(&self, value: Value) {
        let settings = value
            .get("spectra")
            .cloned()
            .unwrap_or(value);

        let mut config = self.state.config.write().await;
        if let Some(cli_path) = settings.get("cliPath").and_then(Value::as_str) {
            let trimmed = cli_path.trim();
            if !trimmed.is_empty() {
                config.cli_path = trimmed.to_string();
            }
        }
        if let Some(lint_on_save) = settings.get("lintOnSave").and_then(Value::as_bool) {
            config.lint_on_save = lint_on_save;
        }
    }

    async fn analyze_and_store(&self, uri: Url, text: String, include_lints: bool) {
        let filename = uri
            .to_file_path()
            .ok()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| uri.to_string());

        let mut options = CompilationOptions::default();
        options.optimize = false;
        options.lint = if include_lints {
            spectra_compiler::LintOptions::all()
        } else {
            spectra_compiler::LintOptions::disabled()
        };

        let analysis = analyze_document(&text, &filename, &options, None);
        let diagnostics = analysis_to_diagnostics(&uri, &analysis);
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
        self.state.documents.write().await.insert(
            uri,
            DocumentState {
                text,
                analysis,
            },
        );
    }

    async fn reanalyze_document(&self, uri: &Url, include_lints: bool) {
        if let Some(document) = self.state.documents.read().await.get(uri).cloned() {
            self.analyze_and_store(uri.clone(), document.text, include_lints)
                .await;
            return;
        }

        if let Ok(path) = uri.to_file_path() {
            match fs::read_to_string(&path) {
                Ok(text) => self.analyze_and_store(uri.clone(), text, include_lints).await,
                Err(error) => {
                    self.client
                        .log_message(
                            MessageType::ERROR,
                            format!("Falha ao ler '{}': {}", path.display(), error),
                        )
                        .await;
                }
            }
        }
    }

    async fn run_workspace_lint(&self, folders: &[PathBuf]) {
        let folder_list = if folders.is_empty() {
            self.state.workspace_folders.read().await.clone()
        } else {
            folders.to_vec()
        };

        for folder in folder_list {
            let mut files = Vec::new();
            collect_spectra_files(&folder, &mut files);
            for file in files {
                let Ok(text) = fs::read_to_string(&file) else {
                    continue;
                };
                let Ok(uri) = Url::from_file_path(&file) else {
                    continue;
                };
                self.analyze_and_store(uri, text, true).await;
            }
        }
    }

    async fn format_document(&self, uri: &Url, text: &str) -> std::result::Result<Option<String>, String> {
        let config = self.state.config.read().await.clone();
        let workspace_folders = self.state.workspace_folders.read().await.clone();
        let cwd = uri
            .to_file_path()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .or_else(|| workspace_folders.first().cloned());

        let mut command = Command::new(&config.cli_path);
        command
            .arg("fmt")
            .arg("--stdin")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }

        let mut child = command
            .spawn()
            .map_err(|error| format!("Falha ao iniciar formatter '{}': {}", config.cli_path, error))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .await
                .map_err(|error| format!("Falha ao enviar conteúdo ao formatter: {}", error))?;
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|error| format!("Falha ao aguardar formatter: {}", error))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                format!("Formatter '{}' terminou com erro.", config.cli_path)
            } else {
                stderr
            });
        }

        let formatted = String::from_utf8(output.stdout)
            .map_err(|error| format!("Saída inválida do formatter: {}", error))?;
        if formatted == text {
            Ok(None)
        } else {
            Ok(Some(formatted))
        }
    }
}

fn analysis_to_diagnostics(uri: &Url, analysis: &DocumentAnalysis) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<Diagnostic> = analysis
        .diagnostics
        .iter()
        .map(|error| compiler_error_to_diagnostic(uri, error))
        .collect();
    diagnostics.extend(analysis.warnings.iter().map(|warning| lint_to_diagnostic(uri, warning)));
    diagnostics
}

fn compiler_error_to_diagnostic(uri: &Url, error: &CompilerError) -> Diagnostic {
    match error {
        CompilerError::Lexical(error) => span_diagnostic(
            uri,
            error.span,
            &error.message,
            DiagnosticSeverity::ERROR,
            Some("lexical".to_string()),
            error.context.as_deref(),
            error.hint.as_deref(),
        ),
        CompilerError::Parse(error) => span_diagnostic(
            uri,
            error.span,
            &error.message,
            DiagnosticSeverity::ERROR,
            Some("parse".to_string()),
            error.context.as_deref(),
            error.hint.as_deref(),
        ),
        CompilerError::Semantic(error) => span_diagnostic(
            uri,
            error.span,
            &error.message,
            DiagnosticSeverity::ERROR,
            Some("semantic".to_string()),
            error.context.as_deref(),
            error.hint.as_deref(),
        ),
        CompilerError::Midend(error) => Diagnostic {
            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("spectra/midend".to_string()),
            message: error.message.clone(),
            ..Default::default()
        },
        CompilerError::Backend(error) => Diagnostic {
            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("spectra/backend".to_string()),
            message: error.message.clone(),
            ..Default::default()
        },
    }
}

fn lint_to_diagnostic(uri: &Url, diagnostic: &LintDiagnostic) -> Diagnostic {
    let mut result = Diagnostic {
        range: span_to_range(diagnostic.span),
        severity: Some(DiagnosticSeverity::WARNING),
        source: Some("spectra/lint".to_string()),
        code: Some(NumberOrString::String(format!("lint({})", diagnostic.rule.code()))),
        message: diagnostic.message.clone(),
        ..Default::default()
    };

    if diagnostic.note.is_some() || diagnostic.secondary_span.is_some() {
        let mut related_information = Vec::new();
        if let Some(note) = &diagnostic.note {
            related_information.push(DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range: span_to_range(diagnostic.span),
                },
                message: note.clone(),
            });
        }
        if let Some(secondary_span) = diagnostic.secondary_span {
            related_information.push(DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range: span_to_range(secondary_span),
                },
                message: "related location".to_string(),
            });
        }
        result.related_information = Some(related_information);
    }

    result
}

fn span_diagnostic(
    uri: &Url,
    span: Span,
    message: &str,
    severity: DiagnosticSeverity,
    phase: Option<String>,
    context: Option<&str>,
    hint: Option<&str>,
) -> Diagnostic {
    let mut related_information = Vec::new();
    if let Some(context) = context {
        related_information.push(DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: span_to_range(span),
            },
            message: context.to_string(),
        });
    }
    if let Some(hint) = hint {
        related_information.push(DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: span_to_range(span),
            },
            message: hint.to_string(),
        });
    }

    Diagnostic {
        range: span_to_range(span),
        severity: Some(severity),
        source: Some(
            phase
                .map(|phase| format!("spectra/{}", phase))
                .unwrap_or_else(|| "spectra".to_string()),
        ),
        message: message.to_string(),
        related_information: if related_information.is_empty() {
            None
        } else {
            Some(related_information)
        },
        ..Default::default()
    }
}

fn span_to_range(span: Span) -> Range {
    Range::new(
        Position::new(
            span.start_location.line.saturating_sub(1) as u32,
            span.start_location.column.saturating_sub(1) as u32,
        ),
        Position::new(
            span.end_location.line.saturating_sub(1) as u32,
            span.end_location.column.saturating_sub(1) as u32,
        ),
    )
}

fn full_document_range(text: &str) -> Range {
    let mut line = 0u32;
    let mut col = 0u32;
    for ch in text.chars() {
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Range::new(Position::new(0, 0), Position::new(line, col))
}

fn collect_spectra_files(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
            if matches!(name, ".git" | "node_modules" | "target" | ".vscode") {
                continue;
            }
            collect_spectra_files(&path, files);
        } else if path.extension().and_then(|value| value.to_str()) == Some("spectra") {
            files.push(path);
        }
    }
}

fn item_to_completion_items(item: &spectra_compiler::ast::Item) -> Vec<CompletionItem> {
    match item {
        spectra_compiler::ast::Item::Import(import) => vec![CompletionItem {
            label: import
                .alias
                .clone()
                .unwrap_or_else(|| import.path.join("::")),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some(format!("import {}", import.path.join("::"))),
            ..Default::default()
        }],
        spectra_compiler::ast::Item::Function(function) => vec![CompletionItem {
            label: function.name.clone(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(format_function_signature(function)),
            ..Default::default()
        }],
        spectra_compiler::ast::Item::Struct(struct_def) => {
            let mut items = vec![CompletionItem {
                label: struct_def.name.clone(),
                kind: Some(CompletionItemKind::STRUCT),
                detail: Some(format!("struct {}", struct_def.name)),
                ..Default::default()
            }];

            for field in &struct_def.fields {
                items.push(CompletionItem {
                    label: field.name.clone(),
                    kind: Some(CompletionItemKind::FIELD),
                    detail: Some(format!(
                        "{}: {}",
                        field.name,
                        format_type_annotation(&field.ty)
                    )),
                    ..Default::default()
                });
            }

            items
        }
        spectra_compiler::ast::Item::Enum(enum_def) => {
            let mut items = vec![CompletionItem {
                label: enum_def.name.clone(),
                kind: Some(CompletionItemKind::ENUM),
                detail: Some(format!("enum {}", enum_def.name)),
                ..Default::default()
            }];

            for variant in &enum_def.variants {
                items.push(CompletionItem {
                    label: variant.name.clone(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    detail: Some(format!("{}::{}", enum_def.name, variant.name)),
                    ..Default::default()
                });
            }

            items
        }
        spectra_compiler::ast::Item::Impl(impl_block) => impl_block
            .methods
            .iter()
            .map(|method| CompletionItem {
                label: method.name.clone(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some(format_method_signature(&impl_block.type_name, method)),
                ..Default::default()
            })
            .collect(),
        spectra_compiler::ast::Item::Trait(trait_def) => {
            let mut items = vec![CompletionItem {
                label: trait_def.name.clone(),
                kind: Some(CompletionItemKind::INTERFACE),
                detail: Some(format!("trait {}", trait_def.name)),
                ..Default::default()
            }];

            for method in &trait_def.methods {
                items.push(CompletionItem {
                    label: method.name.clone(),
                    kind: Some(CompletionItemKind::METHOD),
                    detail: Some(format_trait_method_signature(method)),
                    ..Default::default()
                });
            }

            items
        }
        spectra_compiler::ast::Item::TraitImpl(trait_impl) => trait_impl
            .methods
            .iter()
            .map(|method| CompletionItem {
                label: method.name.clone(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some(format_method_signature(&trait_impl.type_name, method)),
                ..Default::default()
            })
            .collect(),
    }
}

fn document_symbols(module: &spectra_compiler::ast::Module) -> Vec<DocumentSymbol> {
    module
        .items
        .iter()
        .map(item_to_document_symbol)
        .collect()
}

fn item_to_document_symbol(item: &spectra_compiler::ast::Item) -> DocumentSymbol {
    match item {
        spectra_compiler::ast::Item::Import(import) => document_symbol_node(
            import
                .alias
                .clone()
                .unwrap_or_else(|| import.path.join("::")),
            Some(format!("import {}", import.path.join("::"))),
            SymbolKind::MODULE,
            import.span,
            None,
        ),
        spectra_compiler::ast::Item::Function(function) => document_symbol_node(
            function.name.clone(),
            Some(format_function_signature(function)),
            SymbolKind::FUNCTION,
            function.span,
            Some(
                function
                    .params
                    .iter()
                    .map(|param| {
                        document_symbol_node(
                            param.name.clone(),
                            param.ty.as_ref().map(format_type_annotation),
                            SymbolKind::VARIABLE,
                            param.span,
                            None,
                        )
                    })
                    .collect(),
            ),
        ),
        spectra_compiler::ast::Item::Struct(struct_def) => document_symbol_node(
            struct_def.name.clone(),
            Some(format!("struct {}", struct_def.name)),
            SymbolKind::STRUCT,
            struct_def.span,
            Some(
                struct_def
                    .fields
                    .iter()
                    .map(|field| {
                        document_symbol_node(
                            field.name.clone(),
                            Some(format_type_annotation(&field.ty)),
                            SymbolKind::FIELD,
                            field.span,
                            None,
                        )
                    })
                    .collect(),
            ),
        ),
        spectra_compiler::ast::Item::Enum(enum_def) => document_symbol_node(
            enum_def.name.clone(),
            Some(format!("enum {}", enum_def.name)),
            SymbolKind::ENUM,
            enum_def.span,
            Some(
                enum_def
                    .variants
                    .iter()
                    .map(|variant| {
                        document_symbol_node(
                            variant.name.clone(),
                            Some(format_variant_signature(enum_def, variant)),
                            SymbolKind::ENUM_MEMBER,
                            variant.span,
                            None,
                        )
                    })
                    .collect(),
            ),
        ),
        spectra_compiler::ast::Item::Impl(impl_block) => document_symbol_node(
            impl_block
                .trait_name
                .as_ref()
                .map(|trait_name| format!("impl {} for {}", trait_name, impl_block.type_name))
                .unwrap_or_else(|| format!("impl {}", impl_block.type_name)),
            Some("implementation".to_string()),
            SymbolKind::OBJECT,
            impl_block.span,
            Some(impl_block.methods.iter().map(method_to_document_symbol).collect()),
        ),
        spectra_compiler::ast::Item::Trait(trait_def) => document_symbol_node(
            trait_def.name.clone(),
            Some(format!("trait {}", trait_def.name)),
            SymbolKind::INTERFACE,
            trait_def.span,
            Some(
                trait_def
                    .methods
                    .iter()
                    .map(trait_method_to_document_symbol)
                    .collect(),
            ),
        ),
        spectra_compiler::ast::Item::TraitImpl(trait_impl) => document_symbol_node(
            format!("impl {} for {}", trait_impl.trait_name, trait_impl.type_name),
            Some("trait implementation".to_string()),
            SymbolKind::OBJECT,
            trait_impl.span,
            Some(trait_impl.methods.iter().map(method_to_document_symbol).collect()),
        ),
    }
}

fn method_to_document_symbol(method: &spectra_compiler::ast::Method) -> DocumentSymbol {
    document_symbol_node(
        method.name.clone(),
        Some(format_method_signature("Self", method)),
        SymbolKind::METHOD,
        method.span,
        Some(
            method
                .params
                .iter()
                .map(parameter_to_document_symbol)
                .collect(),
        ),
    )
}

fn trait_method_to_document_symbol(method: &spectra_compiler::ast::TraitMethod) -> DocumentSymbol {
    document_symbol_node(
        method.name.clone(),
        Some(format_trait_method_signature(method)),
        SymbolKind::METHOD,
        method.span,
        Some(
            method
                .params
                .iter()
                .map(parameter_to_document_symbol)
                .collect(),
        ),
    )
}

fn parameter_to_document_symbol(param: &spectra_compiler::ast::Parameter) -> DocumentSymbol {
    document_symbol_node(
        param.name.clone(),
        param.type_annotation.as_ref().map(format_type_annotation),
        SymbolKind::VARIABLE,
        param.span,
        None,
    )
}

#[allow(deprecated)]
fn document_symbol_node(
    name: String,
    detail: Option<String>,
    kind: SymbolKind,
    span: Span,
    children: Option<Vec<DocumentSymbol>>,
) -> DocumentSymbol {
    DocumentSymbol {
        name,
        detail,
        kind,
        range: span_to_range(span),
        selection_range: span_to_range(span),
        children,
        tags: None,
        deprecated: None,
    }
}

fn format_function_signature(function: &spectra_compiler::ast::Function) -> String {
    let params = function
        .params
        .iter()
        .map(|param| match &param.ty {
            Some(ty) => format!("{}: {}", param.name, format_type_annotation(ty)),
            None => param.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = function
        .return_type
        .as_ref()
        .map(format_type_annotation)
        .unwrap_or_else(|| "unit".to_string());
    format!("fn {}({}) -> {}", function.name, params, return_type)
}

fn format_method_signature(type_name: &str, method: &spectra_compiler::ast::Method) -> String {
    let params = method
        .params
        .iter()
        .map(format_parameter_signature)
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = method
        .return_type
        .as_ref()
        .map(format_type_annotation)
        .unwrap_or_else(|| "unit".to_string());
    format!("fn {}::{}({}) -> {}", type_name, method.name, params, return_type)
}

fn format_trait_method_signature(method: &spectra_compiler::ast::TraitMethod) -> String {
    let params = method
        .params
        .iter()
        .map(format_parameter_signature)
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = method
        .return_type
        .as_ref()
        .map(format_type_annotation)
        .unwrap_or_else(|| "unit".to_string());
    format!("fn {}({}) -> {}", method.name, params, return_type)
}

fn format_parameter_signature(param: &spectra_compiler::ast::Parameter) -> String {
    if param.is_self {
        return match (param.is_reference, param.is_mutable) {
            (true, true) => "&mut self".to_string(),
            (true, false) => "&self".to_string(),
            (false, _) => "self".to_string(),
        };
    }

    match &param.type_annotation {
        Some(ty) => format!("{}: {}", param.name, format_type_annotation(ty)),
        None => param.name.clone(),
    }
}

fn format_variant_signature(
    enum_def: &spectra_compiler::ast::Enum,
    variant: &spectra_compiler::ast::EnumVariant,
) -> String {
    if let Some(fields) = &variant.struct_data {
        let fields = fields
            .iter()
            .map(|(name, ty)| format!("{}: {}", name, format_type_annotation(ty)))
            .collect::<Vec<_>>()
            .join(", ");
        return format!("{}::{} {{ {} }}", enum_def.name, variant.name, fields);
    }

    if let Some(items) = &variant.data {
        let items = items
            .iter()
            .map(format_type_annotation)
            .collect::<Vec<_>>()
            .join(", ");
        return format!("{}::{}({})", enum_def.name, variant.name, items);
    }

    format!("{}::{}", enum_def.name, variant.name)
}

fn format_type_annotation(ty: &spectra_compiler::ast::TypeAnnotation) -> String {
    match &ty.kind {
        spectra_compiler::ast::TypeAnnotationKind::Simple { segments } => segments.join("::"),
        spectra_compiler::ast::TypeAnnotationKind::Tuple { elements } => format!(
            "({})",
            elements
                .iter()
                .map(format_type_annotation)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        spectra_compiler::ast::TypeAnnotationKind::Function {
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
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        state: BackendState::default(),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}