---
name: phase-orchestrator
description: "Orchestrate workers for a specific development phase. Reads stories from phases/phase_N/stories/ and assigns them to parallel workers."
---

# Phase Orchestrator

Multi-agent orchestration scoped to a single development phase. Manages parallel workers to execute phase stories.

---

## The Job

1. Read phase config and stories from `phases/phase_N/stories/`
2. Determine which stories are ready (dependencies met)
3. Assign ready stories to available workers (respecting parallel limits)
4. Monitor worker progress via status.json files
5. When all stories complete, mark phase as completed

---

## Step 1: Load Phase Context

```python
from app.services.state_manager import read_phase_config, get_phase_dir
from app.services.database import get_phase_stories_db

config = read_phase_config(project_id, phase_number)
stories = await get_phase_stories_db(config["id"])
phase_dir = get_phase_dir(project_id, phase_number)
```

---

## Step 2: Dependency Resolution

For each story, check if all dependencies are completed:

```python
def is_story_ready(story, all_stories):
    deps = json.loads(story.get("dependencies", "[]"))
    if not deps:
        return True
    completed = {s["id"] for s in all_stories if s["status"] == "completed"}
    return all(d in completed for d in deps)
```

---

## Step 3: Worker Assignment

Same parallel limits as project orchestrator (from project config). For each ready story:

1. Spawn a worker process (Claude Code CLI)
2. Pass the story's `spec.md` as context
3. Worker uses `/phase-worker` skill to iterate on the story
4. Track worker assignment in `phase_stories` table

---

## Step 4: Monitor Progress

Poll `status.json` files in each story directory:
- Check `status` field for completion
- Check `criteria` array for progress
- On completion: update `phase_stories` table, check for newly unblocked stories

---

## Step 5: Phase Completion

When all stories are completed:
1. Update phase status to `completed` in database
2. Update `phase_config.json` on disk
3. Report summary of completed work

---

## Git Commits on Story Completion

When a phase worker completes a story, the phase orchestrator should verify the git commit was made inside the project directory with proper details.

### Ensure Git Repo Exists in Project

Before any story work, verify the project has a git repo:

```bash
cd <project_dir>
if [ ! -d ".git" ]; then
    git init
    git add -A
    git commit -m "Initial commit - project setup"
fi
```

### Verify Worker Commits

1. **Check git repo exists** in project directory
2. **Check commit exists** with `[US-P{N}-{XXX}]` prefix: `git log --oneline | grep "US-P{N}-XXX"`
3. **Verify commit message includes:**
   - Story ID and title
   - What was implemented
   - Files changed with descriptions
   - Acceptance criteria met
4. **Check `commit_hash`** is recorded in story's `status.json`
5. Update the submodule reference in magicm if project is a submodule

### If Worker Didn't Commit

If story is marked complete but no commit exists, create one:
```bash
cd <project_dir>

# Init if needed
[ ! -d ".git" ] && git init

git add -A
git commit -m "$(cat <<'EOF'
[US-P{N}-XXX] Story title

## What was implemented
- Summary from status.json criteria notes

## Files changed
- List files from git diff

## Acceptance criteria met
- [x] All criteria from status.json
EOF
)"

# Push if remote configured
git push origin master 2>/dev/null || echo "Local commit saved"
```

### Phase Completion Commit
```bash
# After all stories in a phase complete, make a summary commit
cd <project_dir>

# Verify git history exists
git log --oneline | head -5

git add -A
git commit -m "$(cat <<'EOF'
[Phase ${N}] ${phase_title} complete

## Stories completed
- US-P{N}-001: Title
- US-P{N}-002: Title
- ...

## Summary
Brief description of what Phase N delivered
EOF
)"

# Push if remote configured
git push origin master 2>/dev/null || echo "Local commit saved"

# If submodule, update in magicm
cd /path/to/magicm
git add <project_name>
git commit -m "Update ${project_name} submodule - Phase ${N} complete"
git push origin master
```

---

## Error Handling

- If a worker fails: mark story as `failed`, leave `failed_reasons` in status.json
- Failed stories can be retried by resetting status to `pending`
- Phase can be stopped via API (`POST /api/projects/{id}/phases/{phase_id}/stop`)
