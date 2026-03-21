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
                    if let Some(completion_item) = item_to_completion(item) {
                        items.push(completion_item);
                    }
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

fn item_to_completion(item: &spectra_compiler::ast::Item) -> Option<CompletionItem> {
    match item {
        spectra_compiler::ast::Item::Function(function) => Some(CompletionItem {
            label: function.name.clone(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(format!("{}", function.name)),
            ..Default::default()
        }),
        spectra_compiler::ast::Item::Struct(struct_def) => Some(CompletionItem {
            label: struct_def.name.clone(),
            kind: Some(CompletionItemKind::STRUCT),
            ..Default::default()
        }),
        spectra_compiler::ast::Item::Enum(enum_def) => Some(CompletionItem {
            label: enum_def.name.clone(),
            kind: Some(CompletionItemKind::ENUM),
            ..Default::default()
        }),
        spectra_compiler::ast::Item::Trait(trait_def) => Some(CompletionItem {
            label: trait_def.name.clone(),
            kind: Some(CompletionItemKind::INTERFACE),
            ..Default::default()
        }),
        _ => None,
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