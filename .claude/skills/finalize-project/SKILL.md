---
name: finalize-project
description: "Finalize a completed project phase. Checks requirements, generates README, creates summary. Use when: all tasks complete, ready to ship, need documentation."
---

# Finalize Project

Run this when a project phase completes. Verifies requirements, creates documentation, and prepares for next phase.

---

## The Job

1. Verify all tasks are complete
2. **Check completion requirements** (NEW)
3. Generate/update README.md
4. Create release summary
5. Update progress file with completion stats
6. Prepare for next phase (if any)

**Important:** This runs automatically when all tasks complete, or manually via `/finalize-project`.

---

## Step 1: Verify Completion

Check project state:
```
Project: <name>
Completed: <N>/<N> tasks ✅

All tasks complete!
```

---

## Step 2: Check Completion Requirements (CRITICAL)

Before marking project complete, verify these exist:

### Required Files Checklist

```
Project Completion Requirements:

✅ REQUIRED FILES (MUST have):
- [ ] README.md - Project documentation
- [ ] CLAUDE.md - Instructions for Claude Code
- [ ] PRD_<project>.md - Requirements document
- [ ] progress_<project>.txt - Build log

✅ REQUIRED FOLDERS:
- [ ] .claude/skills/ - Skills for workers (copied from parent)
- [ ] context/ - External context folder with README.md
- [ ] rules/ - Project rules with GLOBAL_RULES.md

⚠️ RECOMMENDED:
- [ ] requirements.txt OR package.json - Dependencies
- [ ] .gitignore - Git ignore file
- [ ] Tests (tests/ or __tests__/)
- [ ] .env.example - Environment template

🔍 QUALITY CHECKS:
- [ ] No TODO comments left unaddressed
- [ ] Typecheck passes (if Python/TypeScript)
- [ ] No hardcoded secrets or credentials
- [ ] All imports resolved
```

### If Requirements Missing

1. **Create missing files automatically:**
   - README.md → Generate from PRD
   - CLAUDE.md → Generate project instructions
   - context/README.md → Create template
   - rules/GLOBAL_RULES.md → Copy from template

2. **Warn if cannot create:**
   - .claude/skills/ missing → Copy from parent project
   - Dependencies missing → List what to add

3. **Block completion if critical:**
   - PRD missing → Cannot finalize
   - No code files → Cannot finalize

4. **Verify Git Submodule Setup:**
   - Check `.git` folder exists in project directory
   - Verify remote origin points to `https://github.com/hungaromakker/<project_name>`
   - Ensure project is listed in magicm's `.gitmodules`
   - If not set up as submodule, run:
     ```bash
     # Initialize git if not already done
     cd <project_dir>
     git init
     git add -A
     git commit -m "Initial commit"
     
     # Create GitHub repo and push
     gh repo create hungaromakker/<project_name> --public --source=. --remote=origin --push
     
     # Add as submodule to magicm (from magicm root)
     cd /path/to/magicm
     git submodule add https://github.com/hungaromakker/<project_name>.git <project_name>
     git commit -m "Add <project_name> as submodule"
     git push origin master
     ```

---

## Step 3: Generate README.md

Create comprehensive README based on:
- PRD goals and features
- **Actual files created** (not promises)
- Tech stack detected from code
- Setup instructions that work

### README Must Include:

1. **Title and Description** - What the project does
2. **Features** - What was ACTUALLY implemented
3. **Quick Start** - How to run it (tested!)
4. **Installation** - Dependencies and setup
5. **Project Structure** - Key directories
6. **API Documentation** - If applicable

---

## Step 4: Generate CLAUDE.md

Create instructions for future Claude Code sessions:

```markdown
# CLAUDE.md - <Project Name>

## Project Overview
[What this project does]

## Commands
[How to run, test, build]

## Architecture
[Key components and their purpose]

## Key Directories
[What goes where]

## Skills Available
[List skills in .claude/skills/]

## Worker Guidelines
[Rules for workers]
```

---

## Step 5: Create Completion Summary

Update progress file:

```markdown
---
## ✅ Phase Complete: <date>

### Delivered
- Feature 1
- Feature 2

### Completion Checklist
- [x] README.md exists
- [x] CLAUDE.md exists
- [x] All tasks complete
- [x] Required folders exist
- [x] Typecheck passes

### Next Steps
- /add-phase - Add new features
- /update-project - Modify scope
---
```

---

## Step 6: Final Actions

1. **Git commit and push** (project is a submodule):
   ```bash
   # Commit and push changes to project repo
   cd <project_dir>
   git add -A
   git commit -m "Phase complete: <phase_name>"
   git push origin master
   
   # Update submodule reference in magicm
   cd /path/to/magicm
   git add <project_name>
   git commit -m "Update <project_name> submodule - phase complete"
   git push origin master
   ```
2. **Update project status** to "completed"
3. **Broadcast completion** to UI

---

## Completion Requirements Reference

### Minimum Required Structure

```
project/
├── README.md              # REQUIRED - Generated
├── CLAUDE.md              # REQUIRED - Generated
├── PRD_<project>.md       # REQUIRED - Must exist
├── progress_<project>.txt # REQUIRED - Must exist
├── .claude/
│   └── skills/            # REQUIRED - Copy from parent
├── context/
│   └── README.md          # REQUIRED - Template
├── rules/
│   └── GLOBAL_RULES.md    # REQUIRED - Template
├── requirements.txt       # Recommended
├── .gitignore             # Recommended
└── [app code...]
```

### Files to Check

```python
REQUIRED_FILES = [
    "README.md",
    "CLAUDE.md",
    f"PRD_{project_id}.md",
    f"progress_{project_id}.txt",
]

REQUIRED_DIRS = [
    ".claude/skills",
    "context",
    "rules",
]

RECOMMENDED_FILES = [
    "requirements.txt",
    "package.json",
    ".gitignore",
    ".env.example",
]
```

---

## Checklist

- [ ] All tasks verified complete
- [ ] **Required files checked and created**
- [ ] **Required folders checked and created**
- [ ] README.md generated/updated
- [ ] CLAUDE.md generated
- [ ] Progress file updated with summary
- [ ] **Git submodule verified** (project has .git, remote origin set)
- [ ] **Changes committed to project repo**
- [ ] **Changes pushed to GitHub**
- [ ] **Submodule reference updated in magicm**
- [ ] **Magicm changes pushed**
- [ ] Project status updated to "completed"
- [ ] Next phase suggestions provided
