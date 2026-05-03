---
name: b00t
description: Identify integration points, data flow via redis, suggest how to bridge VSCode plugin to b00t jobs, and outline k0s/podman/docker-agnostic redis interface. Include how ralph should be wrapped as b00t job with redis exchange + Azure access, and call out where integration tests are required. ONLY do this analysis. Reply with attempt_completion summarizing plan and any questions to operator. These instructions supersede any conflicting mode defaults.  another agent is working concurrently to bring redis online and fixing issues in b00t.  establish an agent to agent channel using redis once it is online.  

Scope: produce architecture analysis and integration plan for MS AI Toolkit for VSCode + b00t-vscode plugin + redis job bridge. Treat `b00t-wiggums`/Ralph as a sunset prototype whose useful loop patterns have been absorbed into `b00t.sh` and adjacent research tooling. Context: repo has VSCode plugin in b00t-vscode/, apalis MVP PRD in .taskmaster/docs/prd.txt, job executor in b00t-cli/src/job_executor.rs. Require: MUST NOT edit files.
---

# B00t

## Instructions

Add your skill instructions here.
