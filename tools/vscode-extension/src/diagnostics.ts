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

interface SpectraJsonReport {
  version: number;
  success: boolean;
  files: SpectraJsonFile[];
}

interface SpectraJsonFile {
  path: string;
  diagnostics: SpectraJsonDiagnostic[];
}

interface SpectraJsonDiagnostic {
  severity: 'error' | 'warning';
  code?: string;
  message: string;
  phase?: string;
  hint?: string;
  range: SpectraJsonRange;
  related?: SpectraJsonRelated[];
}

interface SpectraJsonRange {
  start: SpectraJsonPosition;
  end: SpectraJsonPosition;
}

interface SpectraJsonPosition {
  line: number;
  column: number;
}

interface SpectraJsonRelated {
  message: string;
  range?: SpectraJsonRange;
}

interface DiagnosticEntry {
  uri: vscode.Uri;
  diagnostics: vscode.Diagnostic[];
}

function parseJsonDiagnostics(stdout: string): SpectraJsonReport {
  const trimmed = stdout.trim();
  if (!trimmed) {
    throw new Error('Spectra CLI returned no diagnostics.');
  }

  let parsed: SpectraJsonReport;
  try {
    parsed = JSON.parse(trimmed) as SpectraJsonReport;
  } catch (error) {
    throw new Error(`Failed to parse Spectra diagnostics JSON: ${String(error)}`);
  }

  if (!Array.isArray(parsed.files)) {
    throw new Error('Spectra diagnostics payload is missing file entries.');
  }

  return parsed;
}

function toVscodeRange(jsonRange: SpectraJsonRange): vscode.Range {
  const startLine = Math.max(0, Math.floor(jsonRange.start.line) - 1);
  const startCharacter = Math.max(0, Math.floor(jsonRange.start.column) - 1);
  const endLine = Math.max(0, Math.floor(jsonRange.end.line) - 1);
  const endCharacter = Math.max(0, Math.floor(jsonRange.end.column) - 1);

  if (endLine < startLine || (endLine === startLine && endCharacter < startCharacter)) {
    return new vscode.Range(startLine, startCharacter, startLine, startCharacter);
  }

  return new vscode.Range(startLine, startCharacter, endLine, endCharacter);
}

function jsonDiagnosticToVscode(
  diagnostic: SpectraJsonDiagnostic,
  uri: vscode.Uri
): vscode.Diagnostic {
  const range = toVscodeRange(diagnostic.range);
  const severity = diagnostic.severity === 'warning'
    ? vscode.DiagnosticSeverity.Warning
    : vscode.DiagnosticSeverity.Error;

  const result = new vscode.Diagnostic(range, diagnostic.message, severity);
  result.source = diagnostic.phase ? `spectra/${diagnostic.phase}` : 'spectra';

  if (diagnostic.code) {
    result.code = diagnostic.code;
  }

  const related: vscode.DiagnosticRelatedInformation[] = [];
  if (diagnostic.hint) {
    related.push(
      new vscode.DiagnosticRelatedInformation(
        new vscode.Location(uri, range),
        diagnostic.hint
      )
    );
  }

  if (diagnostic.related) {
    for (const entry of diagnostic.related) {
      const relatedRange = entry.range ? toVscodeRange(entry.range) : range;
      related.push(
        new vscode.DiagnosticRelatedInformation(
          new vscode.Location(uri, relatedRange),
          entry.message
        )
      );
    }
  }

  if (related.length > 0) {
    result.relatedInformation = related;
  }

  return result;
}

function convertReportToEntries(report: SpectraJsonReport): Map<string, DiagnosticEntry> {
  const entries = new Map<string, DiagnosticEntry>();

  for (const file of report.files) {
    const normalizedPath = normalizeFsPath(file.path);
    const uri = vscode.Uri.file(file.path);
    const diagnostics = file.diagnostics.map((diag) => jsonDiagnosticToVscode(diag, uri));
    entries.set(normalizedPath, { uri, diagnostics });
  }

  return entries;
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
      const args = ['repl', '--json', document.fileName];
      this.output.appendLine(`▶ spectra ${args.join(' ')}`);

      const result = await runSpectraCli(args, {
        cliPath,
        cwd: workspaceFolder?.uri.fsPath,
        token: cts.token,
      });

      const stderr = result.stderr.trim();
      if (stderr.length > 0) {
        this.output.appendLine(stderr);
      }

      let report: SpectraJsonReport;
      try {
        report = parseJsonDiagnostics(result.stdout);
      } catch (error) {
        this.output.appendLine('Failed to parse Spectra diagnostics JSON output.');
        if (result.stdout.trim().length > 0) {
          this.output.appendLine(result.stdout.trimEnd());
        }
        vscode.window.showErrorMessage(String(error));
        return;
      }

      const entries = convertReportToEntries(report);
      const normalizedDocPath = normalizeFsPath(document.fileName);

      for (const entry of entries.values()) {
        this.collection.set(entry.uri, entry.diagnostics);
      }

      if (!entries.has(normalizedDocPath)) {
        this.collection.set(document.uri, []);
      }

      if (result.exitCode !== 0 && report.success) {
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

  async runWorkspaceLint(
    folders: readonly vscode.WorkspaceFolder[]
  ): Promise<number | undefined> {
    if (!folders.length) {
      return 0;
    }

    const cliPath = getCliPath();
    const folderPaths = folders.map((folder) => folder.uri.fsPath);
    const args = ['lint', '--json', ...folderPaths];
    this.output.appendLine(`▶ spectra ${args.join(' ')}`);

    try {
      const result = await runSpectraCli(args, {
        cliPath,
        cwd: folderPaths[0],
      });

      const stderr = result.stderr.trim();
      if (stderr.length > 0) {
        this.output.appendLine(stderr);
      }

      let report: SpectraJsonReport;
      try {
        report = parseJsonDiagnostics(result.stdout);
      } catch (error) {
        this.output.appendLine('Failed to parse Spectra lint JSON output.');
        if (result.stdout.trim().length > 0) {
          this.output.appendLine(result.stdout.trimEnd());
        }
        vscode.window.showErrorMessage(String(error));
        return undefined;
      }

      const entries = convertReportToEntries(report);
      this.collection.clear();
      for (const entry of entries.values()) {
        this.collection.set(entry.uri, entry.diagnostics);
      }

      if (result.exitCode !== 0 && report.success) {
        vscode.window.showErrorMessage(
          `Spectra lint failed (exit code ${result.exitCode}). See output for details.`
        );
      }

      return report.files.length;
    } catch (error) {
      vscode.window.showErrorMessage(`Failed to lint workspace: ${String(error)}`);
      this.output.appendLine(String(error));
      return undefined;
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
