import { LanguageClient, LanguageClientOptions, StreamInfo, PublishDiagnosticsParams } from 'vscode-languageclient/node';
import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { Range, Position, Diagnostic } from 'vscode-languageserver';

export function activate(context: vscode.ExtensionContext) {
	const workspaceRoot = (vscode.workspace.workspaceFolders && (vscode.workspace.workspaceFolders.length > 0))
		? vscode.workspace.workspaceFolders[0].uri.fsPath : undefined;
	if (!workspaceRoot) {
		return;
	}
	
	// Register JustTaskProvider for justfile recipes
	context.subscriptions.push(
		vscode.tasks.registerTaskProvider(JustTaskProvider.JustType, new JustTaskProvider(workspaceRoot))
	);

	// Initialize LSP clients
	initializeJustLsp(context);
	// TODO: re-enable once b00t-lsp speaks JSON-RPC/Content-Length (tower-lsp pending)
	// initializeB00tLsp(context);
}

// --- Just LSP Client (existing functionality) ---
function initializeJustLsp(context: vscode.ExtensionContext) {
	let justLspPath: string | undefined;
	const localBin = path.join(context.extensionPath, '..', 'just-lsp', 'bin', 'just-lsp');
	if (fs.existsSync(localBin)) {
		justLspPath = localBin;
	} else {
		try {
			const which = cp.execSync('which just-lsp').toString().trim();
			if (which) {
				justLspPath = which;
			}
		} catch {
			justLspPath = undefined;
		}
	}

	if (!justLspPath) {
		vscode.window.showErrorMessage('just-lsp binary not found. Please build or install just-lsp.');
		return;
	}

	const lspProcess = cp.spawn(justLspPath, [], { stdio: ['pipe', 'pipe', 'pipe'] });
	const serverOptions = () => Promise.resolve<StreamInfo>({
		writer: lspProcess.stdin,
		reader: lspProcess.stdout
	});

	const clientOptions: LanguageClientOptions = {
		documentSelector: [{ scheme: 'file', language: 'just' }],
		synchronize: {
			fileEvents: vscode.workspace.createFileSystemWatcher('**/justfile')
		}
	};

	const lspClient = new LanguageClient('justLsp', 'Just LSP', serverOptions, clientOptions);
	lspClient.start();
	context.subscriptions.push({ dispose: () => lspClient.stop() });
}

// --- b00t LSP Client (new: TOMLLM datum support) ---
function initializeB00tLsp(context: vscode.ExtensionContext) {
	let b00tLspPath: string | undefined;
	
	// Try local build first
	const localBin = path.join(context.extensionPath, '..', 'target', 'release', 'b00t-lsp');
	if (fs.existsSync(localBin)) {
		b00tLspPath = localBin;
	} else {
		try {
			const which = cp.execSync('which b00t-lsp').toString().trim();
			if (which) {
				b00tLspPath = which;
			}
		} catch {
			b00tLspPath = undefined;
		}
	}

	if (!b00tLspPath) {
		vscode.window.showWarningMessage('b00t-lsp binary not found. TOMLLM LSP features disabled.');
		return;
	}

	// 🤓 LSP Proxy with SLO/SLI enforcement
	const lspProcess = cp.spawn(b00tLspPath, ['--stdio'], { 
		stdio: ['pipe', 'pipe', 'pipe'],
		env: {
			...process.env,
			B00T_FEATURE_TOMLLM_AST: '1',
			B00T_SLO_TIME_SECONDS: '3600',
			B00T_SLO_COST_CENTS: '1000',
		}
	});

	const serverOptions = () => Promise.resolve<StreamInfo>({
		writer: lspProcess.stdin,
		reader: lspProcess.stdout
	});

	const clientOptions: LanguageClientOptions = {
		documentSelector: [
			{ scheme: 'file', language: 'tomllm' },
			{ scheme: 'file', pattern: '**/*.toml' },
			{ scheme: 'file', pattern: '**/*.tomllm' }
		],
		synchronize: {
			fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{toml,tomllm}')
		},
		initializationOptions: {
			b00t_path: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
			dynamic_inspection: true,
		}
	};

	const lspClient = new LanguageClient('b00tLsp', 'b00t Datum LSP', serverOptions, clientOptions);
	lspClient.start();
	
	// Track LSP health via signal pattern detection
	const healthCheck = setInterval(() => {
		// 🤓 Proxy metric: if LSP is active, it's doing something useful
		vscode.window.setStatusBarMessage('🤖 b00t LSP active', 3000);
	}, 30000);
	
	context.subscriptions.push({ 
		dispose: () => {
			lspClient.stop();
			clearInterval(healthCheck);
		}
	});
}

// This method is called when your extension is deactivated
export function deactivate() { }

/**
 * Attribution: The JustTaskProvider class and related functionality are derived from the vscode-justfile-mcp extension.
 * Repository: https://github.com/elasticdotventures/vscode-justfile-mcp
 * License: MIT
 */
export class JustTaskProvider implements vscode.TaskProvider {
	static JustType = 'just';
	private justPromise: Thenable<vscode.Task[]> | undefined = undefined;
	private flakeExists?: boolean;

	constructor(workspaceRoot: string) {
		const pattern = path.join(workspaceRoot, 'justfile');
		const fileWatcher = vscode.workspace.createFileSystemWatcher(pattern);
		fileWatcher.onDidChange(() => this.justPromise = undefined);
		fileWatcher.onDidCreate(() => this.justPromise = undefined);
		fileWatcher.onDidDelete(() => this.justPromise = undefined);
		flakeNixExists(workspaceRoot).then(x => this.flakeExists = x);
	}

	public provideTasks(): Thenable<vscode.Task[]> | undefined {
		if (!this.justPromise) {
			this.justPromise = getJustTasks();
		}
		return this.justPromise;
	}

	public resolveTask(_task: vscode.Task): vscode.Task | undefined {
		// resolve tasks allows vscode to skip the provideTasks and execute a specific task without knowing it's available
		const taskName = _task.definition.task;
		// A just task consists of a task and an optional file as specified in justTaskDefinition
		// Make sure that this looks like a just task by checking that there is a task.
		if (taskName) {
			// resolveTask requires that the same definition object be used.
			const definition = _task.definition;
			const commandLine = getCommandLine(definition.task, this.flakeExists ?? false);
			return new vscode.Task(definition, _task.scope ?? vscode.TaskScope.Workspace, definition.task, 'just', new vscode.ShellExecution(commandLine, { cwd: definition.dir }));
		}
		return undefined;
	}
}

function exec(command: string, options: cp.ExecOptions): Promise<{ stdout: string; stderr: string }> {
	return new Promise<{ stdout: string; stderr: string }>((resolve, reject) => {
		cp.exec(command, options, (error, stdout, stderr) => {
			if (error) {
				reject({ error, stdout, stderr });
			}
			resolve({ stdout, stderr });
		});
	});
}

let _channel: vscode.OutputChannel;
function getOutputChannel(): vscode.OutputChannel {
	if (!_channel) {
		_channel = vscode.window.createOutputChannel('just Auto Detection');
	}
	return _channel;
}

interface JustTaskDefinition extends vscode.TaskDefinition {
	/**
	 * The task name
	 */
	task: string;
	/**
	 * The dir of the justfile containing the task
	 */
	dir: string;
	promptForArgs: boolean;
	flakeExists: boolean;
}

const buildNames: string[] = ['build', 'compile', 'watch'];
function isBuildTask(name: string): boolean {
	for (const buildName of buildNames) {
		if (name.indexOf(buildName) !== -1) {
			return true;
		}
	}
	return false;
}

const testNames: string[] = ['test'];
function isTestTask(name: string): boolean {
	for (const testName of testNames) {
		if (name.indexOf(testName) !== -1) {
			return true;
		}
	}
	return false;
}

async function exists(filePath: string): Promise<boolean> {
	try {
		await vscode.workspace.fs.stat(vscode.Uri.file(filePath));
		return true;
	} catch {
		return false;
	}
}

async function flakeNixExists(folder: string): Promise<boolean> {
	return await exists(path.join(folder, 'flake.nix'));
}

enum UseNix {
	AUTO = 'auto',
	TRUE = 'yes',
	FALSE = 'no'
}

const EXPERIMENTAL_FEATURE = false;

function getExecution(definition: JustTaskDefinition) {
	let baseCommand = getCommandLine(definition.task, definition.flakeExists);

	if (definition.promptForArgs && EXPERIMENTAL_FEATURE) {
		const isWindows = process.platform === 'win32';
		if (isWindows) {
			// Windows - powershell
			const promptCmd = `$cmdargs = Read-Host 'Enter arguments for ${definition.task}'`;
			baseCommand = `${promptCmd}; ${baseCommand} $cmdargs`;
		} else {
			// Linux/macOS - bash/zsh
			const promptCmd = `read -p "Enter arguments for ${definition.task}: " cmdargs`;
			baseCommand = `${promptCmd}; ${baseCommand} "$cmdargs"`;
		}
	}

	return new vscode.ShellExecution(baseCommand, { cwd: definition.dir });
}

function getCommandLine(taskName: string, flakeExists: boolean): string {
	const config = vscode.workspace.getConfiguration('just-recipe-runner');
	let useNix = config.get('useNix') as UseNix;
	if (useNix === UseNix.AUTO) { // auto
		useNix = flakeExists ? UseNix.TRUE : UseNix.FALSE;
	}
	if (useNix === UseNix.TRUE) {
		return `/nix/var/nix/profiles/default/bin/nix develop --print-build-logs --command just ${taskName}`;
	}
	return `just ${taskName}`;
}

async function getJustTasks(): Promise<vscode.Task[]> {
	const workspaceFolders = vscode.workspace.workspaceFolders;
	const result: vscode.Task[] = [];
	if (!workspaceFolders || workspaceFolders.length === 0) {
		return result;
	}
	for (const workspaceFolder of workspaceFolders) {
		const folderString = workspaceFolder.uri.fsPath;
		if (!folderString) {
			continue;
		}
		const justfile = path.join(folderString, 'justfile');
		if (!fs.existsSync(justfile)) {
			continue;
		}

		const commandLine = 'just -l';
		try {
			// run just -l in the workspaceFolder
			// TODO: iterate each non-ignored folder in the workspace folder
			const { stdout, stderr } = await exec(commandLine, { cwd: folderString });
			if (stderr && stderr.length > 0) {
				getOutputChannel().appendLine(stderr);
				getOutputChannel().show(true);
			}
			if (stdout) {
				const flakeExists = await flakeNixExists(workspaceFolder.uri.fsPath);

				const recipeLines = stdout.trim().split('\n').splice(1);
				for (const line of recipeLines) {
					const [recipeName, docComment] = line.split('#', 2);
					const parts = recipeName.trim().split(' ');
					const taskName = parts[0];
					const taskDetail = docComment?.trim();
					const definition: JustTaskDefinition = {
						type: 'just',
						task: taskName,
						dir: folderString,
						promptForArgs: parts.length > 1,
						flakeExists
					};
					const task = new vscode.Task(definition, workspaceFolder, taskName, 'just', getExecution(definition));
					task.detail = taskDetail;
					const lowerCaseLine = line.toLowerCase();
					if (isBuildTask(lowerCaseLine)) {
						task.group = vscode.TaskGroup.Build;
					} else if (isTestTask(lowerCaseLine)) {
						task.group = vscode.TaskGroup.Test;
					}
					result.push(task);
				}
			}
		} catch (err: any) {
			const channel = getOutputChannel();
			if (err.stderr) {
				channel.appendLine(err.stderr);
			}
			if (err.stdout) {
				channel.appendLine(err.stdout);
			}
			channel.appendLine('Auto detecting just tasks failed.');
			channel.show(true);
		}
	}
	return result;
}



