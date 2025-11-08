import { runSpectraCli } from './cliClient';
import { getCliPath } from './config';

interface CliMetadata {
  cliPath: string;
  version?: string;
}

let cachedMetadata: CliMetadata | undefined;
let pendingMetadata: Promise<CliMetadata> | undefined;

export async function getSpectraCliMetadata(): Promise<CliMetadata> {
  const cliPath = getCliPath();

  if (cachedMetadata && cachedMetadata.cliPath === cliPath) {
    return cachedMetadata;
  }

  if (!pendingMetadata) {
    pendingMetadata = resolveMetadata(cliPath);
  }

  const metadata = await pendingMetadata;
  cachedMetadata = metadata;
  pendingMetadata = undefined;
  return metadata;
}

async function resolveMetadata(cliPath: string): Promise<CliMetadata> {
  try {
    const result = await runSpectraCli(['--version'], { cliPath });
    if (result.exitCode === 0) {
      const versionLine = (result.stdout || result.stderr)
        .split(/\r?\n/)
        .find((line) => line.trim().length > 0);
      if (versionLine) {
        return { cliPath, version: versionLine.trim() };
      }
    }
  } catch (error) {
    // Capture failure silently; callers can report if needed.
    console.warn(`Spectra CLI version probe failed for "${cliPath}": ${String(error)}`);
  }

  return { cliPath };
}

export function resetSpectraCliMetadata(): void {
  cachedMetadata = undefined;
  pendingMetadata = undefined;
}
