use crate::automation::{ActionType, ExecutionContext, ExecutionStatus, Node, NodeConfig, NodeResult, NodeType, Workflow, WorkflowError};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter};

/// Executes a workflow node by node
pub struct WorkflowEngine {
    app_handle: AppHandle,
}

impl WorkflowEngine {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    /// Execute a complete workflow
    pub async fn execute_workflow(
        &self,
        workflow: &Workflow,
        execution_id: &str,
        context: &mut ExecutionContext,
    ) -> Result<ExecutionStatus, WorkflowError> {
        // Emit execution started event
        let _ = self.app_handle.emit(
            "workflow-execution-started",
            serde_json::json!({
                "execution_id": execution_id,
                "workflow_id": workflow.id,
            }),
        );

        // Find trigger node
        let trigger = workflow.find_trigger_node()
            .ok_or(WorkflowError::NoTriggerNode)?;

        // Execute starting from trigger
        let status = self.execute_node_recursive(workflow, &trigger.id, execution_id, context).await?;

        // Emit execution completed event
        let _ = self.app_handle.emit(
            "workflow-execution-completed",
            serde_json::json!({
                "execution_id": execution_id,
                "workflow_id": workflow.id,
                "status": serde_json::to_string(&status).unwrap(),
            }),
        );

        Ok(status)
    }

    /// Execute a single node and recursively follow its connections
    async fn execute_node_recursive(
        &self,
        workflow: &Workflow,
        node_id: &str,
        execution_id: &str,
        context: &mut ExecutionContext,
    ) -> Result<ExecutionStatus, WorkflowError> {
        let node = workflow.get_node(node_id)
            .ok_or_else(|| WorkflowError::NodeNotFound(node_id.to_string()))?;

        // Emit node started event
        let _ = self.app_handle.emit(
            "workflow-node-started",
            serde_json::json!({
                "execution_id": execution_id,
                "node_id": node_id,
                "node_name": &node.name,
            }),
        );

        // Execute the node
        let result = self.execute_node(node, context).await;

        // Store result in context
        let status = match &result {
            NodeResult::Success { .. } => ExecutionStatus::Completed,
            NodeResult::Failure { .. } => ExecutionStatus::Failed,
            NodeResult::Skipped => ExecutionStatus::Cancelled,
        };

        context.set_result(node_id, result.clone());

        // Emit node completed event
        let _ = self.app_handle.emit(
            "workflow-node-completed",
            serde_json::json!({
                "execution_id": execution_id,
                "node_id": node_id,
                "status": serde_json::to_string(&status).unwrap(),
            }),
        );

        // If node failed, stop execution
        if matches!(result, NodeResult::Failure { .. }) {
            return Ok(ExecutionStatus::Failed);
        }

        // Get outgoing connections and execute next nodes
        let connections = workflow.get_outgoing_connections(node_id);
        
        // For condition nodes, only follow the matching branch
        if let NodeConfig::Condition { true_branch, false_branch, .. } = &node.config {
            let next_branch = match &result {
                NodeResult::Success { output: Some(out) } if out == "true" => true_branch,
                _ => false_branch,
            };
            
            if let Some(conn) = connections.iter().find(|c| c.target_node == *next_branch) {
                Box::pin(self.execute_node_recursive(workflow, &conn.target_node, execution_id, context)).await?;
            }
        } else {
            // For other nodes, execute all connected nodes sequentially
            for conn in connections {
                Box::pin(self.execute_node_recursive(workflow, &conn.target_node, execution_id, context)).await?;
            }
        }

        Ok(ExecutionStatus::Completed)
    }

    /// Execute a single node based on its type
    async fn execute_node(
        &self,
        node: &Node,
        context: &mut ExecutionContext,
    ) -> NodeResult {
        match &node.config {
            NodeConfig::Trigger { trigger_type } => {
                // Trigger nodes just set up the context
                context.set_variable("trigger_time", chrono::Utc::now().to_rfc3339());
                NodeResult::Success { output: None }
            }

            NodeConfig::Action { action_type } => {
                self.execute_action(action_type, context).await
            }

            NodeConfig::Condition { expression, .. } => {
                // Simple condition evaluation - can be extended with expression parser
                let result = self.evaluate_condition(expression, context);
                NodeResult::Success {
                    output: Some(result.to_string()),
                }
            }

            NodeConfig::Notification { title, message, channel } => {
                self.send_notification(title, message, channel).await;
                NodeResult::Success { output: None }
            }
        }
    }

    /// Execute an action based on its type
    async fn execute_action(
        &self,
        action: &ActionType,
        context: &mut ExecutionContext,
    ) -> NodeResult {
        match action {
            ActionType::SshCommand { host, port, username, password, command } => {
                // Execute real SSH command using ssh_exec module
                let pass = password.as_ref().map(|s| s.as_str()).unwrap_or("");
                match crate::ssh_exec::execute_ssh_command(
                    host,
                    *port,
                    username,
                    pass,
                    command,
                    30, // 30 second timeout
                ).await {
                    Ok(output) => {
                        // Store output in context for use by subsequent nodes
                        context.set_variable("last_ssh_output", &output);
                        NodeResult::Success {
                            output: Some(output),
                        }
                    }
                    Err(e) => {
                        NodeResult::Success {
                            output: Some(format!("SSH command failed: {}", e)),
                        }
                    }
                }
            }

            ActionType::FileTransfer { source, destination, host, direction } => {
                // Extract connection details from host string (format: username@host:port)
                let parts: Vec<&str> = host.split('@').collect();
                if parts.len() != 2 {
                    return NodeResult::Success {
                        output: Some(format!("Invalid host format. Expected: username@host:port")),
                    };
                }
                
                let username = parts[0];
                let host_port: Vec<&str> = parts[1].split(':').collect();
                let hostname = host_port[0];
                let port = host_port.get(1).and_then(|p| p.parse::<u16>().ok()).unwrap_or(22);
                
                // Get password from context or use empty string
                let password = context.get_variable("ssh_password").unwrap_or_default();
                
                // Perform file transfer based on direction
                let result = match direction {
                    crate::automation::TransferDirection::Upload => {
                        crate::sftp::upload_file(
                            hostname,
                            port,
                            username,
                            &password,
                            source,
                            destination,
                            None, // No progress callback for now
                        ).await
                    }
                    crate::automation::TransferDirection::Download => {
                        crate::sftp::download_file(
                            hostname,
                            port,
                            username,
                            &password,
                            source,
                            destination,
                            None, // No progress callback for now
                        ).await
                    }
                };
                
                match result {
                    Ok(()) => NodeResult::Success {
                        output: Some(format!("Successfully transferred {} {} {}", 
                            source, 
                            match direction {
                                crate::automation::TransferDirection::Upload => "to",
                                crate::automation::TransferDirection::Download => "from",
                            },
                            destination
                        )),
                    },
                    Err(e) => NodeResult::Success {
                        output: Some(format!("File transfer failed: {}", e)),
                    },
                }
            }

            ActionType::Notification { title, message, channel } => {
                self.send_notification(title, message, channel).await;
                NodeResult::Success { output: None }
            }
        }
    }

    /// Evaluate a simple condition expression
    fn evaluate_condition(&self, expression: &str, context: &ExecutionContext) -> bool {
        // Simple variable substitution and comparison
        // Example: "${result} == 'success'"
        let expanded = self.expand_variables(expression, context);
        
        // Very basic evaluation - checks for common true conditions
        expanded.to_lowercase() == "true" ||
        expanded == "1" ||
        expanded == "success" ||
        expanded.contains("== success") ||
        expanded.contains("= success")
    }

    /// Expand ${variable} placeholders in a string
    fn expand_variables(&self, text: &str, context: &ExecutionContext) -> String {
        let mut result = text.to_string();
        
        // Simple variable substitution - find ${varname}
        for (key, value) in &context.variables {
            let placeholder = format!("${{{}}}", key);
            result = result.replace(&placeholder, value);
        }
        
        result
    }

    /// Send a notification
    async fn send_notification(
        &self,
        title: &str,
        message: &str,
        channel: &crate::automation::NotificationChannel,
    ) {
        let _ = self.app_handle.emit(
            "workflow-notification",
            serde_json::json!({
                "title": title,
                "message": message,
                "channel": serde_json::to_string(channel).unwrap(),
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::{AutomationState, ExecutionRecord, Position, TriggerType, NodeConfig, Node};

    #[tokio::test]
    async fn test_workflow_engine_creation() {
        // This would need a mock AppHandle for full testing
        // For now, just verify the module compiles
    }

    #[test]
    fn test_expand_variables() {
        let mut context = ExecutionContext::new();
        context.set_variable("host", "192.168.1.1");
        context.set_variable("status", "success");

        let engine = WorkflowEngine::new;
        // Test would need proper mocking
    }
}
