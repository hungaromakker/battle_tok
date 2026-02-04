---
name: phase-decompose
description: "Break a finalized development phase into detailed, agent-executable user stories with spec files and status tracking."
---

# Phase Decompose

Break a finalized phase into detailed user stories that agents can execute precisely. Each story gets its own folder with implementation instructions and status tracking.

---

## The Job

1. Read the phase spec and project context
2. Break into right-sized stories (ONE context window each)
3. Create story folders with spec.md and status.json
4. Update database with story records
5. Set phase status to `decomposed`

---

## Step 1: Read Context

Load:
- `phases/phase_N/phase_spec.md` — what this phase implements
- `phases/phase_N/phase_config.json` — phase metadata
- Main project PRD — for full awareness
- Existing codebase — for code patterns and file paths

Identify the phase to decompose (most recent finalized phase, or ask user).

---

## Step 2: Break into Stories

Follow the same sizing rules as the decompose skill:

- Each story completable in ONE context window (~10 min of AI work)
- 2-3 sentence description max per story
- Dependencies first: schema → backend → frontend
- Every story MUST have "Typecheck passes" criterion
- UI stories MUST have "Verify changes work in browser" criterion

Story ID format: `US-P{phase_number}-{NNN}` (e.g., `US-P1-001`, `US-P1-002`)

---

## Step 3: Create Story Folders

For each story, create:

```
phases/phase_N/stories/
├── US-P{N}-001/
│   ├── spec.md          # Detailed implementation instructions
│   └── status.json      # Machine-readable acceptance tracking
├── US-P{N}-002/
│   ├── spec.md
│   └── status.json
└── ...
```

Use state manager:
```python
from app.services.state_manager import create_story_dir

story_dir = create_story_dir(project_id, phase_number, story_id)
```

---

## Step 4: Write spec.md per Story (RICH FORMAT - CRITICAL FOR WORKER SUCCESS)

Workers are AI agents that need CONTEXT and GOALS, not just checklists! Each spec.md should give the worker everything they need to understand and complete the task.

```markdown
# {Story ID}: {Title}

## Description
{What this story implements} - {WHY it matters to the overall project}

## The Core Concept / Why This Matters
{2-5 sentences explaining:}
- What is the purpose of this work?
- How does it fit into the larger system?
- What problem does it solve?
- Why was this approach chosen?

## Goal
{One sentence summarizing what the worker is trying to achieve}

## Files to Create/Modify
- `path/to/file.py` — {what to change and why}
- `path/to/other.py` — {what to add and its purpose}

## Implementation Steps
1. {Step 1 with specific details and reasoning}
2. {Step 2 with code patterns to follow}
3. {Step 3}

## Code Patterns
{Examples from existing codebase showing patterns to follow}

## Acceptance Criteria
- [ ] {Criterion 1 with technical detail}
- [ ] {Criterion 2 with technical detail}
- [ ] Typecheck passes

## Success Looks Like
{Paint a picture of the completed work:}
- What can the worker test/verify when done?
- What behavior should they see?
- How do they know they succeeded?

## Dependencies
- Depends on: {list of story IDs that must complete first, or "None"}
```

### Why Rich Specs Matter:

Workers iterate on tasks in fresh context windows. They need:
1. **Context**: Why am I doing this? What's the big picture?
2. **Goal**: What am I trying to achieve (in one sentence)?
3. **Details**: How should I approach this?
4. **Success Picture**: How do I know when I'm done?

**Bad spec**: Just a checklist → Worker doesn't understand the purpose → Poor implementation
**Good spec**: Rich context + clear goal → Worker understands deeply → Quality implementation

---

## Step 5: Write status.json per Story

```json
{
    "story_id": "US-P1-001",
    "title": "Add notification model",
    "status": "pending",
    "focus": "",
    "criteria": [
        {"text": "Notification table created", "done": false, "notes": ""},
        {"text": "Index on user_id", "done": false, "notes": ""},
        {"text": "Typecheck passes", "done": false, "notes": ""}
    ],
    "checklist": [
        {"item": "Read spec.md for implementation details", "done": false},
        {"item": "Create model file", "done": false},
        {"item": "Add migration", "done": false},
        {"item": "Run typecheck", "done": false},
        {"item": "Commit changes", "done": false}
    ],
    "worker_notes": "",
    "notes_history": [],
    "iteration": 0,
    "failed_reasons": [],
    "dependencies": [],
    "commit_hash": "",
    "files_modified": [],
    "iteration_history": [],
    "learnings": [],
    "created_at": "2025-01-01T00:00:00"
}
```

### Field Reference

| Field | Set by | Description |
|-------|--------|-------------|
| `focus` | Worker | **Current task focus** - What the worker is working on RIGHT NOW |
| `checklist` | Decompose + Worker | Granular implementation steps, worker marks done as they go |
| `notes_history` | Worker | All notes from all iterations with timestamps |
| `worker_notes` | Worker | Current/latest notes (for backward compat) |
| `criteria` | Decompose | Acceptance criteria from spec.md |
| `iteration_history` | Worker | Log of actions taken each iteration |
| `commit_hash` | Worker | Git commit hash when story completes |
| `files_modified` | Worker | List of files created/modified |
| `learnings` | Worker | Useful discoveries for future reference |

**Checklist** should break the story into 4-8 implementation steps based on spec.md:
- Read spec.md / understand requirements
- Create/modify specific files
- Implement specific functions/features
- Run typecheck/tests
- Commit changes

---

## Step 6: Update Database

```python
from app.services.database import create_phase_story_db, update_phase_status_db

# For each story:
await create_phase_story_db(
    story_id=story_id,
    phase_id=phase_id,
    project_id=project_id,
    title=title,
    spec_path=str(spec_path),
    status_path=str(status_path),
    dependencies=json.dumps(dependency_list),
)

# Update phase status
await update_phase_status_db(phase_id, "decomposed")
```

---

## Output Summary

After decomposition, display:

```
Phase {N} "{title}" decomposed into {count} stories:

  US-P{N}-001: {title} (no deps)
  US-P{N}-002: {title} (depends on US-P{N}-001)
  US-P{N}-003: {title} (depends on US-P{N}-001)
  ...

Next step: Run /phase-orchestrator or use the "Start" button on the project page to begin execution.
```

---

## Checklist

### Context Gathering:
- [ ] Read phase_spec.md and phase_config.json
- [ ] Read main project PRD and existing code
- [ ] Understand the WHY behind each story

### Story Quality (CRITICAL):
- [ ] Stories are right-sized (one context window each)
- [ ] Dependencies ordered: schema → backend → frontend
- [ ] **EVERY spec.md has "Why This Matters" section**
- [ ] **EVERY spec.md has "Goal" statement (one sentence)**
- [ ] **EVERY spec.md has "Success Looks Like" section**
- [ ] Every story has "Typecheck passes" criterion
- [ ] UI stories have "Verify changes work in browser" criterion

### File Creation:
- [ ] Created story folders with spec.md and status.json
- [ ] spec.md follows rich format (not just checklists!)
- [ ] status.json has meaningful checklist items

### Database:
- [ ] Inserted stories into database
- [ ] Updated phase status to "decomposed"
