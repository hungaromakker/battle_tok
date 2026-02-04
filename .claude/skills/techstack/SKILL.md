---
name: techstack
description: "Analyze requirements and recommend appropriate technology choices. Use when starting a project or evaluating backend, frontend, database, and infrastructure options."
---

# Tech Stack Skill

Analyze requirements and recommend appropriate technology choices.

## Purpose

Help users choose the right technologies for their project by:
- Understanding their requirements and constraints
- Evaluating options across backend, frontend, database, and infrastructure
- Providing clear trade-offs and alternatives

## Instructions

When analyzing tech stack needs:

1. **Gather Requirements**:
   - What type of application? (web app, API, CLI, mobile, etc.)
   - Expected scale? (prototype, small team, enterprise)
   - Team expertise? (familiar languages/frameworks)
   - Deployment constraints? (cloud, on-prem, serverless)
   - Special requirements? (real-time, ML, heavy computation)

2. **Evaluate Each Layer**:

   **Backend**:
   - Language: Python, Node.js, Go, Rust, Java, etc.
   - Framework: FastAPI, Express, Gin, Actix, Spring, etc.
   - Consider: performance needs, ecosystem, hiring pool

   **Frontend**:
   - Framework: React, Vue, Svelte, HTMX, vanilla JS
   - Styling: Tailwind, CSS modules, styled-components
   - Consider: interactivity level, SEO needs, bundle size

   **Database**:
   - SQL: PostgreSQL, MySQL, SQLite
   - NoSQL: MongoDB, Redis, DynamoDB
   - Consider: data relationships, scale, query patterns

   **Infrastructure**:
   - Hosting: Vercel, Railway, AWS, GCP, self-hosted
   - Containerization: Docker, Kubernetes
   - Consider: cost, complexity, team ops experience

3. **Output Format**:

```
## Recommended Stack

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Backend | ... | ... |
| Frontend | ... | ... |
| Database | ... | ... |
| Infrastructure | ... | ... |

## Alternatives Considered

### [Alternative 1]
- Pros: ...
- Cons: ...
- Best for: ...

## Trade-offs

- [Trade-off 1]: We chose X over Y because...
```

## Output Files

**IMPORTANT: Finding the Project Directory**

Before saving files, find the correct project directory:
1. Check `.magicm/inventory/projects.json` for the `project_dir` field
2. **NEVER save files inside MagicM directory**
3. Save all files inside the project's directory

Save tech stack decisions to `<project_dir>/techstack.md`:

```markdown
# Tech Stack - <project_name>

## Summary

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Backend | ... | ... |
| Frontend | ... | ... |
| Database | ... | ... |
| Infrastructure | ... | ... |

## Detailed Decisions

### Backend
...

### Frontend
...

### Database
...

### Infrastructure
...

## Alternatives Considered
...

## Trade-offs
...
```

## Key Principles

- Prefer boring technology for most projects (proven, stable, well-documented)
- Match complexity to project needs (don't over-engineer)
- Consider the team's existing skills
- Think about long-term maintenance, not just initial development
- For prototypes: optimize for speed of development
- For production: optimize for reliability and maintainability

## System Dependencies by Stack

When recommending a tech stack, include the system dependencies needed:

### Rust Projects
```bash
# Toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# Build essentials
sudo apt install -y build-essential pkg-config cmake

# For GUI/graphics (winit, wgpu, etc.)
sudo apt install -y libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev libwayland-dev libxkbcommon-dev

# For audio (cpal, rodio)
sudo apt install -y libasound2-dev

# For TLS/networking
sudo apt install -y libssl-dev
```

### Python Projects
```bash
sudo apt install -y python3 python3-pip python3-venv
python3 -m venv venv && source venv/bin/activate
pip install ruff mypy pytest
```

### Node.js Projects
```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs
```

### C/C++ Projects
```bash
sudo apt install -y build-essential gcc g++ cmake
```

Include these setup commands in the techstack.md file under a "## Setup" section.

## Files Required for Phase Detection

The MagicM app detects techstack phase completion by looking for:
- `techstack.md` - **REQUIRED** (exact filename)

Make sure this file exists in the project directory for automatic phase detection.

## Next Steps

After saving techstack.md, tell user:
- "Tech stack saved to `<project_dir>/techstack.md`"
- "Next: Run `/prd` to create user stories"
