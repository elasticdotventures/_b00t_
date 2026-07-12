---
git branching: never commit directly to main. always checkout -b task/N-slug, push branch, create PR with test/lint evidence, wait for merge. applies to every repo touched: b00t, app4dog, game-play, artifacts, devices. commit inner repos first then outer. force-push only on own task branches with force-with-lease.
