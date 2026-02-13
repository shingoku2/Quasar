# Phase 2: Connection Enhancements Implementation

Enhance Quasar's connection model with typed connections, multiple ports, indices, and bidirectional indexing inspired by n8n's sophisticated connection system.

## Overview

This phase adds advanced connection capabilities that enable:
- **Typed connections** (main, error, ai, etc.)
- **Multiple inputs/outputs** per node with indices
- **Port-based routing** for different data flows
- **Bidirectional indexing** for efficient graph traversal
- **Error branch routing** using error output type

## Current State Analysis

**Current Connection Structure** (`src-tauri/src/automation.rs:106-113`):
```rust
pub struct Connection {
    pub id: String,
    pub source_node: String,
    pub source_port: String,
    pub target_node: String,
    pub target_port: String,
}
```

**Limitations**:
- Ports are just strings, no type safety
- No support for multiple connections of same type
- No indices for multiple inputs/outputs
- No bidirectional lookup optimization
- Error routing not implemented

## Proposed Changes

### 1. Connection Type Enum

**New enum for connection types**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionType {
    Main,      // Regular data flow
    Error,     // Error handling branch
    Ai,        // AI/LangChain data
    Custom(String), // Extensible for future types
}

impl Default for ConnectionType {
    fn default() -> Self {
        ConnectionType::Main
    }
}
```

### 2. Enhanced Connection Structure

**Updated Connection with types and indices**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub source_node: String,
    pub source_port: String,
    pub source_index: usize,        // NEW: Output index (0, 1, 2...)
    pub target_node: String,
    pub target_port: String,
    pub target_index: usize,        // NEW: Input index (0, 1, 2...)
    #[serde(default)]
    pub connection_type: ConnectionType, // NEW: Type of connection
}
```

### 3. Bidirectional Connection Index

**New helper struct for efficient lookups**:
```rust
pub struct ConnectionIndex {
    // Outgoing: source_node -> list of connections
    outgoing: HashMap<String, Vec<Connection>>,
    // Incoming: target_node -> list of connections
    incoming: HashMap<String, Vec<Connection>>,
    // By type: (node_id, type) -> connections
    by_type: HashMap<(String, ConnectionType), Vec<Connection>>,
}

impl ConnectionIndex {
    pub fn new(connections: &[Connection]) -> Self {
        let mut outgoing = HashMap::new();
        let mut incoming = HashMap::new();
        let mut by_type = HashMap::new();
        
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
```

### 4. Workflow Enhancement

**Add connection index to Workflow**:
```rust
impl Workflow {
    pub fn build_connection_index(&self) -> ConnectionIndex {
        ConnectionIndex::new(&self.connections)
    }
    
    pub fn get_outgoing_by_type(&self, node_id: &str, conn_type: &ConnectionType) -> Vec<&Connection> {
        self.connections.iter()
            .filter(|c| c.source_node == node_id && &c.connection_type == conn_type)
            .collect()
    }
    
    pub fn get_error_connections(&self, node_id: &str) -> Vec<&Connection> {
        self.get_outgoing_by_type(node_id, &ConnectionType::Error)
    }
}
```

### 5. Engine Error Routing

**Update engine to use error connections**:
```rust
// In execute_node_recursive, after error handling:
if let NodeResult::Failure { error } = &result {
    let error_behavior = node.on_error.as_ref().unwrap_or(&ErrorBehavior::StopWorkflow);
    
    match error_behavior {
        ErrorBehavior::ContinueErrorOutput => {
            // Route to error branch
            let error_connections = workflow.get_error_connections(node_id);
            for conn in error_connections {
                Box::pin(self.execute_node_recursive(
                    workflow, 
                    &conn.target_node, 
                    execution_id, 
                    context
                )).await?;
            }
            return Ok(ExecutionStatus::Completed);
        }
        // ... other behaviors
    }
}
```

### 6. Frontend Updates

**Canvas.tsx connection interface**:
```typescript
export interface ConnectionData {
  id: string;
  source: string;
  target: string;
  sourcePort?: string;
  targetPort?: string;
  sourceIndex?: number;     // NEW
  targetIndex?: number;     // NEW
  connectionType?: 'main' | 'error' | 'ai' | string; // NEW
}
```

**Visual differentiation**:
- Main connections: Blue/default color
- Error connections: Red color
- AI connections: Purple color

## Implementation Steps

### Step 1: Backend Connection Types (30 min)
- [ ] Add `ConnectionType` enum to `automation.rs`
- [ ] Update `Connection` struct with new fields
- [ ] Add default implementations
- [ ] Update tests to use new structure

### Step 2: Bidirectional Index (30 min)
- [ ] Add `ConnectionIndex` struct to `automation.rs`
- [ ] Implement index building methods
- [ ] Add helper methods to `Workflow`
- [ ] Add tests for index lookups

### Step 3: Engine Error Routing (30 min)
- [ ] Update `execute_node_recursive` to check error connections
- [ ] Route to error branch when `ContinueErrorOutput` is set
- [ ] Emit events for error branch routing
- [ ] Test error routing logic

### Step 4: Frontend Connection Model (30 min)
- [ ] Update `ConnectionData` interface in `Canvas.tsx`
- [ ] Update connection serialization/deserialization
- [ ] Add connection type to connection creation
- [ ] Update workflow loading to include new fields

### Step 5: Visual Differentiation (45 min)
- [ ] Add color coding for connection types in `Canvas.tsx`
- [ ] Update `renderConnection` to use different colors
- [ ] Add connection type selector (optional)
- [ ] Update connection hover/select states

### Step 6: Testing (45 min)
- [ ] Test main connections work as before
- [ ] Test error connections route correctly
- [ ] Test multiple inputs/outputs with indices
- [ ] Test bidirectional index performance
- [ ] Test backward compatibility

## Files to Modify

### Backend
1. **`src-tauri/src/automation.rs`**
   - Add `ConnectionType` enum (after line 81)
   - Update `Connection` struct (lines 106-113)
   - Add `ConnectionIndex` struct (new, after Connection)
   - Add Workflow helper methods (around line 149)
   - Update tests (lines 443-547)

2. **`src-tauri/src/automation/engine.rs`**
   - Update error routing logic (around line 92-119)
   - Use typed connections for routing (around line 131)

### Frontend
3. **`src/components/automation/Canvas.tsx`**
   - Update `ConnectionData` interface (lines 32-38)
   - Update connection rendering (around line 475)
   - Add color coding logic
   - Update serialization (lines 469-475)

## Success Criteria

- [ ] Connections support types (main, error, ai)
- [ ] Multiple connections of same type supported via indices
- [ ] Error connections route to error branches
- [ ] Bidirectional index provides fast lookups
- [ ] Visual differentiation of connection types
- [ ] Backward compatibility maintained
- [ ] Tests pass for all connection types
- [ ] Performance is acceptable with many connections

## Estimated Time

**Total**: ~3.5 hours
- Backend: 1.5 hours
- Frontend: 1.5 hours
- Testing: 0.5 hours

## Next Phase Preview

**Phase 3** will build on this by:
- Implementing DirectedGraph for workflow analysis
- Adding partial execution (run specific subgraphs)
- Detecting and handling cycles
- Optimizing execution order

## Benefits of Phase 2

1. **Error Handling** - Proper error branch routing
2. **Extensibility** - Support for AI/LangChain nodes
3. **Performance** - Fast bidirectional lookups
4. **Flexibility** - Multiple inputs/outputs per node
5. **Visual Clarity** - Color-coded connection types
