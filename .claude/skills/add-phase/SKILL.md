---
name: add-phase
description: "Add a new phase or iteration to an existing project. Creates a sub-PRD for the next development phase. Use when: project completed first phase, need to extend scope, add features to existing project."
---

# Add Development Phase

Add a new development phase to an existing project that has already been built. Each phase is a focused ~1 week scope of agent work with its own chat, spec, and decomposed stories.

## Parallel Agents Support

When adding a phase, the **AI Team** (Design, Tech, UX, Scope agents) can be activated to help define the phase requirements. The agents discuss in parallel and generate documentation with minimal user input.

**To use parallel agents:**
1. Click the "AI Team" button (people icon) in the chat input area
2. Or they auto-activate when you describe the phase requirements
3. Watch the 4 agents (Design, Tech, UX, Scope) analyze your request
4. Click "Merge & Create PRD" when all agents complete

The agents help by:
- **Design Agent**: Visual and UX considerations for the phase
- **Tech Agent**: Technical implementation approach
- **UX Agent**: User experience and interaction flows
- **Scope Agent**: Feature prioritization and MVP boundaries

---

## The Job

1. Detect if phase was pre-created from the project page UI
2. Read project context (main PRD, existing code, previous phases)
3. Discuss scope with user through back-and-forth conversation
4. When user says "final" — create missing files (spec, config updates)
5. Update database and prompt next step

---

## Step 1: Detect Pre-Created Phase

The project page UI creates phases before opening this chat. When that happens:
- The chat session ID follows the pattern `{project_id}_phase_{N}` (e.g. `magic_engine_phase_1`)
- The phase directory already exists: `project_dir/phases/phase_{N}/`
- `phase_config.json` already exists with status `"discussing"`
- The `stories/` subdirectory already exists
- The DB entry already exists

**Check for this by reading the phase config:**
```python
from app.services.state_manager import read_phase_config, get_project_dir

# Extract phase_number from the chat session ID (e.g. "magic_engine_phase_1" → 1)
config = read_phase_config(project_id, phase_number)
```

**If phase_config.json exists and has status "discussing":**
- The project directory is in `.magicm/inventory/projects.json` under the `project_dir` field
- **NEVER** use `.magicm/projects/` - that's internal state, NOT the actual project
- Read config to get: `title`, `description`, `phase_number`, `chat_id`
- These came from the user's input in the "Add Phase" modal on the project page
- **Do NOT** re-create the directory, config, chat session, or DB entry — they exist
- **DO** still read project context (Step 2) and discuss scope (Step 3)
- The only files you need to create are: `phase_spec.md` (and update config status)

**If no existing phase is detected** (skill invoked standalone without pre-creation):
- Create everything from scratch (directory, config, DB entry, etc.)

---

## Step 2: Read Project Context

**Always do this**, whether pre-created or not.

Read the current project to understand what exists:

- Main PRD file in project directory (e.g. `PRD_*.md`)
- Existing codebase (key files, structure)
- Previous phases in `phases/` directory (if any)
- Project state from `.magicm/state/{project_id}_state.json`

**Output summary:**
```
Project: <name>
Current State: <what has been built>
Previous Phases: <count and brief descriptions>
```

If phase was pre-created, also show:
```
Phase {N}: "{title}"
Description: {description}
```

---

## Step 3: Discussion Phase

If the phase was pre-created, greet the user acknowledging what they already provided:
```
I've loaded Phase {N}: "{title}".
Your description: {description}

Let's refine the scope. What specific features or changes should this phase include?
```

If creating from scratch, ask the user what they want to build next.

Key questions:
1. What features or improvements for this phase?
2. What's the scope? (narrow to ~5-10 stories)
3. Any dependencies on external work?

Continue back-and-forth until scope is clear and bounded.

---

## Step 4: Create Missing Files / Phase Structure

When user says "final" or confirms the scope:

**If pre-created** — only create the missing file:
```
project_dir/phases/phase_N/
├── phase_spec.md          ← CREATE THIS (doesn't exist yet)
├── phase_config.json      ← UPDATE status to "finalized", update title/description if changed
└── stories/               ← Already exists
```

**If created from scratch** — create everything:
```python
from app.services.state_manager import create_phase_dirs, write_phase_config, get_phase_chat_id

phase_dir = create_phase_dirs(project_id, phase_number)
```

---

## Step 5: Write phase_spec.md

**Always create this file** — it does NOT exist yet even for pre-created phases.

**IMPORTANT: This must be a DETAILED, THOROUGH specification.** A good phase_spec.md is 5,000-20,000+ words. Workers rely on this document to know exactly what to build. Lazy/short specs lead to bad worker output.

The spec must include:

1. **Problem Statement** — Why this phase exists, what's broken or missing
2. **Solution Overview** — The approach, with technical rationale for WHY this approach
3. **Data Structures / Layouts** — Exact struct definitions, buffer layouts, schemas
4. **What Changes vs What Stays** — Explicit file-by-file table of changed and unchanged files
5. **Detailed Stories** — Each story must describe ONE small change (completable in one context window):
   - What file(s) to modify
   - What exactly to add/change/remove
   - Acceptance criteria (must be verifiable, not vague)
   - Dependencies on other stories
6. **What CAN'T be done** — Limitations, constraints, things to avoid
7. **Technical deep-dives** — Code snippets, formulas, algorithms workers will need

**Story sizing rules:**
- Each story = ONE context window of work (~10 min AI time)
- If you can't describe the change in 2-3 sentences, split it
- Dependencies first: data structures → backend logic → integration → shader/frontend
- Target 5-12 stories per phase
- Every story needs "cargo check passes" / "typecheck passes" as acceptance criteria

**BAD spec example (too vague):**
```markdown
## What to Implement
Replace the texture system with a buffer system.
### Story 1: Update the renderer
Change the renderer to use buffers instead of textures.
```

**GOOD spec example (detailed):**
```markdown
## What to Implement
Replace `BakedSdfManager`'s 2D texture array with a flat SSBO `wgpu::Buffer`.
The buffer stores 64³ f32 values per brick (1MB each), addressed as:
`buffer[brick_id * 262144 + z * 4096 + y * 64 + x]`

### Story 1: Define BrickCache struct with buffer creation
**What:** Create `BrickCache` struct in `sdf_baker.rs` with fields:
- `buffer: wgpu::Buffer` (STORAGE | COPY_DST)
- `capacity: u32`
- `slot_bitmap: [u64; 4]`
**Constructor:** Query `device.limits().max_storage_buffer_binding_size`,
compute capacity, create buffer.
**Acceptance:** Struct compiles. `cargo check` passes. Buffer created without panic.
```

```markdown
# Phase {N}: {Title}

## Problem Statement
{Why this phase exists — what's broken, missing, or needed}

## Solution Overview
{The approach with technical rationale}

## Data Structures / Layouts
{Exact definitions, schemas, buffer layouts with byte offsets}

## What Changes vs What Stays
### Changed Files
| File | What Changes |
|------|-------------|
| ... | ... |

### Unchanged Files
| File | Why Unchanged |
|------|--------------|
| ... | ... |

## Stories

### Story 1: {Title}
**What:** {Exactly what to do, which file(s)}
**Why:** {Why this must happen before other stories}
**Acceptance:** {Verifiable criteria}

### Story 2: {Title}
**Depends on:** Story 1
**What:** {Details}
**Acceptance:** {Criteria}

{... continue for all stories}

## Technical Considerations
- {Patterns to follow from existing code}
- {Dependencies on existing code}
- {Performance considerations}

## Non-Goals (This Phase)
- {What this phase will NOT include}
```

---

## Step 6: Update phase_config.json

Update the existing config (or write new one) with final values:

```json
{
    "id": "phase_{N}_{project_id}",
    "project_id": "{project_id}",
    "phase_number": N,
    "title": "{Phase Title}",
    "description": "{One-line description}",
    "status": "finalized",
    "chat_id": "{project_id}_phase_{N}",
    "spec_path": "phases/phase_{N}/phase_spec.md",
    "created_at": "{ISO timestamp}"
}
```

Use `write_phase_config()` to save.

---

## Step 7: Update Database

**If pre-created** — update the existing record:
```python
from app.services.database import update_phase_status_db

await update_phase_status_db(phase_id, "finalized")
```

**If created from scratch** — insert new record:
```python
from app.services.database import create_phase_db

await create_phase_db(
    phase_id=config["id"],
    project_id=project_id,
    phase_number=phase_number,
    title=config["title"],
    description=config["description"],
    chat_id=config["chat_id"],
    spec_path=config["spec_path"],
)
```

---

## Step 8: Prompt Next Step

Tell the user:
```
Phase {N} "{title}" has been created and finalized.

Next step: Run /phase-decompose to break this phase into detailed, agent-executable stories.
```

---

## Checklist Before Saving

- [ ] Checked for existing pre-created phase (phase_config.json with status "discussing")
- [ ] Read existing project PRD and code
- [ ] Checked previous phases (if any)
- [ ] Discussed and narrowed scope with user
- [ ] Wrote phase_spec.md with full specification
- [ ] Updated phase_config.json with status "finalized"
- [ ] Created or updated phase record in database
- [ ] Prompted user to run /phase-decompose
