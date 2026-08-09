// b00t Selector Forge — deterministic "propose -> verify -> settle" selector
// resolution, adapted from Intuned/selector-forge's trust boundary:
//   "extension holds session state; browser is source of truth (re-verifies
//    every candidate); AI proposes/ranks but doesn't prove correctness."
// (see _b00t_/datums/VENDOR-SELECTOR-FORGE.tomllmd, integration point #2)
//
// This module has no AI/network dependency by design — it ranks candidates
// with a stability heuristic (id > [data-b00t-*] > name > class > nth-child,
// matching b00t's own enrichment story: [data-b00t-*] attrs are stable
// across redesigns where CSS classes are not) and re-verifies each against
// a caller-supplied `verify(selector) -> matchCount` before settling. In the
// browser that verify function is `document.querySelectorAll(sel).length`;
// in tests it's a plain lookup table (see selector-forge.test.js). This
// split keeps the ranking/settling logic pure and unit-testable under
// Node's built-in test runner without a real DOM or jsdom dependency.
(function (root, factory) {
  if (typeof module === 'object' && module.exports) {
    module.exports = factory();
  } else {
    root.__b00tSelectorForge = factory();
  }
}(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  function esc(s) {
    return String(s).replace(/(["\\])/g, '\\$1');
  }

  // Build ranked selector candidates for a plain element descriptor.
  // Descriptor fields are all optional; absent ones are skipped silently.
  function buildCandidates(desc) {
    desc = desc || {};
    const candidates = [];

    if (desc.id) {
      candidates.push('#' + desc.id);
    }
    if (desc.dataB00tType && desc.dataB00tLabel) {
      candidates.push(
        '[data-b00t-type="' + esc(desc.dataB00tType) + '"][data-b00t-label="' + esc(desc.dataB00tLabel) + '"]'
      );
    }
    if (desc.dataB00tType && desc.dataB00tName) {
      candidates.push(
        '[data-b00t-type="' + esc(desc.dataB00tType) + '"][data-b00t-name="' + esc(desc.dataB00tName) + '"]'
      );
    }
    if (desc.name) {
      candidates.push((desc.tagName || '') + '[name="' + esc(desc.name) + '"]');
    }
    if (desc.classList && desc.classList.length) {
      candidates.push((desc.tagName || '') + '.' + desc.classList.map(esc).join('.'));
    }
    if (desc.nthChildSelector) {
      candidates.push(desc.nthChildSelector);
    }
    return candidates;
  }

  // propose -> test against live DOM -> settle. `verify(selector)` must
  // return the number of elements the selector currently matches; a
  // candidate is accepted the moment it uniquely resolves (count === 1).
  // If none are unique, the lowest-ranked (least stable) candidate is
  // returned as a best-effort fallback with `verified: false` so callers
  // can decide whether to trust it.
  function resolveSelector(desc, verify) {
    const candidates = buildCandidates(desc);
    if (candidates.length === 0) {
      return { selector: null, verified: false, candidates: [] };
    }
    for (const candidate of candidates) {
      if (verify(candidate) === 1) {
        return { selector: candidate, verified: true, candidates };
      }
    }
    return { selector: candidates[candidates.length - 1], verified: false, candidates };
  }

  return { buildCandidates, resolveSelector };
}));
