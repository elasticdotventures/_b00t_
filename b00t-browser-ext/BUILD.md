# b00t Browser Extension - Build & Release

## Local Development

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Build for production
npm run build

# Package for distribution
npm run package
```

## CI/CD Pipeline

### Automatic Builds
- **Triggers**: Push to `main` branch with changes in `b00t-browser-ext/`
- **Output**: GitHub Actions artifacts with versioned packages
- **Retention**: 90 days for build artifacts

### Release Process
1. **Automatic on main**: Creates build artifacts for testing
2. **Manual release**: Create GitHub release tag to publish packages

### Build Artifacts

The GitHub Action creates:

```
artifacts/
├── b00t-browser-ext-chrome-v0.1.0.zip     # Chrome extension package
├── b00t-browser-ext-firefox-v0.1.0.zip    # Firefox extension (if available)  
└── b00t-browser-ext-chrome-v0.1.0/        # Unpacked build directory
    ├── manifest.json
    ├── content.js
    ├── popup.html
    ├── popup.js
    ├── background.js
    └── icons/
```

### Version Management

Version is read from `package.json` and used for:
- Artifact naming
- GitHub release titles
- Extension metadata

### Installation Testing

**Chrome/Edge/Brave:**
```bash
# Download from GitHub Actions artifacts
unzip b00t-browser-ext-chrome-v0.1.0.zip
# Load unpacked in chrome://extensions/
```

**Firefox:**
```bash  
# Download firefox package (when available)
# Install via about:addons
```

### Manual Release

To create a public release:
```bash
git tag v0.1.0
git push origin v0.1.0
```

This triggers the release workflow and publishes packages to GitHub Releases.

## Build Environment

- **Node.js**: 20.x
- **Package Manager**: npm
- **Build Tool**: Plasmo Framework
- **Target**: Chrome MV3, Firefox (planned)
- **Permissions**: Verified for MV3 compatibility

## Development Workflow

1. **Local development**: `npm run dev` with hot reload
2. **Test in browser**: Load unpacked extension
3. **Commit changes**: Triggers automatic CI build  
4. **Download artifacts**: Test packaged version
5. **Create release**: Tag for public distribution

---

🥾 **b00t ecosystem integration** - All builds are versioned and tracked through the CI/CD pipeline.