---
name: b00t-datum-system
description: |
  Work with b00t datum system - TOML-based configuration for AI models,
  providers, and services. Datums are versioned configurations 
  that specify WHICH environment variables are required.
version: 1.0.0
allowed-tools: Read, Write, Edit, Grep, Glob, Bash
---

## What This Skill Does

Manage b00t datums (*.datum files) for configuration:

- Query available models and providers
- Validate required environment variables  
- Load and apply datum configurations
- Create new datum files

## When It Activates

- "create a datum for X"
- "add model to b00t"
- "check which environment variables are needed"
- "list available models"
- "validate provider configuration"

## Datum Files

Located in `datums/` directory:

- `*.datum` - General configuration
- `*.ai_model datum` - Model configurations

## Key Commands

```bash
b00t advice <error>     # Get advice for error patterns
b00t lfmf <lesson>     # Learn from mistakes
b00t Ontology query   # Query capability ontology
```

## Version

1.0.0