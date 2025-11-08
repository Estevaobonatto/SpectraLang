import * as cp from 'child_process';
import * as vscode from 'vscode';

export interface SpectraCliResult {
  stdout: string;
  stderr: string;
  exitCode: number;
}

export interface SpectraCliExecutionOptions {
  cwd?: string;
  input?: string;
  token?: vscode.CancellationToken;
  cliPath: string;
}

export async function runSpectraCli(
  args: readonly string[],
  options: SpectraCliExecutionOptions
): Promise<SpectraCliResult> {
  return new Promise<SpectraCliResult>((resolve, reject) => {
    const { cwd, input, token, cliPath } = options;
    const child = cp.spawn(cliPath, args, {
      cwd,
      shell: process.platform === 'win32',
      env: process.env,
    });

    let stdout = '';
    let stderr = '';

    child.stdout?.on('data', (data: Buffer) => {
      stdout += data.toString();
    });

    child.stderr?.on('data', (data: Buffer) => {
      stderr += data.toString();
    });

    child.on('error', (error) => {
      reject(error);
    });

    child.on('close', (code) => {
      resolve({
        stdout,
        stderr,
        exitCode: typeof code === 'number' ? code : -1,
      });
    });

    if (token) {
      token.onCancellationRequested(() => {
        if (!child.killed) {
          child.kill();
        }
      });
    }

    if (typeof input === 'string' && child.stdin) {
      child.stdin.write(input);
      child.stdin.end();
    }
  });
}
