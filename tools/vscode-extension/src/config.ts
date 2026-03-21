import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';

const SECTION = 'spectra';

function getExecutableName(baseName: string): string {
  return process.platform === 'win32' ? `${baseName}.exe` : baseName;
}

export function getServerPath(context: vscode.ExtensionContext): string {
  const config = vscode.workspace.getConfiguration(SECTION);
  const configuredPath = config.get<string>('serverPath', '').trim();
  if (configuredPath !== '') {
    return configuredPath;
  }

  const executable = getExecutableName('spectra-lsp');
  const candidates = [
    path.resolve(context.extensionPath, '..', '..', 'target', 'debug', executable),
    path.resolve(context.extensionPath, '..', '..', 'target', 'release', executable),
  ];

  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }

  return 'spectra-lsp';
}

export function getCliPath(): string {
  const config = vscode.workspace.getConfiguration(SECTION);
  const cliPath = config.get<string>('cliPath', 'spectra');
  return cliPath.trim() === '' ? 'spectra' : cliPath.trim();
}

export function lintOnSaveEnabled(): boolean {
  return vscode.workspace.getConfiguration(SECTION).get<boolean>('lintOnSave', true);
}

export function formatOnSaveEnabled(): boolean {
  return vscode.workspace.getConfiguration(SECTION).get<boolean>('formatOnSave', false);
}
