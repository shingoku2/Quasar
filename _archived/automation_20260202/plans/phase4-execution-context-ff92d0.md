# Phase 4: Rich Execution Context Implementation

Enhance ProjectTitan's execution context with powerful helper functions, expression evaluation, data transformation utilities, and advanced features inspired by n8n's context-based execution model.

## Overview

This phase adds sophisticated execution context capabilities that enable:
- **Helper functions** for data manipulation
- **Expression evaluation** for dynamic values
- **Data transformation** utilities
- **JSON path queries** for data extraction
- **Environment variable access**
- **Workflow metadata** access

## Current State Analysis

**Current ExecutionContext** (`src-tauri/src/automation.rs:217-243`):
```rust
pub struct ExecutionContext {
    pub variables: HashMap<String, String>,
    pub results: HashMap<String, NodeResult>,
}
```

**Limitations**:
- Only stores simple string variables
- No helper functions
- No expression evaluation
- No data transformation
- No JSON support
- No metadata access

## Proposed Changes

### 1. Enhanced ExecutionContext

**Add rich context data**:
```rust
use serde_json::Value as JsonValue;

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    // Simple key-value variables
    pub variables: HashMap<String, String>,
    // Node execution results
    pub results: HashMap<String, NodeResult>,
    // JSON data storage
    pub json_data: HashMap<String, JsonValue>,
    // Workflow metadata
    pub workflow_id: String,
    pub execution_id: String,
    pub execution_mode: String,
    // Environment variables (read-only)
    pub env: HashMap<String, String>,
    // Execution start time
    pub started_at: chrono::DateTime<chrono::Utc>,
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
}
```

### 2. Helper Functions

**Data access helpers**:
```rust
impl ExecutionContext {
    /// Get variable with default value
    pub fn get_variable_or(&self, key: &str, default: &str) -> String {
        self.variables.get(key)
            .map(|s| s.as_str())
            .unwrap_or(default)
            .to_string()
    }
    
    /// Get JSON data by path (e.g., "user.name")
    pub fn get_json(&self, key: &str, path: &str) -> Option<JsonValue> {
        let data = self.json_data.get(key)?;
        self.json_path_query(data, path)
    }
    
    /// Set JSON data
    pub fn set_json(&mut self, key: impl Into<String>, value: JsonValue) {
        self.json_data.insert(key.into(), value);
    }
    
    /// Get output from a previous node
    pub fn get_node_output(&self, node_id: &str) -> Option<String> {
        match self.results.get(node_id)? {
            NodeResult::Success { output } => output.clone(),
            _ => None,
        }
    }
    
    /// Check if a node succeeded
    pub fn node_succeeded(&self, node_id: &str) -> bool {
        matches!(
            self.results.get(node_id),
            Some(NodeResult::Success { .. })
        )
    }
    
    /// Get environment variable
    pub fn get_env(&self, key: &str) -> Option<&str> {
        self.env.get(key).map(|s| s.as_str())
    }
    
    /// Get execution duration so far
    pub fn execution_duration(&self) -> chrono::Duration {
        chrono::Utc::now() - self.started_at
    }
}
```

### 3. Expression Evaluation

**Simple expression parser**:
```rust
impl ExecutionContext {
    /// Evaluate an expression with variable substitution
    /// Supports: ${variable}, ${node.output}, ${env.VAR}
    pub fn evaluate_expression(&self, expr: &str) -> String {
        let mut result = expr.to_string();
        
        // Replace ${variable} with values
        let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
        
        for cap in re.captures_iter(expr) {
            let full_match = &cap[0];
            let var_path = &cap[1];
            
            let value = self.resolve_variable_path(var_path);
            result = result.replace(full_match, &value);
        }
        
        result
    }
    
    /// Resolve a variable path like "node.output" or "env.HOME"
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
    
    /// Evaluate a boolean expression
    pub fn evaluate_condition(&self, expr: &str) -> bool {
        let expanded = self.evaluate_expression(expr);
        
        // Simple boolean evaluation
        match expanded.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" => true,
            "false" | "0" | "no" | "" => false,
            _ => {
                // Try to evaluate comparisons
                self.evaluate_comparison(&expanded)
            }
        }
    }
    
    fn evaluate_comparison(&self, expr: &str) -> bool {
        // Simple comparison operators: ==, !=, >, <, >=, <=
        if let Some((left, right)) = expr.split_once("==") {
            return left.trim() == right.trim();
        }
        if let Some((left, right)) = expr.split_once("!=") {
            return left.trim() != right.trim();
        }
        // Add more operators as needed
        false
    }
}
```

### 4. JSON Path Queries

**Navigate JSON data**:
```rust
impl ExecutionContext {
    /// Query JSON data using simple path syntax
    /// Examples: "user.name", "items[0].id", "data.users[*].email"
    fn json_path_query(&self, data: &JsonValue, path: &str) -> Option<JsonValue> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = data.clone();
        
        for part in parts {
            // Handle array indexing: items[0]
            if let Some((key, index_str)) = part.split_once('[') {
                let index_str = index_str.trim_end_matches(']');
                
                // Navigate to the key first
                if !key.is_empty() {
                    current = current.get(key)?.clone();
                }
                
                // Then index into array
                if index_str == "*" {
                    // Return all items (not implemented in simple version)
                    return Some(current);
                } else if let Ok(index) = index_str.parse::<usize>() {
                    current = current.get(index)?.clone();
                }
            } else {
                // Simple object property access
                current = current.get(part)?.clone();
            }
        }
        
        Some(current)
    }
}
```

### 5. Data Transformation Helpers

**Common transformations**:
```rust
impl ExecutionContext {
    /// Parse JSON string into JsonValue
    pub fn parse_json(&self, json_str: &str) -> Result<JsonValue, serde_json::Error> {
        serde_json::from_str(json_str)
    }
    
    /// Convert JsonValue to pretty string
    pub fn stringify_json(&self, value: &JsonValue) -> String {
        serde_json::to_string_pretty(value).unwrap_or_default()
    }
    
    /// Merge two JSON objects
    pub fn merge_json(&self, base: &JsonValue, overlay: &JsonValue) -> JsonValue {
        match (base, overlay) {
            (JsonValue::Object(base_map), JsonValue::Object(overlay_map)) => {
                let mut result = base_map.clone();
                for (key, value) in overlay_map {
                    result.insert(key.clone(), value.clone());
                }
                JsonValue::Object(result)
            }
            _ => overlay.clone(),
        }
    }
    
    /// Extract values from array of objects
    pub fn pluck(&self, array: &JsonValue, key: &str) -> Vec<JsonValue> {
        if let JsonValue::Array(items) = array {
            items.iter()
                .filter_map(|item| item.get(key).cloned())
                .collect()
        } else {
            vec![]
        }
    }
}
```

### 6. Engine Integration

**Update engine to use rich context**:
```rust
impl WorkflowEngine {
    pub async fn execute_workflow(
        &self,
        workflow: &Workflow,
        execution_id: &str,
        context: &mut ExecutionContext,
    ) -> Result<ExecutionStatus, WorkflowError> {
        // Context is now initialized with workflow and execution IDs
        // All helper functions are available during execution
        
        // ... execution logic ...
    }
}
```

## Implementation Steps

### Step 1: Enhanced ExecutionContext (45 min)
- [ ] Add new fields to ExecutionContext
- [ ] Update constructor with workflow/execution IDs
- [ ] Add environment variable collection
- [ ] Add execution metadata
- [ ] Update tests

### Step 2: Helper Functions (45 min)
- [ ] Implement `get_variable_or`
- [ ] Implement `get_node_output`
- [ ] Implement `node_succeeded`
- [ ] Implement `get_env`
- [ ] Implement `execution_duration`
- [ ] Add JSON data methods

### Step 3: Expression Evaluation (60 min)
- [ ] Add regex dependency to Cargo.toml
- [ ] Implement `evaluate_expression`
- [ ] Implement `resolve_variable_path`
- [ ] Implement `evaluate_condition`
- [ ] Implement `evaluate_comparison`
- [ ] Add tests for expressions

### Step 4: JSON Support (45 min)
- [ ] Implement `json_path_query`
- [ ] Implement `parse_json`
- [ ] Implement `stringify_json`
- [ ] Implement `merge_json`
- [ ] Implement `pluck`
- [ ] Add tests for JSON operations

### Step 5: Engine Updates (30 min)
- [ ] Update ExecutionContext initialization
- [ ] Pass workflow_id and execution_id
- [ ] Update condition evaluation to use new methods
- [ ] Update variable expansion in engine

### Step 6: Testing (60 min)
- [ ] Test variable substitution
- [ ] Test expression evaluation
- [ ] Test JSON path queries
- [ ] Test data transformations
- [ ] Test environment variable access
- [ ] Integration tests with workflows

## Files to Modify

### Backend
1. **`src-tauri/Cargo.toml`**
   - Add `regex` dependency
   - Add `serde_json` (already present)

2. **`src-tauri/src/automation.rs`**
   - Enhance ExecutionContext struct
   - Add all helper methods
   - Add expression evaluation
   - Add JSON utilities

3. **`src-tauri/src/automation/engine.rs`**
   - Update context initialization
   - Use new helper methods
   - Update condition evaluation

## Success Criteria

- [ ] ExecutionContext has rich metadata
- [ ] Helper functions work correctly
- [ ] Expression evaluation supports ${variable} syntax
- [ ] JSON path queries work
- [ ] Data transformations function properly
- [ ] Environment variables accessible
- [ ] Backward compatibility maintained
- [ ] Tests pass for all features

## Estimated Time

**Total**: ~4.5 hours
- Context enhancement: 1.5 hours
- Expression evaluation: 1 hour
- JSON support: 0.75 hours
- Engine integration: 0.5 hours
- Testing: 0.75 hours

## Benefits of Phase 4

1. **Power** - Rich helper functions for nodes
2. **Flexibility** - Dynamic expressions
3. **Data Handling** - JSON manipulation
4. **Metadata** - Access to workflow context
5. **Environment** - Read environment variables

## Next Phase Preview

**Phase 5** will build on this by:
- Adding workflow settings and static data
- Implementing pin data for testing
- Adding workflow-level features
- Quality of life improvements
