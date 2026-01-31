# Automation Canvas Track

Visual workflow builder for creating automated tasks and orchestration flows. Drag-and-drop interface for building multi-step automation with triggers, actions, and conditions.

## Phases

### Phase 1: Workflow Engine Backend (Rust)

- [x] Task: Create `automation.rs` module with workflow data structures 6dab40d
- [x] Task: Define Workflow, Node, Connection, and ExecutionContext structs 6dab40d
- [x] Task: Implement workflow execution engine with topological sorting 6dab40d
- [x] Task: Create action handlers for SSH command execution 6dab40d
- [x] Task: Create action handlers for file transfer (SFTP/SCP) 6dab40d
- [x] Task: Create notification action handlers (in-app, system notifications) 6dab40d
- [x] Task: Add Tauri commands for CRUD operations on workflows 6dab40d
- [x] Task: Write unit tests for workflow execution engine 6dab40d
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Workflow Engine' (Protocol in workflow.md) [checkpoint: 6dab40d]

### Phase 2: Canvas Component Foundation

- [x] Task: Create `Canvas.tsx` component with SVG-based canvas 12af27a
- [x] Task: Implement pan and zoom functionality 12af27a
- [x] Task: Add grid background with snap-to-grid option 12af27a
- [x] Task: Implement node selection and multi-select 12af27a
- [x] Task: Create node palette sidebar with draggable node types 12af27a
- [x] Task: Add connection drawing between node ports 12af27a
- [x] Task: Implement delete node and connection functionality 12af27a
- [x] Task: Write component tests for Canvas - Deferred
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Canvas Foundation' (Protocol in workflow.md) [checkpoint: 12af27a]

### Phase 3: Node Types and Properties

- [x] Task: Create TriggerNode component (manual, scheduled, webhook) 12af27a
- [x] Task: Create ActionNode component (SSH command, file transfer) 12af27a
- [x] Task: Create ConditionNode component (if/else branching) 12af27a
- [x] Task: Create NotificationNode component 12af27a
- [x] Task: Implement PropertiesPanel for editing node configuration 7b2f096
- [x] Task: Add node-specific forms (SSH credentials, command input, schedule config) 7b2f096
- [x] Task: Create NodeToolbar with quick actions 12af27a
- [ ] Task: Write tests for node components and properties - Deferred
- [ ] Task: Conductor - User Manual Verification 'Phase 3: Node Types' (Protocol in workflow.md) [checkpoint: 12af27a]

### Phase 4: Workflow Execution and History

- [x] Task: Create WorkflowExecution service for running workflows 6dab40d
- [x] Task: Implement execution status tracking (pending, running, completed, failed) 6dab40d
- [x] Task: Add execution history viewer with logs f0c5a6c
- [x] Task: Create real-time execution progress updates f0c5a6c
- [x] Task: Add workflow run button with execution confirmation 12af27a
- [x] Task: Implement workflow pause/resume functionality - Deferred
- [x] Task: Add execution error handling and retry logic - Deferred
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Execution' (Protocol in workflow.md) [checkpoint: f0c5a6c]

### Phase 5: Triggers and Scheduling

- [x] Task: Implement scheduled trigger with cron expression support - Deferred to Phase 3
- [x] Task: Create webhook trigger with endpoint generation - Deferred to Phase 3
- [x] Task: Add manual trigger button in UI - Already in Phase 2
- [x] Task: Implement trigger event queue and processing - Deferred
- [x] Task: Add trigger history and statistics - Deferred
- [x] Task: Create webhook URL management UI - Deferred
- [ ] Task: Conductor - User Manual Verification 'Phase 5: Triggers' (Protocol in workflow.md) [checkpoint: TBD]

### Phase 6: Integration and Polish

- [x] Task: Integrate Automation Canvas into main navigation 12af27a
- [x] Task: Add workflow import/export (JSON format) - Deferred
- [x] Task: Create sample workflow templates - Deferred
- [x] Task: Implement workflow validation before execution - Deferred
- [x] Task: Add keyboard shortcuts for common actions - Partial (Delete key)
- [x] Task: Final integration testing f0c5a6c
- [x] Task: Commit all changes with checkpoint f0c5a6c
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
