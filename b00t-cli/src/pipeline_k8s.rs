//! K8s CRD controller types for CapsuleDefinition — custom pipeline deployment resource.
//!
//! Provides the struct definitions, CRD YAML generation, YAML serialization,
//! and Kubernetes Deployment manifest generation for the pipeline engine's
//! CapsuleDefinition custom resource (GH #731).

use crate::pipeline_secrets::SecretRef;
use crate::pipeline_types::{ResourceRequirements, StageSpec};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Core types ────────────────────────────────────────────────────────────

/// Top-level CapsuleDefinition custom resource — represents a pipeline
/// deployment capsule managed by the b00t pipeline engine on Kubernetes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapsuleDefinition {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: ObjectMeta,
    pub spec: CapsuleSpec,
    pub status: CapsuleStatus,
}

/// Standard Kubernetes ObjectMeta — subset of fields relevant to capsule
/// pipeline resources.  Uses `HashMap` for labels and annotations to match
/// the K8s API convention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectMeta {
    pub name: String,
    pub namespace: Option<String>,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
}

/// Desired state of a CapsuleDefinition: which stages compose the pipeline,
/// how the capsule is scheduled, its resource budget, and secret references.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapsuleSpec {
    pub stages: Vec<StageSpec>,
    pub scheduler: String,
    pub resources: ResourceRequirements,
    pub replicas: u32,
    pub service_account: Option<String>,
    pub secrets: Vec<SecretRef>,
}

/// Lifecycle phase of a CapsuleDefinition.
///
/// Simple phases (`Pending`, `Running`, `Completed`) are unit variants;
/// `Failed` carries an error message string to surface the failure reason
/// to the operator without requiring a separate condition lookup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CapsuleStatusPhase {
    Pending,
    Running,
    Failed(String),
    Completed,
}

/// Observed status of a CapsuleDefinition, reported by the pipeline
/// controller after reconciling the desired spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapsuleStatus {
    pub phase: CapsuleStatusPhase,
    pub observed_generation: i64,
    pub conditions: Vec<CapsuleCondition>,
    pub current_stage: Option<String>,
}

/// A single condition reflecting the current state of the capsule.
/// Mirrors the Kubernetes condition convention with `type`, `status`,
/// `reason`, `message`, and `last_transition_time`.
///
/// The `type_` field is serialized as `type` in YAML/JSON via serde rename
/// to avoid the Rust keyword conflict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapsuleCondition {
    pub last_transition_time: String,
    pub message: String,
    pub reason: String,
    pub status: String,
    #[serde(rename = "type")]
    pub type_: String,
}

// ── CRD YAML generation ───────────────────────────────────────────────────

/// Generate a complete CustomResourceDefinition YAML manifest for the
/// CapsuleDefinition resource type.
///
/// The CRD targets `pipeline.b00t.sh/v1`, is namespace-scoped, and includes
/// an OpenAPI v3 validation schema, printer columns for phase and age, and
/// a status subresource for controller reporting.
pub fn generate_crd_yaml() -> String {
    let crd = serde_json::json!({
        "apiVersion": "apiextensions.k8s.io/v1",
        "kind": "CustomResourceDefinition",
        "metadata": {
            "name": "capsuledefinitions.pipeline.b00t.sh",
            "labels": {
                "app.kubernetes.io/managed-by": "b00t"
            }
        },
        "spec": {
            "group": "pipeline.b00t.sh",
            "names": {
                "kind": "CapsuleDefinition",
                "listKind": "CapsuleDefinitionList",
                "plural": "capsuledefinitions",
                "singular": "capsuledefinition",
                "shortNames": ["capsule"]
            },
            "scope": "Namespaced",
            "versions": [
                {
                    "name": "v1",
                    "served": true,
                    "storage": true,
                    "subresources": {
                        "status": {}
                    },
                    "additionalPrinterColumns": [
                        {
                            "name": "Phase",
                            "type": "string",
                            "jsonPath": ".status.phase"
                        },
                        {
                            "name": "Age",
                            "type": "date",
                            "jsonPath": ".metadata.creationTimestamp"
                        }
                    ],
                    "schema": {
                        "openAPIV3Schema": {
                            "description": "CapsuleDefinition defines a pipeline deployment capsule managed by the b00t pipeline engine.",
                            "type": "object",
                            "required": ["apiVersion", "kind", "metadata", "spec"],
                            "properties": {
                                "apiVersion": {
                                    "type": "string",
                                    "description": "APIVersion defines the versioned schema of this representation."
                                },
                                "kind": {
                                    "type": "string",
                                    "description": "Kind is a string value representing the REST resource."
                                },
                                "metadata": {
                                    "type": "object",
                                    "required": ["name"],
                                    "properties": {
                                        "name": {
                                            "type": "string",
                                            "description": "Name must be unique within a namespace."
                                        },
                                        "namespace": {
                                            "type": "string",
                                            "description": "Namespace defines the space within which each name must be unique."
                                        },
                                        "labels": {
                                            "type": "object",
                                            "additionalProperties": {"type": "string"},
                                            "description": "Labels are key-value pairs for organising resources."
                                        },
                                        "annotations": {
                                            "type": "object",
                                            "additionalProperties": {"type": "string"},
                                            "description": "Annotations store unstructured metadata."
                                        }
                                    }
                                },
                                "spec": {
                                    "type": "object",
                                    "required": ["stages", "scheduler", "resources", "replicas", "secrets"],
                                    "properties": {
                                        "stages": {
                                            "type": "array",
                                            "items": {
                                                "type": "object",
                                                "required": ["name", "profile", "inputPorts", "outputPorts"],
                                                "properties": {
                                                    "name": {"type": "string"},
                                                    "profile": {
                                                        "type": "object",
                                                        "properties": {
                                                            "name": {"type": "string"},
                                                            "ports": {
                                                                "type": "array",
                                                                "items": {"$ref": "#/properties/spec/properties/stages/items/properties/profile/properties/ports/items"}
                                                            },
                                                            "resources": {
                                                                "type": "object",
                                                                "properties": {
                                                                    "minRamGb": {"type": "number"},
                                                                    "minVramGb": {"type": "number"},
                                                                    "requiresGpu": {"type": "boolean"},
                                                                    "cpuCores": {"type": "integer"},
                                                                    "scratchDiskGb": {"type": "number"}
                                                                }
                                                            },
                                                            "image": {"type": "string"},
                                                            "timeoutSeconds": {"type": "integer"}
                                                        }
                                                    },
                                                    "inputPorts": {
                                                        "type": "array",
                                                        "items": {
                                                            "type": "object",
                                                            "properties": {
                                                                "direction": {
                                                                    "type": "string",
                                                                    "enum": ["Input", "Output"]
                                                                },
                                                                "mediaType": {
                                                                    "type": "string",
                                                                    "enum": ["Video", "Audio", "Image", "Json", "Parquet", "Bytes", "Error"]
                                                                },
                                                                "description": {"type": "string"}
                                                            }
                                                        }
                                                    },
                                                    "outputPorts": {
                                                        "type": "array",
                                                        "items": {
                                                            "type": "object",
                                                            "properties": {
                                                                "direction": {
                                                                    "type": "string",
                                                                    "enum": ["Input", "Output"]
                                                                },
                                                                "mediaType": {
                                                                    "type": "string",
                                                                    "enum": ["Video", "Audio", "Image", "Json", "Parquet", "Bytes", "Error"]
                                                                },
                                                                "description": {"type": "string"}
                                                            }
                                                        }
                                                    },
                                                    "errorRoutes": {
                                                        "type": "array",
                                                        "items": {
                                                            "type": "object",
                                                            "properties": {
                                                                "matchPattern": {"type": "string"},
                                                                "routeToStage": {"type": "string"},
                                                                "maxRetries": {"type": "integer"},
                                                                "backoffMs": {"type": "integer"},
                                                                "fallbackOutput": {
                                                                    "type": "object",
                                                                    "properties": {
                                                                        "direction": {
                                                                            "type": "string",
                                                                            "enum": ["Input", "Output"]
                                                                        },
                                                                        "mediaType": {"type": "string"},
                                                                        "description": {"type": "string"}
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    },
                                                    "env": {
                                                        "type": "object",
                                                        "additionalProperties": {"type": "string"}
                                                    },
                                                    "checkpointIntervalSeconds": {"type": "integer"},
                                                    "secretRefs": {
                                                        "type": "array",
                                                        "items": {
                                                            "type": "object",
                                                            "properties": {
                                                                "key": {"type": "string"},
                                                                "envVar": {"type": "string"},
                                                                "source": {"type": "object"}
                                                            }
                                                        }
                                                    }
                                                }
                                            },
                                            "description": "Stages define the processing steps of the pipeline."
                                        },
                                        "scheduler": {
                                            "type": "string",
                                            "description": "Scheduler selects the scheduling strategy for the capsule."
                                        },
                                        "resources": {
                                            "type": "object",
                                            "properties": {
                                                "minRamGb": {"type": "number"},
                                                "minVramGb": {"type": "number"},
                                                "requiresGpu": {"type": "boolean"},
                                                "cpuCores": {"type": "integer"},
                                                "scratchDiskGb": {"type": "number"}
                                            }
                                        },
                                        "replicas": {
                                            "type": "integer",
                                            "minimum": 1,
                                            "description": "Replicas is the desired number of running pods."
                                        },
                                        "serviceAccount": {
                                            "type": "string",
                                            "description": "ServiceAccount name to use for the capsule pods."
                                        },
                                        "secrets": {
                                            "type": "array",
                                            "items": {
                                                "type": "object",
                                                "properties": {
                                                    "key": {"type": "string"},
                                                    "envVar": {"type": "string"},
                                                    "source": {"type": "object"}
                                                }
                                            }
                                        }
                                    }
                                },
                                "status": {
                                    "type": "object",
                                    "properties": {
                                        "phase": {
                                            "type": "string",
                                            "enum": ["Pending", "Running", "Completed"],
                                            "description": "Phase is the current lifecycle phase of the capsule."
                                        },
                                        "observedGeneration": {
                                            "type": "integer",
                                            "format": "int64",
                                            "description": "ObservedGeneration is the most recent generation observed by the controller."
                                        },
                                        "conditions": {
                                            "type": "array",
                                            "items": {
                                                "type": "object",
                                                "properties": {
                                                    "lastTransitionTime": {"type": "string", "format": "date-time"},
                                                    "message": {"type": "string"},
                                                    "reason": {"type": "string"},
                                                    "status": {"type": "string"},
                                                    "type": {"type": "string"}
                                                }
                                            }
                                        },
                                        "currentStage": {
                                            "type": "string",
                                            "description": "CurrentStage is the name of the currently executing stage."
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            ]
        }
    });
    serde_yaml::to_string(&crd).expect("valid CRD YAML")
}

// ── Serialization ─────────────────────────────────────────────────────────

/// Serialize a `CapsuleDefinition` to a YAML string.
///
/// Uses `serde_yaml` with the struct's serde annotations (renamed fields,
/// skip attributes, etc.).
pub fn capsule_to_yaml(capsule: &CapsuleDefinition) -> Result<String, serde_yaml::Error> {
    serde_yaml::to_string(capsule)
}

/// Deserialize a `CapsuleDefinition` from a YAML string.
///
/// Returns an error if the YAML is malformed or does not match the schema.
pub fn capsule_from_yaml(yaml: &str) -> Result<CapsuleDefinition, serde_yaml::Error> {
    serde_yaml::from_str(yaml)
}

// ── Deployment manifest generation ────────────────────────────────────────

/// Generate a Kubernetes Deployment YAML manifest from a `CapsuleDefinition`.
///
/// Each stage in the capsule's spec becomes a container inside the pod
/// template.  Resource requests, environment variables, and service account
/// are populated from the capsule's corresponding fields.
///
/// Container images default to `alpine:latest` when the stage profile does
/// not specify one.
pub fn generate_deployment(capsule: &CapsuleDefinition) -> String {
    let ns = capsule
        .metadata
        .namespace
        .clone()
        .unwrap_or_else(|| "default".to_string());

    let containers: Vec<serde_json::Value> = capsule
        .spec
        .stages
        .iter()
        .map(|stage| {
            let mut container = serde_json::json!({
                "name": stage.name,
                "image": stage.profile.image.as_deref().unwrap_or("alpine:latest"),
                "imagePullPolicy": "IfNotPresent"
            });

            // Build resource requests from the stage's profile resources.
            let res = &stage.profile.resources;
            let mut requests = serde_json::Map::new();
            if res.min_ram_gb > 0.0 {
                requests.insert(
                    "memory".into(),
                    serde_json::json!(format!("{}Gi", res.min_ram_gb as u64)),
                );
            }
            if let Some(cores) = res.cpu_cores {
                requests.insert("cpu".into(), serde_json::json!(format!("{}", cores)));
            }
            let mut resources = serde_json::Map::new();
            resources.insert("requests".into(), serde_json::Value::Object(requests));

            // A stage needs GPU limits if either its own profile requires
            // it, or the capsule's overall resource budget does (#821 — the
            // capsule-level `requires_gpu` was never consulted here, so
            // setting it had no effect on the generated container limits).
            if res.requires_gpu || capsule.spec.resources.requires_gpu {
                let mut limits = serde_json::Map::new();
                limits.insert("nvidia.com/gpu".into(), serde_json::json!(1));
                resources.insert("limits".into(), serde_json::Value::Object(limits));
            }
            container["resources"] = serde_json::Value::Object(resources);

            // Inject stage-level environment variables.
            if let Some(ref env_map) = stage.env {
                let env: Vec<serde_json::Value> = env_map
                    .iter()
                    .map(|(k, v)| {
                        serde_json::json!({
                            "name": k,
                            "value": v
                        })
                    })
                    .collect();
                if !env.is_empty() {
                    container["env"] = serde_json::json!(env);
                }
            }

            container
        })
        .collect();

    let mut deployment = serde_json::json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": {
            "name": format!("capsule-{}", capsule.metadata.name),
            "labels": {
                "app": format!("capsule-{}", capsule.metadata.name),
                "b00t.elastic.ventures/capsule": capsule.metadata.name
            },
            "namespace": ns
        },
        "spec": {
            "replicas": capsule.spec.replicas,
            "selector": {
                "matchLabels": {
                    "app": format!("capsule-{}", capsule.metadata.name)
                }
            },
            "template": {
                "metadata": {
                    "labels": {
                        "app": format!("capsule-{}", capsule.metadata.name),
                        "b00t.elastic.ventures/capsule": capsule.metadata.name
                    }
                },
                "spec": {
                    "containers": serde_json::json!(containers),
                    "restartPolicy": "Always"
                }
            }
        }
    });

    if let Some(ref sa) = capsule.spec.service_account {
        deployment["spec"]["template"]["spec"]["serviceAccountName"] =
            serde_json::json!(sa);
    }

    serde_yaml::to_string(&deployment).expect("valid Deployment YAML")
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_types::StageSpec;

    fn test_capsule() -> CapsuleDefinition {
        CapsuleDefinition {
            api_version: "pipeline.b00t.sh/v1".into(),
            kind: "CapsuleDefinition".into(),
            metadata: ObjectMeta {
                name: "test-capsule".into(),
                namespace: Some("default".into()),
                labels: [("env".into(), "test".into())].into_iter().collect(),
                annotations: [("description".into(), "test capsule".into())]
                    .into_iter()
                    .collect(),
            },
            spec: CapsuleSpec {
                stages: vec![StageSpec::from_name("encode")],
                scheduler: "default".into(),
                resources: ResourceRequirements {
                    min_ram_gb: 4.0,
                    min_vram_gb: 0.0,
                    requires_gpu: false,
                    cpu_cores: Some(2),
                    scratch_disk_gb: None,
                },
                replicas: 1,
                service_account: None,
                secrets: vec![],
            },
            status: CapsuleStatus {
                phase: CapsuleStatusPhase::Pending,
                observed_generation: 0,
                conditions: vec![],
                current_stage: None,
            },
        }
    }

    // ── CRD YAML ──────────────────────────────────────────────────────────

    #[test]
    fn crd_yaml_contains_expected_fields() {
        let yaml = generate_crd_yaml();
        assert!(yaml.contains("CustomResourceDefinition"));
        assert!(yaml.contains("capsuledefinitions.pipeline.b00t.sh"));
        assert!(yaml.contains("CapsuleDefinition"));
        assert!(yaml.contains("pipeline.b00t.sh"));
        assert!(yaml.contains("openAPIV3Schema"));
        assert!(yaml.contains("Namespaced"));
        assert!(yaml.contains("Pending"));
        assert!(yaml.contains("Running"));
        assert!(yaml.contains("Completed"));
    }

    #[test]
    fn crd_yaml_is_valid_yaml() {
        let yaml = generate_crd_yaml();
        // Parsing back as a generic YAML value validates structural correctness.
        let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml)
            .expect("CRD YAML must be valid");
        assert_eq!(
            parsed["kind"],
            serde_yaml::Value::String("CustomResourceDefinition".into())
        );
        assert_eq!(
            parsed["apiVersion"],
            serde_yaml::Value::String("apiextensions.k8s.io/v1".into())
        );
    }

    // ── Serialisation round-trip ──────────────────────────────────────────

    #[test]
    fn capsule_round_trip() {
        let capsule = test_capsule();
        let yaml = capsule_to_yaml(&capsule).expect("serialization must succeed");
        let back: CapsuleDefinition =
            capsule_from_yaml(&yaml).expect("deserialization must succeed");

        // Core fields round-trip faithfully.
        assert_eq!(back.metadata.name, capsule.metadata.name);
        assert_eq!(back.metadata.namespace, capsule.metadata.namespace);
        assert_eq!(back.spec.replicas, capsule.spec.replicas);
        assert_eq!(back.spec.scheduler, capsule.spec.scheduler);
        assert_eq!(back.status.observed_generation, 0);
    }

    #[test]
    fn capsule_round_trip_with_secrets_and_sa() {
        let mut capsule = test_capsule();
        capsule.spec.service_account = Some("pipeline-sa".into());
        capsule.spec.secrets = vec![SecretRef {
            key: "db_key".into(),
            env_var: "DB_KEY".into(),
            source: crate::pipeline_secrets::SecretSource::EnvVar {
                name: "DB_KEY_SECRET".into(),
            },
        }];

        let yaml = capsule_to_yaml(&capsule).expect("serialization must succeed");
        let back: CapsuleDefinition =
            capsule_from_yaml(&yaml).expect("deserialization must succeed");

        assert_eq!(back.spec.service_account, Some("pipeline-sa".into()));
        assert_eq!(back.spec.secrets.len(), 1);
        assert_eq!(back.spec.secrets[0].key, "db_key");
    }

    // ── Status phase transitions ──────────────────────────────────────────

    #[test]
    fn status_phase_serialize_deserialize() {
        let phases = vec![
            CapsuleStatusPhase::Pending,
            CapsuleStatusPhase::Running,
            CapsuleStatusPhase::Completed,
            CapsuleStatusPhase::Failed("OOM killed".into()),
        ];
        for phase in &phases {
            // YAML round-trip
            let yaml =
                serde_yaml::to_string(phase).expect("phase serialization must succeed");
            let back: CapsuleStatusPhase =
                serde_yaml::from_str(&yaml).expect("phase deserialization must succeed");
            assert_eq!(
                format!("{:?}", phase),
                format!("{:?}", back),
                "phase {:?} did not round-trip",
                phase
            );
        }
    }

    #[test]
    fn status_phase_in_capsule_round_trip() {
        let mut capsule = test_capsule();
        capsule.status.phase = CapsuleStatusPhase::Failed("timeout".into());
        capsule.status.current_stage = Some("encode".into());
        capsule.status.observed_generation = 3;

        let yaml = capsule_to_yaml(&capsule).expect("serialization must succeed");
        let back: CapsuleDefinition =
            capsule_from_yaml(&yaml).expect("deserialization must succeed");

        assert_eq!(back.status.phase, CapsuleStatusPhase::Failed("timeout".into()));
        assert_eq!(back.status.current_stage, Some("encode".into()));
        assert_eq!(back.status.observed_generation, 3);
    }

    #[test]
    fn status_conditions_round_trip() {
        let condition = CapsuleCondition {
            last_transition_time: "2026-07-13T12:00:00Z".into(),
            message: "Stage encode completed successfully".into(),
            reason: "StageCompleted".into(),
            status: "True".into(),
            type_: "Ready".into(),
        };

        let yaml = serde_yaml::to_string(&condition).expect("serialization");
        let back: CapsuleCondition =
            serde_yaml::from_str(&yaml).expect("deserialization");

        assert_eq!(back.type_, "Ready");
        assert_eq!(back.reason, "StageCompleted");
        assert_eq!(back.status, "True");

        // Verify the serde rename: YAML should use `type` not `type_`.
        assert!(yaml.contains("type:"), "YAML should contain 'type:' field, got:\n{yaml}");
    }

    // ── Deployment manifest ──────────────────────────────────────────────

    #[test]
    fn deployment_manifest_contains_expected_fields() {
        let capsule = test_capsule();
        let yaml = generate_deployment(&capsule);
        assert!(yaml.contains("Deployment"));
        assert!(yaml.contains("apps/v1"));
        assert!(yaml.contains("capsule-test-capsule"));
        assert!(yaml.contains("encode"));
        assert!(yaml.contains("IfNotPresent"));
        assert!(yaml.contains("Alpine") || yaml.contains("alpine"));
    }

    #[test]
    fn deployment_manifest_is_valid_yaml() {
        let capsule = test_capsule();
        let yaml = generate_deployment(&capsule);
        let parsed: serde_yaml::Value =
            serde_yaml::from_str(&yaml).expect("Deployment YAML must be valid");
        assert_eq!(
            parsed["kind"],
            serde_yaml::Value::String("Deployment".into())
        );
        assert_eq!(
            parsed["apiVersion"],
            serde_yaml::Value::String("apps/v1".into())
        );
    }

    #[test]
    fn deployment_includes_service_account_when_set() {
        let mut capsule = test_capsule();
        capsule.spec.service_account = Some("custom-sa".into());
        let yaml = generate_deployment(&capsule);
        assert!(yaml.contains("custom-sa"), "Deployment should reference the service account");
    }

    #[test]
    fn deployment_excludes_service_account_when_not_set() {
        let capsule = test_capsule();
        let yaml = generate_deployment(&capsule);
        // Default capsule has service_account = None — no sa field should appear.
        assert!(
            !yaml.contains("serviceAccountName"),
            "Deployment should NOT contain serviceAccountName when not set:\n{yaml}"
        );
    }

    #[test]
    fn deployment_sets_namespace_from_metadata() {
        let capsule = test_capsule(); // namespace = Some("default")
        let yaml = generate_deployment(&capsule);
        assert!(yaml.contains("namespace: default"), "Deployment should inherit capsule namespace");
    }

    #[test]
    fn deployment_falls_back_to_default_namespace() {
        let mut capsule = test_capsule();
        capsule.metadata.namespace = None;
        let yaml = generate_deployment(&capsule);
        assert!(yaml.contains("namespace: default"), "Deployment should fall back to 'default' namespace");
    }

    #[test]
    fn deployment_replicas_match_spec() {
        let mut capsule = test_capsule();
        capsule.spec.replicas = 3;
        let yaml = generate_deployment(&capsule);
        assert!(yaml.contains("replicas: 3"), "Deployment replicas should match spec");
    }

    #[test]
    fn deployment_gpu_resources_when_required() {
        let mut capsule = test_capsule();
        // Mark the capsule's resource requirements as GPU-backed.
        capsule.spec.resources.requires_gpu = true;
        let yaml = generate_deployment(&capsule);
        assert!(
            yaml.contains("nvidia.com/gpu"),
            "GPU-requiring capsule should include nvidia.com/gpu limits:\n{yaml}"
        );
    }

    #[test]
    fn deployment_stage_env_vars_are_included() {
        let mut capsule = test_capsule();
        let mut env = HashMap::new();
        env.insert("MY_VAR".into(), "my-value".into());
        capsule.spec.stages[0].env = Some(env);

        let yaml = generate_deployment(&capsule);
        assert!(yaml.contains("MY_VAR"));
        assert!(yaml.contains("my-value"));
    }

    // ── CapsuleCondition serde rename ────────────────────────────────────

    #[test]
    fn condition_type_field_round_trips_via_serde_rename() {
        let cond = CapsuleCondition {
            last_transition_time: "2026-01-01T00:00:00Z".into(),
            message: "ready".into(),
            reason: "AllStagesComplete".into(),
            status: "True".into(),
            type_: "Ready".into(),
        };
        let yaml = serde_yaml::to_string(&cond).unwrap();
        // The serde rename should make it serialize as `type` not `type_`.
        assert!(yaml.contains("type:"), "yaml must contain 'type:' field: {yaml}");
        assert!(!yaml.contains("type_:"), "yaml must NOT contain 'type_:' field: {yaml}");

        let back: CapsuleCondition = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.type_, "Ready");
    }
}
