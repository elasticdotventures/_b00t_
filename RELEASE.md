# Release Process

This document outlines the release process for the _b00t_ dotfiles framework using conventional commits and cocogitto.

## 📋 Prerequisites

- [cocogitto](https://github.com/cocogitto/cocogitto) installed
- Conventional commits following [spec](https://www.conventionalcommits.org/)
- All changes committed and pushed to feature branch

## 🔄 Release Workflow

### 1. Validate Commits
```bash
just cog validate
```
Ensures all commits follow conventional commit format.

**Note**: This repository has 325+ non-compliant commits from before adopting conventional commits. Only new commits (post v1.2.0) need to follow the conventional format. The validation will show errors for historical commits, but this is expected.

### 2. Preview Changes
```bash
just cog changelog
```
Shows what the next version and changelog will look like based on commit history.

### 3. Automated Release
```bash
just cog release
```
This command:
- Analyzes commit history 
- Bumps version automatically (major/minor/patch)
- Generates/updates CHANGELOG.md
- Creates git tag
- Pushes tags to remote

### 4. Manual Version Bump (if needed)
```bash
just cog bump major    # Breaking changes
just cog bump minor    # New features  
just cog bump patch    # Bug fixes
```

## 📝 Commit Types

Based on [Angular commit convention](https://github.com/angular/angular/blob/main/CONTRIBUTING.md#commit):

- `feat`: New feature (minor version bump)
- `fix`: Bug fix (patch version bump)  
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding/updating tests
- `chore`: Maintenance tasks
- `perf`: Performance improvements
- `ci`: CI/CD changes
- `build`: Build system changes

**Breaking changes**: Add `!` after type or include `BREAKING CHANGE:` in footer for major version bump.

## 🎯 Current Process vs Cocogitto

### Manual Process (v1.2.0)
```bash
# What we did for v1.2.0
git add -A
git commit -m "feat: implement agent-aware tokenomics..."
git tag v1.2.0
git push origin issue/39 --tags
gh release create v1.2.0 --title "..." --notes "..."
```

### Cocogitto Process (future releases)
```bash
# What we should do going forward
git add -A
git commit -m "feat: implement agent-aware tokenomics..."
just cog release  # Handles tagging, changelog, and pushing
```

## 🏷️ Version Strategy

- **Major** (x.0.0): Breaking changes, API changes
- **Minor** (x.y.0): New features, backward compatible
- **Patch** (x.y.z): Bug fixes, minor improvements

## 🔧 Configuration

Cocogitto configuration is managed via:
- `cog.toml` (if exists) - Main configuration
- `cog.just` - Available commands via `just cog <command>`

Current available commands:
- `just cog validate` - Check conventional commits
- `just cog changelog` - Preview changelog
- `just cog release` - Automated release
- `just cog bump <version>` - Manual version bump

## 📚 References

- [Conventional Commits](https://www.conventionalcommits.org/)
- [Cocogitto Documentation](https://docs.cocogitto.io/)
- [Semantic Versioning](https://semver.org/)