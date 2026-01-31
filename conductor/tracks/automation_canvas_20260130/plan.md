# Automation Canvas Track

Visual workflow builder for creating automated tasks and orchestration flows. Drag-and-drop interface for building multi-step automation with triggers, actions, and conditions.

## Phases

### Phase 1: Workflow Engine Backend (Rust)

- [ ] Task: Create `automation.rs` module with workflow data structures
- [ ] Task: Define Workflow, Node, Connection, and ExecutionContext structs
- [ ] Task: Implement workflow execution engine with topological sorting
- [ ] Task: Create action handlers for SSH command execution
- [ ] Task: Create action handlers for file transfer (SFTP/SCP)
- [ ] Task: Create notification action handlers (in-app, system notifications)
- [ ] Task: Add Tauri commands for CRUD operations on workflows
- [ ] Task: Write unit tests for workflow execution engine
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Workflow Engine' (Protocol in workflow.md) [checkpoint: TBD]

### Phase 2: Canvas Component Foundation

- [ ] Task: Create `Canvas.tsx` component with SVG-based canvas
- [ ] Task: Implement pan and zoom functionality
- [ ] Task: Add grid background with snap-to-grid option
- [ ] Task: Implement node selection and multi-select
- [ ] Task: Create node palette sidebar with draggable node types
- [ ] Task: Add connection drawing between node ports
- [ ] Task: Implement delete node and connection functionality
- [ ] Task: Write component tests for Canvas
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Canvas Foundation' (Protocol in workflow.md) [checkpoint: TBD]

### Phase 3: Node Types and Properties

- [ ] Task: Create TriggerNode component (manual, scheduled, webhook)
- [ ] Task: Create ActionNode component (SSH command, file transfer)
- [ ] Task: Create ConditionNode component (if/else branching)
- [ ] Task: Create NotificationNode component
- [ ] Task: Implement PropertiesPanel for editing node configuration
- [ ] Task: Add node-specific forms (SSH credentials, command input, schedule config)
- [ ] Task: Create NodeToolbar with quick actions
- [ ] Task: Write tests for node components and properties
- [ ] Task: Conductor - User Manual Verification 'Phase 3: Node Types' (Protocol in workflow.md) [checkpoint: TBD]

### Phase 4: Workflow Execution and History

- [ ] Task: Create WorkflowExecution service for running workflows
- [ ] Task: Implement execution status tracking (pending, running, completed, failed)
- [ ] Task: Add execution history viewer with logs
- [ ] Task: Create real-time execution progress updates
- [ ] Task: Add workflow run button with execution confirmation
- [ ] Task: Implement workflow pause/resume functionality
- [ ] Task: Add execution error handling and retry logic
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Execution' (Protocol in workflow.md) [checkpoint: TBD]

### Phase 5: Triggers and Scheduling

- [ ] Task: Implement scheduled trigger with cron expression support
- [ ] Task: Create webhook trigger with endpoint generation
- [ ] Task: Add manual trigger button in UI
- [ ] Task: Implement trigger event queue and processing
- [ ] Task: Add trigger history and statistics
- [ ] Task: Create webhook URL management UI
- [ ] Task: Conductor - User Manual Verification 'Phase 5: Triggers' (Protocol in workflow.md) [checkpoint: TBD]

### Phase 6: Integration and Polish

- [ ] Task: Integrate Automation Canvas into main navigation
- [ ] Task: Add workflow import/export (JSON format)
- [ ] Task: Create sample workflow templates
- [ ] Task: Implement workflow validation before execution
- [ ] Task: Add keyboard shortcuts for common actions
- [ ] Task: Final integration testing
- [ ] Task: Commit all changes with checkpoint
- [ ] Task: Conductor - User Manual Verification 'Phase 6: Integration' (Protocol in workflow.md) [checkpoint: TBD]

## Success Metrics

- Canvas performance: Support 50+ nodes without lag
- Workflow execution: < 100ms overhead per node
- Code coverage > 80% for automation engine
- Support for: SSH commands, file transfer, notifications, conditions
- Real-time execution updates via Tauri events

## Dependencies

- @xyflow/react for canvas foundation (or custom SVG implementation)
- node-cron or similar for scheduling (Rust backend)
- Existing SSH module for command execution
- Existing notification system

## Risks and Mitigations

- **Risk**: Complex workflow execution logic
  - **Mitigation**: Start with linear workflows, add branching later
- **Risk**: Canvas performance with many nodes
  - **Mitigation**: Virtual rendering, optimize re-renders
- **Risk**: State management complexity
  - **Mitigation**: Use Zustand or Redux Toolkit for canvas state

## Output Files

- `src-tauri/src/automation.rs` - Workflow engine
- `src-tauri/src/automation/engine.rs` - Execution engine
- `src/components/automation/Canvas.tsx` - Main canvas component
- `src/components/automation/nodes/` - Node type components
- `src/components/automation/PropertiesPanel.tsx` - Node configuration
- `src/components/automation/ExecutionHistory.tsx` - Run history viewer
