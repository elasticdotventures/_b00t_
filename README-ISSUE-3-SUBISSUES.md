# Sub-Issues for Issue #3 - Build and Test Systems

This directory contains documentation and scripts for creating systematic sub-issues to build and test each component of the b00t system.

## Overview

Issue #3 ("replace crudini with toml") has been broken down into 7 systematic sub-issues covering:

1. **Rust Workspace** - Core Rust crates build/test
2. **NPM/TypeScript** - JavaScript/TypeScript packages  
3. **Python Systems** - Python packages and PyO3 bindings
4. **Containers** - Docker and devcontainer infrastructure
5. **Integration Testing** - Cross-system integration tests
6. **CI/CD Pipeline** - GitHub Actions improvements
7. **Documentation** - Developer onboarding docs

## Files

- **`ISSUE_3_SUB_ISSUES.md`** - Detailed breakdown of all 7 sub-issues with tasks, acceptance criteria, and dependencies
- **`create-issue-3-subissues.sh`** - Automated script to create GitHub issues from the breakdown
- **`README-ISSUE-3-SUBISSUES.md`** - This file

## Quick Start

### Option 1: Automated Creation (Recommended)

```bash
# Ensure you have GitHub CLI installed and authenticated
gh auth status

# Run the automated script (requires GH_TOKEN in CI environment)
./create-issue-3-subissues.sh
```

The script will:
- Create 7 sub-issues in the repository
- Add appropriate labels to each issue
- Comment on the parent issue #3 with references to all sub-issues

### Option 2: Manual Creation

If automated creation isn't possible, use the detailed descriptions in `ISSUE_3_SUB_ISSUES.md` to manually create issues via the GitHub web interface.

For each sub-issue:
1. Copy the title from the markdown
2. Copy the description (everything under that sub-issue section)
3. Add the recommended labels
4. Link to parent issue #3

## Sub-Issue Details

### Issue 1: Rust Workspace Build & Test
**Labels:** `rust`, `testing`, `build`  
**Priority:** 1 (Foundation)

Build and test 8 Rust workspace crates including cross-compilation.

### Issue 2: NPM/TypeScript Systems
**Labels:** `typescript`, `npm`, `testing`, `build`  
**Priority:** 3 (Integrations)

Build and test 6 NPM packages including WASM compilation.

### Issue 3: Python Systems
**Labels:** `python`, `testing`, `build`  
**Priority:** 4 (PyO3 dependencies)

Build and test Python packages with PyO3 bindings.

### Issue 4: Container Infrastructure
**Labels:** `docker`, `container`, `testing`  
**Priority:** 2 (Deployment critical)

Build and optimize Docker containers for deployment.

### Issue 5: Integration Testing
**Labels:** `testing`, `integration`  
**Priority:** 6 (System cohesion)

Create end-to-end integration test framework.

### Issue 6: CI/CD Pipeline
**Labels:** `ci-cd`, `github-actions`, `testing`  
**Priority:** 5 (Automation)

Enhance GitHub Actions with comprehensive testing.

### Issue 7: Documentation
**Labels:** `documentation`  
**Priority:** 7 (Contributor enablement)

Create comprehensive build/test documentation.

## Implementation Strategy

Follow the priority order:

1. **Rust** → Establishes foundation
2. **Containers** → Critical for deployment
3. **NPM/TypeScript** → Important integrations
4. **Python** → PyO3 bindings dependency
5. **CI/CD** → Automates testing
6. **Integration** → Verifies system cohesion
7. **Documentation** → Enables contributors

## Success Criteria

- ✅ All systems build successfully on all platforms
- ✅ All tests pass in CI/CD pipeline
- ✅ Code coverage >70%
- ✅ Zero critical security vulnerabilities
- ✅ Release process automated and documented
- ✅ New contributor can build/test within 30 minutes

## Relationship to Issue #3

The original issue #3 ("replace crudini with toml") requires systematic build and test procedures to ensure TOML handling works correctly across all systems. These sub-issues provide that systematic approach.

## Contributing

When working on sub-issues:

1. Check out the detailed task list in `ISSUE_3_SUB_ISSUES.md`
2. Follow the acceptance criteria for each sub-issue
3. Update the checklist as you complete tasks
4. Document any deviations or additional findings
5. Link PRs to the appropriate sub-issue

## Questions?

- Review the full breakdown in `ISSUE_3_SUB_ISSUES.md`
- Check the parent issue #3 for context
- Refer to existing CI/CD workflows in `.github/workflows/`

---

**Created:** 2025-11-19  
**Parent Issue:** #3  
**Repository:** elasticdotventures/_b00t_
