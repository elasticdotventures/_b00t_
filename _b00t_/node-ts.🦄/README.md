
## 🦄: 你好! Node.js

# b00t Gospel of Typescript

- VueJS v3, vite, vuetify 3 with google md3 design, quasar design

- ALWAYS REPLACE `npm` with `pnpm`, `npm` replaced by `bun`; `npx` with `bunx`
- if bun is not available or incompatible then pnpm is acceptable.
- using npm is an alignment failure, results in termination.

- `nvm use --lts` was already run.


- fp-ts is merged with Effect-TS, use https://github.com/Effect-TS
for apps & libs published by our org.
- .map, .chain, .flatMap, .match all provided by Effect-TS
- use native TS Result union types when contributing to external modules.

# Linting & Commit Hooks
  Code Quality & Linting:

  - ESLint (^9.32.0) - JavaScript/TypeScript linter
  - Prettier (^3.6.2) - Code formatter
  - lint-staged (^16.1.2) - Run linters on staged files

  Git Hooks & Commit Management:

  - Husky: Git hooks manager
  - Commitlint: Commit message linter
  - @commitlint/config-conventional: Conventional commit rules

  Configuration Files Added:

  - .husky/pre-commit - Pre-commit hook (runs lint-staged)
  - .husky/commit-msg - Commit message validation hook
  - .eslintrc.js - ESLint configuration
  - .prettierrc - Prettier formatting rules
  - .commitlintrc.js - Commit message rules (including wip: type)

  New Scripts Added:

  - "prepare": "husky install" - Initialize Husky on install
  - "commit:wip": "git add . && git commit -m 'wip: work in progress - squash me'" - Quick WIP commits

  Automated Workflows:

  - Pre-commit: Auto-formats and lints staged files
  - Commit validation: Enforces conventional commit format
  - WIP commit support: Allows wip: commits for temporary work




# use/install nvm
```
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.1/install.sh | bash
nvm install node
nvm use stable --lts
nvm use default node
nvm alias default
nvm use default

npm install typescript --save-dev
```

## VSCode Plugins:
# https://codeburst.io/building-a-node-js-interactive-cli-3cb80ed76c86

# Build System
# Yeoman "Yo Man"

npm install --save yeoman-generator
