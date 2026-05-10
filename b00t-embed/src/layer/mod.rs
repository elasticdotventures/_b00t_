pub mod agent;
pub mod bouncer;
pub mod router;
pub mod source;
pub mod stack;
pub mod store;
pub mod trait_def;

pub use bouncer::*;
pub use source::*;
pub use stack::*;
pub use store::*;
pub use trait_def::*;

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a swappable embedding layer
/// Analogous to an OCI container layer digest
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayerId(String);

impl LayerId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for LayerId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for LayerId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&LayerId> for LayerId {
    fn from(id: &LayerId) -> Self {
        id.clone()
    }
}

impl From<std::borrow::Cow<'_, str>> for LayerId {
    fn from(c: std::borrow::Cow<'_, str>) -> Self {
        Self(c.into_owned())
    }
}

/// Shape and dtype specification for a single tensor in a layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorSpec {
    pub name: String,
    pub shape: Vec<usize>,
    pub dtype: &'static str,
}

impl TensorSpec {
    pub fn new(name: impl Into<String>, shape: Vec<usize>, dtype: &'static str) -> Self {
        Self {
            name: name.into(),
            shape,
            dtype,
        }
    }
}

/// Current lifecycle status of a loaded layer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerStatus {
    /// Registered but not yet loaded (OCI: manifest known, layers not pulled)
    Registered,
    /// Tensors are being loaded from source (OCI: layer download in progress)
    Loading,
    /// Tensors are active in the model (OCI: layer mounted)
    Active,
    /// Tensors are swapped out (OCI: layer unmounted but cached)
    Inactive,
    /// Layer encountered an error (OCI: layer corrupted)
    Error(String),
}

impl fmt::Display for LayerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayerStatus::Registered => write!(f, "registered"),
            LayerStatus::Loading => write!(f, "loading"),
            LayerStatus::Active => write!(f, "active"),
            LayerStatus::Inactive => write!(f, "inactive"),
            LayerStatus::Error(e) => write!(f, "error: {e}"),
        }
    }
}

/// Descriptor returned by the layer store for each registered layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerDescriptor {
    pub id: LayerId,
    pub status: LayerStatus,
    pub embedding_dim: usize,
    pub tensor_count: usize,
    pub source_kind: &'static str,
    pub model_architecture: &'static str,
    pub relevance_score: f32,
}

/// Errors originating from the layer lifecycle
#[derive(Debug, thiserror::Error)]
pub enum LayerError {
    #[error("layer {id} not found in registry")]
    NotFound { id: LayerId },
    #[error("layer {id} already active")]
    AlreadyActive { id: LayerId },
    #[error("layer {id} not active")]
    NotActive { id: LayerId },
    #[error("layer {id} source error: {detail}")]
    SourceError { id: LayerId, detail: String },
    #[error("tensor shape mismatch in layer {id}: {detail}")]
    ShapeMismatch { id: LayerId, detail: String },
    #[error("bouncer gate rejected: {gate}: {reason}")]
    GateRejected { gate: String, reason: String },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl LayerError {
    pub fn not_found(id: impl Into<LayerId>) -> Self {
        Self::NotFound { id: id.into() }
    }

    pub fn already_active(id: impl Into<LayerId>) -> Self {
        Self::AlreadyActive { id: id.into() }
    }

    pub fn not_active(id: impl Into<LayerId>) -> Self {
        Self::NotActive { id: id.into() }
    }

    pub fn source_error(id: impl Into<LayerId>, detail: impl Into<String>) -> Self {
        Self::SourceError {
            id: id.into(),
            detail: detail.into(),
        }
    }

    pub fn shape_mismatch(id: impl Into<LayerId>, detail: impl Into<String>) -> Self {
        Self::ShapeMismatch {
            id: id.into(),
            detail: detail.into(),
        }
    }

    pub fn gate_rejected(gate: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::GateRejected {
            gate: gate.into(),
            reason: reason.into(),
        }
    }
}
