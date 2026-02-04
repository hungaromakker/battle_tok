---
name: update-project
description: "Modify an existing project's scope, tasks, or configuration. Also repairs broken state (missing DB entries, incomplete configs, 404 errors). Use when: need to change requirements, add/remove tasks, update acceptance criteria, change priorities, OR fix broken project/phase state."
---

# Update Project

Modify an existing project's PRD, tasks, or configuration without starting from scratch.
**Also handles repairs** for broken state (missing database entries, incomplete configs, missing fields).

---

## The Job

1. Load current project state and PRD
2. Understand what needs to change (or diagnose what's broken)
3. Make targeted updates (not full rewrite)
4. Preserve completed work and learnings
5. Update state files AND database consistently

**Important:** Preserve what's working. Only change what's needed.

---

## Update Types

### 0. Repair Broken State (NEW)
Fix inconsistencies between JSON files and SQLite database:
- **Missing DB entries**: Project in JSON but not in `inventory` or `project_state` tables
- **Missing config fields**: `phase_config.json` missing `chat_id`, `id`, `project_id`, etc.
- **Status mismatches**: Phase shows wrong status (e.g., no Start button because status isn't "decomposed")
- **Orphaned records**: DB entries without corresponding JSON files

**Repair process:**
1. Read JSON config files
2. Query database tables (`inventory`, `project_state`, `phases`)
3. Compare and identify discrepancies
4. Fix by updating BOTH JSON and database atomically
5. Verify fix by checking UI renders correctly

**Common repairs:**
```python
# Missing phase fields - ensure all required fields present:
required_phase_fields = {
    "id": f"phase_{N}_{project_id}",
    "project_id": project_id,
    "phase_number": N,
    "title": "...",
    "description": "...",
    "status": "decomposed",  # Required for Start button
    "chat_id": f"{project_id}_phase_{N}",  # Required for chat icon
    "spec_path": f"phases/phase_{N}/phase_spec.md",
    "created_at": "...",
    "updated_at": "..."
}

# Sync to database
from app.services.database import upsert_phase_db
await upsert_phase_db(config)
```

### 1. Add Tasks
Add new user stories to existing PRD:
- Insert at correct position (respect dependencies)
- Number sequentially after existing stories
- Update state file with new pending tasks

### 2. Modify Tasks
Change existing task scope or criteria:
- Update PRD story text
- Update acceptance criteria
- Reset task status to "pending" if criteria changed significantly

### 3. Remove Tasks
Delete tasks that are no longer needed:
- Remove from PRD
- Remove from state (if pending/in_progress)
- Keep completed tasks in history

### 4. Reorder/Reprioritize
Change task execution order:
- Update dependencies in PRD
- Update dependencies in state file
- Ensure no circular dependencies

### 5. Update Configuration
Change project settings:
- max_iterations
- max_parallel_workers
- auto_commit

---

## Step 1: Load Current State

Read and display:
```
Project: <name>
Status: <running/ready/prd_complete>
Tasks: <completed>/<total> complete
Active Workers: <count>

Current Tasks:
✅ US-001: <title> (completed)
🔄 US-002: <title> (in_progress - worker_xxx)
⏳ US-003: <title> (pending)
...
```

---

## Step 2: Understand Changes

Ask the user what they want to change:

```
What would you like to update?

A. Add new tasks/stories
B. Modify existing task (change scope/criteria)
C. Remove a task
D. Change task order/dependencies
E. Update project configuration
F. Multiple changes (describe)
G. Repair broken state (404 errors, missing buttons, incomplete configs)
```

For each change type, ask targeted follow-up questions.

**For repair requests (option G):**
1. Ask which specific issue they're seeing (404 error, missing button, etc.)
2. Check JSON files for the project/phase
3. Query database tables to compare
4. Show discrepancies found
5. Fix all issues atomically

---

## Step 3: Make Updates

### Adding Tasks

1. Determine insertion point (after which story?)
2. Check dependencies (what must complete first?)
3. Write new story following PRD format
4. Add to both PRD file and state file

```python
# State file update
new_task = {
    "id": "US-XXX",
    "title": "...",
    "description": "...",
    "acceptance_criteria": [...],
    "dependencies": ["US-YYY"],
    "status": "pending"
}
state["tasks"].append(new_task)
```

### Modifying Tasks

1. Find task in PRD and state
2. Update relevant fields
3. If criteria changed significantly:
   - Reset status to "pending"
   - Clear assigned_worker
4. Save both files

### Removing Tasks

1. Check if task is in_progress (warn user)
2. Check if other tasks depend on it (update dependencies)
3. Remove from PRD
4. Remove from state (keep if completed for history)

---

## Step 4: Validate Changes

Before saving, verify:
- [ ] No circular dependencies
- [ ] All dependencies reference existing tasks
- [ ] Task IDs are unique
- [ ] PRD and state file are consistent
- [ ] Completed tasks preserved

---

## Step 5: Save Changes

Update files:
- `PRD_<projectname>.md` - Updated PRD
- `.ralph/state/<projectname>_state.json` - Updated state
- `progress_<projectname>.txt` - Add change note

```markdown
# progress_<projectname>.txt

---
## Project Updated: <date>
Changes:
- Added US-025: <title>
- Modified US-010: Changed acceptance criteria
- Removed US-012: No longer needed
---
```

---

## Safety Rules

1. **Never delete completed tasks** - Archive instead
2. **Warn before modifying in_progress tasks** - Worker may be working on it
3. **Validate dependencies** - No orphan or circular deps
4. **Backup before major changes** - Create .backup files

---

## Example Session

**User:** "I need to add image upload to the chat feature"

**Assistant:**
```
Current Project: magicm
Tasks: 15/25 complete

Adding new task...

Where should this task go in the dependency chain?
A. After US-011 (Chat page layout) - recommended
B. After US-013 (Chat submission)
C. Independent (no dependencies)
D. Other: [specify]

User: A

Created:
### US-026: Add image upload to chat
**Description:** As a user, I want to upload images in chat so agents can see my designs.

**Acceptance Criteria:**
- [ ] Upload button next to message input
- [ ] Accept PNG, JPG, GIF, WEBP (max 10MB)
- [ ] Preview before sending
- [ ] Typecheck passes
- [ ] Verify changes work in browser

**Dependencies:** US-011

Added to PRD and state file.
```

---

## Checklist

- [ ] Loaded current project state
- [ ] Understood specific changes needed
- [ ] Validated no dependency issues
- [ ] Updated PRD file
- [ ] Updated state file
- [ ] Added change note to progress file
- [ ] Preserved completed work
