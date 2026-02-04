---
name: fix-phase
description: "Fix broken phase configurations (missing chat_id, wrong status, missing DB entries). Use when: phase missing Start button, chat icon not showing, 404 errors on phase access."
---

# Fix Phase

Quickly repair a phase that has configuration issues. This skill uses the `/fix` API endpoint.

---

## When to Use This Skill

Use this skill when you see these symptoms:

| Symptom | Cause | Fix Applied |
|---------|-------|-------------|
| No "Start" button on phase | Status is not `decomposed` | Changes status to `decomposed` |
| No chat icon on phase | Missing `chat_id` field | Adds `chat_id: {project_id}_phase_{N}` |
| Phase 404 error | Missing DB entry | Syncs config to database |
| Phase shows wrong status | Status mismatch disk vs DB | Syncs from disk to DB |
| Story completed on disk but shows pending in UI | Missing `completed_at` in DB | Syncs status + completed_at from JSON |

---

## The Job

1. Identify the project ID and phase ID
2. Call the fix API endpoint
3. Report what was fixed

---

## Step 1: Identify the Phase

Get the project_id and phase_id from context. Phase ID format is typically `phase_{N}_{project_id}`.

If you only have the phase number, construct it:
```
phase_id = f"phase_{phase_number}_{project_id}"
```

---

## Step 2: Call the Fix API

Use the MagicM API to fix the phase:

```bash
curl -X POST "http://localhost:8001/api/projects/{project_id}/phases/{phase_id}/fix"
```

Or use Python:
```python
import requests

response = requests.post(
    f"http://localhost:8001/api/projects/{project_id}/phases/{phase_id}/fix"
)
result = response.json()
print(f"Fixes applied: {result['fixes_applied']}")
print(f"Stories synced: {result['stories_synced']}")
print(f"New status: {result['new_status']}")
```

---

## Step 3: Report Results

The API returns:
```json
{
    "phase_id": "phase_2_konyvelo_program",
    "fixes_applied": [
        "Added missing chat_id",
        "Changed status from 'ready' to 'decomposed'"
    ],
    "stories_synced": 3,
    "new_status": "decomposed"
}
```

Report to user:
- What fixes were applied
- New status
- Whether stories were resynced

---

## What the Fix Endpoint Repairs

The `/fix` endpoint automatically:

1. **Adds missing fields to phase_config.json:**
   - `id` - phase identifier
   - `project_id` - parent project
   - `description` - copied from title if missing
   - `chat_id` - enables chat icon (`{project_id}_phase_{N}`)
   - `spec_path` - path to SPEC.md if it exists
   - `created_at` / `updated_at` - timestamps

2. **Corrects status based on stories:**
   - If stories exist but status is `ready`/`finalized`/`discussing` → changes to `decomposed`
   - If no stories but status is `decomposed`/`orchestrating` → changes to `finalized`

3. **Syncs to database:**
   - Updates phase record in SQLite
   - Resyncs story statuses from disk
   - Sets `completed_at` timestamp for completed stories (reads from JSON or uses current time)

---

## UI Alternative

Users can also click the **Fix** button directly on the phase card in the project page. This button appears on all phases except `orchestrating` and `completed`.

---

## Example Usage

**User says:** "The phase 2 is missing the Start button"

**Assistant should:**
1. Use this skill (invoke `/fix-phase`)
2. Call the fix endpoint for that phase
3. Report what was fixed
4. Suggest refreshing the page

```
Fixed phase 2:
- Added missing chat_id: konyvelo_program_phase_2
- Changed status from 'ready' to 'decomposed'

The Start button should now appear. Please refresh the page.
```

---

## Checklist

- [ ] Identified project_id and phase_id
- [ ] Called POST /api/projects/{project_id}/phases/{phase_id}/fix
- [ ] Reported fixes applied to user
- [ ] Suggested page refresh
