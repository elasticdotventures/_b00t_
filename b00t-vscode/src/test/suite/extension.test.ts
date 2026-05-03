 import './setup';
import * as assert from 'assert';
import { activate } from '../../extension';
import vscode from './mock-vscode';

suite('Extension Test Suite', () => {
    test('LSP client initializes without error', async () => {
        const context = {
            subscriptions: [],
            extensionPath: process.cwd(),
            globalState: {
                get: () => undefined,
                update: async () => {},
                setKeysForSync: () => {}
            },
            workspaceState: {
                get: () => undefined,
                update: async () => {}
            }
        } as unknown as any;

        await assert.doesNotReject(() => Promise.resolve(activate(context)));
    });

    test('Sample test', () => {
        assert.strictEqual(-1, [1, 2, 3].indexOf(5));
        assert.strictEqual(-1, [1, 2, 3].indexOf(0));
    });
});
