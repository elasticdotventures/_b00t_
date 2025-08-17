# 🥾 b00t-c0re-lib

Core library for the b00t ecosystem providing unified configuration management, AI coordination, and secret validation services.

## 🎯 Overview

This library serves as the foundational layer for b00t's unified datum schema system, providing:

- **Unified Configuration Management**: Single source of truth for all b00t ecosystem configuration
- **TypeScript-Rust Bridge**: Automatic generation of TypeScript types from Rust structs
- **Secret Validation**: Comprehensive cloud service credential validation
- **AI Coordination**: LLM provider management and orchestration
- **Template Engine**: Dynamic configuration templating with Rhai scripting

## 🔄 Unified Datum Schema Architecture

The core innovation of b00t-c0re-lib is the **unified datum schema system** that eliminates duplication between Rust backend and TypeScript frontend by maintaining a single source of truth.

### How It Works

```
┌─────────────────┐    ts-rs     ┌─────────────────┐    Import    ┌─────────────────┐
│  Rust Structs   │ ────────────▶│ TypeScript Types │ ────────────▶│   Vue Frontend  │
│  (Source Truth) │              │   (.ts files)   │              │   (Dashboard)   │
└─────────────────┘              └─────────────────┘              └─────────────────┘
        │                                   │                             │
        │ schemars                         │ JSON Schema                  │ Validation
        ▼                                   ▼                             ▼
┌─────────────────┐              ┌─────────────────┐              ┌─────────────────┐
│  JSON Schemas   │              │   Type Guards   │              │  Form Controls  │
│  (.json files)  │              │   & Utilities   │              │  & Components   │
└─────────────────┘              └─────────────────┘              └─────────────────┘
```

### Key Components

#### 🏗️ B00tUnifiedConfig - The Center of Truth

Located in `src/b00t_config.rs`, this is the master configuration structure:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export, export_to = "/path/to/dashboard/src/types/")]
pub struct B00tUnifiedConfig {
    pub user: UserConfig,           // User preferences & identity
    pub cloud: CloudServicesConfig, // Cloud provider configurations 
    pub ai: AiConfiguration,        // AI/LLM provider settings
    pub development: DevelopmentConfig, // Dev environment preferences
    pub security: SecurityConfig,   // Security & keyring settings
    pub metadata: ConfigMetadata,   // Versioning & audit trail
}
```

#### 🔐 Secret Validation System

`src/secret_validation.rs` provides comprehensive credential validation for:

- **Cloudflare**: Account tokens, zone validation, worker access
- **AWS**: Access keys, regions, service permissions  
- **Qdrant**: Vector database connections, collection access
- **AI Providers**: OpenAI, Anthropic, Google, Azure API keys
- **DNS Services**: Cloudflare DNS, Route53 validation

#### 🔄 Schema Generation Pipeline

The `src/bin/generate_schemas.rs` binary automatically generates:

1. **TypeScript Types** via `ts-rs` for type-safe frontend development
2. **JSON Schemas** via `schemars` for runtime validation
3. **Index Files** with type guards and utility functions

## 🚀 Usage

### Generate TypeScript Types & Schemas

```bash
just generate-schemas
```

This exports types to the dashboard at:
- `dashboard/src/types/*.ts` - TypeScript type definitions
- `dashboard/src/schemas/*.json` - JSON validation schemas

### Run Tests

```bash
# Test configuration types
just test-config

# Test secret validation
just test-secrets  

# Run all tests
just test
```

### Development Workflow

```bash
# Full development cycle
just dev

# Release preparation  
just release
```

## 🔗 Integration with Dashboard

The dashboard project consumes the generated types:

```typescript
import type { 
  B00tUnifiedConfig, 
  UserConfig, 
  AiConfiguration 
} from '@/types'

// Type-safe configuration object
const config = ref<B00tUnifiedConfig>({
  user: { username: "operator", email: "op@b00t.dev" },
  cloud: { /* cloud config */ },
  ai: { /* ai config */ }
})
```

See the **ConfigTest.vue** page for a comprehensive example of type usage.

## 🏗️ Architecture Principles

### DRY (Don't Repeat Yourself)
- **Single Source**: All configuration schemas defined once in Rust
- **Auto-Generation**: TypeScript types generated automatically  
- **Consistent APIs**: Shared validation and serialization logic

### Type Safety
- **Compile-Time Checks**: Rust compiler enforces schema validity
- **Runtime Validation**: JSON schemas validate user input
- **Editor Support**: Full IntelliSense and type checking in Vue

### Extensibility  
- **Plugin Architecture**: Easy addition of new cloud services
- **Schema Versioning**: Migration support for configuration updates
- **Custom Validation**: Service-specific credential validation

## 📦 Dependencies

### Core Libraries
- **serde**: Serialization/deserialization framework
- **ts-rs**: TypeScript type generation from Rust
- **schemars**: JSON Schema generation
- **reqwest**: HTTP client for secret validation
- **tokio**: Async runtime for validation operations

### Optional Features
- **keyring**: Secure credential storage
- **rhai**: Dynamic templating engine
- **redis**: Distributed coordination (when available)

## 🔧 Configuration

Environment variables for secret validation:
- `CLOUDFLARE_API_TOKEN` - For testing Cloudflare validation
- `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` - For AWS validation  
- `QDRANT_URL` / `QDRANT_API_KEY` - For vector database validation

## 📋 Entanglement with Dashboard

This library is **entangled** 🪢 with the [b00t Dashboard](../../../promptexecution/infrastructure/b00t/dashboard/) project:

- Dashboard consumes generated TypeScript types from this library
- Schema updates here automatically propagate to frontend
- Secret validation methods support dashboard credential testing
- Unified configuration ensures consistency across the ecosystem

## 🤝 Contributing

1. Modify Rust structs in `src/b00t_config.rs` or related modules
2. Run `just generate-schemas` to update TypeScript types
3. Test changes with `just dev`  
4. Dashboard automatically receives updated types on next build

## 📜 License

Licensed under the same terms as the b00t ecosystem.