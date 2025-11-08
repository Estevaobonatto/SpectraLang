#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

function computeRange(content, match, tokenLength) {
  const before = content.slice(0, match);
  const lines = before.split(/\r?\n/);
  const line = lines.length;
  const column = (lines[lines.length - 1] || '').length + 1;
  return {
    start: { line, column },
    end: { line, column: column + tokenLength }
  };
}

function analyzeFile(filePath) {
  const content = fs.readFileSync(filePath, 'utf8');
  const diagnostics = [];

  const errorIndex = content.indexOf('error_trigger');
  if (errorIndex >= 0) {
    diagnostics.push({
      severity: 'error',
      code: 'mock/error',
      message: 'Mock CLI detected an error token.',
      phase: 'lint',
      hint: 'Remove or rename the error token to satisfy the mock rule.',
      range: computeRange(content, errorIndex, 'error_trigger'.length)
    });
  }

  const warningIndex = content.indexOf('warn_trigger');
  if (warningIndex >= 0) {
    diagnostics.push({
      severity: 'warning',
      code: 'mock/warn',
      message: 'Mock CLI detected a warning token.',
      phase: 'lint',
      hint: 'Warnings are informational in the mock CLI.',
      range: computeRange(content, warningIndex, 'warn_trigger'.length)
    });
  }

  return {
    path: filePath,
    diagnostics
  };
}

function expandTargets(targets) {
  const results = [];
  for (const target of targets) {
    const stat = fs.statSync(target);
    if (stat.isDirectory()) {
      for (const entry of fs.readdirSync(target)) {
        const fullPath = path.join(target, entry);
        results.push(...expandTargets([fullPath]));
      }
    } else if (target.endsWith('.spectra')) {
      results.push(target);
    }
  }
  return results;
}

function outputReport(files) {
  const hasError = files.some((file) => file.diagnostics.some((diag) => diag.severity === 'error'));
  const report = {
    version: 1,
    success: !hasError,
    files: files.filter((file) => file.diagnostics.length > 0)
  };
  process.stdout.write(JSON.stringify(report));
  process.exit(hasError ? 65 : 0);
}

function formatSource(content) {
  const formatted = content.replace(/value=1/g, 'value = 1');
  return formatted.endsWith('\n') ? formatted : `${formatted}\n`;
}

function main() {
  const args = process.argv.slice(2);
  if (args.length === 0) {
    console.error('mock-spectra-cli: missing command');
    process.exit(64);
  }

  if (args[0] === 'repl' && args[1] === '--json' && args[2]) {
    const filePath = path.resolve(args[2]);
    const fileEntry = analyzeFile(filePath);
    outputReport([fileEntry]);
    return;
  }

  if (args[0] === 'lint' && args[1] === '--json' && args.length >= 3) {
    const targets = args.slice(2).map((value) => path.resolve(value));
    const files = expandTargets(targets).map((filePath) => analyzeFile(filePath));
    outputReport(files);
    return;
  }

  if (args[0] === 'fmt' && args[1] === '--stdin') {
    let input = '';
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', (chunk) => {
      input += chunk;
    });
    process.stdin.on('end', () => {
      const output = formatSource(input);
      process.stdout.write(output);
      process.exit(0);
    });
    process.stdin.resume();
    return;
  }

  console.error(`mock-spectra-cli: unsupported arguments: ${args.join(' ')}`);
  process.exit(64);
}

main();
