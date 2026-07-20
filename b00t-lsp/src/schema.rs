//! Hand-authored JSON Schema for the `[b00t]` datum stanza.
//!
//! 🤓 deriving schemars::JsonSchema on the real BootDatum (b00t-cli/src/lib.rs:595)
//! would drag ~20 nested config structs into the derive graph; this is the honest
//! 80%: the fields agents and taplo actually touch. The `type` enum is generated
//! from the same constants the diagnostics use, so the two cannot drift.

use crate::analysis::{KNOWN_CONTENT_TAGS, VALID_TYPE_TOKENS};
use serde_json::{Value, json};

/// Build the b00t datum JSON schema (draft-07, taplo-compatible).
pub fn datum_schema() -> Value {
    let type_enum: Vec<&str> = VALID_TYPE_TOKENS
        .iter()
        .chain(KNOWN_CONTENT_TAGS.iter())
        .copied()
        .collect();
    let string_array = json!({ "type": "array", "items": { "type": "string" } });

    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": "https://b00t.promptexecution.com/schemas/b00t-datum.schema.json",
        "title": "b00t datum",
        "description": "b00t datum dialect (.toml/.tomllm/.tomllmd): [b00t] stanza + free-form sections",
        "type": "object",
        "properties": {
            "b00t": {
                "type": "object",
                "description": "Core datum stanza (BootDatum, b00t-cli/src/lib.rs)",
                "properties": {
                    "name": { "type": "string", "description": "Datum name" },
                    "type": {
                        "type": "string",
                        "description": "Datum type token or content tag",
                        "enum": type_enum
                    },
                    "hint": { "type": "string", "description": "One-line human/agent hint" },
                    "desires": { "type": "string" },
                    "status": { "type": "string" },
                    "status_msg": { "type": "string" },
                    "enabled": { "type": "boolean" },
                    "auto_install": { "type": "boolean" },
                    "requires_sudo": { "type": "boolean" },
                    "version": { "type": "string" },
                    "version_regex": { "type": "string" },
                    "update": { "type": "string" },
                    "command": { "type": "string" },
                    "args": string_array,
                    "script": { "type": "string" },
                    "image": { "type": "string" },
                    "docker_args": string_array,
                    "oci_uri": { "type": "string" },
                    "url": { "type": "string" },
                    "branch": { "type": "string" },
                    "clone_path": { "type": "string" },
                    "namespace": { "type": "string" },
                    "chart_path": { "type": "string" },
                    "values_file": { "type": "string" },
                    "package_name": { "type": "string" },
                    "keywords": string_array,
                    "aliases": string_array,
                    "skills": string_array,
                    "compliance": string_array,
                    "require": string_array,
                    "depends_on": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Datum names this datum depends on"
                    },
                    "composes_with": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Datum names this datum composes with"
                    },
                    "members": string_array,
                    "env": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    },
                    "schema": {
                        "type": "object",
                        "description": "[b00t.schema] header (TomllmDoc, b00t-datum-core)",
                        "properties": {
                            "version": { "type": "string" },
                            "type": { "type": "string" },
                            "type_tags": string_array
                        },
                        "additionalProperties": true
                    }
                },
                "additionalProperties": true
            }
        },
        "additionalProperties": true
    })
}
