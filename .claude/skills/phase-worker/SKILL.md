---
name: phase-worker
description: "Self-reflecting worker agent for phase stories. Reads from stories/US-P{N}-{XXX}/status.json and iterates using the Ralph loop pattern."
---

# Phase Worker - Self-Reflecting Ralph Loop

Worker agent that iterates on a phase story using the Ralph loop pattern. Reads/writes phase story `status.json` instead of `.magicm/state/workers/`.

---

## The Job

Each iteration follows these steps:

1. **Read State**: Load `phases/phase_N/stories/US-P{N}-{XXX}/status.json`
2. **Read Spec**: Load `phases/phase_N/stories/US-P{N}-{XXX}/spec.md` for instructions
3. **Review Notes**: Check `worker_notes` from previous iteration
4. **Check Progress**: Review `criteria` array to see what's done
5. **Work**: Complete the next logical step toward acceptance criteria
6. **Update State**: Record progress in `status.json`
7. **Leave Notes**: Write `worker_notes` for the next iteration
8. **Increment**: Bump `iteration` counter

---

## Step 1: Read Story State

Load the story's status file:

```json
{
    "story_id": "US-P1-001",
    "title": "Add notification model",
    "status": "in_progress",
    "focus": "Create the Notification table with proper schema and user_id foreign key",
    "criteria": [
        {"text": "Notification table created", "done": false, "notes": ""},
        {"text": "Index on user_id", "done": false, "notes": ""},
        {"text": "Typecheck passes", "done": false, "notes": ""}
    ],
    "checklist": [
        {"item": "Read spec.md for implementation details", "done": true},
        {"item": "Create model file", "done": true},
        {"item": "Add migration", "done": false},
        {"item": "Run typecheck", "done": false},
        {"item": "Commit changes", "done": false}
    ],
    "worker_notes": "Created the model file, need to add index next",
    "notes_history": [
        {"iteration": 0, "note": "Starting story - reading spec.md", "timestamp": "2026-01-28T10:00:00Z"},
        {"iteration": 1, "note": "Created model file, need to add index next", "timestamp": "2026-01-28T10:15:00Z"}
    ],
    "iteration": 1,
    "failed_reasons": [],
    "dependencies": [],
    "commit_hash": "",
    "files_modified": [],
    "iteration_history": [
        {"iteration": 1, "action": "Created model file", "notes": "Need to add index next", "completed_at": "..."}
    ],
    "learnings": [],
    "created_at": "..."
}
```

### Status.json Field Reference

| Field | Description |
|-------|-------------|
| `focus` | **Current task focus** - What the worker is working on RIGHT NOW (updated each iteration) |
| `checklist` | **Implementation checklist** - Granular steps within the story, tracked with done/not-done |
| `notes_history` | **Notes log** - All notes from all iterations, preserved with timestamps |
| `worker_notes` | **Current notes** - Latest notes (for backward compatibility) |
| `criteria` | **Acceptance criteria** - High-level completion requirements |
| `iteration_history` | **Iteration log** - Action taken each iteration |

**Focus** should be a short (1-2 sentence) description of the current task:
- "Creating the InMemoryDB class with customer loading"
- "Fixing typecheck error in notification.py line 45"
- "Adding SSE endpoint for progress updates"

**Checklist** breaks the story into smaller steps:
```json
"checklist": [
    {"item": "Read spec.md for implementation details", "done": true},
    {"item": "Create service file skeleton", "done": true},
    {"item": "Implement load_customers method", "done": false},
    {"item": "Add indexes for fast lookup", "done": false},
    {"item": "Run typecheck", "done": false}
]
```

---

## Step 2: Read Story Spec

Load `spec.md` for detailed implementation instructions:
- Files to create/modify
- Code patterns to follow
- Step-by-step implementation guide

---

## Step 3: Determine Next Step

Based on:
1. Which criteria are not yet `done`
2. Notes from previous iteration
3. The spec's implementation steps

Pick the next logical piece of work.

---

## Step 4: Do the Work

Make actual file changes in the project codebase:
- Create/modify files as specified in spec.md
- Follow existing code patterns
- Run typecheck if applicable
- Run tests if the project has them
- Verify in browser if it's a UI story

### Handling Build/Link Errors

If you encounter missing dependency errors during build:

**Rust linking errors** (`unable to find library -lX11`):
```bash
# Install the missing library
sudo apt install -y libx11-dev  # for -lX11
sudo apt install -y libgl1-mesa-dev  # for -lGL
sudo apt install -y libssl-dev  # for -lssl
sudo apt install -y libasound2-dev  # for -lasound

# Common Rust GUI/graphics deps (install all at once):
sudo apt install -y libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev libwayland-dev
```

**Python/Node errors**:
```bash
# Python virtual env issues
python -m venv venv && source venv/bin/activate
pip install -r requirements.txt

# Node module issues
rm -rf node_modules package-lock.json
npm install
```

**Command not found errors**:
```bash
# Rust/Cargo not found
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# Build tools not found
sudo apt install -y build-essential pkg-config cmake
```

After installing dependencies, retry the build command. Record what was installed in `worker_notes` so future iterations know the setup is complete.

### Running Tests

**Always run tests before marking a story complete:**

```bash
# Python/FastAPI
cd <project_dir>/backend
pytest tests/ -v

# Next.js/TypeScript
cd <project_dir>/frontend
npm run typecheck
npm run test

# Rust
cd <project_dir>
cargo test
cargo clippy
```

### Browser Verification

For UI stories with "Verify changes work in browser" criterion:

1. Start the dev server if not running
2. Navigate to the affected page
3. Test the specific functionality mentioned in criteria
4. Check for console errors
5. Verify responsive behavior if applicable

Record verification results in the criterion's `notes` field.

---

## Step 5: Update Status

After each iteration:

```json
{
    "criteria": [
        {"text": "Notification table created", "done": true, "notes": "Created in app/models/notification.py"},
        {"text": "Index on user_id", "done": true, "notes": "Added in migration"},
        {"text": "Typecheck passes", "done": false, "notes": "Need to fix import"}
    ],
    "worker_notes": "Model and index done. Typecheck has import error in notification.py line 5.",
    "iteration": 2,
    "status": "in_progress",
    "commit_hash": ""
}
```

On completion (all criteria done):

```json
{
    "criteria": [
        {"text": "Notification table created", "done": true, "notes": "Created in app/models/notification.py"},
        {"text": "Index on user_id", "done": true, "notes": "Added in migration"},
        {"text": "Typecheck passes", "done": true, "notes": "Verified clean build"}
    ],
    "worker_notes": "All criteria met. Committed and pushed.",
    "iteration": 3,
    "status": "completed",
    "commit_hash": "a1b2c3d4e5f6789..."
}
```

---

## Step 6: Check Completion

If ALL criteria are `done`:
1. **Git commit** with detailed message (see Git Commits section below)
2. **Push** to project GitHub
3. **Record `commit_hash`** in status.json
4. Set `status` to `completed`
5. Clear `worker_notes` (or leave final summary)
6. Update `phase_stories` table via database

If max iterations reached without completion:
1. Set `status` to `failed`
2. Add reason to `failed_reasons`
3. Leave detailed `worker_notes` for manual review

---

## Blocked Status

If stuck after 3 iterations on the same issue:

```python
# Track stuck iterations - check if last 3 iterations had same issue
iteration_notes = [h.get("notes", "") for h in status.get("iteration_history", [])[-3:]]
same_issue_count = sum(1 for n in iteration_notes if current_issue in n)

if same_issue_count >= 3:
    status["status"] = "failed"
    status["failed_reasons"].append(f"Stuck on: {current_issue}")
    status["worker_notes"] = f"""
    BLOCKED after {status['iteration']} iterations.
    
    Issue: {current_issue}
    
    Attempted solutions:
    - Iteration {i-2}: {approach_1}
    - Iteration {i-1}: {approach_2}  
    - Iteration {i}: {approach_3}
    
    All approaches failed. Needs manual review.
    """
```

### When to Mark as Failed/Blocked

- **3+ iterations** on the same error without progress
- **Dependency issues** that can't be resolved (missing packages, incompatible versions)
- **External blockers** (API down, missing credentials, etc.)
- **Ambiguous requirements** that need clarification from human

Always leave detailed `worker_notes` explaining:
1. What was tried
2. Why each approach failed
3. What a human should look at

---

## Example Iteration Flow

```
Iteration 1 (Fresh worker):
  - Reads status.json: iteration=0, worker_notes=""
  - Reads spec.md for implementation instructions
  - Creates initial code structure
  - Runs typecheck - fails with import error
  - Writes: worker_notes="Basic structure done. Typecheck fails: missing import for UserModel"
  - Updates: iteration=1, status="in_progress"

Iteration 2 (Fresh worker instance):
  - Reads status.json: iteration=1
  - Reads notes: "Typecheck fails: missing import for UserModel"
  - Fixes the import
  - Runs typecheck - passes!
  - Updates criteria: "Typecheck passes" = done
  - Writes: worker_notes="Import fixed. Need to implement the actual feature now."
  - Updates: iteration=2

Iteration 3 (Fresh worker instance):
  - Reads notes about implementing feature
  - Implements the main functionality per spec.md
  - Runs tests - passes
  - Writes: worker_notes="Feature implemented. Need browser verification."
  - Updates: iteration=3

Iteration 4 (Fresh worker instance):
  - Reads notes about browser verification
  - Starts dev server, tests in browser
  - All criteria met!
  - Git commits with detailed message
  - Records commit hash
  - Sets status="completed"
```

---

## Output Messages

### During Work

```
=== Phase Worker - Iteration 3 ===
Story: US-P3-016 - Add notification preferences to user settings

Previous notes: "Import fixed. Need to implement the actual feature now."

Working on: Implementing notification preferences component

Progress: 1/4 criteria complete
  [x] Typecheck passes
  [ ] User can toggle email notifications
  [ ] User can select notification frequency
  [ ] Preferences persist after reload

Action: Created NotificationPrefs.tsx component with toggle and dropdown

Next: Need to wire up API endpoint and test persistence
```

### On Completion

```
=== Story Complete ===
Story: US-P3-016 - Add notification preferences to user settings
Total Iterations: 4

Files Modified:
  - backend/app/models/user.py
  - backend/app/api/users.py
  - frontend/src/components/settings/NotificationPrefs.tsx
  - frontend/src/app/settings/page.tsx

Acceptance Criteria:
  [x] User can toggle email notifications on/off
  [x] User can select notification frequency (immediate/daily/weekly)
  [x] Preferences persist after page reload
  [x] Typecheck passes

Git Commit: a1b2c3d4e5f6789...
Status: COMPLETED
```

### On Blocked/Failed

```
=== Story BLOCKED ===
Story: US-P3-016 - Add notification preferences to user settings
Iterations: 8

Blocker: Cannot resolve database migration conflict
  - Iteration 6: Tried dropping and recreating migration
  - Iteration 7: Tried manual ALTER TABLE
  - Iteration 8: Tried fresh migration with new name
  - All approaches failed due to existing data constraint

Status: FAILED - Needs manual database review
```

---

## Language Documentation

Workers can access local AI-optimized documentation in `languages/` without loading full context.

### Quick Lookup
```bash
# Search for a topic
grep -r "TOPIC" languages/nextjs/    # Next.js 16.1.4
grep -r "TOPIC" languages/rust/      # Rust 1.93.0
grep -r "TOPIC" languages/python/    # Python/FastAPI

# Common searches
grep -r "use server\|Server Action" languages/nextjs/   # Server Actions
grep -r "HashMap\|Vec" languages/rust/                  # Collections
grep -r "BaseModel" languages/python/                   # Pydantic
```

### Available Docs
| Language | Key Files |
|----------|-----------|
| Next.js | `glossary.md`, `directives.md`, `routing.md`, `caching.md` |
| Rust | `crates/std/*.md`, `crates/tokio/`, `crates/serde/` |
| Python | `fastapi/overview.md` |

### Read Full Doc When Needed
```bash
cat languages/nextjs/directives.md    # "use client/server/cache"
cat languages/rust/crates/std/iter.md # All iterator methods
```

---

## Git Commits Inside Project Directory

**IMPORTANT:** Each project maintains its own git history. When a story completes, commit changes inside the project directory with the story ID and full details of what was implemented.

### Initialize Git Repo if Needed

Before committing, check if the project has a git repo. If not, initialize one:

```bash
cd <project_dir>

# Check if git repo exists
if [ ! -d ".git" ]; then
    git init
    git add -A
    git commit -m "Initial commit - project setup"
fi
```

### When to Commit
- **After each story completes** (all acceptance criteria met)
- **Before marking story as complete** in status.json
- **Record commit hash** in status.json `commit_hash` field

### Commit Message Format

Use this detailed format that includes the story context:

```
[US-P{N}-XXX] Story title from spec.md

## What was implemented
- Concise summary of the main change
- Any secondary changes or supporting work

## Files changed
- path/to/file1.py - what was changed
- path/to/file2.tsx - what was added

## Acceptance criteria met
- [x] Criterion 1 from status.json
- [x] Criterion 2 from status.json
- [x] Typecheck passes
```

### Example Commit Message

```
[US-P3-016] Add notification preferences to user settings

## What was implemented
- Added notification_preferences column to users table
- Created preferences UI component in settings page
- Implemented API endpoint for saving preferences

## Files changed
- backend/app/models/user.py - added notification_preferences field
- backend/alembic/versions/xxx_add_notification_prefs.py - migration
- frontend/src/components/settings/NotificationPrefs.tsx - new component
- frontend/src/app/settings/page.tsx - integrated preferences component

## Acceptance criteria met
- [x] User can toggle email notifications on/off
- [x] User can select notification frequency (immediate/daily/weekly)
- [x] Preferences persist after page reload
- [x] Typecheck passes
```

### Commit Inside Project Directory

```bash
# Navigate to project directory
cd <project_dir>

# Initialize git if not already a repo
if [ ! -d ".git" ]; then
    git init
    git add -A
    git commit -m "Initial commit - project setup"
fi

# Stage all changes
git add -A

# Commit with detailed message (use heredoc for multiline)
git commit -m "$(cat <<'EOF'
[US-P1-001] Story title here

## What was implemented
- Main change description

## Files changed
- file1.py - description

## Acceptance criteria met
- [x] Criterion 1
- [x] Typecheck passes
EOF
)"

# Push to remote if configured (optional)
git push origin master 2>/dev/null || echo "No remote configured, local commit saved"
```

### Update Submodule Reference in magicm (if project is a submodule)

If the project is set up as a submodule of magicm:

```bash
# Go to magicm root
cd /path/to/magicm

# Stage the submodule update (records new commit hash)
git add <project_name>

# Commit the reference update
git commit -m "Update ${project_name} submodule - ${story_id}"

# Push magicm changes
git push origin master
```

### Record Commit Hash in status.json

After committing, update the story's status.json with the commit hash:

```json
{
    "story_id": "US-P1-001",
    "status": "completed",
    "commit_hash": "abc123def456...",
    "criteria": [...]
}
```

Get the commit hash with:
```bash
git rev-parse HEAD
```

---

## Key Differences from Main Worker

| Aspect | Main Worker | Phase Worker |
|--------|-------------|--------------|
| State file | `.magicm/state/workers/worker_{id}.json` | `phases/phase_N/stories/{id}/status.json` |
| Instructions | Task from project state | `spec.md` in story folder |
| Scope | Project-level task | Phase-scoped story |
| Criteria | From project state acceptance_criteria | From `status.json` criteria array |

---

## Checklist

For each iteration:

- [ ] Read story state from `phases/phase_N/stories/US-P{N}-{XXX}/status.json`
- [ ] Read `spec.md` for implementation instructions
- [ ] Check `worker_notes` from previous iteration
- [ ] Check `criteria` array for pending items
- [ ] Work on next logical step
- [ ] Update `files_modified` with any new changes
- [ ] Update `criteria` for completed items (with notes)
- [ ] Write `worker_notes` for next iteration
- [ ] Add entry to `iteration_history`
- [ ] Increment `iteration` counter
- [ ] If all criteria met: set `status = "completed"`
- [ ] If stuck 3+ iterations: set `status = "failed"`, add to `failed_reasons`
- [ ] Write updated status.json

**Testing & Verification (REQUIRED):**
- [ ] Run typecheck: `npm run typecheck` or `cargo check` or `mypy`
- [ ] Run tests if project has them: `pytest` / `npm test` / `cargo test`
- [ ] Fix any lint errors introduced
- [ ] For UI stories: verify changes work in browser
- [ ] Record test results in criterion notes

**On Story Completion (Git in Project Directory):**
- [ ] Navigate to project directory: `cd <project_dir>`
- [ ] Initialize git if needed: `git init` (if no `.git` folder exists)
- [ ] Write detailed commit message with:
  - [ ] Story ID and title: `[US-P{N}-XXX] Title from spec.md`
  - [ ] What was implemented (main changes)
  - [ ] Files changed with descriptions
  - [ ] All acceptance criteria that were met
- [ ] Commit changes: `git add -A && git commit -m "..."`
- [ ] Push if remote exists: `git push origin master` (optional)
- [ ] Record commit hash in status.json: `git rev-parse HEAD`
- [ ] If submodule: update in magicm: `cd /magicm && git add <project> && git commit && git push`

**On Blocked/Failed:**
- [ ] Document what was tried in `worker_notes`
- [ ] List specific errors and attempted solutions
- [ ] Add clear reason to `failed_reasons`
- [ ] Leave actionable notes for manual review

---

## Browser Automation - CRITICAL RULE

**NEVER use Playwright MCP tools.** All `mcp__playwright__*` tools are PROHIBITED.

**INSTEAD**, use the agent-browser CLI for all browser automation:

```bash
# Open a page
agent-browser open http://localhost:3000

# Get element refs for interaction
agent-browser snapshot -i --json
# Returns: {"@e1": "button 'Submit'", "@e2": "textbox 'Email'", ...}

# Interact using refs
agent-browser click @e1
agent-browser fill @e2 "test@example.com"

# Take screenshots
agent-browser screenshot page.png

# Close browser
agent-browser close
```

Or use MagicMv2 plugins: BrowserTestingPlugin, AgentBrowserPlugin, VisualValidatorPlugin.

See `validation/worker_rules.md` Rule 15 for full details.
