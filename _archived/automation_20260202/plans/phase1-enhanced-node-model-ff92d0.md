# Phase 1: Enhanced Node Model Implementation

Enhance ProjectTitan's node structure with versioning, flexible parameters, credentials support, and advanced error handling options inspired by n8n's architecture.

## Overview

This phase adds foundational improvements to the `Node` struct that will enable:
- **Node versioning** for backward compatibility
- **Flexible parameters** using JSON for extensibility
- **Credential references** for secure authentication
- **Advanced error handling** (retry, continue-on-fail, error routing)
- **Disabled nodes** without deletion
- **Notes and metadata** for documentation

## Current State Analysis

**Current Node Structure** (`src-tauri/src/automation.rs:68-75`):
```rust
pub struct Node {
    pub id: String,
    pub node_type: NodeType,
    pub name: String,
    pub position: Position,
    pub config: NodeConfig,
}
```

**Limitations**:
- No versioning - breaking changes require migration
- Rigid config enum - hard to extend
- No error handling options
- Can't disable nodes temporarily
- No credential management
- No retry logic

## Proposed Changes

### 1. Enhanced Node Structure

**New fields to add**:
```rust
pub struct Node {
    pub id: String,
    pub node_type: NodeType,
    pub name: String,
    pub type_version: u32,              // NEW: Version for backward compatibility
    pub position: Position,
    pub config: NodeConfig,
    pub disabled: Option<bool>,         // NEW: Disable without deleting
    pub notes: Option<String>,          // NEW: Documentation
    pub notes_in_flow: Option<bool>,    // NEW: Show notes on canvas
    
    // Error handling
    pub on_error: Option<ErrorBehavior>, // NEW: What to do on error
    pub continue_on_fail: Option<bool>,  // NEW: Continue workflow on failure
    pub retry_on_fail: Option<bool>,     // NEW: Retry failed execution
    pub max_tries: Option<u32>,          // NEW: Max retry attempts
    pub wait_between_tries: Option<u32>, // NEW: Delay between retries (ms)
    
    // Credentials
    pub credentials: Option<HashMap<String, CredentialReference>>, // NEW
    
    // Advanced
    pub parameters: Option<serde_json::Value>, // NEW: Flexible params
    pub always_output_data: Option<bool>,      // NEW: Output even if no data
}
```

### 2. New Supporting Types

**ErrorBehavior enum**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorBehavior {
    StopWorkflow,           // Stop entire workflow
    ContinueRegularOutput,  // Continue with empty output
    ContinueErrorOutput,    // Continue and route to error branch
}
```

**CredentialReference struct**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialReference {
    pub id: Option<String>,  // Credential ID from vault
    pub name: String,        // Credential name
}
```

### 3. Backward Compatibility

**Default values for optional fields**:
- `type_version`: defaults to 1
- `disabled`: defaults to false
- `on_error`: defaults to `StopWorkflow`
- `continue_on_fail`: defaults to false
- `retry_on_fail`: defaults to false
- `max_tries`: defaults to 1
- All other `Option` fields default to `None`

**Serialization**: Use `#[serde(default)]` and `#[serde(skip_serializing_if = "Option::is_none")]` to maintain clean JSON

### 4. Engine Updates

**WorkflowEngine changes** (`src-tauri/src/automation/engine.rs`):

1. **Check disabled nodes**:
```rust
// Skip disabled nodes
if node.disabled.unwrap_or(false) {
    return Ok(ExecutionStatus::Cancelled);
}
```

2. **Implement retry logic**:
```rust
let max_tries = node.max_tries.unwrap_or(1);
let wait_ms = node.wait_between_tries.unwrap_or(0);

for attempt in 1..=max_tries {
    let result = self.execute_node_impl(node, context).await;
    
    if matches!(result, NodeResult::Success { .. }) {
        return result;
    }
    
    if attempt < max_tries && node.retry_on_fail.unwrap_or(false) {
        tokio::time::sleep(Duration::from_millis(wait_ms as u64)).await;
        continue;
    }
    
    return result;
}
```

3. **Handle error behavior**:
```rust
match node.on_error.as_ref().unwrap_or(&ErrorBehavior::StopWorkflow) {
    ErrorBehavior::StopWorkflow => return Ok(ExecutionStatus::Failed),
    ErrorBehavior::ContinueRegularOutput => {
        // Continue with empty output
        context.set_result(node_id, NodeResult::Success { output: None });
    }
    ErrorBehavior::ContinueErrorOutput => {
        // Route to error branch (Phase 2 feature)
        context.set_result(node_id, NodeResult::Failure { error });
    }
}
```

### 5. Frontend Updates

**Canvas.tsx changes**:

1. **Update node creation** to include new fields:
```typescript
const newNode: NodeData = {
  id: `${type}-${Date.now()}`,
  type: type,
  position: { x: 100, y: 100 },
  data: {
    label: `New ${type}`,
    config: defaultConfig,
    typeVersion: 1,        // NEW
    disabled: false,       // NEW
    notes: '',            // NEW
    onError: 'stop_workflow', // NEW
    retryOnFail: false,   // NEW
    maxTries: 1,          // NEW
  }
};
```

2. **PropertiesPanel.tsx enhancements**:
- Add "Advanced" section with error handling options
- Add "Notes" text area
- Add "Disabled" toggle
- Add "Retry" configuration
- Add "Credentials" selector (if credentials exist)

### 6. Migration Strategy

**No migration needed** because:
- All new fields are optional with sensible defaults
- Existing workflows will deserialize with defaults
- Frontend will handle missing fields gracefully

**If explicit migration desired**:
```rust
impl Node {
    pub fn migrate_to_v1(&mut self) {
        if self.type_version == 0 {
            self.type_version = 1;
            self.disabled = Some(false);
            self.on_error = Some(ErrorBehavior::StopWorkflow);
        }
    }
}
```

## Implementation Steps

### Step 1: Backend Data Structures (30 min)
- [ ] Add `ErrorBehavior` enum to `automation.rs`
- [ ] Add `CredentialReference` struct to `automation.rs`
- [ ] Update `Node` struct with new fields
- [ ] Add `#[serde(default)]` attributes
- [ ] Update tests to use new structure

### Step 2: Engine Error Handling (45 min)
- [ ] Implement disabled node check in `execute_node_recursive`
- [ ] Add retry logic wrapper around node execution
- [ ] Implement error behavior handling
- [ ] Add logging for retries and error handling
- [ ] Update node execution events to include retry info

### Step 3: Frontend Data Model (30 min)
- [ ] Update `NodeData` interface in `Canvas.tsx`
- [ ] Update node creation to include new fields
- [ ] Update serialization/deserialization logic
- [ ] Ensure backward compatibility with existing workflows

### Step 4: UI Components (60 min)
- [ ] Add "Advanced" section to `PropertiesPanel.tsx`
- [ ] Add error handling dropdown (stop/continue/error-output)
- [ ] Add retry configuration (checkbox, max tries, wait time)
- [ ] Add disabled toggle
- [ ] Add notes text area with toggle for "show in flow"
- [ ] Add credentials selector (placeholder for now)

### Step 5: Testing (45 min)
- [ ] Test node with retry logic
- [ ] Test disabled nodes are skipped
- [ ] Test error behaviors (stop/continue)
- [ ] Test backward compatibility with old workflows
- [ ] Test UI updates and persistence

## Files to Modify

### Backend
1. **`src-tauri/src/automation.rs`**
   - Add `ErrorBehavior` enum (after line 65)
   - Add `CredentialReference` struct (after line 65)
   - Update `Node` struct (lines 68-75)
   - Update tests (lines 393-497)

2. **`src-tauri/src/automation/engine.rs`**
   - Add disabled check (line 59)
   - Add retry logic (around line 73)
   - Add error behavior handling (around line 95)

### Frontend
3. **`src/components/automation/Canvas.tsx`**
   - Update `NodeData` interface (lines 10-18)
   - Update node creation (various locations)
   - Update serialization (lines 200-350)

4. **`src/components/automation/PropertiesPanel.tsx`**
   - Add Advanced section
   - Add error handling controls
   - Add retry configuration
   - Add notes and disabled toggle

## Success Criteria

- [ ] Nodes support versioning (`type_version` field)
- [ ] Nodes can be disabled without deletion
- [ ] Retry logic works (configurable attempts and delays)
- [ ] Error behaviors work (stop/continue/error-output)
- [ ] Notes can be added to nodes
- [ ] Credential references can be attached (structure only)
- [ ] Existing workflows load without errors
- [ ] UI exposes all new features
- [ ] Tests pass for all new functionality

## Estimated Time

**Total**: ~3.5 hours
- Backend: 1.25 hours
- Frontend: 1.5 hours  
- Testing: 0.75 hours

## Next Phase Preview

**Phase 2** will build on this foundation by:
- Adding connection types (main, error, ai)
- Implementing error output routing
- Supporting multiple inputs/outputs per node
- Building bidirectional connection index

## Questions Before Starting

None - all requirements are clear from user responses. Ready to implement.
