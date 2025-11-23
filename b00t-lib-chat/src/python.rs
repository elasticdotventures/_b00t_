//! Python bindings for b00t-acp using PyO3

#[cfg(feature = "python")]
use pyo3::prelude::*;
#[cfg(feature = "python")]
use pyo3::types::{PyDict, PyString};
#[cfg(feature = "python")]
use crate::{Agent, AgentConfig, ACPMessage, MessageType, JsonValue};
#[cfg(feature = "python")]
use std::collections::HashMap;

#[cfg(feature = "python")]
#[pyclass]
#[derive(Clone)]
pub struct PyAgentConfig {
    inner: AgentConfig,
}

#[cfg(feature = "python")]
#[pymethods]
impl PyAgentConfig {
    #[new]
    fn new(agent_id: String, nats_url: String, namespace: String) -> Self {
        Self {
            inner: AgentConfig::new(agent_id, nats_url, namespace),
        }
    }

    fn with_jwt(&mut self, jwt_token: String) -> PyResult<()> {
        self.inner = self.inner.clone().with_jwt(jwt_token);
        Ok(())
    }

    fn with_role(&mut self, role: String) -> PyResult<()> {
        self.inner = self.inner.clone().with_role(role);
        Ok(())
    }

    fn with_timeout(&mut self, timeout_ms: u64) -> PyResult<()> {
        self.inner = self.inner.clone().with_timeout(timeout_ms);
        Ok(())
    }

    #[getter]
    fn agent_id(&self) -> String {
        self.inner.agent_id.clone()
    }

    #[getter]
    fn nats_url(&self) -> String {
        self.inner.nats_url.clone()
    }

    #[getter]
    fn namespace(&self) -> String {
        self.inner.namespace.clone()
    }

    #[getter]
    fn role(&self) -> String {
        self.inner.role.clone()
    }

    #[getter]
    fn timeout_ms(&self) -> u64 {
        self.inner.timeout_ms
    }
}

#[cfg(feature = "python")]
#[pyclass]
pub struct PyAgent {
    inner: Option<Agent>,
    runtime: tokio::runtime::Runtime,
}

#[cfg(feature = "python")]
#[pymethods]
impl PyAgent {
    #[new]
    fn new(config: &PyAgentConfig) -> PyResult<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;
        
        Ok(Self {
            inner: None,
            runtime,
        })
    }

    fn connect(&mut self, config: &PyAgentConfig) -> PyResult<()> {
        let agent = self.runtime.block_on(async {
            Agent::new(config.inner.clone()).await
        }).map_err(|e| pyo3::exceptions::PyConnectionError::new_err(format!("Connection failed: {}", e)))?;

        self.inner = Some(agent);
        Ok(())
    }

    fn start(&self) -> PyResult<()> {
        if let Some(agent) = &self.inner {
            self.runtime.block_on(async {
                agent.start().await
            }).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Start failed: {}", e)))?;
        } else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err("Agent not connected"));
        }
        Ok(())
    }

    fn stop(&self) -> PyResult<()> {
        if let Some(agent) = &self.inner {
            self.runtime.block_on(async {
                agent.stop().await
            }).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Stop failed: {}", e)))?;
        }
        Ok(())
    }

    fn send_status(&self, description: &str, payload: &PyDict) -> PyResult<()> {
        if let Some(agent) = &self.inner {
            let json_payload = python_dict_to_json(payload)?;
            
            self.runtime.block_on(async {
                agent.send_status(description, json_payload).await
            }).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Send status failed: {}", e)))?;
        } else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err("Agent not connected"));
        }
        Ok(())
    }

    fn send_propose(&self, action: &str, payload: &PyDict) -> PyResult<()> {
        if let Some(agent) = &self.inner {
            let json_payload = python_dict_to_json(payload)?;
            
            self.runtime.block_on(async {
                agent.send_propose(action, json_payload).await
            }).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Send propose failed: {}", e)))?;
        } else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err("Agent not connected"));
        }
        Ok(())
    }

    fn complete_step(&self) -> PyResult<()> {
        if let Some(agent) = &self.inner {
            self.runtime.block_on(async {
                agent.complete_step().await
            }).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Complete step failed: {}", e)))?;
        } else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err("Agent not connected"));
        }
        Ok(())
    }

    fn wait_for_step_complete(&self, step: u64) -> PyResult<()> {
        if let Some(agent) = &self.inner {
            self.runtime.block_on(async {
                agent.wait_for_step_complete(step).await
            }).map_err(|e| pyo3::exceptions::PyTimeoutError::new_err(format!("Step wait failed: {}", e)))?;
        } else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err("Agent not connected"));
        }
        Ok(())
    }

    fn current_step(&self) -> PyResult<u64> {
        if let Some(agent) = &self.inner {
            Ok(self.runtime.block_on(async {
                agent.current_step().await
            }))
        } else {
            Err(pyo3::exceptions::PyRuntimeError::new_err("Agent not connected"))
        }
    }

    fn add_agent(&self, agent_id: String) -> PyResult<()> {
        if let Some(agent) = &self.inner {
            self.runtime.block_on(async {
                agent.add_agent(agent_id).await
            }).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Add agent failed: {}", e)))?;
        } else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err("Agent not connected"));
        }
        Ok(())
    }

    fn remove_agent(&self, agent_id: String) -> PyResult<()> {
        if let Some(agent) = &self.inner {
            self.runtime.block_on(async {
                agent.remove_agent(&agent_id).await
            }).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Remove agent failed: {}", e)))?;
        } else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err("Agent not connected"));
        }
        Ok(())
    }

    fn known_agents(&self) -> PyResult<Vec<String>> {
        if let Some(agent) = &self.inner {
            Ok(self.runtime.block_on(async {
                agent.known_agents().await
            }))
        } else {
            Err(pyo3::exceptions::PyRuntimeError::new_err("Agent not connected"))
        }
    }

    fn is_connected(&self) -> PyResult<bool> {
        if let Some(agent) = &self.inner {
            Ok(self.runtime.block_on(async {
                agent.is_connected().await
            }))
        } else {
            Ok(false)
        }
    }
}

#[cfg(feature = "python")]
#[pyclass]
#[derive(Clone)]
pub struct PyACPMessage {
    inner: ACPMessage,
}

#[cfg(feature = "python")]
#[pymethods]
impl PyACPMessage {
    #[new]
    fn new(agent_id: String, step: u64, message_type: &str, payload: &PyDict) -> PyResult<Self> {
        let msg_type = match message_type.to_lowercase().as_str() {
            "status" => MessageType::Status,
            "propose" => MessageType::Propose,
            "step" => MessageType::Step,
            _ => return Err(pyo3::exceptions::PyValueError::new_err("Invalid message type")),
        };

        let json_payload = python_dict_to_json(payload)?;

        let inner = match msg_type {
            MessageType::Status => ACPMessage::status(agent_id, step, json_payload),
            MessageType::Propose => ACPMessage::propose(agent_id, step, json_payload),
            MessageType::Step => ACPMessage::step_complete(agent_id, step),
        };

        Ok(Self { inner })
    }

    #[getter]
    fn step(&self) -> u64 {
        self.inner.step
    }

    #[getter]
    fn agent_id(&self) -> String {
        self.inner.agent_id.clone()
    }

    #[getter]
    fn message_type(&self) -> String {
        format!("{:?}", self.inner.message_type).to_uppercase()
    }

    #[getter]
    fn payload(&self, py: Python) -> PyResult<PyObject> {
        json_to_python(py, &self.inner.payload)
    }

    #[getter]
    fn timestamp(&self) -> String {
        self.inner.timestamp.to_rfc3339()
    }

    fn subject(&self) -> String {
        self.inner.subject()
    }
}

#[cfg(feature = "python")]
fn python_dict_to_json(dict: &PyDict) -> PyResult<JsonValue> {
    let json_str = dict.to_string();
    serde_json::from_str(&json_str)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("JSON conversion failed: {}", e)))
}

#[cfg(feature = "python")]
fn json_to_python(py: Python, value: &JsonValue) -> PyResult<PyObject> {
    match value {
        JsonValue::Null => Ok(py.None()),
        JsonValue::Bool(b) => Ok(b.to_object(py)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.to_object(py))
            } else if let Some(f) = n.as_f64() {
                Ok(f.to_object(py))
            } else {
                Ok(n.to_string().to_object(py))
            }
        }
        JsonValue::String(s) => Ok(s.to_object(py)),
        JsonValue::Array(arr) => {
            let py_list = pyo3::types::PyList::empty(py);
            for item in arr {
                py_list.append(json_to_python(py, item)?)?;
            }
            Ok(py_list.to_object(py))
        }
        JsonValue::Object(obj) => {
            let py_dict = PyDict::new(py);
            for (key, value) in obj {
                py_dict.set_item(key, json_to_python(py, value)?)?;
            }
            Ok(py_dict.to_object(py))
        }
    }
}

#[cfg(feature = "python")]
#[pymodule]
fn b00t_acp(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyAgentConfig>()?;
    m.add_class::<PyAgent>()?;
    m.add_class::<PyACPMessage>()?;
    
    // Add version info
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    
    // Add message type constants
    m.add("STATUS", "STATUS")?;
    m.add("PROPOSE", "PROPOSE")?;
    m.add("STEP", "STEP")?;
    
    Ok(())
}