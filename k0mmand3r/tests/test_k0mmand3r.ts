// file: tests/test_k0mmand3r.ts

import { KmdLineWasm } from '../pkg/k0mmand3r';
import assert from 'assert';

describe('Wasm Tests', () => {
    it('should correctly parse KmdLine with verb and parameter', () => {
        const result = KmdLineWasm.parse('/verb --param1=value1');
        assert.strictEqual(result.verb(), 'verb');
        assert.strictEqual(result.params(), '{"param1":"value1"}');
      });

      it('should correctly parse content without a command', () => {
        const result = KmdLineWasm.parse('just some random content');
        assert.strictEqual(result.verb(), undefined);
        assert.strictEqual(result.content(), 'just some random content');
      });

      it('should correctly parse multiple parameters', () => {
        const result = KmdLineWasm.parse('/multiverb --param1=value1 --param2=value2');
        assert.strictEqual(result.verb(), 'multiverb');
        assert.strictEqual(result.params(), '{"param1":"value1","param2":"value2"}');
      });

      it('should handle only verb without parameters', () => {
        const result = KmdLineWasm.parse('/onlyverb');
        assert.strictEqual(result.verb(), 'onlyverb');
        assert.strictEqual(result.params(), undefined);
      });

      it('should correctly parse command with tags', () => {
        const result = KmdLineWasm.parse('/tagverb --tag1 --tag2');
        assert.strictEqual(result.verb(), 'tagverb');
        assert.strictEqual(result.params(), '{"tag1":"","tag2":""}');
      });

  // Additional test cases
});

