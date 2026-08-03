---
session guards must not leak across test runs: guards persisted in ~/.b00t/session-guards.json can block unrelated audited commands if not isolated or torn down; tests MUST clean up session guards on teardown
