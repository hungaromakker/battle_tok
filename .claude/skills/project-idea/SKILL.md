---
name: project-idea
description: "Capture and refine a project idea before PRD creation. Creates structured folders for ideas, exclusions, constraints, context, and rules. Initializes git repo as submodule. Run this before /prd."
---

# Project Idea Skill

Capture and refine a project idea before PRD creation. Creates structured folders with everything needed for autonomous development. **Sets up project as a git submodule of the main magicm repository.**

## Parallel Agents Support

When starting a project, the **AI Team** (Design, Tech, UX, Scope agents) automatically activates to help define requirements. The 4 agents discuss your idea in parallel and generate documentation with minimal user input.

**How it works:**
1. When you describe your project idea, the AI Team bar appears at the bottom of the chat
2. Watch the 4 agents (Design, Tech, UX, Scope) analyze your request simultaneously
3. Each agent provides insights from their perspective
4. Click "Merge & Create PRD" when all agents complete to generate a comprehensive PRD

**The agents help by:**
- **Design Agent**: Visual style, branding, aesthetics
- **Tech Agent**: Technology stack, architecture, implementation approach
- **UX Agent**: User flows, interaction patterns, usability
- **Scope Agent**: MVP features, priorities, what to include/exclude

## Purpose

Run this BEFORE `/prd` to:
- Capture the user's raw idea
- Clarify what they WANT and DON'T WANT
- Document technical constraints
- Create required project infrastructure
- Set up context for external inputs (designs, APIs, etc.)
- **Initialize git repository and add as submodule**

## Instructions

When the user provides an initial idea:

1. **Check Current Project Context FIRST**:
   - Look at the current chat/conversation context to find the project_id
   - Check `.magicm/state/{project_id}_state.json` to see if we're already in a project
   - If state file exists with a `project_dir`, **USE THAT FOLDER** - do NOT create a new one
   - The current project name is the context - do NOT ask for a new project name

2. **Only Ask for Project Name if NO Context**:
   - Only if there's no current project context, ask for a lowercase name with underscores
   - Check `.magicm/inventory/projects.json` to see if project already exists
   - If new project needed, create folder at SAME LEVEL as MagicM (e.g., `../food/`)
   - **NEVER create project folders inside MagicM directory**
   - **NEVER create a new folder if already in a project context**

3. **Create Folder Structure** (inside the EXISTING project directory, e.g., `joli/`):
```
<existing_project_dir>/
├── .claude/
│   └── skills/           # Copy skills from parent project
├── ideas/
│   ├── original_prompt.md
│   ├── refined_idea.md
│   └── features.md
├── exclusions/
│   ├── dont_want.md
│   └── out_of_scope.md
├── constraints/
│   └── technical.md
├── context/              # For external inputs (designs, API docs, etc.)
│   └── README.md
├── rules/                # Project-specific rules
│   └── GLOBAL_RULES.md
├── design/               # Design documentation (if applicable)
├── structure/            # Project structure docs
├── validation/           # Pre-commit checks, validation rules
├── summary.md
└── progress_<project_name>.txt
```

4. **Initialize Git Repository as Submodule** (CRITICAL - do this after folder creation):

   ```bash
   # Navigate to the new project folder
   cd <project_dir>
   
   # Initialize git repository
   git init
   
   # Create initial .gitignore
   cat > .gitignore << 'EOF'
   # Python
   __pycache__/
   *.py[cod]
   venv/
   .venv/
   .env
   .env.local
   
   # Node
   node_modules/
   bun.lockb
   
   # Rust
   target/
   Cargo.lock
   
   # IDE
   .idea/
   .vscode/
   *.swp
   
   # OS
   .DS_Store
   Thumbs.db
   
   # Build artifacts
   dist/
   build/
   
   # Logs
   *.log
   logs/
   
   # Runtime state (MagicM)
   .magicm/
   EOF
   
   # Add all files and create initial commit
   git add -A
   git commit -m "Initial project structure for <project_name>"
   ```

   **Create GitHub Repository:**
   ```bash
   # Create GitHub repo using gh CLI (requires gh auth)
   gh repo create hungaromakker/<project_name> --public --source=. --remote=origin --push
   
   # OR if repo already exists:
   git remote add origin https://github.com/hungaromakker/<project_name>.git
   git push -u origin master
   ```

   **Add as Submodule to magicm:**
   ```bash
   # Go to magicm root (parent of MagicM folder)
   cd /path/to/magicm
   
   # Add the project as a submodule
   git submodule add https://github.com/hungaromakker/<project_name>.git <project_name>
   
   # Commit the submodule addition
   git add .gitmodules <project_name>
   git commit -m "Add <project_name> as git submodule"
   git push origin master
   ```

   **IMPORTANT:** The project should be accessible from both:
   - The standalone repo: `https://github.com/hungaromakker/<project_name>`
   - As a submodule in magicm: `magicm/<project_name>/`

5. **Capture Original Prompt**: Save their exact words to `ideas/original_prompt.md`

6. **Clarifying Conversation** - Ask about:

   **What They Want:**
   - What problem does this solve?
   - Who will use it?
   - What's the ONE thing it must do well?

   **What They DON'T Want:**
   - Features to avoid (auth, payments, notifications, etc.)
   - Complexity to avoid
   - Anti-patterns they've seen

   **Technical Constraints:**
   - Preferred tech stack
   - Deployment requirements
   - Hard technical requirements

   **External Context:**
   - Do they have design mockups to include?
   - Any API documentation to reference?
   - Existing code or schemas to integrate with?

7. **Save Structured Files**: Create all the markdown files with their answers

8. **Create Context README**: Explain what goes in the context folder

9. **Create Rules File**: Add project-specific rules (or copy global template)

10. **Create Summary**: Write `summary.md` with a quick reference for the PRD agent

11. **Copy Skills**: Copy `.claude/skills/` from parent project so workers have access

12. **Final Git Commit**: Commit all created files to the project repo
    ```bash
    cd <project_dir>
    git add -A
    git commit -m "Add project idea documentation and structure"
    git push origin master
    
    # Update submodule reference in magicm
    cd /path/to/magicm
    git add <project_name>
    git commit -m "Update <project_name> submodule"
    git push origin master
    ```

13. **Next Step**: Tell them to run `/prd` to continue (the current project context will be used automatically)

## Output Files

### summary.md
```markdown
# Project Summary for PRD Generation

**Project:** <project_name>

## What We're Building
[2-3 sentences]

## Must Have
- [Feature 1]
- [Feature 2]

## Must NOT Have
- [Exclusion 1]
- [Exclusion 2]

## Technical Stack
- [Stack choices]

## External Context Available
- [ ] Design mockups (add to context/)
- [ ] API documentation (add to context/)
- [ ] Database schemas (add to context/)

## Success Looks Like
- [Criteria]

---
**Ready for PRD generation. Run `/prd` to continue.**
```

### context/README.md
```markdown
# Context Folder

Add external context files here that workers need:
- Design mockups (PNG, JPG)
- API documentation (MD, JSON)
- Database schemas (SQL, ERD)
- Reference code or implementations

Workers check this folder before starting each task.
```

### rules/GLOBAL_RULES.md
```markdown
# Project Rules

## Required Before Completion
- [ ] README.md exists
- [ ] All tasks complete
- [ ] Typecheck passes
- [ ] Tests pass (if applicable)

## Worker Guidelines
- Always check context/ folder first
- Follow coding conventions in structure/
- Update progress file with learnings
```

## Key Principles

- **CRITICAL: Check project context FIRST** - if already in a project (e.g., `joli`), use that folder
- **NEVER create a new folder when user gives a descriptive name** - the descriptive name is the idea title, NOT a new project name
- Capture exclusions explicitly - they're as important as features
- Use their exact words when possible
- If they're unsure, offer options
- The goal is to NARROW the scope, not expand it
- Don't create the PRD - just capture the idea
- **Create folder structure inside the EXISTING project folder** - workers need it

## Files Required for Phase Detection

The MagicM app detects idea phase completion by looking for:
- `ideas/*.md` (e.g., `ideas/original_prompt.md`) - **REQUIRED**
- `summary.md` - **REQUIRED**

Make sure these files exist in the project directory for automatic phase detection.

## Completion Checklist

Before telling user to run `/project-structure`:
- [ ] All folders created
- [ ] .claude/skills/ copied from parent
- [ ] `ideas/original_prompt.md` exists - **REQUIRED FOR DETECTION**
- [ ] `summary.md` exists - **REQUIRED FOR DETECTION**
- [ ] context/README.md exists
- [ ] rules/GLOBAL_RULES.md exists
- [ ] progress_<project>.txt created (empty)
- [ ] **Git repository initialized in project folder**
- [ ] **GitHub repo created (hungaromakker/<project_name>)**
- [ ] **Project added as submodule to magicm**
- [ ] **Initial commit pushed to both repos**

## Git Submodule Benefits

Setting up projects as submodules provides:
- **Independent development**: Each project has its own git history
- **Easy syncing**: Pull changes on any machine with `git submodule update`
- **Standalone cloning**: Projects can be cloned independently or as part of magicm
- **Clean separation**: Project code is isolated from orchestrator code

## Syncing Projects Across Machines

**On a new machine (clone with submodules):**
```bash
git clone --recurse-submodules https://github.com/hungaromakker/magicm.git
```

**On existing clone (init submodules):**
```bash
git submodule update --init --recursive
```

**Pull latest for all submodules:**
```bash
git submodule update --remote --merge
```

**Work on a specific project:**
```bash
cd <project_name>
git checkout master
git pull
# make changes
git add -A && git commit -m "changes"
git push

# Update submodule reference in magicm
cd ..
git add <project_name>
git commit -m "Update <project_name> submodule"
git push
```
