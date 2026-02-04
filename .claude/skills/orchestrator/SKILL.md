---
name: orchestrate
description: "Start or continue a multi-agent orchestrated project. Manages parallel workers, PRD discussions, and task execution. Use to start new projects or resume existing ones."
---

# Ralph Orchestrator

Multi-agent orchestration system that manages parallel workers to build projects from PRDs.

---

## The Job

1. Ask user: "Start new project or continue existing?"
2. If **new project**: Trigger PRD discussion flow
3. If **existing project**: Show inventory, let user pick
4. Initialize orchestrator state for selected project
5. Begin orchestration (or hand off to agent manager)

---

## Step 1: Project Selection

Ask the user:

```
What would you like to do?

A. Start a new project (creates PRD first)
B. Continue an existing project
C. View project status
```

---

## Step 2A: New Project Flow

If user selects **A (new project)**:

1. Ask for project name (lowercase, underscores, no spaces)
2. Trigger the PRD skill (`/prd`) to create PRD
3. After PRD is complete, add project to inventory
4. Initialize state file at `state/orchestrator_state.json`
5. Return to orchestration ready state

### Create Project Entry

```typescript
// Add to inventory/projects.json
{
  "id": "<project_name>",
  "name": "<Project Name>",
  "prd_path": "PRD_<project_name>.md",
  "status": "new",
  "created_at": "<timestamp>",
  "updated_at": "<timestamp>",
  "state_path": "state/"
}
```

---

## Step 2B: Continue Existing Project Flow

If user selects **B (continue existing)**:

1. Read `src/ralph_orchestrator/inventory/projects.json`
2. Display project list with status:

```
Available Projects:

1. my_app (in_progress) - Last updated: 2024-01-15
   Progress: 5/15 tasks complete

2. user_auth (paused) - Last updated: 2024-01-10
   Progress: 12/20 tasks complete

3. api_gateway (new) - Created: 2024-01-14
   Progress: 0/8 tasks complete

Enter number to continue, or 'back' to go back:
```

3. Load selected project's state
4. Set as current project in inventory
5. Resume orchestration from last state

---

## Step 2C: View Project Status

If user selects **C (view status)**:

1. Read current project state from `state/orchestrator_state.json`
2. Display summary:

```
Project: my_app
Status: in_progress
Current Phase: backend (2/4 phases)

Tasks:
  Completed: 5
  In Progress: 2 (worker_1, worker_3)
  Pending: 8

Active Workers:
  - worker_1: US-006 (iteration 3) "Adding API endpoint..."
  - worker_3: US-007 (iteration 1) "Setting up database schema..."

Recent Commits:
  - abc123 [US-005] Add user model
  - def456 [US-004] Create base schema
```

---

## Step 3: Initialize Orchestrator State

For a new or resumed project, ensure state file exists:

### State Directory Structure

```
state/
├── orchestrator_state.json    # Main orchestrator state
├── phases.json                # Phase breakdown
├── learnings.json             # Captured learnings
├── workers/                   # Worker state files
│   ├── worker_1.json
│   └── worker_2.json
└── completions/               # Task completion reports
    ├── US-001.json
    └── US-002.json
```

### Initialize State File

Create `state/orchestrator_state.json` if it doesn't exist:

```json
{
  "project_id": "<project_name>",
  "status": "new",
  "current_phase": "",
  "phases": [],
  "tasks": [],
  "agents": [],
  "learnings": [],
  "created_at": "<timestamp>",
  "updated_at": "<timestamp>"
}
```

---

## Step 4: Parse PRD and Create Tasks

After state is initialized, parse the PRD file to extract tasks:

1. Read `PRD_<project_name>.md`
2. Find all user stories (### US-XXX: Title pattern)
3. Extract acceptance criteria for each story
4. Create task entries in state

```json
{
  "id": "US-001",
  "title": "Add priority field to database",
  "description": "As a developer, I need to store task priority...",
  "acceptance_criteria": [
    "Add priority column with default 'medium'",
    "Generate and run migration",
    "Typecheck passes"
  ],
  "status": "pending",
  "assigned_agent": null,
  "dependencies": [],
  "phase_id": "schema"
}
```

---

## Step 5: Hand Off to Agent Manager

Once state is initialized and tasks are created:

1. Update project status to `in_progress`
2. Log: "Project initialized with N tasks across M phases"
3. Inform user: "Ready to start building. Run agent manager to begin task allocation."

Or if auto-starting:

1. Spawn agent manager
2. Begin orchestration loop

---

## Configuration

The orchestrator respects settings from `orchestrator.config.json`:

```json
{
  "max_parallel_workers": 3,
  "max_iterations_per_task": 20,
  "retry_limit": 3,
  "auto_commit": true,
  "log_level": "info"
}
```

If no config file exists, use defaults.

---

## Error Handling

- **No projects in inventory**: Suggest starting a new project
- **Project state file missing**: Offer to reinitialize from PRD
- **PRD file missing**: Error and ask user to create PRD first
- **Invalid project selection**: Re-prompt with valid options

---

## Output Messages

### On New Project Start
```
Creating new project: my_app

Next steps:
1. Answer PRD questions to define your project
2. Review generated PRD
3. Start building with parallel workers

Starting PRD discussion...
```

### On Project Resume
```
Resuming project: my_app

Status: in_progress
Phase: backend (2/4)
Tasks: 5 complete, 2 in progress, 8 pending

Ready to continue. Starting agent manager...
```

### On Completion
```
Project my_app is already complete!

All 15 tasks finished.
Total commits: 19
Learnings captured: 12

Run your app with: npm run dev
```

---

## Checklist

- [ ] Asked user for action (new/continue/status)
- [ ] For new: Got project name and triggered PRD creation
- [ ] For continue: Listed projects and loaded selection
- [ ] Initialized/loaded orchestrator state file
- [ ] Parsed PRD and created task entries (if new)
- [ ] State directory structure created
- [ ] Project added to inventory (if new)
- [ ] Ready for agent manager handoff
