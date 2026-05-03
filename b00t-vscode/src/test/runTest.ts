// src/test/runTest.ts
import * as path from 'path';
import { runTests, downloadAndUnzipVSCode } from '@vscode/test-electron';

async function main() {
  try {
    const extensionDevelopmentPath = path.resolve(__dirname, '../../');
    const extensionTestsPath = path.resolve(__dirname, './suite/index');
    const vscodeExecutablePath = await downloadAndUnzipVSCode('stable');
    await runTests({
        vscodeExecutablePath,
        extensionDevelopmentPath,
        extensionTestsPath,
        launchArgs: ['--disable-extensions']
    });
    // Explicitly require and run the test suite after VSCode launches
    const testModule = require(extensionTestsPath);
    if (typeof testModule.run === 'function') {
        testModule.run();
    }
  } catch (err) {
    console.error('Failed to run tests');
    process.exit(1);
  }
}

main();