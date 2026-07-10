//! b00t-lsp — LSP server (stdio) + `--check` CI mode for b00t datum dialects.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use b00t_lsp::analysis::{
    self, Diag, Severity, WorkspaceIndex,
};

struct State {
    index: Option<WorkspaceIndex>,
    docs: HashMap<Url, String>,
}

struct Backend {
    client: Client,
    state: Mutex<State>,
}

fn to_lsp_diag(d: &Diag) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position::new(d.line, d.col_start),
            end: Position::new(d.line, d.col_end),
        },
        severity: Some(match d.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
        }),
        source: Some("b00t-lsp".into()),
        message: d.message.clone(),
        ..Default::default()
    }
}

fn url_to_path(url: &Url) -> Option<PathBuf> {
    url.to_file_path().ok()
}

impl Backend {
    async fn publish(&self, uri: Url, content: &str) {
        let path = match url_to_path(&uri) {
            Some(p) => p,
            None => return,
        };
        let diags = {
            let state = self.state.lock().expect("state lock");
            analysis::diagnostics(&path, content, state.index.as_ref())
        };
        let lsp_diags = diags.iter().map(to_lsp_diag).collect();
        self.client.publish_diagnostics(uri, lsp_diags, None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Build the workspace index from the workspace root (prefer _b00t_/ if present).
        #[allow(deprecated)]
        let root = params
            .root_uri
            .as_ref()
            .and_then(url_to_path)
            .unwrap_or_else(|| PathBuf::from("."));
        let scan_root = if root.join("_b00t_").is_dir() {
            root.join("_b00t_")
        } else {
            root
        };
        {
            let mut state = self.state.lock().expect("state lock");
            state.index = Some(WorkspaceIndex::scan(&scan_root));
        }

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "b00t-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "b00t-lsp ready")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        {
            let mut state = self.state.lock().expect("state lock");
            state.docs.insert(uri.clone(), text.clone());
        }
        self.publish(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // FULL sync: last content change is the whole document.
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            {
                let mut state = self.state.lock().expect("state lock");
                state.docs.insert(uri.clone(), change.text.clone());
            }
            self.publish(uri, &change.text).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = {
            let state = self.state.lock().expect("state lock");
            state.docs.get(&uri).cloned()
        };
        if let Some(text) = text {
            self.publish(uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut state = self.state.lock().expect("state lock");
        state.docs.remove(&params.text_document.uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let Some(path) = url_to_path(&uri) else {
            return Ok(None);
        };
        let content = {
            let state = self.state.lock().expect("state lock");
            state.docs.get(&uri).cloned()
        };
        let content = match content {
            Some(c) => c,
            None => std::fs::read_to_string(&path).unwrap_or_default(),
        };
        Ok(analysis::hover(&path, &content).map(|md| Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: md,
            }),
            range: None,
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let pos = params.text_document_position_params.position;
        let uri = params.text_document_position_params.text_document.uri;
        let content = {
            let state = self.state.lock().expect("state lock");
            state.docs.get(&uri).cloned()
        };
        let Some(content) = content else {
            return Ok(None);
        };
        let Some(dep) = analysis::dep_ref_at(&content, pos.line, pos.character) else {
            return Ok(None);
        };
        let target = {
            let state = self.state.lock().expect("state lock");
            state
                .index
                .as_ref()
                .and_then(|idx| idx.resolve(&dep.name).map(|f| f.path.clone()))
        };
        let Some(target) = target else {
            return Ok(None);
        };
        let Ok(target_uri) = Url::from_file_path(&target) else {
            return Ok(None);
        };
        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri: target_uri,
            range: Range::default(),
        })))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let Some(path) = url_to_path(&uri) else {
            return Ok(None);
        };
        let locations = {
            let state = self.state.lock().expect("state lock");
            let Some(index) = state.index.as_ref() else {
                return Ok(None);
            };
            index
                .references_to(&path)
                .into_iter()
                .filter_map(|(file, dep)| {
                    Url::from_file_path(&file.path).ok().map(|u| Location {
                        uri: u,
                        range: Range {
                            start: Position::new(dep.line, dep.col_start),
                            end: Position::new(dep.line, dep.col_end),
                        },
                    })
                })
                .collect::<Vec<_>>()
        };
        Ok(Some(locations))
    }
}

/// `--check [dir]`: run diagnostics over a datum tree; exit 1 on any Error.
/// This is the CI surface used by `just validate-datums` when taplo is absent.
fn run_check(dir: &Path) -> i32 {
    let index = WorkspaceIndex::scan(dir);
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut files = 0usize;

    let paths: Vec<PathBuf> = index.files.iter().map(|f| f.path.clone()).collect();
    for path in paths {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        files += 1;
        for d in analysis::diagnostics(&path, &content, Some(&index)) {
            let sev = match d.severity {
                Severity::Error => {
                    errors += 1;
                    "error"
                }
                Severity::Warning => {
                    warnings += 1;
                    "warning"
                }
            };
            println!(
                "{}:{}:{}: {}: {}",
                path.display(),
                d.line + 1,
                d.col_start + 1,
                sev,
                d.message
            );
        }
    }
    println!("b00t-lsp --check: {files} files, {errors} errors, {warnings} warnings");
    if errors > 0 { 1 } else { 0 }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version") {
        println!("b00t-lsp {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if let Some(pos) = args.iter().position(|a| a == "--check") {
        let dir = args
            .get(pos + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("_b00t_"));
        std::process::exit(run_check(&dir));
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        state: Mutex::new(State {
            index: None,
            docs: HashMap::new(),
        }),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
