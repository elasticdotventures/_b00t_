//! WebAssembly/JavaScript bindings for b00t-acp

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;
#[cfg(feature = "wasm")]
use js_sys::{Object, Reflect, Array, JSON, Promise};
#[cfg(feature = "wasm")]
use web_sys::console;
#[cfg(feature = "wasm")]
use serde_wasm_bindgen::{to_value, from_value};
#[cfg(feature = "wasm")]
use crate::{Agent, AgentConfig, ACPMessage, MessageType, JsonValue};
#[cfg(feature = "wasm")]
use std::collections::HashMap;

#[cfg(feature = "wasm")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[cfg(feature = "wasm")]
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct JsAgentConfig {
    inner: AgentConfig,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl JsAgentConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(agent_id: String, nats_url: String, namespace: String) -> JsAgentConfig {
        Self {
            inner: AgentConfig::new(agent_id, nats_url, namespace),
        }
    }

    #[wasm_bindgen(js_name = withJwt)]
    pub fn with_jwt(&mut self, jwt_token: String) {
        self.inner = self.inner.clone().with_jwt(jwt_token);
    }

    #[wasm_bindgen(js_name = withRole)]
    pub fn with_role(&mut self, role: String) {
        self.inner = self.inner.clone().with_role(role);
    }

    #[wasm_bindgen(js_name = withTimeout)]
    pub fn with_timeout(&mut self, timeout_ms: u64) {
        self.inner = self.inner.clone().with_timeout(timeout_ms);
    }

    #[wasm_bindgen(getter, js_name = agentId)]
    pub fn agent_id(&self) -> String {
        self.inner.agent_id.clone()
    }

    #[wasm_bindgen(getter, js_name = natsUrl)]
    pub fn nats_url(&self) -> String {
        self.inner.nats_url.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn namespace(&self) -> String {
        self.inner.namespace.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn role(&self) -> String {
        self.inner.role.clone()
    }

    #[wasm_bindgen(getter, js_name = timeoutMs)]
    pub fn timeout_ms(&self) -> u64 {
        self.inner.timeout_ms
    }
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct JsAgent {
    inner: Option<Agent>,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl JsAgent {
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsAgent {
        Self { inner: None }
    }

    #[wasm_bindgen]
    pub async fn connect(&mut self, config: &JsAgentConfig) -> Result<(), JsValue> {
        let agent = Agent::new(config.inner.clone())
            .await
            .map_err(|e| JsValue::from_str(&format!("Connection failed: {}", e)))?;

        self.inner = Some(agent);
        Ok(())
    }

    #[wasm_bindgen]
    pub async fn start(&self) -> Result<(), JsValue> {
        if let Some(agent) = &self.inner {
            agent.start()
                .await
                .map_err(|e| JsValue::from_str(&format!("Start failed: {}", e)))?;
        } else {
            return Err(JsValue::from_str("Agent not connected"));
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub async fn stop(&self) -> Result<(), JsValue> {
        if let Some(agent) = &self.inner {
            agent.stop()
                .await
                .map_err(|e| JsValue::from_str(&format!("Stop failed: {}", e)))?;
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = sendStatus)]
    pub async fn send_status(&self, description: &str, payload: JsValue) -> Result<(), JsValue> {
        if let Some(agent) = &self.inner {
            let json_payload: JsonValue = from_value(payload)
                .map_err(|e| JsValue::from_str(&format!("Payload conversion failed: {}", e)))?;
            
            agent.send_status(description, json_payload)
                .await
                .map_err(|e| JsValue::from_str(&format!("Send status failed: {}", e)))?;
        } else {
            return Err(JsValue::from_str("Agent not connected"));
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = sendPropose)]
    pub async fn send_propose(&self, action: &str, payload: JsValue) -> Result<(), JsValue> {
        if let Some(agent) = &self.inner {
            let json_payload: JsonValue = from_value(payload)
                .map_err(|e| JsValue::from_str(&format!("Payload conversion failed: {}", e)))?;
            
            agent.send_propose(action, json_payload)
                .await
                .map_err(|e| JsValue::from_str(&format!("Send propose failed: {}", e)))?;
        } else {
            return Err(JsValue::from_str("Agent not connected"));
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = completeStep)]
    pub async fn complete_step(&self) -> Result<(), JsValue> {
        if let Some(agent) = &self.inner {
            agent.complete_step()
                .await
                .map_err(|e| JsValue::from_str(&format!("Complete step failed: {}", e)))?;
        } else {
            return Err(JsValue::from_str("Agent not connected"));
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = waitForStepComplete)]
    pub async fn wait_for_step_complete(&self, step: u64) -> Result<(), JsValue> {
        if let Some(agent) = &self.inner {
            agent.wait_for_step_complete(step)
                .await
                .map_err(|e| JsValue::from_str(&format!("Step wait failed: {}", e)))?;
        } else {
            return Err(JsValue::from_str("Agent not connected"));
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = currentStep)]
    pub async fn current_step(&self) -> Result<u64, JsValue> {
        if let Some(agent) = &self.inner {
            Ok(agent.current_step().await)
        } else {
            Err(JsValue::from_str("Agent not connected"))
        }
    }

    #[wasm_bindgen(js_name = addAgent)]
    pub async fn add_agent(&self, agent_id: String) -> Result<(), JsValue> {
        if let Some(agent) = &self.inner {
            agent.add_agent(agent_id)
                .await
                .map_err(|e| JsValue::from_str(&format!("Add agent failed: {}", e)))?;
        } else {
            return Err(JsValue::from_str("Agent not connected"));
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = removeAgent)]
    pub async fn remove_agent(&self, agent_id: String) -> Result<(), JsValue> {
        if let Some(agent) = &self.inner {
            agent.remove_agent(&agent_id)
                .await
                .map_err(|e| JsValue::from_str(&format!("Remove agent failed: {}", e)))?;
        } else {
            return Err(JsValue::from_str("Agent not connected"));
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = knownAgents)]
    pub async fn known_agents(&self) -> Result<Array, JsValue> {
        if let Some(agent) = &self.inner {
            let agents = agent.known_agents().await;
            let js_array = Array::new();
            for agent_id in agents {
                js_array.push(&JsValue::from_str(&agent_id));
            }
            Ok(js_array)
        } else {
            Err(JsValue::from_str("Agent not connected"))
        }
    }

    #[wasm_bindgen(js_name = isConnected)]
    pub async fn is_connected(&self) -> bool {
        if let Some(agent) = &self.inner {
            agent.is_connected().await
        } else {
            false
        }
    }
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct JsACPMessage {
    inner: ACPMessage,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl JsACPMessage {
    #[wasm_bindgen(constructor)]
    pub fn new(agent_id: String, step: u64, message_type: &str, payload: JsValue) -> Result<JsACPMessage, JsValue> {
        let msg_type = match message_type.to_lowercase().as_str() {
            "status" => MessageType::Status,
            "propose" => MessageType::Propose,
            "step" => MessageType::Step,
            _ => return Err(JsValue::from_str("Invalid message type")),
        };

        let json_payload: JsonValue = from_value(payload)
            .map_err(|e| JsValue::from_str(&format!("Payload conversion failed: {}", e)))?;

        let inner = match msg_type {
            MessageType::Status => ACPMessage::status(agent_id, step, json_payload),
            MessageType::Propose => ACPMessage::propose(agent_id, step, json_payload),
            MessageType::Step => ACPMessage::step_complete(agent_id, step),
        };

        Ok(Self { inner })
    }

    #[wasm_bindgen(getter)]
    pub fn step(&self) -> u64 {
        self.inner.step
    }

    #[wasm_bindgen(getter, js_name = agentId)]
    pub fn agent_id(&self) -> String {
        self.inner.agent_id.clone()
    }

    #[wasm_bindgen(getter, js_name = messageType)]
    pub fn message_type(&self) -> String {
        format!("{:?}", self.inner.message_type).to_uppercase()
    }

    #[wasm_bindgen(getter)]
    pub fn payload(&self) -> Result<JsValue, JsValue> {
        to_value(&self.inner.payload)
            .map_err(|e| JsValue::from_str(&format!("Payload conversion failed: {}", e)))
    }

    #[wasm_bindgen(getter)]
    pub fn timestamp(&self) -> String {
        self.inner.timestamp.to_rfc3339()
    }

    #[wasm_bindgen]
    pub fn subject(&self) -> String {
        self.inner.subject()
    }

    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner)
            .map_err(|e| JsValue::from_str(&format!("JSON serialization failed: {}", e)))
    }

    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<JsACPMessage, JsValue> {
        let inner: ACPMessage = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("JSON deserialization failed: {}", e)))?;
        
        Ok(Self { inner })
    }
}

// Utility functions for JavaScript interop
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct MessageTypes;

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl MessageTypes {
    #[wasm_bindgen(getter, js_name = STATUS)]
    pub fn status() -> String {
        "STATUS".to_string()
    }

    #[wasm_bindgen(getter, js_name = PROPOSE)]
    pub fn propose() -> String {
        "PROPOSE".to_string()
    }

    #[wasm_bindgen(getter, js_name = STEP)]
    pub fn step() -> String {
        "STEP".to_string()
    }
}

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = createStepWildcard)]
pub fn create_step_wildcard(step: u64) -> String {
    ACPMessage::step_wildcard(step)
}

#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = createAgentWildcard)]
pub fn create_agent_wildcard(agent_id: &str) -> String {
    ACPMessage::agent_wildcard(agent_id)
}

#[cfg(feature = "wasm")]
#[wasm_bindgen(start)]
pub fn main() {
    console_log!("b00t-acp WebAssembly module loaded");
}

// TypeScript definitions export
#[cfg(feature = "wasm")]
#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = r#"
export interface ACPPayload {
  [key: string]: any;
}

export interface ConnectionOptions {
  agentId: string;
  natsUrl: string;
  namespace: string;
  jwtToken?: string;
  role?: string;
  timeoutMs?: number;
}

export interface StepResult {
  step: number;
  completed: boolean;
  pendingAgents: string[];
}

export declare class AgentConfig {
  constructor(agentId: string, natsUrl: string, namespace: string);
  withJwt(jwtToken: string): void;
  withRole(role: string): void;
  withTimeout(timeoutMs: number): void;
  readonly agentId: string;
  readonly natsUrl: string;
  readonly namespace: string;
  readonly role: string;
  readonly timeoutMs: number;
}

export declare class Agent {
  constructor();
  connect(config: AgentConfig): Promise<void>;
  start(): Promise<void>;
  stop(): Promise<void>;
  sendStatus(description: string, payload: ACPPayload): Promise<void>;
  sendPropose(action: string, payload: ACPPayload): Promise<void>;
  completeStep(): Promise<void>;
  waitForStepComplete(step: number): Promise<void>;
  currentStep(): Promise<number>;
  addAgent(agentId: string): Promise<void>;
  removeAgent(agentId: string): Promise<void>;
  knownAgents(): Promise<string[]>;
  isConnected(): Promise<boolean>;
}

export declare class ACPMessage {
  constructor(agentId: string, step: number, messageType: string, payload: ACPPayload);
  readonly step: number;
  readonly agentId: string;
  readonly messageType: string;
  readonly payload: ACPPayload;
  readonly timestamp: string;
  subject(): string;
  toJson(): string;
  static fromJson(json: string): ACPMessage;
}

export declare const MESSAGE_TYPES: {
  readonly STATUS: "STATUS";
  readonly PROPOSE: "PROPOSE";
  readonly STEP: "STEP";
};

export declare function createStepWildcard(step: number): string;
export declare function createAgentWildcard(agentId: string): string;
"#;