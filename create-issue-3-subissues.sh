#!/usr/bin/env bash
# Script to create GitHub sub-issues for Issue #3: Build and Test Each System
# Generated from ISSUE_3_SUB_ISSUES.md

set -euo pipefail

REPO="elasticdotventures/_b00t_"
PARENT_ISSUE="3"

echo "🚀 Creating sub-issues for #${PARENT_ISSUE} in ${REPO}"
echo ""

# Function to create an issue and capture its number
create_issue() {
    local title="$1"
    local body_file="$2"
    local labels="$3"
    
    echo "📝 Creating: ${title}"
    
    # Create the issue and extract the issue number
    issue_url=$(gh issue create \
        --repo "${REPO}" \
        --title "${title}" \
        --body-file "${body_file}" \
        --label "${labels}")
    
    issue_number=$(echo "${issue_url}" | grep -oE '[0-9]+$')
    echo "   ✅ Created issue #${issue_number}: ${issue_url}"
    echo ""
    
    echo "${issue_number}"
}

# Create temporary files for each sub-issue body
TEMP_DIR=$(mktemp -d)
trap 'rm -rf ${TEMP_DIR}' EXIT

# Sub-Issue 1: Rust Workspace
cat > "${TEMP_DIR}/issue1.md" << 'EOF'
**Parent Issue:** #3 - Replace crudini with toml

Establish comprehensive build and test procedures for all Rust workspace members.

## Workspace Members
- `b00t-cli` - Main CLI binary
- `b00t-mcp` - MCP server binary
- `b00t-c0re-lib` - Core library
- `b00t-lib-chat` - Chat library
- `b00t-grok` - Grok system
- `b00t-py` - Python bindings (PyO3)
- `b00t-ipc` - IPC system
- `k0mmand3r` - Commander system

## Tasks
- [ ] Verify `cargo build --workspace` succeeds for all members
- [ ] Verify `cargo test --workspace` passes for all members
- [ ] Run `cargo clippy --workspace -- -D warnings` (lint)
- [ ] Run `cargo fmt --check` (format check)
- [ ] Test cross-compilation targets:
  - `x86_64-unknown-linux-gnu`
  - `aarch64-unknown-linux-gnu`
  - `armv7-unknown-linux-gnueabihf`
  - `x86_64-apple-darwin`
  - `aarch64-apple-darwin`
- [ ] Verify binary functionality:
  - `b00t-cli --version`
  - `b00t-cli status`
  - `b00t-mcp --help`
- [ ] Document build requirements in `b00t-cli/README.md`
- [ ] Document test coverage gaps

## Acceptance Criteria
- All workspace crates build without errors
- All tests pass
- No clippy warnings with `-D warnings`
- Code is properly formatted
- Cross-compilation succeeds for all targets
- Binaries execute basic commands successfully

## Dependencies
- Rust toolchain (stable)
- Cross-compilation tools for ARM targets
- CI/CD environment setup

See [ISSUE_3_SUB_ISSUES.md](../blob/main/ISSUE_3_SUB_ISSUES.md) for complete details.
EOF

# Sub-Issue 2: NPM/TypeScript
cat > "${TEMP_DIR}/issue2.md" << 'EOF'
**Parent Issue:** #3 - Replace crudini with toml

Establish build and test procedures for all NPM packages and TypeScript projects.

## NPM Packages
- `b00t-browser-ext` - Browser extension
- `b00t-c0re-npm` - Core NPM package (TypeScript + Rust WASM)
- `b00t-mcp-npm` - MCP NPM package
- `b00t-npm` - Main NPM package
- `b00t-vscode` - VS Code extension
- `b00t-tf` - Terraform integration

## Tasks
- [ ] Verify `npm install` succeeds for each package
- [ ] Run `npm test` for packages with tests
- [ ] Run `npm run build` for packages with build scripts
- [ ] Run TypeScript compiler checks (`tsc --noEmit`)
- [ ] Run linter (`npm run lint` or `eslint`)
- [ ] Test WASM compilation in `b00t-c0re-npm`
- [ ] Verify browser extension manifest validity
- [ ] Test VS Code extension activation
- [ ] Check for outdated dependencies (`npm outdated`)
- [ ] Document build/test procedures per package

## Acceptance Criteria
- All NPM packages install successfully
- All builds complete without errors
- All tests pass
- No TypeScript compilation errors
- No linting errors
- WASM builds successfully
- Browser extension loads correctly
- VS Code extension activates without errors

## Dependencies
- Node.js (v18+)
- npm or pnpm
- TypeScript
- wasm-pack (for WASM compilation)
- Browser for extension testing

See [ISSUE_3_SUB_ISSUES.md](../blob/main/ISSUE_3_SUB_ISSUES.md) for complete details.
EOF

# Sub-Issue 3: Python Systems
cat > "${TEMP_DIR}/issue3.md" << 'EOF'
**Parent Issue:** #3 - Replace crudini with toml

Establish build and test procedures for Python packages.

## Python Packages
- `b00t-grok-py` - Python grok implementation
- `b00t-j0b-py` - Job system Python package
- `b00t-py` - Python bindings (PyO3)
- `k0mmand3r` - Commander (has Python components)

## Tasks
- [ ] Verify Python version compatibility (3.12+)
- [ ] Install dependencies using `pip install -e .` or `uv pip install`
- [ ] Run `pytest` for all packages with tests
- [ ] Run `mypy` for type checking
- [ ] Run `ruff check` or `flake8` for linting
- [ ] Run `ruff format --check` or `black --check` for format
- [ ] Test PyO3 bindings in `b00t-py`
- [ ] Verify wheel building (`python -m build`)
- [ ] Test package installation from wheel
- [ ] Document virtual environment setup

## Acceptance Criteria
- All Python packages install successfully
- All tests pass (pytest)
- Type checking passes (mypy)
- Linting passes (ruff/flake8)
- Code formatting is correct
- PyO3 bindings compile and work
- Wheels build successfully
- Installed packages import correctly

## Dependencies
- Python 3.12+
- pip, uv, or poetry
- pytest
- mypy
- ruff or flake8/black
- Rust toolchain (for PyO3)

See [ISSUE_3_SUB_ISSUES.md](../blob/main/ISSUE_3_SUB_ISSUES.md) for complete details.
EOF

# Sub-Issue 4: Container/Docker
cat > "${TEMP_DIR}/issue4.md" << 'EOF'
**Parent Issue:** #3 - Replace crudini with toml

Establish Docker container build and test procedures.

## Container Targets
- Main b00t container (`Dockerfile`)
- CLI-specific container (`Dockerfile.b00t-cli`)
- Development container (`.devcontainer`)

## Tasks
- [ ] Build main container: `docker build -t b00t:latest .`
- [ ] Build CLI container: `docker build -f Dockerfile.b00t-cli -t b00t-cli:latest .`
- [ ] Test container functionality:
  - `docker run --rm b00t:latest b00t --version`
  - `docker run --rm b00t:latest b00t status`
- [ ] Verify container size optimization
- [ ] Test multi-stage builds work correctly
- [ ] Verify all required tools present in container
- [ ] Test devcontainer configuration
- [ ] Document container usage and configuration
- [ ] Push containers to GHCR (GitHub Container Registry)

## Acceptance Criteria
- All containers build successfully
- Container images are optimized (<500MB)
- b00t commands work inside containers
- All required tools available in containers
- Devcontainer works in VS Code/Codespaces
- Containers published to GHCR
- Documentation updated

## Dependencies
- Docker or Podman
- GitHub Container Registry access
- Multi-arch build support (buildx)

See [ISSUE_3_SUB_ISSUES.md](../blob/main/ISSUE_3_SUB_ISSUES.md) for complete details.
EOF

# Sub-Issue 5: Integration Testing
cat > "${TEMP_DIR}/issue5.md" << 'EOF'
**Parent Issue:** #3 - Replace crudini with toml

Establish end-to-end integration tests that verify inter-system communication and functionality.

## Test Scenarios
- CLI → MCP server communication
- Rust ↔ Python FFI (PyO3 bindings)
- Rust ↔ JavaScript/WASM integration
- File system operations (datums, skills, learn)
- Configuration management (TOML parsing/writing)
- Session management
- Multi-agent coordination (hive)

## Tasks
- [ ] Create integration test directory structure
- [ ] Write CLI integration tests
  - `b00t learn <skill>` workflow
  - `b00t lfmf <topic> <lesson>` workflow
  - `b00t session` lifecycle
- [ ] Write MCP integration tests
  - Server startup/shutdown
  - Tool invocation
  - State management
- [ ] Write cross-language tests
  - Python calling Rust via PyO3
  - JavaScript calling Rust via WASM
- [ ] Create test fixtures and data
- [ ] Document test execution procedures
- [ ] Add integration tests to CI pipeline

## Acceptance Criteria
- Integration test framework established
- Core workflows have integration tests
- All integration tests pass
- Tests run in CI/CD
- Documentation explains how to run tests
- Test coverage report available

## Dependencies
- All subsystems built and working
- Test frameworks (pytest, jest, cargo test)
- CI/CD environment

See [ISSUE_3_SUB_ISSUES.md](../blob/main/ISSUE_3_SUB_ISSUES.md) for complete details.
EOF

# Sub-Issue 6: CI/CD Pipeline
cat > "${TEMP_DIR}/issue6.md" << 'EOF'
**Parent Issue:** #3 - Replace crudini with toml

Improve GitHub Actions workflows to systematically test all components.

## Current Workflows
- `build-release.yml` - Release builds
- `publish-crates.yml` - Crates.io publishing
- `b00t-npm-release.yml` - NPM publishing
- `b00t-mcp-npm-release.yml` - MCP NPM publishing
- `browser-ext-release.yml` - Browser extension
- `b00t-cli-container.yml` - Container builds
- `rust-k0mmand3r.yml` - k0mmand3r specific

## Tasks
- [ ] Create unified test workflow (`test.yml`)
  - Rust workspace tests
  - NPM package tests
  - Python package tests
  - Integration tests
- [ ] Add matrix testing for multiple platforms
  - Ubuntu (x86_64, aarch64)
  - macOS (Intel, ARM)
  - Windows WSL2
- [ ] Add dependency caching to speed up CI
- [ ] Create pre-commit workflow for PR validation
- [ ] Add code coverage reporting
- [ ] Add security scanning (cargo audit, npm audit)
- [ ] Document CI/CD architecture
- [ ] Add workflow status badges to README

## Acceptance Criteria
- Unified test workflow runs on all PRs
- All platforms tested in CI
- Tests complete in <15 minutes
- Coverage reports generated
- Security vulnerabilities detected
- Documentation updated
- Status badges visible in README

## Dependencies
- GitHub Actions access
- Secrets configured (CARGO_TOKEN, NPM_TOKEN)
- Code coverage service (Codecov)

See [ISSUE_3_SUB_ISSUES.md](../blob/main/ISSUE_3_SUB_ISSUES.md) for complete details.
EOF

# Sub-Issue 7: Documentation
cat > "${TEMP_DIR}/issue7.md" << 'EOF'
**Parent Issue:** #3 - Replace crudini with toml

Document all build and test procedures for new developers and contributors.

## Documentation Needed
- Development environment setup
- Building from source (all systems)
- Running tests (all systems)
- CI/CD architecture
- Release procedures
- Troubleshooting guide

## Tasks
- [ ] Create `CONTRIBUTING.md` with build/test instructions
- [ ] Document `BUILDING.md` with platform-specific guides
- [ ] Create `TESTING.md` with test execution guide
- [ ] Update README with quick start for developers
- [ ] Document CI/CD in `.github/README.md`
- [ ] Create troubleshooting guide for common issues
- [ ] Add architecture diagrams for system overview
- [ ] Document release process in `RELEASING.md`

## Acceptance Criteria
- Complete developer documentation available
- New contributor can build from source following docs
- All test procedures documented
- Common issues have solutions
- Architecture is visually documented
- Release process is clear

## Dependencies
- Complete understanding of all systems
- Diagram tools (mermaid, graphviz)

See [ISSUE_3_SUB_ISSUES.md](../blob/main/ISSUE_3_SUB_ISSUES.md) for complete details.
EOF

# Create all issues
echo "Creating 7 sub-issues..."
echo ""

issue1=$(create_issue "Systematic build and test for Rust workspace components" "${TEMP_DIR}/issue1.md" "rust,testing,build")
issue2=$(create_issue "Systematic build and test for NPM/TypeScript packages" "${TEMP_DIR}/issue2.md" "typescript,npm,testing,build")
issue3=$(create_issue "Systematic build and test for Python packages" "${TEMP_DIR}/issue3.md" "python,testing,build")
issue4=$(create_issue "Systematic build and test for container infrastructure" "${TEMP_DIR}/issue4.md" "docker,container,testing")
issue5=$(create_issue "Create integration testing framework across systems" "${TEMP_DIR}/issue5.md" "testing,integration")
issue6=$(create_issue "Enhance CI/CD pipeline with comprehensive system tests" "${TEMP_DIR}/issue6.md" "ci-cd,github-actions,testing")
issue7=$(create_issue "Create comprehensive build/test documentation" "${TEMP_DIR}/issue7.md" "documentation")

# Update parent issue with references
echo "📋 Updating parent issue #${PARENT_ISSUE} with sub-issue references..."

PARENT_UPDATE=$(cat << EOF
## Sub-Issues Created

This issue has been broken down into systematic sub-issues for building and testing each system:

- #${issue1} - Rust workspace components
- #${issue2} - NPM/TypeScript packages  
- #${issue3} - Python packages
- #${issue4} - Container infrastructure
- #${issue5} - Integration testing framework
- #${issue6} - CI/CD pipeline improvements
- #${issue7} - Build/test documentation

See [ISSUE_3_SUB_ISSUES.md](ISSUE_3_SUB_ISSUES.md) for complete breakdown and details.

## Implementation Priority
1. Rust Workspace (#${issue1})
2. Containers (#${issue4})
3. NPM/TypeScript (#${issue2})
4. Python (#${issue3})
5. CI/CD (#${issue6})
6. Integration Testing (#${issue5})
7. Documentation (#${issue7})
EOF
)

# Comment on parent issue
gh issue comment "${PARENT_ISSUE}" --repo "${REPO}" --body "${PARENT_UPDATE}"

echo ""
echo "✅ All sub-issues created successfully!"
echo ""
echo "Summary:"
echo "  - Rust Workspace: #${issue1}"
echo "  - NPM/TypeScript: #${issue2}"
echo "  - Python Systems: #${issue3}"
echo "  - Containers: #${issue4}"
echo "  - Integration Tests: #${issue5}"
echo "  - CI/CD Pipeline: #${issue6}"
echo "  - Documentation: #${issue7}"
echo ""
echo "📝 Parent issue #${PARENT_ISSUE} updated with references"
echo ""
echo "Next steps:"
echo "  1. Review each sub-issue"
echo "  2. Assign appropriate team members"
echo "  3. Start with priority 1 (Rust Workspace)"
echo "  4. Track progress in project board"
