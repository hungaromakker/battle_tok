---
name: project-structure
description: "Define project structure, visual design, and technical practices through discussion. Creates design, structure, and validation files for workers."
---

# Project Structure & Design Skill

Have a design discussion to define project structure, visual design, and technical practices before implementation.

## Purpose

Through back-and-forth conversation, establish:
- **Visual Design**: Look, feel, UI patterns, colors, typography
- **Technical Structure**: Folder organization, file naming, depth of architecture
- **Scope & Practices**: How deep to go, patterns to follow, things to avoid
- **Build vs Import**: For each feature, decide whether to build custom or import
- **Validation Rules**: What workers check before each commit

## Core Philosophy

**Default: Build your own tools instead of importing packages.**

Why build custom:
- No dependency bloat
- Full control
- No breaking changes from updates
- Long-term maintainability

**Package Manager: Always use `bun`, never npm/yarn.**

## Conversation Flow

### Phase 1: Visual Design Discussion

Start with the look and feel:

"Let's talk about how this app should look and feel. Tell me about your vision..."

Ask about:
- **Style**: Modern/minimal? Playful? Corporate? Dark mode?
- **Reference apps**: "Any apps you want this to feel like?"
- **Colors**: Brand colors? Preferences?
- **Complexity**: Simple dashboard? Rich interactions?

Probe deeper:
- "When you imagine using this, what does the main screen look like?"
- "How much visual polish for MVP vs future?"
- "Any UI patterns you love or hate?"

### Phase 2: Technical Structure Discussion

"Now let's talk about how the code should be organized..."

Ask about:
- **Depth**: "How modular should this be? Simple flat structure or deeply nested?"
- **Separation**: "Strict separation of concerns or pragmatic colocation?"
- **Patterns**: "Any architectural patterns you want? MVC, Clean Architecture, feature-folders?"

Probe deeper:
- "Do you prefer files grouped by feature or by type?"
- "How much abstraction? Minimal or lots of layers?"
- "Any past projects you want to mirror the structure of?"

### Phase 3: Build vs Import Discussion

"Let's talk about dependencies. My default is to build custom solutions rather than importing packages..."

Explain the philosophy:
- "Building custom means no dependency bloat, full control, no breaking updates"
- "I'll ask for each major feature: build custom or import?"

Go through likely needs:
- "Date formatting - build a simple formatter or import date-fns?"
- "Form validation - build custom validators or import zod?"
- "HTTP client - build a fetch wrapper or import axios?"
- "State management - build with Context or import zustand?"
- "UI components - build custom or import a library?"

Track decisions in a table.

### Phase 4: Coding Practices Discussion

"What other coding practices matter to you?"

**Default established:**
- Use `bun` (never npm/yarn)
- Build custom over import
- TypeScript strict mode

Ask about:
- **Testing**: "Test coverage requirements?"
- **Documentation**: "Inline comments? JSDoc? Minimal?"
- **Patterns**: "Specific patterns to always/never use?"

Probe deeper:
- "What bad code patterns drive you crazy?"
- "How much should workers 'ask permission' vs 'just do it'?"

### Phase 4: Scope Discussion

"Let's define the boundaries..."

- "What's the depth of this project - MVP, v1, or production-ready?"
- "What should workers NOT overthink?"
- "Where should they go deep vs stay shallow?"

## Output Files

**IMPORTANT: Finding the Project Directory**

Before creating files, find the correct project directory:
1. Check `.magicm/inventory/projects.json` for the `project_dir` field
2. **NEVER save files inside MagicM directory**
3. Save all files inside the project's directory (e.g., `/home/user/magicm/food/` not `/home/user/magicm/MagicM/`)

After discussion, create inside the PROJECT DIRECTORY:

```
<project_dir>/
├── design/
│   ├── visual_design.md       # Look, feel, colors, typography
│   ├── ui_patterns.md         # Components, interactions, layouts
│   └── references.md          # Reference apps, inspirations
├── structure/
│   ├── folder_structure.md    # Directory organization
│   ├── naming_conventions.md  # How to name things
│   ├── architecture.md        # Patterns, layers, depth
│   └── coding_practices.md    # Code standards
├── validation/
│   ├── pre_commit_checks.md   # What to verify before commit
│   └── worker_rules.md        # Instructions for workers
└── project_config.md          # Summary for PRD
```

### `design/visual_design.md` example:
```markdown
# Visual Design

## Overall Feel
Modern, clean, minimal. Think Linear or Notion.

## Colors
- Primary: #3B82F6 (blue)
- Background: #0F172A (dark slate)
- Text: #E2E8F0 (light gray)

## Typography
- Headings: Inter, bold
- Body: Inter, regular
- Code: JetBrains Mono

## Principles
- Lots of whitespace
- Subtle shadows
- Rounded corners (8px)
- Smooth transitions
```

### `structure/coding_practices.md` example:
```markdown
# Coding Practices

## Core Philosophy: Build Over Import

DEFAULT: Build custom solutions instead of importing packages.

When to build custom:
- Can implement in < 100 lines
- Simple, well-understood problem
- Want full control

When OK to import:
- Crypto/security (never roll your own)
- Database drivers
- Framework itself

## Package Manager: Bun Only
bun install, bun run dev, bun run typecheck, bun test

## Build vs Import Decisions

| Need | Decision | Reason |
|------|----------|--------|
| Date formatting | BUILD | < 50 lines |
| Form validation | BUILD | Custom to our needs |
| HTTP client | BUILD | fetch wrapper |
| Auth | BUILD | Full control |
| Database driver | IMPORT | Necessary |
```

### `validation/pre_commit_checks.md` example:
```markdown
# Pre-Commit Validation

## Required Checks
1. `bun run typecheck` - must pass
2. `bun run lint` - must pass
3. Naming check - manual review

## Naming Validation
- [ ] Files: PascalCase for components, camelCase for utils
- [ ] Database: snake_case columns
- [ ] Variables: camelCase
- [ ] Constants: SCREAMING_SNAKE_CASE

## Dependency Check
- [ ] Did you import a new package?
- [ ] Could you have built it in < 100 lines?
- [ ] If yes, build it instead

## If Types Don't Match
If database column is `user_id` but code uses `userId`:
1. Check naming_conventions.md for correct format
2. Change the CODE to match conventions
3. Update ALL references
4. Re-run typecheck

## If Validation Fails
DO NOT COMMIT. Fix issues first.
```

## Wrap Up

After saving files:

```
Design and structure defined!

Created:
- design/ (visual design, UI patterns)
- structure/ (folders, naming, practices)
- validation/ (pre-commit rules)

Summary saved to project_config.md for PRD.

Next: Run `/prd <project>` to create user stories.
Workers will follow these rules and validate before commits.
```

## Files Required for Phase Detection

The MagicM app detects structure phase completion by looking for:
- `structure/*.md` (e.g., `structure/architecture.md`) - **REQUIRED**
- `design/*.md` (e.g., `design/visual_design.md`) - **REQUIRED**

Make sure at least one file exists in `structure/` or `design/` folders for automatic phase detection.

## Key Principles

- This is a DISCUSSION, not a form
- Probe deeper when answers are vague
- Capture their exact words when possible
- It's OK to suggest things and ask "what about..."
- Focus on what THEY want, not best practices dogma
- The goal is clarity for workers, not perfection
