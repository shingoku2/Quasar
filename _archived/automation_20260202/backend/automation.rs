use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod engine;
pub mod graph;

/// Type alias for pin data (node_id -> array of JSON values)
pub type PinData = HashMap<String, Vec<serde_json::Value>>;

/// Types of nodes in a workflow
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Trigger,
    Action,
    Condition,
    Notification,
}

/// Trigger types for workflow execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    Manual,
    Scheduled(String), // cron expression
    Webhook(String),   // webhook endpoint path
}

/// Action types that can be performed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    SshCommand {
        host: String,
        port: u16,
        username: String,
        password: Option<String>,
        command: String,
    },
    FileTransfer {
        source: String,
        destination: String,
        host: String,
        direction: TransferDirection,
    },
    Notification {
        title: String,
        message: String,
        channel: NotificationChannel,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    InApp,
    System,
    Both,
}

/// Error handling behavior for nodes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorBehavior {
    StopWorkflow,           // Stop entire workflow on error
    ContinueRegularOutput,  // Continue with empty output
    ContinueErrorOutput,    // Continue and route to error branch
}

/// Reference to a credential in the vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialReference {
    pub id: Option<String>,  // Credential ID from vault
    pub name: String,        // Credential name
}

/// Workflow-level settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSettings {
    pub timezone: Option<String>,
    pub execution_timeout: Option<u64>,  // seconds
    pub save_execution_progress: bool,
    pub save_manual_executions: bool,
    pub save_data_error_execution: Option<String>,
    pub save_data_success_execution: Option<String>,
    pub error_workflow: Option<String>,  // Workflow ID to run on error
    pub caller_policy: Option<String>,
    pub execution_order: Option<String>,  // "v0" or "v1"
}

impl Default for WorkflowSettings {
    fn default() -> Self {
        Self {
            timezone: Some("UTC".to_string()),
            execution_timeout: Some(300),  // 5 minutes
            save_execution_progress: true,
            save_manual_executions: true,
            save_data_error_execution: Some("all".to_string()),
            save_data_success_execution: Some("all".to_string()),
            error_workflow: None,
            caller_policy: None,
            execution_order: Some("v1".to_string()),
        }
    }
}

/// Type of connection between nodes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionType {
    Main,              // Regular data flow
    Error,             // Error handling branch
    Ai,                // AI/LangChain data
    Custom(String),    // Extensible for future types
}

impl Default for ConnectionType {
    fn default() -> Self {
        ConnectionType::Main
    }
}

/// A node in the workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub node_type: NodeType,
    pub name: String,
    #[serde(default = "default_type_version")]
    pub type_version: u32,
    pub position: Position,
    pub config: NodeConfig,
    
    // Optional fields for advanced features
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes_in_flow: Option<bool>,
    
    // Error handling
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_error: Option<ErrorBehavior>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continue_on_fail: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_on_fail: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tries: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_between_tries: Option<u32>,
    
    // Credentials
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<HashMap<String, CredentialReference>>,
    
    // Advanced
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_output_data: Option<bool>,
}

fn default_type_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Configuration specific to each node type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeConfig {
    Trigger {
        trigger_type: TriggerType,
    },
    Action {
        action_type: ActionType,
    },
    Condition {
        expression: String,
        true_branch: String,  // node id
        false_branch: String, // node id
    },
    Notification {
        title: String,
        message: String,
        channel: NotificationChannel,
    },
}

/// Connection between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub source_node: String,
    pub source_port: String,
    #[serde(default)]
    pub source_index: usize,
    pub target_node: String,
    pub target_port: String,
    #[serde(default)]
    pub target_index: usize,
    #[serde(default)]
    pub connection_type: ConnectionType,
}

/// Bidirectional index for efficient connection lookups
pub struct ConnectionIndex {
    outgoing: HashMap<String, Vec<Connection>>,
    incoming: HashMap<String, Vec<Connection>>,
    by_type: HashMap<(String, ConnectionType), Vec<Connection>>,
}

impl ConnectionIndex {
    pub fn new(connections: &[Connection]) -> Self {
        let mut outgoing: HashMap<String, Vec<Connection>> = HashMap::new();
        let mut incoming: HashMap<String, Vec<Connection>> = HashMap::new();
        let mut by_type: HashMap<(String, ConnectionType), Vec<Connection>> = HashMap::new();
        
        for conn in connections {
            outgoing.entry(conn.source_node.clone())
                .or_insert_with(Vec::new)
                .push(conn.clone());
            
            incoming.entry(conn.target_node.clone())
                .or_insert_with(Vec::new)
                .push(conn.clone());
            
            by_type.entry((conn.source_node.clone(), conn.connection_type.clone()))
                .or_insert_with(Vec::new)
                .push(conn.clone());
        }
        
        Self { outgoing, incoming, by_type }
    }
    
    pub fn get_outgoing(&self, node_id: &str) -> &[Connection] {
        self.outgoing.get(node_id).map(|v| v.as_slice()).unwrap_or(&[])
    }
    
    pub fn get_incoming(&self, node_id: &str) -> &[Connection] {
        self.incoming.get(node_id).map(|v| v.as_slice()).unwrap_or(&[])
    }
    
    pub fn get_by_type(&self, node_id: &str, conn_type: &ConnectionType) -> &[Connection] {
        self.by_type.get(&(node_id.to_string(), conn_type.clone()))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

/// A workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub nodes: Vec<Node>,
    pub connections: Vec<Connection>,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    
    // Workflow features
    #[serde(default)]
    pub settings: WorkflowSettings,
    #[serde(default)]
    pub static_data: HashMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_data: Option<PinData>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl Workflow {
    pub fn new(name: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: None,
            nodes: Vec::new(),
            connections: Vec::new(),
            enabled: false,
            created_at: now,
            updated_at: now,
            settings: WorkflowSettings::default(),
            static_data: HashMap::new(),
            pin_data: None,
            tags: Vec::new(),
            category: None,
            author: None,
            version: None,
        }
    }

    /// Find the trigger node (entry point) of the workflow
    pub fn find_trigger_node(&self) -> Option<&Node> {
        self.nodes.iter().find(|n| matches!(n.node_type, NodeType::Trigger))
    }

    /// Build a bidirectional connection index for efficient lookups
    pub fn build_connection_index(&self) -> ConnectionIndex {
        ConnectionIndex::new(&self.connections)
    }

    /// Get all outgoing connections from a node
    pub fn get_outgoing_connections(&self, node_id: &str) -> Vec<&Connection> {
        self.connections.iter().filter(|c| c.source_node == node_id).collect()
    }

    /// Get all incoming connections to a node
    pub fn get_incoming_connections(&self, node_id: &str) -> Vec<&Connection> {
        self.connections.iter().filter(|c| c.target_node == node_id).collect()
    }
    
    /// Get outgoing connections of a specific type
    pub fn get_outgoing_by_type(&self, node_id: &str, conn_type: &ConnectionType) -> Vec<&Connection> {
        self.connections.iter()
            .filter(|c| c.source_node == node_id && &c.connection_type == conn_type)
            .collect()
    }
    
    /// Get error connections from a node
    pub fn get_error_connections(&self, node_id: &str) -> Vec<&Connection> {
        self.get_outgoing_by_type(node_id, &ConnectionType::Error)
    }

    /// Get a node by ID
    pub fn get_node(&self, node_id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == node_id)
    }
    
    /// Build a directed graph representation of the workflow
    pub fn build_graph(&self) -> graph::DirectedGraph {
        graph::DirectedGraph::from_workflow(self)
    }
    
    /// Validate that the workflow has no circular dependencies
    pub fn validate_no_cycles(&self) -> Result<(), WorkflowError> {
        let graph = self.build_graph();
        if graph.has_cycle() {
            Err(WorkflowError::CircularDependency)
        } else {
            Ok(())
        }
    }
    
    /// Get the optimal execution order for all nodes
    pub fn get_execution_order(&self) -> Result<Vec<String>, WorkflowError> {
        let graph = self.build_graph();
        graph.topological_sort()
    }
    
    /// Get the set of nodes needed to execute a specific target node
    pub fn get_partial_execution_nodes(&self, target_node: &str) -> HashSet<String> {
        let graph = self.build_graph();
        graph.get_execution_subgraph(target_node)
    }
    
    // Static data methods
    pub fn get_static_data(&self, key: &str) -> Option<&serde_json::Value> {
        self.static_data.get(key)
    }
    
    pub fn set_static_data(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.static_data.insert(key.into(), value);
    }
    
    pub fn clear_static_data(&mut self) {
        self.static_data.clear();
    }
    
    // Pin data methods
    pub fn pin_node_data(&mut self, node_id: impl Into<String>, data: Vec<serde_json::Value>) {
        if self.pin_data.is_none() {
            self.pin_data = Some(HashMap::new());
        }
        self.pin_data.as_mut().unwrap().insert(node_id.into(), data);
    }
    
    pub fn get_pinned_data(&self, node_id: &str) -> Option<&Vec<serde_json::Value>> {
        self.pin_data.as_ref()?.get(node_id)
    }
    
    pub fn unpin_node_data(&mut self, node_id: &str) {
        if let Some(pin_data) = &mut self.pin_data {
            pin_data.remove(node_id);
        }
    }
    
    pub fn clear_pin_data(&mut self) {
        self.pin_data = None;
    }
    
    // Tag methods
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag = tag.into();
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }
    
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
    }
    
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(&tag.to_string())
    }

    /// Perform topological sort of nodes based on connections
    pub fn topological_sort(&self) -> Result<Vec<&Node>, WorkflowError> {
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_mark = HashSet::new();

        // Build adjacency list
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
        for conn in &self.connections {
            adjacency.entry(&conn.source_node).or_default().push(&conn.target_node);
        }

        fn visit<'a>(
            node_id: &'a str,
            adjacency: &HashMap<&'a str, Vec<&'a str>>,
            visited: &mut HashSet<&'a str>,
            temp_mark: &mut HashSet<&'a str>,
            sorted: &mut Vec<&'a str>,
        ) -> Result<(), WorkflowError> {
            if temp_mark.contains(node_id) {
                return Err(WorkflowError::CircularDependency);
            }
            if visited.contains(node_id) {
                return Ok(());
            }
            temp_mark.insert(node_id);

            if let Some(neighbors) = adjacency.get(node_id) {
                for neighbor in neighbors {
                    visit(neighbor, adjacency, visited, temp_mark, sorted)?;
                }
            }

            temp_mark.remove(node_id);
            visited.insert(node_id);
            sorted.push(node_id);
            Ok(())
        }

        // Find trigger node as starting point
        let trigger = self.find_trigger_node()
            .ok_or(WorkflowError::NoTriggerNode)?;

        // Visit from trigger
        visit(&trigger.id, &adjacency, &mut visited, &mut temp_mark, &mut sorted)?;

        // Map back to nodes and reverse to get correct execution order
        Ok(sorted.iter()
            .rev()
            .filter_map(|id| self.get_node(id))
            .collect())
    }
}

/// Execution context passed between nodes during workflow execution
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub variables: HashMap<String, String>,
    pub results: HashMap<String, NodeResult>,
    pub json_data: HashMap<String, serde_json::Value>,
    pub workflow_id: String,
    pub execution_id: String,
    pub execution_mode: String,
    pub env: HashMap<String, String>,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new("default".to_string(), "default".to_string())
    }
}

impl ExecutionContext {
    pub fn new(workflow_id: String, execution_id: String) -> Self {
        Self {
            variables: HashMap::new(),
            results: HashMap::new(),
            json_data: HashMap::new(),
            workflow_id,
            execution_id,
            execution_mode: "manual".to_string(),
            env: std::env::vars().collect(),
            started_at: chrono::Utc::now(),
        }
    }

    pub fn set_variable(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(key.into(), value.into());
    }

    pub fn get_variable(&self, key: &str) -> Option<&str> {
        self.variables.get(key).map(|s| s.as_str())
    }
    
    pub fn get_variable_or(&self, key: &str, default: &str) -> String {
        self.get_variable(key).unwrap_or(default).to_string()
    }

    pub fn set_result(&mut self, node_id: impl Into<String>, result: NodeResult) {
        self.results.insert(node_id.into(), result);
    }
    
    pub fn set_json(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.json_data.insert(key.into(), value);
    }
    
    pub fn get_json(&self, key: &str) -> Option<&serde_json::Value> {
        self.json_data.get(key)
    }
    
    pub fn get_node_output(&self, node_id: &str) -> Option<String> {
        match self.results.get(node_id)? {
            NodeResult::Success { output } => output.clone(),
            _ => None,
        }
    }
    
    pub fn node_succeeded(&self, node_id: &str) -> bool {
        matches!(
            self.results.get(node_id),
            Some(NodeResult::Success { .. })
        )
    }
    
    pub fn get_env(&self, key: &str) -> Option<&str> {
        self.env.get(key).map(|s| s.as_str())
    }
    
    pub fn execution_duration(&self) -> chrono::Duration {
        chrono::Utc::now() - self.started_at
    }
    
    pub fn evaluate_expression(&self, expr: &str) -> String {
        use regex::Regex;
        let mut result = expr.to_string();
        
        let re = Regex::new(r"\$\{([^}]+)\}").unwrap();
        
        let replacements: Vec<(String, String)> = re.captures_iter(expr)
            .map(|cap| {
                let full_match = cap[0].to_string();
                let var_path = &cap[1];
                let value = self.resolve_variable_path(var_path);
                (full_match, value)
            })
            .collect();
        
        for (pattern, value) in replacements {
            result = result.replace(&pattern, &value);
        }
        
        result
    }
    
    fn resolve_variable_path(&self, path: &str) -> String {
        let parts: Vec<&str> = path.split('.').collect();
        
        match parts.as_slice() {
            ["env", key] => {
                self.get_env(key).unwrap_or("").to_string()
            }
            ["node", node_id, "output"] => {
                self.get_node_output(node_id).unwrap_or_default()
            }
            ["workflow", "id"] => self.workflow_id.clone(),
            ["execution", "id"] => self.execution_id.clone(),
            ["execution", "mode"] => self.execution_mode.clone(),
            [var_name] => {
                self.get_variable(var_name).unwrap_or("").to_string()
            }
            _ => String::new(),
        }
    }
    
    pub fn evaluate_condition(&self, expr: &str) -> bool {
        let expanded = self.evaluate_expression(expr);
        
        match expanded.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" => true,
            "false" | "0" | "no" | "" => false,
            _ => self.evaluate_comparison(&expanded),
        }
    }
    
    fn evaluate_comparison(&self, expr: &str) -> bool {
        if let Some((left, right)) = expr.split_once("==") {
            return left.trim() == right.trim();
        }
        if let Some((left, right)) = expr.split_once("!=") {
            return left.trim() != right.trim();
        }
        if let Some((left, right)) = expr.split_once(">=") {
            if let (Ok(l), Ok(r)) = (left.trim().parse::<f64>(), right.trim().parse::<f64>()) {
                return l >= r;
            }
        }
        if let Some((left, right)) = expr.split_once("<=") {
            if let (Ok(l), Ok(r)) = (left.trim().parse::<f64>(), right.trim().parse::<f64>()) {
                return l <= r;
            }
        }
        if let Some((left, right)) = expr.split_once('>') {
            if let (Ok(l), Ok(r)) = (left.trim().parse::<f64>(), right.trim().parse::<f64>()) {
                return l > r;
            }
        }
        if let Some((left, right)) = expr.split_once('<') {
            if let (Ok(l), Ok(r)) = (left.trim().parse::<f64>(), right.trim().parse::<f64>()) {
                return l < r;
            }
        }
        false
    }
    
    pub fn parse_json(&self, json_str: &str) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(json_str)
    }
    
    pub fn stringify_json(&self, value: &serde_json::Value) -> String {
        serde_json::to_string_pretty(value).unwrap_or_default()
    }
}

/// Result of executing a node
#[derive(Debug, Clone)]
pub enum NodeResult {
    Success { output: Option<String> },
    Failure { error: String },
    Skipped,
}

/// Workflow execution errors
#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error("No trigger node found in workflow")]
    NoTriggerNode,
    
    #[error("Circular dependency detected in workflow")]
    CircularDependency,
    
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Invalid workflow configuration: {0}")]
    InvalidConfiguration(String),
}

/// Execution status for a workflow run
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// A record of a workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub id: String,
    pub workflow_id: String,
    pub status: ExecutionStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub node_results: HashMap<String, NodeExecutionResult>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionResult {
    pub status: ExecutionStatus,
    pub output: Option<String>,
    pub error: Option<String>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// In-memory storage for workflows and executions
pub struct AutomationState {
    workflows: Arc<RwLock<HashMap<String, Workflow>>>,
    executions: Arc<RwLock<HashMap<String, ExecutionRecord>>>,
}

impl AutomationState {
    pub fn new() -> Self {
        Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
            executions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_workflow(&self, workflow: Workflow) -> String {
        let id = workflow.id.clone();
        let mut workflows = self.workflows.write().await;
        workflows.insert(id.clone(), workflow);
        id
    }

    pub async fn get_workflow(&self, id: &str) -> Option<Workflow> {
        let workflows = self.workflows.read().await;
        workflows.get(id).cloned()
    }

    pub async fn update_workflow(&self, workflow: Workflow) -> Result<(), WorkflowError> {
        let mut workflows = self.workflows.write().await;
        if !workflows.contains_key(&workflow.id) {
            return Err(WorkflowError::NodeNotFound(workflow.id));
        }
        workflows.insert(workflow.id.clone(), workflow);
        Ok(())
    }

    pub async fn delete_workflow(&self, id: &str) -> bool {
        let mut workflows = self.workflows.write().await;
        workflows.remove(id).is_some()
    }

    pub async fn list_workflows(&self) -> Vec<Workflow> {
        let workflows = self.workflows.read().await;
        workflows.values().cloned().collect()
    }

    pub async fn save_workflow(&self, workflow: Workflow) {
        let mut workflows = self.workflows.write().await;
        workflows.insert(workflow.id.clone(), workflow);
    }

    pub async fn create_execution(&self, workflow_id: &str) -> String {
        let id = Uuid::new_v4().to_string();
        let record = ExecutionRecord {
            id: id.clone(),
            workflow_id: workflow_id.to_string(),
            status: ExecutionStatus::Pending,
            started_at: chrono::Utc::now(),
            completed_at: None,
            node_results: HashMap::new(),
            error_message: None,
        };
        let mut executions = self.executions.write().await;
        executions.insert(id.clone(), record);
        id
    }

    pub async fn update_execution(&self, record: ExecutionRecord) {
        let mut executions = self.executions.write().await;
        executions.insert(record.id.clone(), record);
    }

    pub async fn get_execution(&self, id: &str) -> Option<ExecutionRecord> {
        let executions = self.executions.read().await;
        executions.get(id).cloned()
    }

    pub async fn list_executions(&self, workflow_id: Option<&str>) -> Vec<ExecutionRecord> {
        let executions = self.executions.read().await;
        executions.values()
            .filter(|e| workflow_id.map_or(true, |id| e.workflow_id == id))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_workflow() -> Workflow {
        let mut workflow = Workflow::new("Test Workflow");
        
        // Add trigger node
        let trigger = Node {
            id: "trigger_1".to_string(),
            node_type: NodeType::Trigger,
            name: "Manual Trigger".to_string(),
            type_version: 1,
            position: Position { x: 100.0, y: 100.0 },
            config: NodeConfig::Trigger {
                trigger_type: TriggerType::Manual,
            },
            disabled: None,
            notes: None,
            notes_in_flow: None,
            on_error: None,
            continue_on_fail: None,
            retry_on_fail: None,
            max_tries: None,
            wait_between_tries: None,
            credentials: None,
            parameters: None,
            always_output_data: None,
        };
        
        // Add action node
        let action = Node {
            id: "action_1".to_string(),
            node_type: NodeType::Action,
            name: "SSH Command".to_string(),
            type_version: 1,
            position: Position { x: 300.0, y: 100.0 },
            config: NodeConfig::Action {
                action_type: ActionType::SshCommand {
                    host: "192.168.1.1".to_string(),
                    port: 22,
                    username: "admin".to_string(),
                    password: None,
                    command: "uptime".to_string(),
                },
            },
            disabled: None,
            notes: None,
            notes_in_flow: None,
            on_error: None,
            continue_on_fail: None,
            retry_on_fail: None,
            max_tries: None,
            wait_between_tries: None,
            credentials: None,
            parameters: None,
            always_output_data: None,
        };
        
        workflow.nodes.push(trigger);
        workflow.nodes.push(action);
        
        // Connect trigger to action
        workflow.connections.push(Connection {
            id: "conn_1".to_string(),
            source_node: "trigger_1".to_string(),
            source_port: "output".to_string(),
            source_index: 0,
            target_node: "action_1".to_string(),
            target_port: "input".to_string(),
            target_index: 0,
            connection_type: ConnectionType::Main,
        });
        
        workflow
    }

    #[test]
    fn test_workflow_creation() {
        let workflow = Workflow::new("My Workflow");
        assert_eq!(workflow.name, "My Workflow");
        assert!(workflow.find_trigger_node().is_none());
    }

    #[test]
    fn test_find_trigger_node() {
        let workflow = create_test_workflow();
        let trigger = workflow.find_trigger_node();
        assert!(trigger.is_some());
        assert_eq!(trigger.unwrap().id, "trigger_1");
    }

    #[test]
    fn test_topological_sort() {
        let workflow = create_test_workflow();
        let sorted = workflow.topological_sort().unwrap();
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].id, "action_1"); // Action comes first in reverse topological order
        assert_eq!(sorted[1].id, "trigger_1");
    }

    #[test]
    fn test_get_connections() {
        let workflow = create_test_workflow();
        let outgoing = workflow.get_outgoing_connections("trigger_1");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].target_node, "action_1");
        
        let incoming = workflow.get_incoming_connections("action_1");
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].source_node, "trigger_1");
    }

    #[tokio::test]
    async fn test_automation_state() {
        let state = AutomationState::new();
        
        // Create workflow
        let workflow = create_test_workflow();
        let id = state.create_workflow(workflow.clone()).await;
        
        // Get workflow
        let retrieved = state.get_workflow(&id).await;
        assert!(retrieved.is_some());
        
        // List workflows
        let workflows = state.list_workflows().await;
        assert_eq!(workflows.len(), 1);
        
        // Create execution
        let exec_id = state.create_execution(&id).await;
        let execution = state.get_execution(&exec_id).await;
        assert!(execution.is_some());
        assert!(matches!(execution.unwrap().status, ExecutionStatus::Pending));
    }
}
