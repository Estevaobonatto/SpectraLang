import * as path from 'path';
import * as vscode from 'vscode';
import { runSpectraCli } from './cliClient';
import { getCliPath, lintOnSaveEnabled } from './config';

function normalizeFsPath(value: string): string {
  let normalized = value.replace(/^\\\\\?\\/, '');
  if (!path.isAbsolute(normalized)) {
    normalized = path.resolve(normalized);
  }
  if (process.platform === 'win32') {
    normalized = normalized.toLowerCase();
  }
  return path.normalize(normalized);
}

function inferSeverity(
  explicit: string | undefined,
  descriptor: string
): vscode.DiagnosticSeverity {
  if (explicit) {
    return explicit.toLowerCase() === 'warning'
      ? vscode.DiagnosticSeverity.Warning
      : vscode.DiagnosticSeverity.Error;
  }

  const lower = descriptor.toLowerCase();
  if (lower.includes(' warning')) {
    return vscode.DiagnosticSeverity.Warning;
  }
  return vscode.DiagnosticSeverity.Error;
}

function extractDiagnosticCode(descriptor: string): string | undefined {
  const lintMatch = descriptor.match(/lint\(([^)]+)\)/i);
  if (lintMatch) {
    return lintMatch[1];
  }
  const phaseMatch = descriptor.match(/^(lexical|parse|semantic)\b/i);
  if (phaseMatch) {
    return phaseMatch[1].toLowerCase();
  }
  return undefined;
}

function parseDiagnostics(
  output: string,
  document: vscode.TextDocument
): vscode.Diagnostic[] {
  const diagnostics: vscode.Diagnostic[] = [];
  const normalizedTarget = normalizeFsPath(document.fileName);
  const lines = output.split(/\r?\n/);

  const patternWithSeverity = /^(warning|error):\s+(.+?):(\d+):(\d+):\s+(.*)$/i;
  const patternWithoutSeverity = /^(.+?):(\d+):(\d+):\s+(.*)$/;

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) {
      continue;
    }

    let match = patternWithSeverity.exec(line);
    let severityToken: string | undefined;
    if (!match) {
      match = patternWithoutSeverity.exec(line);
    } else {
      severityToken = match[1];
    }

    if (!match) {
      continue;
    }

    const [, filePathRaw, lineText, columnText, descriptor] = match;
    const filePath = normalizeFsPath(filePathRaw);
    if (filePath !== normalizedTarget) {
      continue;
    }

    const lineIndex = Number(lineText) - 1;
    const columnIndex = Math.max(0, Number(columnText) - 1);
    if (!Number.isFinite(lineIndex) || !Number.isFinite(columnIndex)) {
      continue;
    }

    if (lineIndex < 0 || lineIndex >= document.lineCount) {
      continue;
    }

    const severity = inferSeverity(severityToken, descriptor);
    const lineLength = document.lineAt(lineIndex).text.length;
    const endColumn = Math.min(lineLength, columnIndex + 1);

    const diagnostic = new vscode.Diagnostic(
      new vscode.Range(lineIndex, columnIndex, lineIndex, endColumn),
      descriptor.trim(),
      severity
    );
    diagnostic.source = 'spectra';

    const code = extractDiagnosticCode(descriptor);
    if (code) {
      diagnostic.code = code;
    }

    diagnostics.push(diagnostic);
  }

  return diagnostics;
}

export class SpectraDiagnosticsManager implements vscode.Disposable {
  private readonly collection = vscode.languages.createDiagnosticCollection('spectra');
  private readonly running = new Map<string, vscode.CancellationTokenSource>();

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly output: vscode.OutputChannel
  ) {}

  dispose(): void {
    this.collection.dispose();
    for (const token of this.running.values()) {
      token.dispose();
    }
    this.running.clear();
  }

  async runDiagnostics(document: vscode.TextDocument): Promise<void> {
    if (document.languageId !== 'spectra') {
      return;
    }

    if (document.isUntitled) {
      vscode.window.showInformationMessage(
        'Save the Spectra file before running diagnostics.'
      );
      return;
    }

    if (document.isDirty) {
      const choice = await vscode.window.showWarningMessage(
        'Save changes before running Spectra diagnostics.',
        'Save and Continue',
        'Cancel'
      );
      if (choice === 'Save and Continue') {
        const didSave = await document.save();
        if (!didSave) {
          return;
        }
      } else {
        return;
      }
    }

    const uriKey = document.uri.toString();
    const existing = this.running.get(uriKey);
    existing?.cancel();
    existing?.dispose();

    const cts = new vscode.CancellationTokenSource();
    this.running.set(uriKey, cts);

    const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
    const cliPath = getCliPath();

    try {
      const args = ['check', document.fileName, '--lint'];
      this.output.appendLine(`▶ spectra ${args.join(' ')}`);

      const result = await runSpectraCli(args, {
        cliPath,
        cwd: workspaceFolder?.uri.fsPath,
        token: cts.token,
      });

      const combinedOutput = `${result.stdout}\n${result.stderr}`;
      if (combinedOutput.trim().length > 0) {
        this.output.appendLine(combinedOutput.trimEnd());
      }

      const diagnostics = parseDiagnostics(combinedOutput, document);
      this.collection.set(document.uri, diagnostics);

      if (result.exitCode !== 0 && diagnostics.length === 0) {
        vscode.window.showErrorMessage(
          `Spectra diagnostics failed (exit code ${result.exitCode}). See output for details.`
        );
      }
    } catch (error) {
      vscode.window.showErrorMessage(
        `Failed to run Spectra diagnostics: ${String(error)}`
      );
      this.output.appendLine(String(error));
    } finally {
      this.running.delete(uriKey);
      cts.dispose();
    }
  }

  attachListeners(): void {
    this.context.subscriptions.push(
      vscode.workspace.onDidOpenTextDocument(async (document: vscode.TextDocument) => {
        if (document.languageId === 'spectra' && lintOnSaveEnabled()) {
          await this.runDiagnostics(document);
        }
      })
    );

    this.context.subscriptions.push(
      vscode.workspace.onDidSaveTextDocument(async (document: vscode.TextDocument) => {
        if (document.languageId === 'spectra' && lintOnSaveEnabled()) {
          await this.runDiagnostics(document);
        }
      })
    );

    this.context.subscriptions.push(
      vscode.workspace.onDidCloseTextDocument((document: vscode.TextDocument) => {
        if (document.languageId === 'spectra') {
          this.collection.delete(document.uri);
        }
      })
    );
  }

  clear(document: vscode.TextDocument): void {
    this.collection.delete(document.uri);
  }
}
