import * as vscode from 'vscode';
import * as fs from 'fs';
import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from 'vscode-languageclient/node';
import { formatOnSaveEnabled, getCliPath, getServerPath, lintOnSaveEnabled, setExtensionPath } from './config';
import { runSpectraCli } from './cliClient';

const RUN_DIAGNOSTICS_COMMAND = 'spectra.diagnostics.run';
const LINT_WORKSPACE_COMMAND = 'spectra.lintWorkspace';
const COMPILE_CURRENT_FILE_COMMAND = 'spectra.compileCurrentFile';
const CHECK_CURRENT_FILE_COMMAND = 'spectra.checkCurrentFile';
const RUN_CURRENT_FILE_COMMAND = 'spectra.runCurrentFile';
const COMPILER_ACTIONS_COMMAND = 'spectra.compilerActions';
const NEW_PROJECT_COMMAND = 'spectra.newProject';

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel('Spectra');
  context.subscriptions.push(outputChannel);

  // Propagar extensionPath para o módulo config para que getCliPath() encontre o
  // binário bundled em server/spectra-cli.exe mesmo sem ele estar no PATH.
  setExtensionPath(context.extensionPath);

  // Registrar todos os comandos primeiro — independem do LSP estar disponível.
  registerCommands(context);
  registerFormatOnSaveHook(context);

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration('spectra.serverPath')) {
        void restartClient(context);
      }
    })
  );

  // Iniciar LSP de forma não-bloqueante: falha não impede os comandos CLI.
  try {
    client = await startClient(context, outputChannel);
  } catch {
    // Erros já foram logados e notificados dentro de startClient.
    // A extensão continua funcionando sem LSP (comandos CLI disponíveis).
  }
}

function registerCommands(context: vscode.ExtensionContext): void {
  // NOTA: RUN_DIAGNOSTICS_COMMAND e LINT_WORKSPACE_COMMAND NÃO são registrados
  // aqui. Eles são anunciados em execute_command_provider no servidor LSP e o
  // ExecuteCommandFeature do vscode-languageclient os registra automaticamente
  // ao inicializar o cliente. Registrá-los manualmente causaria conflito
  // "command already exists".

  context.subscriptions.push(
    vscode.commands.registerCommand(COMPILE_CURRENT_FILE_COMMAND, async () => {
      await executeCliCommandForActiveDocument('compile', 'Compilar arquivo atual');
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(CHECK_CURRENT_FILE_COMMAND, async () => {
      await executeCliCommandForActiveDocument('check', 'Validar arquivo atual');
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(RUN_CURRENT_FILE_COMMAND, async () => {
      await executeCliCommandForActiveDocument('run', 'Executar arquivo atual');
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(COMPILER_ACTIONS_COMMAND, async () => {
      await showCompilerActionsQuickPick();
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(NEW_PROJECT_COMMAND, async () => {
      await createNewProject();
    })
  );
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}

async function startClient(
  context: vscode.ExtensionContext,
  output: vscode.OutputChannel
): Promise<LanguageClient> {
  const serverPath = getServerPath(context);
  const usesPathLookup = serverPath === 'spectra-lsp';

  if (!usesPathLookup && !fs.existsSync(serverPath)) {
    const message = `Spectra language server não encontrado em ${serverPath}. Reinstale a extensão com o instalador do repositório ou configure spectra.serverPath.`;
    output.appendLine(message);
    throw new Error(message);
  }

  const executable: Executable = {
    command: serverPath,
    transport: TransportKind.stdio,
  };

  const serverOptions: ServerOptions = {
    run: executable,
    debug: executable,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'spectra' },
      { scheme: 'untitled', language: 'spectra' },
    ],
    outputChannel: output,
    synchronize: {
      configurationSection: 'spectra',
      fileEvents: vscode.workspace.createFileSystemWatcher('**/*.spectra'),
    },
    initializationOptions: {
      spectra: {
        cliPath: getCliPath(),
        lintOnSave: lintOnSaveEnabled(),
      },
    },
  };

  const nextClient = new LanguageClient(
    'spectra',
    'Spectra Language Server',
    serverOptions,
    clientOptions
  );

  try {
    await nextClient.start();
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    output.appendLine(`Falha ao iniciar spectra-lsp: ${detail}`);
    void vscode.window.showWarningMessage(
      'Spectra: servidor de linguagem não pôde ser iniciado. Funcionalidades LSP (hover, go-to-definition) unavailable. Configure spectra.serverPath se necessário.'
    );
    throw error;
  }

  context.subscriptions.push(nextClient);
  output.appendLine(`Spectra language server started from ${serverPath}`);
  return nextClient;
}

async function restartClient(context: vscode.ExtensionContext): Promise<void> {
  if (!outputChannel) {
    return;
  }

  if (client) {
    await client.stop();
  }

  client = await startClient(context, outputChannel);
}

function registerFormatOnSaveHook(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.workspace.onWillSaveTextDocument((event) => {
      if (!formatOnSaveEnabled() || event.document.languageId !== 'spectra') {
        return;
      }

      const editorConfig = vscode.workspace.getConfiguration('editor', event.document.uri);
      const formattingOptions: vscode.FormattingOptions = {
        insertSpaces: editorConfig.get<boolean>('insertSpaces', true),
        tabSize: editorConfig.get<number>('tabSize', 4),
      };

      event.waitUntil(
        vscode.commands.executeCommand<vscode.TextEdit[]>(
          'vscode.executeFormatDocumentProvider',
          event.document.uri,
          formattingOptions
        )
      );
    })
  );
}

async function executeCliCommandForActiveDocument(
  command: 'compile' | 'check' | 'run',
  progressTitle: string
): Promise<void> {
  const document = await getActiveSpectraDocumentForCommand();
  if (!document) {
    return;
  }

  const cliPath = getCliPath();
  const args = [command, document.fileName];
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);

  outputChannel?.show(true);
  outputChannel?.appendLine(`▶ spectra ${args.join(' ')}`);

  try {
    const result = await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: `Spectra: ${progressTitle}`,
        cancellable: false,
      },
      async () =>
        runSpectraCli(args, {
          cliPath,
          cwd: workspaceFolder?.uri.fsPath,
        })
    );

    const stdout = result.stdout.trimEnd();
    const stderr = result.stderr.trimEnd();

    if (stdout) {
      outputChannel?.appendLine(stdout);
    }

    if (stderr) {
      outputChannel?.appendLine(stderr);
    }

    if (result.exitCode === 0) {
      const message = successMessageForCliCommand(command);
      vscode.window.showInformationMessage(message);
      return;
    }

    vscode.window.showErrorMessage(
      `Spectra '${command}' terminou com código ${result.exitCode}. Veja o canal de saída Spectra.`
    );
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    outputChannel?.appendLine(detail);
    vscode.window.showErrorMessage(
      `Falha ao executar 'spectra ${command}': ${detail}`
    );
  }
}

async function getActiveSpectraDocumentForCommand(): Promise<vscode.TextDocument | undefined> {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== 'spectra') {
    vscode.window.showInformationMessage('Abra um arquivo Spectra para usar os comandos do compilador.');
    return undefined;
  }

  const document = editor.document;
  if (document.isUntitled) {
    vscode.window.showInformationMessage('Salve o arquivo Spectra antes de usar os comandos do compilador.');
    return undefined;
  }

  if (!document.isDirty) {
    return document;
  }

  const choice = await vscode.window.showWarningMessage(
    'Salve as alterações antes de executar o compilador Spectra.',
    'Salvar e Continuar',
    'Cancelar'
  );

  if (choice !== 'Salvar e Continuar') {
    return undefined;
  }

  const didSave = await document.save();
  return didSave ? document : undefined;
}

function successMessageForCliCommand(command: 'compile' | 'check' | 'run'): string {
  switch (command) {
    case 'compile':
      return 'Arquivo Spectra compilado com sucesso.';
    case 'check':
      return 'Validação do arquivo Spectra concluída sem erros.';
    case 'run':
      return 'Execução do arquivo Spectra concluída com sucesso.';
  }
}

// ---------------------------------------------------------------------------
// Quick Pick: ações do compilador
// ---------------------------------------------------------------------------

interface CompilerActionItem extends vscode.QuickPickItem {
  action: () => Promise<void>;
}

async function showCompilerActionsQuickPick(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  const hasSpectraFile = editor?.document.languageId === 'spectra';

  const items: CompilerActionItem[] = [
    {
      label: '$(play) Executar arquivo atual',
      description: 'spectra run',
      detail: 'Compila e executa o arquivo .spectra ativo',
      action: () => executeCliCommandForActiveDocument('run', 'Executar arquivo atual'),
    },
    {
      label: '$(check) Validar arquivo atual',
      description: 'spectra check',
      detail: 'Verifica tipos e erros sem compilar',
      action: () => executeCliCommandForActiveDocument('check', 'Validar arquivo atual'),
    },
    {
      label: '$(tools) Compilar arquivo atual',
      description: 'spectra compile',
      detail: 'Compila o arquivo .spectra ativo',
      action: () => executeCliCommandForActiveDocument('compile', 'Compilar arquivo atual'),
    },
    {
      label: '$(warning) Lint workspace',
      description: 'spectra lint',
      detail: 'Executa lint em todos os arquivos da workspace',
      action: async () => {
        await vscode.commands.executeCommand(LINT_WORKSPACE_COMMAND);
      },
    },
    {
      label: '$(file-code) Formatar documento',
      description: 'spectra fmt',
      detail: 'Formata o arquivo .spectra ativo',
      action: async () => {
        if (!editor || editor.document.languageId !== 'spectra') {
          vscode.window.showInformationMessage('Abra um arquivo Spectra para formatar.');
          return;
        }
        await vscode.commands.executeCommand('editor.action.formatDocument');
      },
    },
    {
      label: '$(add) Novo Projeto',
      description: 'spectra new',
      detail: 'Cria um novo projeto Spectra em uma pasta',
      action: () => createNewProject(),
    },
  ];

  const filteredItems = hasSpectraFile
    ? items
    : items.filter((item) => !item.description?.startsWith('spectra run') &&
                               !item.description?.startsWith('spectra check') &&
                               !item.description?.startsWith('spectra compile') &&
                               !item.description?.startsWith('spectra fmt'));

  const selected = await vscode.window.showQuickPick(filteredItems, {
    title: 'Spectra: Ações do Compilador',
    placeHolder: 'Escolha uma ação para executar',
    matchOnDescription: true,
    matchOnDetail: true,
  });

  if (selected) {
    await selected.action();
  }
}

// ---------------------------------------------------------------------------
// Novo Projeto
// ---------------------------------------------------------------------------

async function createNewProject(): Promise<void> {
  const projectName = await vscode.window.showInputBox({
    title: 'Novo Projeto Spectra',
    prompt: 'Nome do projeto',
    placeHolder: 'meu-projeto',
    validateInput: (value) => {
      if (!value.trim()) {
        return 'O nome do projeto não pode estar vazio.';
      }
      if (!/^[a-zA-Z0-9_-]+$/.test(value.trim())) {
        return 'Use apenas letras, números, hífens e underscores.';
      }
      return undefined;
    },
  });

  if (!projectName) {
    return;
  }

  const folderUris = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    openLabel: 'Criar projeto aqui',
    title: 'Escolha onde criar o projeto',
  });

  if (!folderUris || folderUris.length === 0) {
    return;
  }

  const parentFolder = folderUris[0].fsPath;
  const projectPath = require('path').join(parentFolder, projectName.trim());
  const cliPath = getCliPath();
  const args = ['new', projectPath];

  outputChannel?.show(true);
  outputChannel?.appendLine(`▶ spectra ${args.join(' ')}`);

  try {
    const result = await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: `Spectra: Criando projeto '${projectName}'`,
        cancellable: false,
      },
      async () => runSpectraCli(args, { cliPath, cwd: parentFolder })
    );

    const stdout = result.stdout.trimEnd();
    const stderr = result.stderr.trimEnd();

    if (stdout) {
      outputChannel?.appendLine(stdout);
    }
    if (stderr) {
      outputChannel?.appendLine(stderr);
    }

    if (result.exitCode !== 0) {
      vscode.window.showErrorMessage(
        `Falha ao criar o projeto '${projectName}' (código ${result.exitCode}). Veja o canal de saída Spectra.`
      );
      return;
    }

    const openChoice = await vscode.window.showInformationMessage(
      `Projeto '${projectName}' criado com sucesso em ${projectPath}.`,
      'Abrir Pasta'
    );

    if (openChoice === 'Abrir Pasta') {
      await vscode.commands.executeCommand(
        'vscode.openFolder',
        vscode.Uri.file(projectPath),
        { forceNewWindow: false }
      );
    }
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    outputChannel?.appendLine(detail);
    vscode.window.showErrorMessage(`Falha ao executar 'spectra new': ${detail}`);
  }
}
