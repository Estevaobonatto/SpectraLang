use serde_json::Value;
use spectra_compiler::{
    analyze_document, collect_let_inlay_hints, CompilationOptions, CompilerError, DocumentAnalysis,
    LintDiagnostic, Span,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
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
            cli_path: "spectralang".to_string(),
            lint_on_save: true,
        }
    }
}

#[derive(Debug, Clone)]
struct DocumentState {
    text: String,
    analysis: DocumentAnalysis,
}

#[derive(Debug, Clone)]
struct CachedWorkspaceSymbol {
    name: String,
    detail: Option<String>,
    kind: SymbolKind,
    span: Span,
    container_name: Option<String>,
}

#[derive(Debug, Clone)]
struct CachedReference {
    key: String,
    location: Location,
}

#[derive(Debug, Clone)]
struct WorkspaceCacheEntry {
    symbols: Vec<CachedWorkspaceSymbol>,
    references: Vec<CachedReference>,
    modified: Option<SystemTime>,
}

#[derive(Debug, Default)]
struct BackendState {
    documents: RwLock<HashMap<Url, DocumentState>>,
    workspace_cache: RwLock<HashMap<Url, WorkspaceCacheEntry>>,
    workspace_folders: RwLock<Vec<PathBuf>>,
    config: RwLock<ServerConfig>,
    debounce_handles: tokio::sync::Mutex<HashMap<Url, tokio::task::AbortHandle>>,
}

#[derive(Debug)]
struct Backend {
    client: Client,
    state: Arc<BackendState>,
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
                document_highlight_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: Some(vec![",".to_string()]),
                    ..Default::default()
                }),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
                    code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
                    ..Default::default()
                })),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                        legend: semantic_tokens_legend(),
                        range: Some(false),
                        full: Some(SemanticTokensFullOptions::Bool(true)),
                        ..Default::default()
                    }),
                ),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        COMMAND_RUN_DIAGNOSTICS.to_string(),
                        COMMAND_LINT_WORKSPACE.to_string(),
                    ],
                    ..Default::default()
                }),
                inlay_hint_provider: Some(OneOf::Left(true)),
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
        let mut cache = self.state.workspace_cache.write().await;
        for removed in params.event.removed {
            if let Ok(path) = removed.uri.to_file_path() {
                folders.retain(|folder| folder != &path);
                cache.retain(|uri, _| {
                    uri.to_file_path()
                        .ok()
                        .map(|file| !file.starts_with(&path))
                        .unwrap_or(true)
                });
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
            let uri = params.text_document.uri;
            let text = change.text;

            // Abort any previously scheduled debounce task for this document.
            if let Some(old) = self.state.debounce_handles.lock().await.remove(&uri) {
                old.abort();
            }

            // Schedule a new analysis after a 300 ms typing pause.
            let client = self.client.clone();
            let state = Arc::clone(&self.state);
            let uri_debounce = uri.clone();
            let text_debounce = text;

            let join = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(300)).await;
                // Remove our own handle now that we are executing.
                state.debounce_handles.lock().await.remove(&uri_debounce);
                do_analyze_and_store(&client, &state, uri_debounce, text_debounce, false).await;
            });

            self.state
                .debounce_handles
                .lock()
                .await
                .insert(uri, join.abort_handle());
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
        self.refresh_cache_from_disk(&params.text_document.uri).await;
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
        let declaration_range = span_to_range(definition_span);
        let current_uri = text_position.text_document.uri.clone();
        let mut locations = Vec::new();
        let mut seen = HashSet::new();

        for (span, info) in &document.analysis.symbols {
            if info.def_span != Some(definition_span) {
                continue;
            }

            if !include_declaration && *span == definition_span {
                continue;
            }

            let location = Location {
                uri: current_uri.clone(),
                range: span_to_range(*span),
            };
            if seen.insert(location_key(&location)) {
                locations.push(location);
            }
        }

        if include_declaration && !locations.iter().any(|location| location.range == span_to_range(definition_span)) {
            let location = Location {
                uri: current_uri.clone(),
                range: declaration_range,
            };
            if seen.insert(location_key(&location)) {
                locations.push(location);
            }
        }

        if !symbol.info.is_local {
            if let Some(reference_key) = reference_key_for_resolved_symbol(&document, &symbol) {
                self.ensure_workspace_cache().await;
                let cache = self.state.workspace_cache.read().await;
                for entry in cache.values() {
                    for reference in &entry.references {
                        if reference.key != reference_key {
                            continue;
                        }
                        if !include_declaration
                            && reference.location.uri == current_uri
                            && reference.location.range == declaration_range
                        {
                            continue;
                        }
                        if seen.insert(location_key(&reference.location)) {
                            locations.push(reference.location.clone());
                        }
                    }
                }
            }
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

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        self.ensure_workspace_cache().await;
        let symbols = self.workspace_symbols(&params.query).await;
        Ok(Some(symbols))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let text_position = params.text_document_position_params;
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

        let mut highlights = Vec::new();
        for (span, info) in &document.analysis.symbols {
            if info.def_span != Some(definition_span) {
                continue;
            }

            let kind = if *span == definition_span {
                Some(DocumentHighlightKind::WRITE)
            } else {
                Some(DocumentHighlightKind::READ)
            };

            highlights.push(DocumentHighlight {
                range: span_to_range(*span),
                kind,
            });
        }

        if highlights.is_empty() {
            highlights.push(DocumentHighlight {
                range: span_to_range(definition_span),
                kind: Some(DocumentHighlightKind::WRITE),
            });
        }

        Ok(Some(highlights))
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let text_position = params.text_document_position_params;
        let Some(document) = self.state.documents.read().await.get(&text_position.text_document.uri).cloned() else {
            return Ok(None);
        };
        let Some(module) = &document.analysis.module else {
            return Ok(None);
        };

        let line = text_position.position.line as usize + 1;
        let column = text_position.position.character as usize + 1;
        let Some(call_site) = find_call_site(module, line, column) else {
            return Ok(None);
        };
        let Some(label) = signature_label_for_call(&document, &call_site) else {
            return Ok(None);
        };

        let parameters = split_signature_parameters(&label)
            .into_iter()
            .map(|param| ParameterInformation {
                label: ParameterLabel::Simple(param),
                documentation: None,
            })
            .collect::<Vec<_>>();

        let active_parameter = active_parameter_index(&document.text, &call_site, text_position.position)
            .min(parameters.len().saturating_sub(1));

        Ok(Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label,
                documentation: None,
                parameters: Some(parameters),
                active_parameter: Some(active_parameter as u32),
            }],
            active_signature: Some(0),
            active_parameter: Some(active_parameter as u32),
        }))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let Some(document) = self.state.documents.read().await.get(&params.text_document.uri).cloned() else {
            return Ok(None);
        };

        let mut actions = Vec::new();
        for diagnostic in &params.context.diagnostics {
            if let Some(action) = quick_fix_for_diagnostic(&params.text_document.uri, &document, diagnostic) {
                actions.push(CodeActionOrCommand::CodeAction(action));
            }
        }

        Ok((!actions.is_empty()).then_some(actions))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let Some(document) = self.state.documents.read().await.get(&params.text_document.uri).cloned() else {
            return Ok(None);
        };
        let Some(module) = &document.analysis.module else {
            return Ok(None);
        };

        let data = semantic_tokens_for_document(&document.text, module, &document.analysis);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let Some(document) = self
            .state
            .documents
            .read()
            .await
            .get(&params.text_document.uri)
            .cloned()
        else {
            return Ok(None);
        };

        let raw_hints = collect_let_inlay_hints(&document.analysis);
        if raw_hints.is_empty() {
            return Ok(None);
        }

        let hints = raw_hints
            .into_iter()
            .map(|hint| {
                // Position the hint right after the variable name:
                // span starts at the `let` keyword (1-indexed), `let ` = 4 chars.
                let line = hint.let_span.start_location.line.saturating_sub(1) as u32;
                let col = (hint.let_span.start_location.column.saturating_sub(1)
                    + 4
                    + hint.name.len()) as u32;

                InlayHint {
                    position: Position::new(line, col),
                    label: InlayHintLabel::String(format!(": {}", hint.ty)),
                    kind: Some(InlayHintKind::TYPE),
                    text_edits: None,
                    tooltip: None,
                    padding_left: Some(false),
                    padding_right: Some(true),
                    data: None,
                }
            })
            .collect();

        Ok(Some(hints))
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
        do_analyze_and_store(&self.client, &self.state, uri, text, include_lints).await;
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

    async fn workspace_symbols(&self, query: &str) -> Vec<SymbolInformation> {
        let normalized_query = query.trim().to_ascii_lowercase();
        let cache = self.state.workspace_cache.read().await;
        let mut results = Vec::new();

        for (uri, entry) in cache.iter() {
            for symbol in &entry.symbols {
                let haystack = format!(
                    "{} {}",
                    symbol.name.to_ascii_lowercase(),
                    symbol.detail.clone().unwrap_or_default().to_ascii_lowercase()
                );
                if !normalized_query.is_empty() && !haystack.contains(&normalized_query) {
                    continue;
                }

                results.push(workspace_symbol_information(uri, symbol));
            }
        }

        results
    }

    async fn update_workspace_cache(
        &self,
        uri: &Url,
        text: String,
        analysis: DocumentAnalysis,
        modified: Option<SystemTime>,
    ) {
        let symbols = analysis
            .module
            .as_ref()
            .map(|module| workspace_symbol_entries_for_module(&text, module))
            .unwrap_or_default();
        let references = reference_entries_for_analysis(uri, &analysis);

        self.state.workspace_cache.write().await.insert(
            uri.clone(),
            WorkspaceCacheEntry {
                symbols,
                references,
                modified,
            },
        );
    }

    async fn ensure_workspace_cache(&self) {
        let folders = self.state.workspace_folders.read().await.clone();
        let open_documents = self.state.documents.read().await.clone();
        let mut known = HashSet::new();

        for folder in folders {
            let mut files = Vec::new();
            collect_spectra_files(&folder, &mut files);
            for file in files {
                let Ok(uri) = Url::from_file_path(&file) else {
                    continue;
                };
                known.insert(uri.clone());

                if let Some(document) = open_documents.get(&uri) {
                    self.update_workspace_cache(&uri, document.text.clone(), document.analysis.clone(), None)
                        .await;
                    continue;
                }

                let modified = fs::metadata(&file).and_then(|meta| meta.modified()).ok();
                let should_refresh = {
                    let cache = self.state.workspace_cache.read().await;
                    cache.get(&uri)
                        .map(|entry| entry.modified != modified)
                        .unwrap_or(true)
                };

                if !should_refresh {
                    continue;
                }

                let Ok(text) = fs::read_to_string(&file) else {
                    continue;
                };
                let analysis = analyze_cache_document(&text, &file.to_string_lossy());
                self.update_workspace_cache(&uri, text, analysis, modified).await;
            }
        }

        self.state.workspace_cache.write().await.retain(|uri, _| known.contains(uri) || open_documents.contains_key(uri));
    }

    async fn refresh_cache_from_disk(&self, uri: &Url) {
        let Ok(path) = uri.to_file_path() else {
            return;
        };
        let Ok(text) = fs::read_to_string(&path) else {
            self.state.workspace_cache.write().await.remove(uri);
            return;
        };

        let analysis = analyze_cache_document(&text, &path.to_string_lossy());
        let modified = fs::metadata(&path).and_then(|meta| meta.modified()).ok();
        self.update_workspace_cache(uri, text, analysis, modified).await;
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

/// Standalone analysis function — used both by `analyze_and_store` and the
/// debounce task spawned in `did_change`.  Takes cloneable handles so it can
/// be called from inside `tokio::spawn`.
async fn do_analyze_and_store(
    client: &Client,
    state: &BackendState,
    uri: Url,
    text: String,
    include_lints: bool,
) {
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
    client.publish_diagnostics(uri.clone(), diagnostics, None).await;

    // Update workspace symbol cache.
    let symbols = analysis
        .module
        .as_ref()
        .map(|module| workspace_symbol_entries_for_module(&text, module))
        .unwrap_or_default();
    let references = reference_entries_for_analysis(&uri, &analysis);
    state.workspace_cache.write().await.insert(
        uri.clone(),
        WorkspaceCacheEntry {
            symbols,
            references,
            modified: cache_modified_time_for_uri(&uri),
        },
    );

    state
        .documents
        .write()
        .await
        .insert(uri, DocumentState { text, analysis });
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
        CompilerError::Lexical(error) => {
            let mut d = span_diagnostic(
                uri,
                error.span,
                &error.message,
                DiagnosticSeverity::ERROR,
                Some("lexical".to_string()),
                error.context.as_deref(),
                error.hint.as_deref(),
            );
            if let Some(code) = &error.code {
                d.code = Some(NumberOrString::String(code.clone()));
            }
            d
        }
        CompilerError::Parse(error) => {
            let mut d = span_diagnostic(
                uri,
                error.span,
                &error.message,
                DiagnosticSeverity::ERROR,
                Some("parse".to_string()),
                error.context.as_deref(),
                error.hint.as_deref(),
            );
            if let Some(code) = &error.code {
                d.code = Some(NumberOrString::String(code.clone()));
            }
            d
        }
        CompilerError::Semantic(error) => {
            let mut d = span_diagnostic(
                uri,
                error.span,
                &error.message,
                DiagnosticSeverity::ERROR,
                Some("semantic".to_string()),
                error.context.as_deref(),
                error.hint.as_deref(),
            );
            if let Some(code) = &error.code {
                d.code = Some(NumberOrString::String(code.clone()));
            }
            d
        }
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
        spectra_compiler::ast::Item::TypeAlias(ta) => vec![CompletionItem {
            label: ta.name.clone(),
            kind: Some(CompletionItemKind::VALUE),
            detail: Some(format!("type {}", ta.name)),
            ..Default::default()
        }],
        spectra_compiler::ast::Item::Const(c) => vec![CompletionItem {
            label: c.name.clone(),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: Some(format!("const {}", c.name)),
            ..Default::default()
        }],
        spectra_compiler::ast::Item::Static(s) => vec![CompletionItem {
            label: s.name.clone(),
            kind: Some(CompletionItemKind::VALUE),
            detail: Some(format!("static {}", s.name)),
            ..Default::default()
        }],
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
        spectra_compiler::ast::Item::TypeAlias(ta) => document_symbol_node(
            ta.name.clone(),
            Some("type alias".to_string()),
            SymbolKind::TYPE_PARAMETER,
            ta.span,
            None,
        ),
        spectra_compiler::ast::Item::Const(c) => document_symbol_node(
            c.name.clone(),
            Some("constant".to_string()),
            SymbolKind::CONSTANT,
            c.span,
            None,
        ),
        spectra_compiler::ast::Item::Static(s) => document_symbol_node(
            s.name.clone(),
            Some("static variable".to_string()),
            SymbolKind::VARIABLE,
            s.span,
            None,
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
        spectra_compiler::ast::TypeAnnotationKind::Generic { name, type_args } => format!(
            "{}<{}>",
            name,
            type_args
                .iter()
                .map(format_type_annotation)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        spectra_compiler::ast::TypeAnnotationKind::DynTrait { trait_name } => format!("dyn {}", trait_name),
    }
}

#[derive(Debug, Clone, Copy)]
enum CallKind {
    Function { callee_span: Span },
    Method { call_span: Span },
}

#[derive(Debug, Clone, Copy)]
struct CallSite {
    span: Span,
    search_start: usize,
    kind: CallKind,
}

const TOKEN_NAMESPACE: u32 = 0;
const TOKEN_TYPE: u32 = 1;
const TOKEN_ENUM: u32 = 2;
const TOKEN_INTERFACE: u32 = 3;
const TOKEN_FUNCTION: u32 = 4;
const TOKEN_METHOD: u32 = 5;
const TOKEN_VARIABLE: u32 = 6;
const TOKEN_PARAMETER: u32 = 7;
const TOKEN_PROPERTY: u32 = 8;
const TOKEN_ENUM_MEMBER: u32 = 9;

fn semantic_tokens_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::NAMESPACE,
            SemanticTokenType::TYPE,
            SemanticTokenType::ENUM,
            SemanticTokenType::INTERFACE,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::METHOD,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::PARAMETER,
            SemanticTokenType::PROPERTY,
            SemanticTokenType::ENUM_MEMBER,
        ],
        token_modifiers: Vec::new(),
    }
}

fn find_call_site(module: &spectra_compiler::ast::Module, line: usize, column: usize) -> Option<CallSite> {
    let mut best = None;

    for item in &module.items {
        match item {
            spectra_compiler::ast::Item::Function(function) => {
                find_call_site_in_block(&function.body, line, column, &mut best);
            }
            spectra_compiler::ast::Item::Impl(impl_block) => {
                for method in &impl_block.methods {
                    find_call_site_in_block(&method.body, line, column, &mut best);
                }
            }
            spectra_compiler::ast::Item::Trait(trait_def) => {
                for method in &trait_def.methods {
                    if let Some(body) = &method.body {
                        find_call_site_in_block(body, line, column, &mut best);
                    }
                }
            }
            spectra_compiler::ast::Item::TraitImpl(trait_impl) => {
                for method in &trait_impl.methods {
                    find_call_site_in_block(&method.body, line, column, &mut best);
                }
            }
            spectra_compiler::ast::Item::Import(_) | spectra_compiler::ast::Item::Struct(_) | spectra_compiler::ast::Item::Enum(_) => {}
            spectra_compiler::ast::Item::TypeAlias(_) | spectra_compiler::ast::Item::Const(_) | spectra_compiler::ast::Item::Static(_) => {}
        }
    }

    best
}

fn find_call_site_in_block(
    block: &spectra_compiler::ast::Block,
    line: usize,
    column: usize,
    best: &mut Option<CallSite>,
) {
    for statement in &block.statements {
        find_call_site_in_statement(statement, line, column, best);
    }
}

fn find_call_site_in_statement(
    statement: &spectra_compiler::ast::Statement,
    line: usize,
    column: usize,
    best: &mut Option<CallSite>,
) {
    match &statement.kind {
        spectra_compiler::ast::StatementKind::Let(let_stmt) => {
            if let Some(value) = &let_stmt.value {
                find_call_site_in_expression(value, line, column, best);
            }
        }
        spectra_compiler::ast::StatementKind::Assignment(assign_stmt) => {
            find_call_site_in_expression(&assign_stmt.value, line, column, best);
            match &assign_stmt.target {
                spectra_compiler::ast::LValue::IndexAccess { array, index } => {
                    find_call_site_in_expression(array, line, column, best);
                    find_call_site_in_expression(index, line, column, best);
                }
                spectra_compiler::ast::LValue::FieldAccess { object, .. } => {
                    find_call_site_in_expression(object, line, column, best);
                }
                spectra_compiler::ast::LValue::Identifier(_) => {}
            }
        }
        spectra_compiler::ast::StatementKind::Return(ret_stmt) => {
            if let Some(value) = &ret_stmt.value {
                find_call_site_in_expression(value, line, column, best);
            }
        }
        spectra_compiler::ast::StatementKind::Expression(expr) => {
            find_call_site_in_expression(expr, line, column, best);
        }
        spectra_compiler::ast::StatementKind::While(loop_stmt) => {
            find_call_site_in_expression(&loop_stmt.condition, line, column, best);
            find_call_site_in_block(&loop_stmt.body, line, column, best);
        }
        spectra_compiler::ast::StatementKind::DoWhile(loop_stmt) => {
            find_call_site_in_block(&loop_stmt.body, line, column, best);
            find_call_site_in_expression(&loop_stmt.condition, line, column, best);
        }
        spectra_compiler::ast::StatementKind::For(loop_stmt) => {
            find_call_site_in_expression(&loop_stmt.iterable, line, column, best);
            find_call_site_in_block(&loop_stmt.body, line, column, best);
        }
        spectra_compiler::ast::StatementKind::Loop(loop_stmt) => {
            find_call_site_in_block(&loop_stmt.body, line, column, best);
        }
        spectra_compiler::ast::StatementKind::Switch(switch_stmt) => {
            find_call_site_in_expression(&switch_stmt.value, line, column, best);
            for case in &switch_stmt.cases {
                find_call_site_in_expression(&case.pattern, line, column, best);
                find_call_site_in_block(&case.body, line, column, best);
            }
            if let Some(default) = &switch_stmt.default {
                find_call_site_in_block(default, line, column, best);
            }
        }
        spectra_compiler::ast::StatementKind::IfLet(if_let_stmt) => {
            find_call_site_in_expression(&if_let_stmt.value, line, column, best);
            find_call_site_in_block(&if_let_stmt.then_block, line, column, best);
            if let Some(else_block) = &if_let_stmt.else_block {
                find_call_site_in_block(else_block, line, column, best);
            }
        }
        spectra_compiler::ast::StatementKind::WhileLet(while_let_stmt) => {
            find_call_site_in_expression(&while_let_stmt.value, line, column, best);
            find_call_site_in_block(&while_let_stmt.body, line, column, best);
        }
        spectra_compiler::ast::StatementKind::Break | spectra_compiler::ast::StatementKind::Continue => {}
    }
}

fn find_call_site_in_expression(
    expr: &spectra_compiler::ast::Expression,
    line: usize,
    column: usize,
    best: &mut Option<CallSite>,
) {
    if !span_contains(expr.span, line, column) {
        return;
    }

    match &expr.kind {
        spectra_compiler::ast::ExpressionKind::Call { callee, arguments } => {
            record_call_site(
                CallSite {
                    span: expr.span,
                    search_start: callee.span.end,
                    kind: CallKind::Function {
                        callee_span: callee.span,
                    },
                },
                best,
            );
            find_call_site_in_expression(callee, line, column, best);
            for arg in arguments {
                find_call_site_in_expression(arg, line, column, best);
            }
        }
        spectra_compiler::ast::ExpressionKind::MethodCall { object, arguments, .. } => {
            record_call_site(
                CallSite {
                    span: expr.span,
                    search_start: object.span.end,
                    kind: CallKind::Method { call_span: expr.span },
                },
                best,
            );
            find_call_site_in_expression(object, line, column, best);
            for arg in arguments {
                find_call_site_in_expression(arg, line, column, best);
            }
        }
        spectra_compiler::ast::ExpressionKind::Binary { left, right, .. } => {
            find_call_site_in_expression(left, line, column, best);
            find_call_site_in_expression(right, line, column, best);
        }
        spectra_compiler::ast::ExpressionKind::Unary { operand, .. }
        | spectra_compiler::ast::ExpressionKind::Try(operand)
        | spectra_compiler::ast::ExpressionKind::Grouping(operand) => {
            find_call_site_in_expression(operand, line, column, best);
        }
        spectra_compiler::ast::ExpressionKind::Range { start, end, .. }
        | spectra_compiler::ast::ExpressionKind::IndexAccess { array: start, index: end } => {
            find_call_site_in_expression(start, line, column, best);
            find_call_site_in_expression(end, line, column, best);
        }
        spectra_compiler::ast::ExpressionKind::If {
            condition,
            then_block,
            elif_blocks,
            else_block,
        } => {
            find_call_site_in_expression(condition, line, column, best);
            find_call_site_in_block(then_block, line, column, best);
            for (elif_expr, elif_block) in elif_blocks {
                find_call_site_in_expression(elif_expr, line, column, best);
                find_call_site_in_block(elif_block, line, column, best);
            }
            if let Some(else_block) = else_block {
                find_call_site_in_block(else_block, line, column, best);
            }
        }
        spectra_compiler::ast::ExpressionKind::Unless {
            condition,
            then_block,
            else_block,
        } => {
            find_call_site_in_expression(condition, line, column, best);
            find_call_site_in_block(then_block, line, column, best);
            if let Some(else_block) = else_block {
                find_call_site_in_block(else_block, line, column, best);
            }
        }
        spectra_compiler::ast::ExpressionKind::ArrayLiteral { elements }
        | spectra_compiler::ast::ExpressionKind::TupleLiteral { elements } => {
            for element in elements {
                find_call_site_in_expression(element, line, column, best);
            }
        }
        spectra_compiler::ast::ExpressionKind::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                find_call_site_in_expression(value, line, column, best);
            }
        }
        spectra_compiler::ast::ExpressionKind::FieldAccess { object, .. } => {
            find_call_site_in_expression(object, line, column, best);
        }
        spectra_compiler::ast::ExpressionKind::EnumVariant {
            data,
            struct_data,
            ..
        } => {
            if let Some(data) = data {
                for value in data {
                    find_call_site_in_expression(value, line, column, best);
                }
            }
            if let Some(struct_data) = struct_data {
                for (_, value) in struct_data {
                    find_call_site_in_expression(value, line, column, best);
                }
            }
        }
        spectra_compiler::ast::ExpressionKind::Match { scrutinee, arms } => {
            find_call_site_in_expression(scrutinee, line, column, best);
            for arm in arms {
                find_call_site_in_expression(&arm.body, line, column, best);
            }
        }
        spectra_compiler::ast::ExpressionKind::Lambda { body, .. } => {
            find_call_site_in_expression(body, line, column, best);
        }
        spectra_compiler::ast::ExpressionKind::Block(block) => {
            find_call_site_in_block(block, line, column, best);
        }
        spectra_compiler::ast::ExpressionKind::Cast { expr, .. } => {
            find_call_site_in_expression(expr, line, column, best);
        }
        spectra_compiler::ast::ExpressionKind::Identifier(_)
        | spectra_compiler::ast::ExpressionKind::NumberLiteral(_)
        | spectra_compiler::ast::ExpressionKind::StringLiteral(_)
        | spectra_compiler::ast::ExpressionKind::BoolLiteral(_)
        | spectra_compiler::ast::ExpressionKind::CharLiteral(_)
        | spectra_compiler::ast::ExpressionKind::FString(_)
        | spectra_compiler::ast::ExpressionKind::TupleAccess { .. } => {}
    }
}

fn record_call_site(candidate: CallSite, best: &mut Option<CallSite>) {
    match best {
        Some(current) if span_len(current.span) <= span_len(candidate.span) => {}
        _ => *best = Some(candidate),
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

fn signature_label_for_call(document: &DocumentState, call_site: &CallSite) -> Option<String> {
    match call_site.kind {
        CallKind::Function { callee_span } => document
            .analysis
            .symbol_at(callee_span.start_location.line, callee_span.start_location.column)
            .and_then(|symbol| symbol.definition.map(|definition| definition.label)),
        CallKind::Method { call_span } => document
            .analysis
            .symbols
            .get(&call_span)
            .and_then(|info| info.def_span)
            .and_then(|definition_span| document.analysis.definitions.get(&definition_span))
            .map(|definition| definition.label.clone()),
    }
}

fn split_signature_parameters(label: &str) -> Vec<String> {
    let Some(open_idx) = label.find('(') else {
        return Vec::new();
    };
    let Some(close_idx) = label.rfind(')') else {
        return Vec::new();
    };
    if close_idx <= open_idx + 1 {
        return Vec::new();
    }

    split_top_level(&label[open_idx + 1..close_idx], ',')
        .into_iter()
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn active_parameter_index(text: &str, call_site: &CallSite, position: Position) -> usize {
    let cursor_offset = position_to_offset(text, position);
    let Some(open_paren_offset) = find_open_paren_offset(text, call_site.search_start, call_site.span.end) else {
        return 0;
    };
    if cursor_offset <= open_paren_offset + 1 {
        return 0;
    }

    let scan_end = cursor_offset.min(call_site.span.end).min(text.len());
    let scan_start = (open_paren_offset + 1).min(scan_end);
    let snippet = &text[scan_start..scan_end];
    split_top_level(snippet, ',').len().saturating_sub(1)
}

fn split_top_level(input: &str, separator: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut angle_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escape = false;

    for (idx, ch) in input.char_indices() {
        if escape {
            escape = false;
            continue;
        }

        match ch {
            '\\' if in_string || in_char => {
                escape = true;
            }
            '"' if !in_char => in_string = !in_string,
            '\'' if !in_string => in_char = !in_char,
            '(' if !in_string && !in_char => paren_depth += 1,
            ')' if !in_string && !in_char => paren_depth = paren_depth.saturating_sub(1),
            '[' if !in_string && !in_char => bracket_depth += 1,
            ']' if !in_string && !in_char => bracket_depth = bracket_depth.saturating_sub(1),
            '{' if !in_string && !in_char => brace_depth += 1,
            '}' if !in_string && !in_char => brace_depth = brace_depth.saturating_sub(1),
            '<' if !in_string && !in_char => angle_depth += 1,
            '>' if !in_string && !in_char => angle_depth = angle_depth.saturating_sub(1),
            _ if ch == separator
                && !in_string
                && !in_char
                && paren_depth == 0
                && bracket_depth == 0
                && angle_depth == 0
                && brace_depth == 0 => {
                parts.push(input[start..idx].to_string());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(input[start..].to_string());
    parts
}

fn find_open_paren_offset(text: &str, start: usize, end: usize) -> Option<usize> {
    let upper = end.min(text.len());
    let lower = start.min(upper);
    text[lower..upper]
        .char_indices()
        .find_map(|(idx, ch)| (ch == '(').then_some(lower + idx))
}

fn position_to_offset(text: &str, position: Position) -> usize {
    let mut line = 0u32;
    let mut character = 0u32;

    for (idx, ch) in text.char_indices() {
        if line == position.line && character == position.character {
            return idx;
        }

        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }

    text.len()
}

fn offset_to_position(text: &str, target_offset: usize) -> Position {
    let mut line = 0u32;
    let mut character = 0u32;

    for (idx, ch) in text.char_indices() {
        if idx >= target_offset {
            return Position::new(line, character);
        }
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }

    // offset beyond end of text → clamp to last position
    Position::new(line, character)
}

fn analyze_cache_document(text: &str, filename: &str) -> DocumentAnalysis {
    let mut options = CompilationOptions::default();
    options.optimize = false;
    options.lint = spectra_compiler::LintOptions::disabled();
    analyze_document(text, filename, &options, None)
}

fn cache_modified_time_for_uri(uri: &Url) -> Option<SystemTime> {
    uri.to_file_path()
        .ok()
        .and_then(|path| fs::metadata(path).ok())
        .and_then(|meta| meta.modified().ok())
}

fn location_key(location: &Location) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        location.uri,
        location.range.start.line,
        location.range.start.character,
        location.range.end.line,
        location.range.end.character
    )
}

fn reference_key_for_resolved_symbol(
    document: &DocumentState,
    symbol: &spectra_compiler::ResolvedSymbol,
) -> Option<String> {
    symbol
        .definition
        .as_ref()
        .map(|definition| definition_key(&definition.label))
        .or_else(|| {
            document
                .analysis
                .definitions
                .get(&symbol.span)
                .map(|definition| definition_key(&definition.label))
        })
}

fn definition_key(label: &str) -> String {
    let trimmed = label.trim();
    if let Some(rest) = trimmed.strip_prefix("fn ") {
        return rest.split('(').next().unwrap_or(rest).trim().to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("struct ") {
        return rest.trim().to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("enum ") {
        return rest.trim().to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("trait ") {
        return rest.trim().to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("variant ") {
        return rest.trim().to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("field ") {
        return rest.split(':').next().unwrap_or(rest).trim().to_string();
    }
    trimmed.to_string()
}

fn reference_entries_for_analysis(uri: &Url, analysis: &DocumentAnalysis) -> Vec<CachedReference> {
    let mut references = Vec::new();

    for (span, info) in &analysis.symbols {
        if info.is_local {
            continue;
        }
        let Some(definition_span) = info.def_span else {
            continue;
        };
        let Some(definition) = analysis.definitions.get(&definition_span) else {
            continue;
        };

        references.push(CachedReference {
            key: definition_key(&definition.label),
            location: Location {
                uri: uri.clone(),
                range: span_to_range(*span),
            },
        });
    }

    references
}

fn workspace_symbol_entries_for_module(
    text: &str,
    module: &spectra_compiler::ast::Module,
) -> Vec<CachedWorkspaceSymbol> {
    let mut symbols = Vec::new();

    for item in &module.items {
        collect_workspace_symbol_entries(text, item, &mut symbols);
    }

    symbols
}

fn collect_workspace_symbol_entries(
    text: &str,
    item: &spectra_compiler::ast::Item,
    output: &mut Vec<CachedWorkspaceSymbol>,
) {
    match item {
        spectra_compiler::ast::Item::Import(import) => {
            output.push(CachedWorkspaceSymbol {
                name: import.alias.clone().unwrap_or_else(|| import.path.join("::")),
                detail: Some("import".to_string()),
                kind: SymbolKind::MODULE,
                span: import.span,
                container_name: None,
            });
        }
        spectra_compiler::ast::Item::Function(function) => {
            if let Some(name_span) = named_subspan(text, function.span, &function.name) {
                output.push(CachedWorkspaceSymbol {
                    name: function.name.clone(),
                    detail: Some(format_function_signature(function)),
                    kind: SymbolKind::FUNCTION,
                    span: name_span,
                    container_name: None,
                });
            }
            for param in &function.params {
                output.push(CachedWorkspaceSymbol {
                    name: param.name.clone(),
                    detail: param.ty.as_ref().map(format_type_annotation),
                    kind: SymbolKind::VARIABLE,
                    span: param.span,
                    container_name: Some(function.name.clone()),
                });
            }
        }
        spectra_compiler::ast::Item::Struct(struct_def) => {
            if let Some(name_span) = named_subspan(text, struct_def.span, &struct_def.name) {
                output.push(CachedWorkspaceSymbol {
                    name: struct_def.name.clone(),
                    detail: Some("struct".to_string()),
                    kind: SymbolKind::STRUCT,
                    span: name_span,
                    container_name: None,
                });
            }
            for field in &struct_def.fields {
                output.push(CachedWorkspaceSymbol {
                    name: field.name.clone(),
                    detail: Some(format_type_annotation(&field.ty)),
                    kind: SymbolKind::FIELD,
                    span: field.span,
                    container_name: Some(struct_def.name.clone()),
                });
            }
        }
        spectra_compiler::ast::Item::Enum(enum_def) => {
            if let Some(name_span) = named_subspan(text, enum_def.span, &enum_def.name) {
                output.push(CachedWorkspaceSymbol {
                    name: enum_def.name.clone(),
                    detail: Some("enum".to_string()),
                    kind: SymbolKind::ENUM,
                    span: name_span,
                    container_name: None,
                });
            }
            for variant in &enum_def.variants {
                output.push(CachedWorkspaceSymbol {
                    name: variant.name.clone(),
                    detail: Some(format_variant_signature(enum_def, variant)),
                    kind: SymbolKind::ENUM_MEMBER,
                    span: variant.span,
                    container_name: Some(enum_def.name.clone()),
                });
            }
        }
        spectra_compiler::ast::Item::Impl(impl_block) => {
            for method in &impl_block.methods {
                if let Some(name_span) = named_subspan(text, method.span, &method.name) {
                    output.push(CachedWorkspaceSymbol {
                        name: method.name.clone(),
                        detail: Some(format_method_signature(&impl_block.type_name, method)),
                        kind: SymbolKind::METHOD,
                        span: name_span,
                        container_name: Some(impl_block.type_name.clone()),
                    });
                }
            }
        }
        spectra_compiler::ast::Item::Trait(trait_def) => {
            if let Some(name_span) = named_subspan(text, trait_def.span, &trait_def.name) {
                output.push(CachedWorkspaceSymbol {
                    name: trait_def.name.clone(),
                    detail: Some("trait".to_string()),
                    kind: SymbolKind::INTERFACE,
                    span: name_span,
                    container_name: None,
                });
            }
            for method in &trait_def.methods {
                if let Some(name_span) = named_subspan(text, method.span, &method.name) {
                    output.push(CachedWorkspaceSymbol {
                        name: method.name.clone(),
                        detail: Some(format_trait_method_signature(method)),
                        kind: SymbolKind::METHOD,
                        span: name_span,
                        container_name: Some(trait_def.name.clone()),
                    });
                }
            }
        }
        spectra_compiler::ast::Item::TraitImpl(trait_impl) => {
            for method in &trait_impl.methods {
                if let Some(name_span) = named_subspan(text, method.span, &method.name) {
                    output.push(CachedWorkspaceSymbol {
                        name: method.name.clone(),
                        detail: Some(format_method_signature(&trait_impl.type_name, method)),
                        kind: SymbolKind::METHOD,
                        span: name_span,
                        container_name: Some(trait_impl.type_name.clone()),
                    });
                }
            }
        }
        spectra_compiler::ast::Item::TypeAlias(_) | spectra_compiler::ast::Item::Const(_) | spectra_compiler::ast::Item::Static(_) => {}
    }
}

#[allow(deprecated)]
fn workspace_symbol_information(uri: &Url, symbol: &CachedWorkspaceSymbol) -> SymbolInformation {
    SymbolInformation {
        name: symbol.name.clone(),
        kind: symbol.kind,
        tags: None,
        deprecated: None,
        location: Location {
            uri: uri.clone(),
            range: span_to_range(symbol.span),
        },
        container_name: symbol.container_name.clone(),
    }
}

fn semantic_tokens_for_document(
    text: &str,
    module: &spectra_compiler::ast::Module,
    analysis: &DocumentAnalysis,
) -> Vec<SemanticToken> {
    let (declaration_tokens, declaration_kinds) = semantic_declaration_tokens(text, module);
    let mut all_tokens = declaration_tokens;

    for (span, info) in &analysis.symbols {
        let token_type = info
            .def_span
            .and_then(|definition_span| declaration_kinds.get(&definition_span).copied())
            .unwrap_or_else(|| if info.is_local { TOKEN_VARIABLE } else { TOKEN_FUNCTION });

        all_tokens.push((*span, token_type));
    }

    all_tokens.sort_by_key(|(span, _)| (span.start_location.line, span.start_location.column, span.end));
    all_tokens.dedup_by(|left, right| left.0 == right.0 && left.1 == right.1);

    encode_semantic_tokens(all_tokens)
}

fn semantic_declaration_tokens(
    text: &str,
    module: &spectra_compiler::ast::Module,
) -> (Vec<(Span, u32)>, HashMap<Span, u32>) {
    let mut tokens = Vec::new();
    let mut kinds = HashMap::new();

    for item in &module.items {
        match item {
            spectra_compiler::ast::Item::Import(import) => {
                for span in ordered_name_subspans(text, import.span, &import.path) {
                    tokens.push((span, TOKEN_NAMESPACE));
                }
                if let Some(names) = &import.names {
                    for imported_name in names {
                        if let Some(span) = named_subspan(text, import.span, imported_name) {
                            let token_type = imported_symbol_token_type(module, imported_name);
                            tokens.push((span, token_type));
                        }
                    }
                }
                if let Some(alias) = &import.alias {
                    if let Some(span) = named_subspan(text, import.span, alias) {
                        tokens.push((span, TOKEN_NAMESPACE));
                    }
                }
                kinds.insert(import.span, TOKEN_NAMESPACE);
            }
            spectra_compiler::ast::Item::Function(function) => {
                if let Some(span) = named_subspan(text, function.span, &function.name) {
                    tokens.push((span, TOKEN_FUNCTION));
                }
                kinds.insert(function.span, TOKEN_FUNCTION);
                for type_param in &function.type_params {
                    if let Some(span) = named_subspan(text, type_param.span, &type_param.name) {
                        tokens.push((span, TOKEN_TYPE));
                    }
                    for bound in &type_param.bounds {
                        if let Some(span) = named_subspan(text, type_param.span, bound) {
                            tokens.push((span, TOKEN_INTERFACE));
                        }
                    }
                }
                for param in &function.params {
                    if let Some(span) = named_subspan(text, param.span, &param.name) {
                        tokens.push((span, TOKEN_PARAMETER));
                    }
                    kinds.insert(param.span, TOKEN_PARAMETER);
                    if let Some(ty) = &param.ty {
                        collect_type_annotation_tokens(text, ty, TOKEN_TYPE, &mut tokens);
                    }
                }
                if let Some(return_type) = &function.return_type {
                    collect_type_annotation_tokens(text, return_type, TOKEN_TYPE, &mut tokens);
                }
            }
            spectra_compiler::ast::Item::Struct(struct_def) => {
                if let Some(span) = named_subspan(text, struct_def.span, &struct_def.name) {
                    tokens.push((span, TOKEN_TYPE));
                }
                kinds.insert(struct_def.span, TOKEN_TYPE);
                for type_param in &struct_def.type_params {
                    if let Some(span) = named_subspan(text, type_param.span, &type_param.name) {
                        tokens.push((span, TOKEN_TYPE));
                    }
                }
                for field in &struct_def.fields {
                    if let Some(span) = named_subspan(text, field.span, &field.name) {
                        tokens.push((span, TOKEN_PROPERTY));
                    }
                    kinds.insert(field.span, TOKEN_PROPERTY);
                    collect_type_annotation_tokens(text, &field.ty, TOKEN_TYPE, &mut tokens);
                }
            }
            spectra_compiler::ast::Item::Enum(enum_def) => {
                if let Some(span) = named_subspan(text, enum_def.span, &enum_def.name) {
                    tokens.push((span, TOKEN_ENUM));
                }
                kinds.insert(enum_def.span, TOKEN_ENUM);
                for type_param in &enum_def.type_params {
                    if let Some(span) = named_subspan(text, type_param.span, &type_param.name) {
                        tokens.push((span, TOKEN_TYPE));
                    }
                }
                for variant in &enum_def.variants {
                    if let Some(span) = named_subspan(text, variant.span, &variant.name) {
                        tokens.push((span, TOKEN_ENUM_MEMBER));
                    }
                    kinds.insert(variant.span, TOKEN_ENUM_MEMBER);
                    if let Some(data) = &variant.data {
                        for ty in data {
                            collect_type_annotation_tokens(text, ty, TOKEN_TYPE, &mut tokens);
                        }
                    }
                    if let Some(fields) = &variant.struct_data {
                        for (_, ty) in fields {
                            collect_type_annotation_tokens(text, ty, TOKEN_TYPE, &mut tokens);
                        }
                    }
                }
            }
            spectra_compiler::ast::Item::Impl(impl_block) => {
                if let Some(span) = named_subspan(text, impl_block.span, &impl_block.type_name) {
                    tokens.push((span, TOKEN_TYPE));
                }
                if let Some(trait_name) = &impl_block.trait_name {
                    if let Some(span) = named_subspan(text, impl_block.span, trait_name) {
                        tokens.push((span, TOKEN_INTERFACE));
                    }
                }
                for method in &impl_block.methods {
                    if let Some(span) = named_subspan(text, method.span, &method.name) {
                        tokens.push((span, TOKEN_METHOD));
                    }
                    kinds.insert(method.span, TOKEN_METHOD);
                    for param in &method.params {
                        if let Some(span) = named_subspan(text, param.span, &param.name) {
                            tokens.push((span, TOKEN_PARAMETER));
                        }
                        kinds.insert(param.span, TOKEN_PARAMETER);
                        if let Some(ty) = &param.type_annotation {
                            collect_type_annotation_tokens(text, ty, TOKEN_TYPE, &mut tokens);
                        }
                    }
                    if let Some(return_type) = &method.return_type {
                        collect_type_annotation_tokens(text, return_type, TOKEN_TYPE, &mut tokens);
                    }
                }
            }
            spectra_compiler::ast::Item::Trait(trait_def) => {
                if let Some(span) = named_subspan(text, trait_def.span, &trait_def.name) {
                    tokens.push((span, TOKEN_INTERFACE));
                }
                kinds.insert(trait_def.span, TOKEN_INTERFACE);
                for parent_trait in &trait_def.parent_traits {
                    if let Some(span) = named_subspan(text, trait_def.span, parent_trait) {
                        tokens.push((span, TOKEN_INTERFACE));
                    }
                }
                for method in &trait_def.methods {
                    if let Some(span) = named_subspan(text, method.span, &method.name) {
                        tokens.push((span, TOKEN_METHOD));
                    }
                    kinds.insert(method.span, TOKEN_METHOD);
                    for param in &method.params {
                        if let Some(span) = named_subspan(text, param.span, &param.name) {
                            tokens.push((span, TOKEN_PARAMETER));
                        }
                        kinds.insert(param.span, TOKEN_PARAMETER);
                        if let Some(ty) = &param.type_annotation {
                            collect_type_annotation_tokens(text, ty, TOKEN_TYPE, &mut tokens);
                        }
                    }
                    if let Some(return_type) = &method.return_type {
                        collect_type_annotation_tokens(text, return_type, TOKEN_TYPE, &mut tokens);
                    }
                }
            }
            spectra_compiler::ast::Item::TypeAlias(_) | spectra_compiler::ast::Item::Const(_) | spectra_compiler::ast::Item::Static(_) => {}
            spectra_compiler::ast::Item::TraitImpl(trait_impl) => {
                if let Some(span) = named_subspan(text, trait_impl.span, &trait_impl.trait_name) {
                    tokens.push((span, TOKEN_INTERFACE));
                }
                if let Some(span) = named_subspan(text, trait_impl.span, &trait_impl.type_name) {
                    tokens.push((span, TOKEN_TYPE));
                }
                for method in &trait_impl.methods {
                    if let Some(span) = named_subspan(text, method.span, &method.name) {
                        tokens.push((span, TOKEN_METHOD));
                    }
                    kinds.insert(method.span, TOKEN_METHOD);
                    for param in &method.params {
                        if let Some(span) = named_subspan(text, param.span, &param.name) {
                            tokens.push((span, TOKEN_PARAMETER));
                        }
                        kinds.insert(param.span, TOKEN_PARAMETER);
                        if let Some(ty) = &param.type_annotation {
                            collect_type_annotation_tokens(text, ty, TOKEN_TYPE, &mut tokens);
                        }
                    }
                    if let Some(return_type) = &method.return_type {
                        collect_type_annotation_tokens(text, return_type, TOKEN_TYPE, &mut tokens);
                    }
                }
            }
        }
    }

    (tokens, kinds)
}

fn imported_symbol_token_type(module: &spectra_compiler::ast::Module, imported_name: &str) -> u32 {
    if module
        .imported_function_return_types
        .iter()
        .any(|(name, _)| name == imported_name)
    {
        TOKEN_FUNCTION
    } else if is_type_like_name(imported_name) {
        TOKEN_TYPE
    } else {
        TOKEN_NAMESPACE
    }
}

fn ordered_name_subspans(text: &str, container: Span, names: &[String]) -> Vec<Span> {
    if container.end > text.len() || container.start > container.end {
        return Vec::new();
    }

    let slice = &text[container.start..container.end];
    let mut spans = Vec::new();
    let mut search_from = 0usize;

    for name in names {
        if let Some(relative) = slice[search_from..].find(name) {
            let start = container.start + search_from + relative;
            let end = start + name.len();
            spans.push(span_from_offsets(text, start, end));
            search_from = end.saturating_sub(container.start);
        }
    }

    spans
}

fn collect_type_annotation_tokens(
    text: &str,
    ty: &spectra_compiler::ast::TypeAnnotation,
    final_kind: u32,
    tokens: &mut Vec<(Span, u32)>,
) {
    match &ty.kind {
        spectra_compiler::ast::TypeAnnotationKind::Simple { segments } => {
            let spans = ordered_name_subspans(text, ty.span, segments);
            for (index, span) in spans.into_iter().enumerate() {
                let kind = if index + 1 == segments.len() {
                    final_kind
                } else {
                    TOKEN_NAMESPACE
                };
                tokens.push((span, kind));
            }
        }
        spectra_compiler::ast::TypeAnnotationKind::Tuple { elements } => {
            for element in elements {
                collect_type_annotation_tokens(text, element, TOKEN_TYPE, tokens);
            }
        }
        spectra_compiler::ast::TypeAnnotationKind::Function {
            params,
            return_type,
        } => {
            for param in params {
                collect_type_annotation_tokens(text, param, TOKEN_TYPE, tokens);
            }
            collect_type_annotation_tokens(text, return_type, TOKEN_TYPE, tokens);
        }
        spectra_compiler::ast::TypeAnnotationKind::Generic { name: _, type_args } => {
            for arg in type_args {
                collect_type_annotation_tokens(text, arg, TOKEN_TYPE, tokens);
            }
        }
        spectra_compiler::ast::TypeAnnotationKind::DynTrait { .. } => {}
    }
}

fn is_type_like_name(name: &str) -> bool {
    name.chars().next().map(|ch| ch.is_ascii_uppercase()).unwrap_or(false)
}

fn encode_semantic_tokens(tokens: Vec<(Span, u32)>) -> Vec<SemanticToken> {
    let mut encoded = Vec::new();
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;

    for (span, token_type) in tokens {
        if span.start_location.line != span.end_location.line {
            continue;
        }

        let line = span.start_location.line.saturating_sub(1) as u32;
        let start = span.start_location.column.saturating_sub(1) as u32;
        let length = span.end_location.column.saturating_sub(span.start_location.column) as u32;
        if length == 0 {
            continue;
        }

        let delta_line = line.saturating_sub(prev_line);
        let delta_start = if delta_line == 0 {
            start.saturating_sub(prev_start)
        } else {
            start
        };

        encoded.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: 0,
        });

        prev_line = line;
        prev_start = start;
    }

    encoded
}

fn named_subspan(text: &str, container: Span, name: &str) -> Option<Span> {
    if name.is_empty() || container.end > text.len() || container.start > container.end {
        return None;
    }

    let slice = &text[container.start..container.end];
    let mut search_from = 0usize;
    while let Some(relative) = slice[search_from..].find(name) {
        let relative_start = search_from + relative;
        let relative_end = relative_start + name.len();

        let before = slice[..relative_start].chars().next_back();
        let after = slice[relative_end..].chars().next();
        let boundary_before = before.map(is_identifier_char).unwrap_or(false);
        let boundary_after = after.map(is_identifier_char).unwrap_or(false);
        if !boundary_before && !boundary_after {
            let start = container.start + relative_start;
            let end = container.start + relative_end;
            return Some(span_from_offsets(text, start, end));
        }

        search_from = relative_end;
    }

    None
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn span_from_offsets(text: &str, start: usize, end: usize) -> Span {
    let start_location = offset_to_location(text, start);
    let end_location = offset_to_location(text, end);
    Span {
        start,
        end,
        start_location,
        end_location,
    }
}

fn offset_to_location(text: &str, offset: usize) -> spectra_compiler::span::Location {
    let mut line = 1usize;
    let mut column = 1usize;

    for (idx, ch) in text.char_indices() {
        if idx >= offset {
            break;
        }

        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    spectra_compiler::span::Location { line, column }
}

fn quick_fix_for_diagnostic(uri: &Url, document: &DocumentState, diagnostic: &Diagnostic) -> Option<CodeAction> {
    let code = match &diagnostic.code {
        Some(NumberOrString::String(code)) => code.as_str(),
        _ => "",
    };

    // Lint-specific fixes
    match code {
        "lint(unused-binding)" => return unused_binding_quick_fix(uri, &document.text, diagnostic),
        "lint(shadowing)" => return shadowing_quick_fix(uri, document, diagnostic),
        "lint(unreachable-code)" => return unreachable_code_quick_fix(uri, diagnostic),
        _ => {}
    }

    // Semantic error fixes routed by code (E001–E009).
    match code {
        // E002: Variable already declared — rename the new binding.
        "E002" => return duplicate_binding_quick_fix(uri, &document.text, diagnostic),
        // E005: Return statement missing value — insert a default literal.
        "E005" => return missing_return_value_quick_fix(uri, &document.text, diagnostic, &diagnostic.message),
        // E001: Undefined variable — prefix with `_` to suppress or declare it.
        // E003: Type mismatch in assignment — fall through to hint-based fix.
        // E004: Return type mismatch — fall through to hint-based fix.
        // Others: fall through to pattern-matching fallbacks.
        _ => {}
    }

    // Fallback: pattern-match on message strings for errors without a code.
    semantic_error_quick_fix(uri, document, diagnostic)
        .or_else(|| hint_based_quick_fix(uri, &document.text, diagnostic))
}

/// Tenta gerar um quick fix para erros semânticos comuns, reconhecendo padrões
/// de mensagem produzidos pelo compilador.
fn semantic_error_quick_fix(
    uri: &Url,
    document: &DocumentState,
    diagnostic: &Diagnostic,
) -> Option<CodeAction> {
    let msg = diagnostic.message.as_str();

    // "Variable 'x' is already declared in this scope"
    // → renomear a nova binding para um nome único
    if msg.contains("is already declared in this scope") {
        return duplicate_binding_quick_fix(uri, &document.text, diagnostic);
    }

    // "Return statement missing value of type ..."
    // → inserir um valor padrão compatível com o tipo mencionado
    if msg.starts_with("Return statement missing value of type") {
        return missing_return_value_quick_fix(uri, &document.text, diagnostic, msg);
    }

    None
}

fn duplicate_binding_quick_fix(uri: &Url, text: &str, diagnostic: &Diagnostic) -> Option<CodeAction> {
    let binding_name = extract_quoted_name(&diagnostic.message)?;
    let replacement = unique_identifier_name(text, &binding_name);
    let binding_range = find_name_range_in_range(text, diagnostic.range, &binding_name)?;

    Some(quick_fix_action(
        uri,
        diagnostic,
        format!("Renomear '{}' para '{}'", binding_name, replacement),
        vec![TextEdit {
            range: binding_range,
            new_text: replacement,
        }],
    ))
}

fn missing_return_value_quick_fix(
    uri: &Url,
    text: &str,
    diagnostic: &Diagnostic,
    message: &str,
) -> Option<CodeAction> {
    // Extrair o tipo da mensagem. Formato: 'Return statement missing value of type Int'
    let type_str = message
        .strip_prefix("Return statement missing value of type")
        .map(|s| s.trim())
        // Remover delimitadores de debug como `Int` ou `{...}`
        .map(|s| s.trim_matches(|c: char| c == '`' || c == '"'))
        .unwrap_or("");

    let default_value = match type_str.to_lowercase().as_str() {
        "int" => "0",
        "bool" => "false",
        "string" => "\"\"",
        "float" => "0.0",
        "char" => "'\\0'",
        _ => return None,
    };

    // Localizar a posição do `return` sem valor — o range aponta para o `return` keyword.
    // Inserimos o valor entre `return` e `;`.
    let start_offset = position_to_offset(text, diagnostic.range.start);
    let end_offset = position_to_offset(text, diagnostic.range.end);
    if start_offset >= text.len() || end_offset > text.len() {
        return None;
    }

    let slice = &text[start_offset..end_offset];
    // Encontra o ponto de inserção: logo após 'return', antes do ';' ou final do range
    let insert_relative = if let Some(semi) = slice.find(';') {
        semi
    } else {
        slice.len()
    };
    let insert_offset = start_offset + insert_relative;
    let insert_pos = offset_to_position(text, insert_offset);
    let insert_range = Range::new(insert_pos, insert_pos);

    Some(quick_fix_action(
        uri,
        diagnostic,
        format!("Inserir valor de retorno padrão ({})", default_value),
        vec![TextEdit {
            range: insert_range,
            new_text: format!(" {}", default_value),
        }],
    ))
}

fn unused_binding_quick_fix(uri: &Url, text: &str, diagnostic: &Diagnostic) -> Option<CodeAction> {
    let binding_name = extract_quoted_name(&diagnostic.message)?;
    if binding_name.starts_with('_') {
        return None;
    }

    let binding_range = find_name_range_in_range(text, diagnostic.range, &binding_name)?;
    let edit = TextEdit {
        range: binding_range,
        new_text: format!("_{}", binding_name),
    };

    Some(quick_fix_action(
        uri,
        diagnostic,
        format!("Prefixar '{}' com _", binding_name),
        vec![edit],
    ))
}

fn shadowing_quick_fix(uri: &Url, document: &DocumentState, diagnostic: &Diagnostic) -> Option<CodeAction> {
    let binding_name = extract_quoted_name(&diagnostic.message)?;
    let replacement = unique_identifier_name(&document.text, &binding_name);
    let definition_range = find_name_range_in_range(&document.text, diagnostic.range, &binding_name)?;
    let definition_span = range_to_span(&document.text, definition_range);

    let mut edits = Vec::new();
    for (span, info) in &document.analysis.symbols {
        if info.def_span != Some(definition_span) {
            continue;
        }
        edits.push(TextEdit {
            range: span_to_range(*span),
            new_text: replacement.clone(),
        });
    }

    if edits.is_empty() {
        edits.push(TextEdit {
            range: definition_range,
            new_text: replacement.clone(),
        });
    }

    Some(quick_fix_action(
        uri,
        diagnostic,
        format!("Renomear '{}' para '{}'", binding_name, replacement),
        edits,
    ))
}

fn unreachable_code_quick_fix(uri: &Url, diagnostic: &Diagnostic) -> Option<CodeAction> {
    Some(quick_fix_action(
        uri,
        diagnostic,
        "Remover código inalcançável".to_string(),
        vec![TextEdit {
            range: diagnostic.range,
            new_text: String::new(),
        }],
    ))
}

fn hint_based_quick_fix(uri: &Url, text: &str, diagnostic: &Diagnostic) -> Option<CodeAction> {
    let hints = diagnostic
        .related_information
        .as_ref()?
        .iter()
        .map(|info| info.message.as_str())
        .collect::<Vec<_>>();

    for hint in hints {
        let inserted = if hint.contains("matching \" character") {
            Some("\"")
        } else if hint.contains("matching ' character") {
            Some("'")
        } else {
            None
        }?;

        let insert_offset = position_to_offset(text, diagnostic.range.end);
        let insert_range = span_to_range(span_from_offsets(text, insert_offset, insert_offset));
        return Some(quick_fix_action(
            uri,
            diagnostic,
            format!("Aplicar hint: inserir {}", inserted),
            vec![TextEdit {
                range: insert_range,
                new_text: inserted.to_string(),
            }],
        ));
    }

    None
}

fn quick_fix_action(uri: &Url, diagnostic: &Diagnostic, title: String, edits: Vec<TextEdit>) -> CodeAction {
    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);

    CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        is_preferred: Some(true),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        ..Default::default()
    }
}

fn extract_quoted_name(message: &str) -> Option<String> {
    let start = message.find('\'')?;
    let end = message[start + 1..].find('\'')? + start + 1;
    Some(message[start + 1..end].to_string())
}

fn unique_identifier_name(text: &str, base: &str) -> String {
    let mut index = 1usize;
    loop {
        let candidate = format!("{}_{}", base, index);
        if !contains_identifier(text, &candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn contains_identifier(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let end = start + name.len();
        let after = if end < text.len() { text[end..].chars().next() } else { None };
        !before.map(is_identifier_char).unwrap_or(false) && !after.map(is_identifier_char).unwrap_or(false)
    })
}

fn find_name_range_in_range(text: &str, range: Range, name: &str) -> Option<Range> {
    let start_offset = position_to_offset(text, range.start);
    let end_offset = position_to_offset(text, range.end);
    if start_offset >= end_offset || end_offset > text.len() {
        return None;
    }

    let slice = &text[start_offset..end_offset];
    let relative = slice.find(name)?;
    let before = if relative == 0 { None } else { slice[..relative].chars().next_back() };
    let after_index = relative + name.len();
    let after = if after_index >= slice.len() { None } else { slice[after_index..].chars().next() };
    if before.map(is_identifier_char).unwrap_or(false) || after.map(is_identifier_char).unwrap_or(false) {
        return None;
    }

    let absolute_start = start_offset + relative;
    let absolute_end = absolute_start + name.len();
    Some(span_to_range(span_from_offsets(text, absolute_start, absolute_end)))
}

fn range_to_span(text: &str, range: Range) -> Span {
    let start = position_to_offset(text, range.start);
    let end = position_to_offset(text, range.end);
    span_from_offsets(text, start, end)
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        state: Arc::new(BackendState::default()),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
