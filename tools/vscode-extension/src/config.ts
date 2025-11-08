import * as vscode from 'vscode';

const SECTION = 'spectra';

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
