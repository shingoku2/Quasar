# Phase 3: Graph-Based Execution Implementation

Implement intelligent workflow execution using graph analysis, partial execution, cycle detection, and optimized execution order inspired by n8n's DirectedGraph approach.

## Overview

This phase adds sophisticated graph-based execution that enables:
- **DirectedGraph structure** for workflow analysis
- **Cycle detection** to prevent infinite loops
- **Partial execution** - run only specific subgraphs
- **Execution optimization** - smart node ordering
- **Dependency analysis** - understand node relationships

## Current State Analysis

**Current Execution** (`src-tauri/src/automation/engine.rs`):
- Simple recursive execution from trigger node
- No cycle detection
- No partial execution support
- No execution order optimization
- Executes all connected nodes

**Limitations**:
- Can't detect circular dependencies
- Can't run partial workflows
- No optimization of execution order
- No dependency analysis
- Risk of infinite loops

## Proposed Changes

### 1. DirectedGraph Structure

**New graph representation**:
```rust
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
    
    pub fn get_successors(&self, node_id: &str) -> &[String] {
        self.adjacency.get(node_id).map(|v| v.as_slice()).unwrap_or(&[])
    }
    
    pub fn get_predecessors(&self, node_id: &str) -> &[String] {
        self.reverse_adjacency.get(node_id).map(|v| v.as_slice()).unwrap_or(&[])
    }
    
    pub fn has_node(&self, node_id: &str) -> bool {
        self.nodes.contains(node_id)
    }
}
```

### 2. Cycle Detection

**Detect circular dependencies**:
```rust
impl DirectedGraph {
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
    
    /// Find all cycles in the graph
    pub fn find_cycles(&self) -> Vec<Vec<String>> {
        // Implementation for finding actual cycle paths
        // Returns list of node sequences that form cycles
        vec![] // Placeholder
    }
}
```

### 3. Partial Execution

**Execute only a subgraph**:
```rust
impl DirectedGraph {
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
    
    /// Get subgraph needed to execute a specific node
    pub fn get_execution_subgraph(&self, target_node: &str) -> HashSet<String> {
        let mut subgraph = self.get_ancestors(target_node);
        subgraph.insert(target_node.to_string());
        subgraph
    }
    
    /// Get all descendants of a node
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
}
```

### 4. Execution Order Optimization

**Topological sort for optimal execution**:
```rust
impl DirectedGraph {
    /// Get topological sort of nodes (execution order)
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
        
        // Process nodes
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
        
        if result.len() != self.nodes.len() {
            return Err(WorkflowError::CircularDependency);
        }
        
        Ok(result)
    }
}
```

### 5. Workflow Integration

**Add graph methods to Workflow**:
```rust
impl Workflow {
    pub fn build_graph(&self) -> DirectedGraph {
        DirectedGraph::from_workflow(self)
    }
    
    pub fn validate_no_cycles(&self) -> Result<(), WorkflowError> {
        let graph = self.build_graph();
        if graph.has_cycle() {
            Err(WorkflowError::CircularDependency)
        } else {
            Ok(())
        }
    }
    
    pub fn get_execution_order(&self) -> Result<Vec<String>, WorkflowError> {
        let graph = self.build_graph();
        graph.topological_sort()
    }
    
    pub fn get_partial_execution_nodes(&self, target_node: &str) -> HashSet<String> {
        let graph = self.build_graph();
        graph.get_execution_subgraph(target_node)
    }
}
```

### 6. Engine Updates

**Use graph for execution**:
```rust
impl WorkflowEngine {
    pub async fn execute_workflow_optimized(
        &self,
        workflow: &Workflow,
        execution_id: &str,
        context: &mut ExecutionContext,
        target_node: Option<&str>,
    ) -> Result<ExecutionStatus, WorkflowError> {
        // Validate no cycles
        workflow.validate_no_cycles()?;
        
        // Get execution order
        let execution_order = if let Some(target) = target_node {
            // Partial execution - only nodes needed for target
            let subgraph = workflow.get_partial_execution_nodes(target);
            let full_order = workflow.get_execution_order()?;
            full_order.into_iter()
                .filter(|n| subgraph.contains(n))
                .collect()
        } else {
            // Full execution
            workflow.get_execution_order()?
        };
        
        // Execute nodes in optimal order
        for node_id in execution_order {
            let node = workflow.get_node(&node_id)
                .ok_or_else(|| WorkflowError::NodeNotFound(node_id.clone()))?;
            
            // Skip disabled nodes
            if node.disabled.unwrap_or(false) {
                continue;
            }
            
            // Execute node
            self.execute_node_with_retry(node, context).await;
        }
        
        Ok(ExecutionStatus::Completed)
    }
}
```

## Implementation Steps

### Step 1: DirectedGraph Structure (45 min)
- [ ] Create `graph.rs` module in `automation/`
- [ ] Implement `DirectedGraph` struct
- [ ] Add `from_workflow` constructor
- [ ] Implement successor/predecessor getters
- [ ] Add tests for graph construction

### Step 2: Cycle Detection (30 min)
- [ ] Implement `has_cycle` method
- [ ] Add DFS-based cycle detection
- [ ] Implement `find_cycles` for detailed reporting
- [ ] Add tests for cycle detection

### Step 3: Partial Execution (30 min)
- [ ] Implement `get_ancestors` method
- [ ] Implement `get_descendants` method
- [ ] Add `get_execution_subgraph` method
- [ ] Add tests for subgraph extraction

### Step 4: Topological Sort (30 min)
- [ ] Implement `topological_sort` method
- [ ] Use Kahn's algorithm for sorting
- [ ] Handle cycle detection in sort
- [ ] Add tests for execution order

### Step 5: Workflow Integration (30 min)
- [ ] Add `build_graph` to Workflow
- [ ] Add `validate_no_cycles` method
- [ ] Add `get_execution_order` method
- [ ] Add `get_partial_execution_nodes` method

### Step 6: Engine Optimization (45 min)
- [ ] Add `execute_workflow_optimized` method
- [ ] Support partial execution parameter
- [ ] Use topological order for execution
- [ ] Emit graph analysis events

### Step 7: Testing (60 min)
- [ ] Test cycle detection with circular workflows
- [ ] Test partial execution
- [ ] Test execution order optimization
- [ ] Test complex workflow graphs
- [ ] Performance testing with large graphs

## Files to Modify/Create

### Backend
1. **`src-tauri/src/automation/graph.rs`** (NEW)
   - DirectedGraph implementation
   - Cycle detection
   - Topological sort
   - Graph analysis methods

2. **`src-tauri/src/automation.rs`**
   - Add `pub mod graph;`
   - Add graph-related methods to Workflow

3. **`src-tauri/src/automation/engine.rs`**
   - Add optimized execution method
   - Use graph for execution planning

4. **`src-tauri/src/lib.rs`**
   - Expose new execution methods if needed

## Success Criteria

- [ ] DirectedGraph correctly represents workflows
- [ ] Cycle detection prevents infinite loops
- [ ] Partial execution runs only necessary nodes
- [ ] Topological sort optimizes execution order
- [ ] Complex workflows execute correctly
- [ ] Performance is acceptable for large graphs
- [ ] Tests pass for all graph operations

## Estimated Time

**Total**: ~4 hours
- Graph structure: 1.25 hours
- Workflow integration: 0.5 hours
- Engine updates: 0.75 hours
- Testing: 1.5 hours

## Benefits of Phase 3

1. **Safety** - Cycle detection prevents infinite loops
2. **Efficiency** - Optimal execution order
3. **Flexibility** - Partial execution support
4. **Analysis** - Understand workflow dependencies
5. **Debugging** - Better error messages for graph issues

## Next Phase Preview

**Phase 4** will build on this by:
- Adding rich execution context with helper functions
- Implementing expression evaluation
- Adding data transformation helpers
- Supporting binary data handling
