---
name: decompose
description: "Break complex problems into FEATURE-BASED tasks. Use when a feature or problem needs to be split into vertical slices for parallel execution."
---

# Decompose Skill

Break complex problems into **FEATURE-BASED** tasks following the parallel execution pattern.

## Purpose

Analyze a complex feature or problem and decompose it into VERTICAL SLICES that:
- Each represents a complete feature (~30 min of focused AI work)
- Include schema + API + UI + tests in ONE task
- Can run in PARALLEL with other features
- Have dependencies based on shared resources, not themes

## Feature-Based vs Theme-Based Decomposition

**Theme-Based (AVOID):**
```
Task 1: Create all database tables
Task 2: Create all API endpoints (depends: Task 1)
Task 3: Create all UI components (depends: Task 2)
Result: Sequential execution, only 1 worker active!
```

**Feature-Based (USE):**
```
Task 1: User Feature (table + API + UI + tests)
Task 2: Task Feature (table + API + UI + tests)
Task 3: Dashboard Feature (queries + UI)
Result: Tasks 1 & 2 run in parallel!
```

## Instructions

When the user provides a problem or feature request:

1. **Understand the Scope**: Ask clarifying questions if requirements are ambiguous

2. **Identify FEATURES (not components)**: Group by entity or user action:
   - "User Management" → user table + auth endpoints + profile UI
   - "Task System" → task table + CRUD endpoints + task list UI
   - "Reporting" → queries + dashboard components

3. **Create Feature Tasks**: For each feature, include the FULL VERTICAL SLICE:
   - Database schema for this feature
   - API endpoints for this feature
   - UI components for this feature
   - Tests for this feature

4. **Compute Dependencies Based on SHARED RESOURCES**:
   - Feature B depends on Feature A if B uses tables/endpoints A creates
   - Features using different resources have NO dependencies (parallel!)

5. **Output Format**: Present tasks with RICH CONTEXT (workers need motivation!):

```markdown
## Task 1: [Task Name]
**Dependencies**: None | Task X, Task Y
**Description**: [What this task does and WHY it matters]

**Why This Matters**:
[2-3 sentences explaining the purpose and how it fits the larger system]

**Goal**: [One sentence summarizing the achievement]

**Acceptance Criteria**:
- [ ] Criterion 1 (must be verifiable, include technical detail)
- [ ] Criterion 2
- [ ] Typecheck passes

**Success Looks Like**: [What can the worker verify when done?]

## Task 2: [Task Name]
...
```

### Why Rich Task Descriptions Matter:

Workers are AI agents iterating in fresh context windows. They need:
- **Context**: Why am I doing this?
- **Goal**: What am I achieving (one sentence)?
- **Success Picture**: How do I know when I'm done?

**Bad task**: Just a checklist → Worker doesn't understand → Poor work
**Good task**: Rich context + goal → Worker understands deeply → Quality work

## Bad vs Good Decomposition

**Theme-Based (BAD - Sequential Bottleneck):**
```
Task 1: Add User model
Task 2: Add Task model
Task 3: Create all auth endpoints (depends: 1)
Task 4: Create all task endpoints (depends: 2, 3)
Task 5: Add all UI components (depends: 3, 4)
Result: Only 1-2 workers active at a time!
```

**Feature-Based (GOOD - Parallel Execution):**
```
Task 1: Authentication Feature
  - User table with email/password
  - POST /auth/register endpoint
  - POST /auth/login endpoint with JWT
  - Login/signup UI forms
  - Auth tests
  Dependencies: None

Task 2: Task Management Feature
  - Task table with title/status/user_id
  - CRUD endpoints for tasks
  - Task list/detail UI
  - Task tests
  Dependencies: None (parallel with Task 1!)

Task 3: Dashboard Feature
  - Dashboard queries
  - Stats components
  Dependencies: Task 1, Task 2 (uses both tables)
```

**Why Feature-Based?**
- Tasks 1 & 2 run in PARALLEL (different entities, no shared resources)
- Only Task 3 waits (needs data from both features)
- Result: 2x-3x faster completion!

## Saving Tasks

**IMPORTANT: Finding the Project Directory**

Before saving tasks, find the correct project directory:
1. Check `.magicm/inventory/projects.json` for the `project_dir` field
2. **NEVER save files inside MagicM directory**
3. Save tasks inside the project's state file

### Save Tasks to Project State

After decomposition, save tasks to the project state using the API or state file:

```python
# Tasks should be saved to .magicm/state/{project_id}_state.json
# Each task should have:
{
    "id": "US-001",
    "title": "Task title",
    "description": "As a [user], I want [feature] so that [benefit]",
    "acceptance_criteria": [
        "Criterion 1",
        "Criterion 2",
        "Typecheck passes"
    ],
    "status": "pending",  # pending | in_progress | completed
    "dependencies": []  # List of task IDs this depends on
}
```

The tasks array should be saved to the project state:
```json
{
    "project_id": "food",
    "tasks": [
        {"id": "US-001", ...},
        {"id": "US-002", ...}
    ]
}
```

## Files Required for Phase Detection

The MagicM app detects decompose phase completion by looking for:
- **Tasks in state file** - `state.get("tasks", [])` must be non-empty

The app checks `.magicm/state/{project_id}_state.json` for the `tasks` array.
If tasks exist, the phase advances to ORCHESTRATING.

## Key Principles

### Size:
- If you can't describe the change in 2-3 sentences, it's too big

### Rich Descriptions (CRITICAL):
- **EVERY task must have "Why This Matters" context**
- **EVERY task must have a "Goal" statement (one sentence)**
- **EVERY task should have "Success Looks Like" picture**
- Workers need motivation, not just checklists!

### Acceptance Criteria:
- Every task must have "Typecheck passes" as acceptance criterion
- UI tasks should include "Verify changes work in browser"
- Avoid vague criteria: "Works correctly", "Good UX", "Handles edge cases"

### Saving:
- **Always save tasks to the project state file for phase detection**
