use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod engine;

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

/// A node in the workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub node_type: NodeType,
    pub name: String,
    pub position: Position,
    pub config: NodeConfig,
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
    pub target_node: String,
    pub target_port: String,
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
        }
    }

    /// Find the trigger node (entry point) of the workflow
    pub fn find_trigger_node(&self) -> Option<&Node> {
        self.nodes.iter().find(|n| matches!(n.node_type, NodeType::Trigger))
    }

    /// Get all outgoing connections from a node
    pub fn get_outgoing_connections(&self, node_id: &str) -> Vec<&Connection> {
        self.connections.iter().filter(|c| c.source_node == node_id).collect()
    }

    /// Get all incoming connections to a node
    pub fn get_incoming_connections(&self, node_id: &str) -> Vec<&Connection> {
        self.connections.iter().filter(|c| c.target_node == node_id).collect()
    }

    /// Get a node by ID
    pub fn get_node(&self, node_id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == node_id)
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

        // Map back to nodes
        Ok(sorted.iter()
            .filter_map(|id| self.get_node(id))
            .collect())
    }
}

/// Execution context passed between nodes during workflow execution
#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    pub variables: HashMap<String, String>,
    pub results: HashMap<String, NodeResult>,
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            results: HashMap::new(),
        }
    }

    pub fn set_variable(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(key.into(), value.into());
    }

    pub fn get_variable(&self, key: &str) -> Option<&str> {
        self.variables.get(key).map(|s| s.as_str())
    }

    pub fn set_result(&mut self, node_id: impl Into<String>, result: NodeResult) {
        self.results.insert(node_id.into(), result);
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
            position: Position { x: 100.0, y: 100.0 },
            config: NodeConfig::Trigger {
                trigger_type: TriggerType::Manual,
            },
        };
        
        // Add action node
        let action = Node {
            id: "action_1".to_string(),
            node_type: NodeType::Action,
            name: "SSH Command".to_string(),
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
        };
        
        workflow.nodes.push(trigger);
        workflow.nodes.push(action);
        
        // Connect trigger to action
        workflow.connections.push(Connection {
            id: "conn_1".to_string(),
            source_node: "trigger_1".to_string(),
            source_port: "output".to_string(),
            target_node: "action_1".to_string(),
            target_port: "input".to_string(),
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
