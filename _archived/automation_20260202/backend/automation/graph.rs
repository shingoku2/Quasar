use crate::automation::{Workflow, WorkflowError};
use std::collections::{HashMap, HashSet, VecDeque};

/// Directed graph representation of a workflow
pub struct DirectedGraph {
    // Node ID -> list of successor node IDs
    adjacency: HashMap<String, Vec<String>>,
    // Node ID -> list of predecessor node IDs
    reverse_adjacency: HashMap<String, Vec<String>>,
    // All node IDs in the graph
    nodes: HashSet<String>,
}

impl DirectedGraph {
    /// Create a directed graph from a workflow
    pub fn from_workflow(workflow: &Workflow) -> Self {
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        let mut reverse_adjacency: HashMap<String, Vec<String>> = HashMap::new();
        let mut nodes = HashSet::new();
        
        // Add all nodes
        for node in &workflow.nodes {
            nodes.insert(node.id.clone());
            adjacency.entry(node.id.clone()).or_insert_with(Vec::new);
            reverse_adjacency.entry(node.id.clone()).or_insert_with(Vec::new);
        }
        
        // Build adjacency lists from connections
        for conn in &workflow.connections {
            adjacency.entry(conn.source_node.clone())
                .or_insert_with(Vec::new)
                .push(conn.target_node.clone());
            
            reverse_adjacency.entry(conn.target_node.clone())
                .or_insert_with(Vec::new)
                .push(conn.source_node.clone());
        }
        
        Self { adjacency, reverse_adjacency, nodes }
    }
    
    /// Get successor nodes (nodes that come after this one)
    pub fn get_successors(&self, node_id: &str) -> &[String] {
        self.adjacency.get(node_id).map(|v| v.as_slice()).unwrap_or(&[])
    }
    
    /// Get predecessor nodes (nodes that come before this one)
    pub fn get_predecessors(&self, node_id: &str) -> &[String] {
        self.reverse_adjacency.get(node_id).map(|v| v.as_slice()).unwrap_or(&[])
    }
    
    /// Check if a node exists in the graph
    pub fn has_node(&self, node_id: &str) -> bool {
        self.nodes.contains(node_id)
    }
    
    /// Detect if the graph contains cycles using DFS
    pub fn has_cycle(&self) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        
        for node in &self.nodes {
            if !visited.contains(node) {
                if self.has_cycle_util(node, &mut visited, &mut rec_stack) {
                    return true;
                }
            }
        }
        false
    }
    
    fn has_cycle_util(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> bool {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        
        for successor in self.get_successors(node) {
            if !visited.contains(successor) {
                if self.has_cycle_util(successor, visited, rec_stack) {
                    return true;
                }
            } else if rec_stack.contains(successor) {
                return true; // Back edge found - cycle detected
            }
        }
        
        rec_stack.remove(node);
        false
    }
    
    /// Get all ancestors of a node (nodes that must execute before it)
    pub fn get_ancestors(&self, node_id: &str) -> HashSet<String> {
        let mut ancestors = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(node_id.to_string());
        
        while let Some(current) = queue.pop_front() {
            for predecessor in self.get_predecessors(&current) {
                if ancestors.insert(predecessor.clone()) {
                    queue.push_back(predecessor.clone());
                }
            }
        }
        
        ancestors
    }
    
    /// Get subgraph needed to execute a specific node (node + all ancestors)
    pub fn get_execution_subgraph(&self, target_node: &str) -> HashSet<String> {
        let mut subgraph = self.get_ancestors(target_node);
        subgraph.insert(target_node.to_string());
        subgraph
    }
    
    /// Get all descendants of a node (nodes that come after it)
    pub fn get_descendants(&self, node_id: &str) -> HashSet<String> {
        let mut descendants = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(node_id.to_string());
        
        while let Some(current) = queue.pop_front() {
            for successor in self.get_successors(&current) {
                if descendants.insert(successor.clone()) {
                    queue.push_back(successor.clone());
                }
            }
        }
        
        descendants
    }
    
    /// Get topological sort of nodes (optimal execution order)
    /// Returns error if graph contains cycles
    pub fn topological_sort(&self) -> Result<Vec<String>, WorkflowError> {
        if self.has_cycle() {
            return Err(WorkflowError::CircularDependency);
        }
        
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();
        
        // Calculate in-degrees
        for node in &self.nodes {
            in_degree.insert(node.clone(), self.get_predecessors(node).len());
        }
        
        // Add nodes with no dependencies to queue
        for (node, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node.clone());
            }
        }
        
        // Process nodes using Kahn's algorithm
        while let Some(node) = queue.pop_front() {
            result.push(node.clone());
            
            for successor in self.get_successors(&node) {
                if let Some(degree) = in_degree.get_mut(successor) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(successor.clone());
                    }
                }
            }
        }
        
        // If we didn't process all nodes, there's a cycle
        if result.len() != self.nodes.len() {
            return Err(WorkflowError::CircularDependency);
        }
        
        Ok(result)
    }
    
    /// Get the number of nodes in the graph
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    
    /// Get the number of edges in the graph
    pub fn edge_count(&self) -> usize {
        self.adjacency.values().map(|v| v.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::{Node, NodeType, NodeConfig, TriggerType, Position, Connection, ConnectionType};

    fn create_test_workflow() -> Workflow {
        let mut workflow = Workflow::new("Test Graph Workflow");
        
        // Create a simple linear workflow: trigger -> action1 -> action2
        let trigger = Node {
            id: "trigger".to_string(),
            node_type: NodeType::Trigger,
            name: "Trigger".to_string(),
            type_version: 1,
            position: Position { x: 0.0, y: 0.0 },
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
        
        let action1 = Node {
            id: "action1".to_string(),
            node_type: NodeType::Action,
            name: "Action 1".to_string(),
            type_version: 1,
            position: Position { x: 100.0, y: 0.0 },
            config: NodeConfig::Action {
                action_type: crate::automation::ActionType::Notification {
                    title: "Test".to_string(),
                    message: "Test".to_string(),
                    channel: crate::automation::NotificationChannel::InApp,
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
        
        let action2 = Node {
            id: "action2".to_string(),
            node_type: NodeType::Action,
            name: "Action 2".to_string(),
            type_version: 1,
            position: Position { x: 200.0, y: 0.0 },
            config: NodeConfig::Action {
                action_type: crate::automation::ActionType::Notification {
                    title: "Test".to_string(),
                    message: "Test".to_string(),
                    channel: crate::automation::NotificationChannel::InApp,
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
        workflow.nodes.push(action1);
        workflow.nodes.push(action2);
        
        workflow.connections.push(Connection {
            id: "conn1".to_string(),
            source_node: "trigger".to_string(),
            source_port: "output".to_string(),
            source_index: 0,
            target_node: "action1".to_string(),
            target_port: "input".to_string(),
            target_index: 0,
            connection_type: ConnectionType::Main,
        });
        
        workflow.connections.push(Connection {
            id: "conn2".to_string(),
            source_node: "action1".to_string(),
            source_port: "output".to_string(),
            source_index: 0,
            target_node: "action2".to_string(),
            target_port: "input".to_string(),
            target_index: 0,
            connection_type: ConnectionType::Main,
        });
        
        workflow
    }

    #[test]
    fn test_graph_construction() {
        let workflow = create_test_workflow();
        let graph = DirectedGraph::from_workflow(&workflow);
        
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
        assert!(graph.has_node("trigger"));
        assert!(graph.has_node("action1"));
        assert!(graph.has_node("action2"));
    }

    #[test]
    fn test_successors_predecessors() {
        let workflow = create_test_workflow();
        let graph = DirectedGraph::from_workflow(&workflow);
        
        assert_eq!(graph.get_successors("trigger"), &["action1"]);
        assert_eq!(graph.get_successors("action1"), &["action2"]);
        assert_eq!(graph.get_successors("action2").len(), 0);
        
        assert_eq!(graph.get_predecessors("trigger").len(), 0);
        assert_eq!(graph.get_predecessors("action1"), &["trigger"]);
        assert_eq!(graph.get_predecessors("action2"), &["action1"]);
    }

    #[test]
    fn test_no_cycle() {
        let workflow = create_test_workflow();
        let graph = DirectedGraph::from_workflow(&workflow);
        
        assert!(!graph.has_cycle());
    }

    #[test]
    fn test_topological_sort() {
        let workflow = create_test_workflow();
        let graph = DirectedGraph::from_workflow(&workflow);
        
        let order = graph.topological_sort().unwrap();
        assert_eq!(order.len(), 3);
        
        // Trigger should come before action1
        let trigger_pos = order.iter().position(|n| n == "trigger").unwrap();
        let action1_pos = order.iter().position(|n| n == "action1").unwrap();
        let action2_pos = order.iter().position(|n| n == "action2").unwrap();
        
        assert!(trigger_pos < action1_pos);
        assert!(action1_pos < action2_pos);
    }

    #[test]
    fn test_ancestors() {
        let workflow = create_test_workflow();
        let graph = DirectedGraph::from_workflow(&workflow);
        
        let ancestors = graph.get_ancestors("action2");
        assert_eq!(ancestors.len(), 2);
        assert!(ancestors.contains("trigger"));
        assert!(ancestors.contains("action1"));
    }

    #[test]
    fn test_execution_subgraph() {
        let workflow = create_test_workflow();
        let graph = DirectedGraph::from_workflow(&workflow);
        
        let subgraph = graph.get_execution_subgraph("action2");
        assert_eq!(subgraph.len(), 3);
        assert!(subgraph.contains("trigger"));
        assert!(subgraph.contains("action1"));
        assert!(subgraph.contains("action2"));
    }
}
