---
name: agent_manager
description: "Manages worker task allocation and coordination. Reads orchestrator state, finds ready tasks, assigns them to idle workers, respects parallelism limits, and updates state file."
---

# Agent Manager

The Agent Manager is responsible for allocating tasks to available workers and coordinating parallel execution.

---

## The Job

1. Read current orchestrator state
2. Find tasks that are ready (dependencies satisfied)
3. Find idle workers (or spawn new ones up to limit)
4. Assign ready tasks to idle workers
5. Update state file with assignments
6. Monitor workers and handle completions

---

## Step 1: Read Current State

Load the orchestrator state from `state/orchestrator_state.json`:

```typescript
import { readStateFile, getOrchestratorStatePath } from '../agents/base_agent';
import { OrchestratorState } from '../schemas/types';

const state = await readStateFile<OrchestratorState>(getOrchestratorStatePath());
```

---

## Step 2: Check for Ready Tasks

Use the dependency resolver to find tasks ready for assignment:

```typescript
import { getReadyTasks, getParallelizableTasks } from '../tasks/dependency_resolver';

// Get all pending tasks with satisfied dependencies
const readyTasks = getReadyTasks(state);

// Filter to tasks that can run in parallel with currently running tasks
const runningTaskIds = state.agents
  .filter(a => a.status === 'busy' && a.current_task)
  .map(a => a.current_task!);

const parallelizableTasks = getParallelizableTasks(runningTaskIds, state);
```

---

## Step 3: Find Available Workers

Check for idle workers or determine if new ones can be spawned:

```typescript
import { loadConfig } from '../config/orchestrator_config';

const config = await loadConfig();
const maxWorkers = config.max_parallel_workers; // default: 3

// Find idle workers
const idleWorkers = state.agents.filter(
  a => a.type === 'worker' && a.status === 'idle'
);

// Count busy workers
const busyWorkers = state.agents.filter(
  a => a.type === 'worker' && a.status === 'busy'
);

const canSpawnMore = busyWorkers.length < maxWorkers;
const slotsAvailable = maxWorkers - busyWorkers.length;
```

---

## Step 4: Assign Tasks to Workers

For each available slot and ready task, make an assignment:

```typescript
import { assignTask } from '../agents/agent_manager';

const assignments: TaskAssignment[] = [];

for (let i = 0; i < Math.min(readyTasks.length, slotsAvailable); i++) {
  const task = readyTasks[i];
  const worker = idleWorkers[i] || await spawnNewWorker(state);

  const assignment = await assignTask(state, task.id, worker.id);
  assignments.push(assignment);
}
```

---

## Step 5: Update State File

Write the updated state with new assignments:

```typescript
import { writeStateFile } from '../agents/base_agent';

// Update state with assignments
for (const assignment of assignments) {
  // Update task status
  const taskIndex = state.tasks.findIndex(t => t.id === assignment.task_id);
  state.tasks[taskIndex].status = 'assigned';
  state.tasks[taskIndex].assigned_agent = assignment.agent_id;
  state.tasks[taskIndex].started_at = assignment.assigned_at;

  // Update worker status
  const workerIndex = state.agents.findIndex(a => a.id === assignment.agent_id);
  state.agents[workerIndex].status = 'busy';
  state.agents[workerIndex].current_task = assignment.task_id;
}

state.updated_at = new Date().toISOString();
await writeStateFile(getOrchestratorStatePath(), state);
```

---

## Allocation Logic Rules

The agent manager follows these rules when allocating tasks:

### 1. Respect Dependency Order
- Only assign tasks whose dependencies are all completed
- Use `getReadyTasks()` from dependency resolver

### 2. Respect Parallelism Limits
- Never exceed `max_parallel_workers` (default: 3)
- Count busy workers before spawning new ones

### 3. Avoid Conflicts
- Use `canRunInParallel()` to check for file conflicts
- Don't assign conflicting tasks simultaneously

### 4. Prefer Existing Workers
- Reuse idle workers before spawning new ones
- Workers maintain context from previous tasks

### 5. Phase Awareness
- Prioritize tasks from the current phase
- Tasks in earlier phases should complete first

---

## State Updates

### On Task Assignment

```json
{
  "tasks": [{
    "id": "US-005",
    "status": "assigned",
    "assigned_agent": "worker_abc123",
    "started_at": "2024-01-15T10:30:00Z"
  }],
  "agents": [{
    "id": "worker_abc123",
    "type": "worker",
    "status": "busy",
    "current_task": "US-005",
    "last_activity": "2024-01-15T10:30:00Z"
  }]
}
```

### On Task Completion (via completion reporter)

```json
{
  "tasks": [{
    "id": "US-005",
    "status": "completed",
    "completed_at": "2024-01-15T10:45:00Z",
    "iteration_count": 4
  }],
  "agents": [{
    "id": "worker_abc123",
    "status": "idle",
    "current_task": null
  }]
}
```

---

## Output Messages

### When Assigning Tasks
```
Agent Manager: Allocating tasks...

Ready tasks: 5
Idle workers: 1
Busy workers: 2
Available slots: 1

Assignments:
  - US-007 -> worker_abc123

State updated.
```

### When No Tasks Ready
```
Agent Manager: No tasks ready for assignment.

Pending tasks: 8
Blocked by dependencies: 8
In progress: 3

Waiting for workers to complete current tasks...
```

### When All Workers Busy
```
Agent Manager: All worker slots occupied.

Ready tasks: 4
Busy workers: 3 (max reached)

Waiting for workers to complete current tasks...
```

---

## Error Handling

- **No state file**: Error and suggest running orchestrator first
- **Circular dependency**: Error with cycle path details
- **All tasks completed**: Return success, signal project completion
- **All tasks blocked**: Error with blocked task details
- **Worker spawn failure**: Log error, try next available slot

---

## Configuration

Respects settings from `orchestrator.config.json`:

```json
{
  "max_parallel_workers": 3,
  "max_iterations_per_task": 20,
  "retry_limit": 3
}
```

---

## Checklist

- [ ] Read orchestrator state
- [ ] Identified ready tasks (dependencies satisfied)
- [ ] Found idle workers or determined spawn capacity
- [ ] Made assignments up to max_parallel_workers limit
- [ ] Updated task statuses to 'assigned'
- [ ] Updated worker statuses to 'busy'
- [ ] Wrote updated state file
- [ ] Logged allocation summary
