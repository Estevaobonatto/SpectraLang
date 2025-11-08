import * as path from 'path';
import { expect } from 'chai';
import * as vscode from 'vscode';
import { suite, suiteSetup, test } from 'mocha';

function getMockCliPath(): string {
  const base = path.resolve(__dirname, '../../../test-fixtures/mock-cli');
  if (process.platform === 'win32') {
    return path.join(base, 'mock-spectra-cli.cmd');
  }
  return path.join(base, 'mock-spectra-cli');
}

async function waitForDiagnostics(
  uri: vscode.Uri,
  predicate: (diagnostics: readonly vscode.Diagnostic[]) => boolean,
  timeoutMs = 5000
): Promise<readonly vscode.Diagnostic[]> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const diagnostics = vscode.languages.getDiagnostics(uri);
    if (predicate(diagnostics)) {
      return diagnostics;
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for diagnostics for ${uri.fsPath}`);
}

async function waitForDocumentText(
  document: vscode.TextDocument,
  expected: string,
  timeoutMs = 5000
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (document.getText() === expected) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error('Timed out waiting for formatted document content.');
}

suite('Spectra VS Code Extension', () => {
  const workspaceRoot = path.resolve(__dirname, '../../../test-fixtures/workspace');
  let extension: vscode.Extension<unknown> | undefined;

  suiteSetup(async () => {
    extension = vscode.extensions.getExtension('spectralang.spectra-vscode-extension');
    expect(extension).to.not.equal(undefined);

    const config = vscode.workspace.getConfiguration('spectra');
    await config.update('cliPath', getMockCliPath(), vscode.ConfigurationTarget.Workspace);
    await config.update('lintOnSave', false, vscode.ConfigurationTarget.Workspace);

    await extension?.activate();
  });

  test('activates and registers commands', async () => {
    expect(extension).to.not.equal(undefined);

    const registeredCommands = await vscode.commands.getCommands(true);
    expect(registeredCommands).to.include('spectra.diagnostics.run');
    expect(registeredCommands).to.include('spectra.lintWorkspace');
  });

  test('produces diagnostics for the active document', async () => {
    const documentUri = vscode.Uri.file(path.join(workspaceRoot, 'src', 'contains_error.spectra'));
    const document = await vscode.workspace.openTextDocument(documentUri);
    await vscode.window.showTextDocument(document);

    await vscode.commands.executeCommand('spectra.diagnostics.run');

    const diagnostics = await waitForDiagnostics(documentUri, (items) => items.length > 0);
    const codes = diagnostics.map((diag) => diag.code);
    expect(codes).to.include('mock/error');
    expect(diagnostics[0].source).to.equal('spectra/lint');
  });

  test('workspace lint aggregates diagnostics across files', async () => {
    const lintResult = await vscode.commands.executeCommand('spectra.lintWorkspace');
    expect(lintResult).to.be.undefined;

    const errorUri = vscode.Uri.file(path.join(workspaceRoot, 'src', 'contains_error.spectra'));
    const warningUri = vscode.Uri.file(path.join(workspaceRoot, 'src', 'warning_only.spectra'));

    const errorDiagnostics = await waitForDiagnostics(errorUri, (items) => items.some((diag) => diag.code === 'mock/error'));
    const warningDiagnostics = await waitForDiagnostics(warningUri, (items) => items.some((diag) => diag.code === 'mock/warn'));

    expect(errorDiagnostics.some((diag) => diag.severity === vscode.DiagnosticSeverity.Error)).to.be.true;
    expect(warningDiagnostics.some((diag) => diag.severity === vscode.DiagnosticSeverity.Warning)).to.be.true;
  });

  test('formatter applies CLI output to the active document', async () => {
    const documentUri = vscode.Uri.file(path.join(workspaceRoot, 'src', 'needs_formatting.spectra'));
    const raw = await vscode.workspace.fs.readFile(documentUri);
    const original = Buffer.from(raw).toString('utf8');
    const expected = original.replace(/value=1/g, 'value = 1');

    const document = await vscode.workspace.openTextDocument(documentUri);
    await vscode.window.showTextDocument(document);

    await vscode.commands.executeCommand('editor.action.formatDocument');

    await waitForDocumentText(document, expected);
  });
});
