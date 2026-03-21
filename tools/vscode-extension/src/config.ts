import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';

const SECTION = 'spectra';

function getExecutableName(baseName: string): string {
  return process.platform === 'win32' ? `${baseName}.exe` : baseName;
}

function existingPath(candidates: string[]): string | undefined {
  for (const candidate of candidates) {
    if (candidate && fs.existsSync(candidate)) {
      return candidate;
    }
  }

  return undefined;
}

export function getServerPath(context: vscode.ExtensionContext): string {
  const config = vscode.workspace.getConfiguration(SECTION);
  const configuredPath = config.get<string>('serverPath', '').trim();
  if (configuredPath !== '') {
    return configuredPath;
  }

  const executable = getExecutableName('spectra-lsp');
  const bundledCandidates = [
    path.resolve(context.extensionPath, 'server', executable),
    path.resolve(context.extensionPath, 'bin', executable),
  ];
  const bundled = existingPath(bundledCandidates);
  if (bundled) {
    return bundled;
  }

  const workspaceCandidates = (vscode.workspace.workspaceFolders ?? []).flatMap((folder) => [
    path.resolve(folder.uri.fsPath, 'target', 'debug', executable),
    path.resolve(folder.uri.fsPath, 'target', 'release', executable),
    path.resolve(folder.uri.fsPath, 'tools', 'vscode-extension', 'server', executable),
  ]);
  const workspaceBinary = existingPath(workspaceCandidates);
  if (workspaceBinary) {
    return workspaceBinary;
  }

  const legacyCandidates = [
    path.resolve(context.extensionPath, '..', '..', 'target', 'debug', executable),
    path.resolve(context.extensionPath, '..', '..', 'target', 'release', executable),
  ];
  const legacy = existingPath(legacyCandidates);
  if (legacy) {
    return legacy;
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
