// Tests for selector-forge.js — the propose -> verify -> settle candidate
// resolution pattern borrowed from Intuned/selector-forge (see
// _b00t_/datums/VENDOR-SELECTOR-FORGE.tomllmd), reimplemented locally without
// an AI backend: candidates are ranked by stability heuristics, then
// re-verified against a live-DOM stand-in before one is chosen. Runs under
// Node's built-in test runner: `node --test selector-forge.test.js`.

const test = require('node:test');
const assert = require('node:assert/strict');
const { buildCandidates, resolveSelector } = require('./selector-forge.js');

test('buildCandidates ranks id above data-b00t attrs above class/nth-child', () => {
  const desc = {
    id: 'email-input',
    tagName: 'input',
    dataB00tType: 'input',
    dataB00tLabel: 'Email',
    dataB00tName: 'email',
    name: 'email',
    classList: ['form-control', 'v2'],
    nthChildSelector: 'form > div:nth-child(3) > input',
  };
  const candidates = buildCandidates(desc);
  assert.equal(candidates[0], '#email-input');
  assert.ok(candidates.includes('[data-b00t-type="input"][data-b00t-label="Email"]'));
  assert.equal(candidates[candidates.length - 1], desc.nthChildSelector);
});

test('buildCandidates skips absent fields without throwing', () => {
  const candidates = buildCandidates({ tagName: 'button' });
  assert.deepEqual(candidates, []);
});

test('resolveSelector settles on the first candidate that uniquely re-verifies', () => {
  const desc = {
    id: 'dup-id', // pretend two elements share this id (invalid HTML but happens)
    dataB00tType: 'button',
    dataB00tLabel: 'Submit',
  };
  // Fake DOM: '#dup-id' matches 2 elements (not unique), the data-b00t
  // candidate matches exactly 1 (the real element) -> re-verification wins.
  const fakeCounts = {
    '#dup-id': 2,
    '[data-b00t-type="button"][data-b00t-label="Submit"]': 1,
  };
  const verify = (sel) => fakeCounts[sel] ?? 0;
  const result = resolveSelector(desc, verify);
  assert.equal(result.selector, '[data-b00t-type="button"][data-b00t-label="Submit"]');
  assert.equal(result.verified, true);
});

test('resolveSelector falls back to the last candidate when none are unique, flagged unverified', () => {
  const desc = { id: 'ambiguous' };
  const verify = () => 3; // every candidate matches multiple elements
  const result = resolveSelector(desc, verify);
  assert.equal(result.selector, '#ambiguous');
  assert.equal(result.verified, false);
});

test('resolveSelector returns null selector when there are no candidates', () => {
  const result = resolveSelector({}, () => 0);
  assert.equal(result.selector, null);
  assert.equal(result.verified, false);
});
