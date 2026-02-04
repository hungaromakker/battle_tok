---
name: create-readme
description: "Generate or update README.md for a project. Analyzes codebase, extracts setup instructions, documents API. Use anytime documentation is needed."
---

# Create/Update README

Generate comprehensive README.md by analyzing actual project files.

---

## The Job

1. Scan project directory structure
2. Detect tech stack from files
3. Extract setup instructions from config files
4. Document API endpoints (if applicable)
5. Generate/update README.md

**Important:** Base documentation on ACTUAL files, not just PRD promises.

---

## Step 1: Analyze Project

Scan the project directory:

```python
# Files to look for:
config_files = [
    "requirements.txt", "pyproject.toml",  # Python
    "package.json",  # Node.js
    "Cargo.toml",  # Rust
    "go.mod",  # Go
    "setup.sh", "Makefile",  # Build scripts
    ".env.example", "config.py",  # Configuration
]

entry_points = [
    "app/main.py", "main.py", "server.py",  # Python
    "src/index.ts", "index.js",  # Node
    "cmd/main.go",  # Go
]
```

Output analysis:
```
Project: <name>
Root: <path>

Tech Stack Detected:
- Language: Python 3.x
- Framework: FastAPI
- Frontend: HTMX + Alpine.js + Tailwind
- Database: SQLite

Entry Point: app/main.py
Config: .env.example, app/config.py

Key Directories:
- app/ (14 files)
- templates/ (8 files)
- static/ (3 files)
```

---

## Step 2: Extract Information

### From requirements.txt / package.json:
- Dependencies and versions
- Dev dependencies
- Scripts (npm run xxx)

### From config files:
- Required environment variables
- Default ports
- Database connection

### From source code:
- API endpoints (FastAPI routes, Express routes)
- Main features implemented
- CLI commands

---

## Step 3: Generate README Sections

### Title and Description
From PRD introduction or infer from code.

### Features
List actual implemented features (not PRD promises):
- ✅ Feature that exists in code
- (Skip features not yet implemented)

### Installation
Based on actual config files:
```bash
# If requirements.txt exists:
pip install -r requirements.txt

# If package.json exists:
npm install

# If setup.sh exists:
./setup.sh
```

### Configuration
Document ALL environment variables found:
```bash
# .env.example
DATABASE_URL=sqlite:///app.db
SECRET_KEY=your-secret-key
PORT=8001
```

### Running
Detect from:
- package.json scripts
- Makefile targets
- Common patterns (uvicorn, npm run dev)

### API Documentation
For FastAPI projects, extract routes:
```python
# Scan for @app.get, @app.post, @router.xxx
```

Generate table:
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | /api/projects | List all projects |
| POST | /api/projects | Create new project |

---

## Step 4: Write README.md

```markdown
# <Project Name>

<Description from PRD or inferred>

## Features

- ✅ Feature 1 (implemented)
- ✅ Feature 2 (implemented)

## Requirements

- Python 3.11+
- Node.js 18+ (optional)

## Installation

```bash
# Clone repository
git clone <repo>
cd <project>

# Create virtual environment
python -m venv venv
source venv/bin/activate  # Linux/Mac
# or
.\venv\Scripts\activate  # Windows

# Install dependencies
pip install -r requirements.txt
```

## Configuration

Copy `.env.example` to `.env` and configure:

```bash
cp .env.example .env
```

| Variable | Description | Default |
|----------|-------------|---------|
| DATABASE_URL | Database connection | sqlite:///app.db |
| PORT | Server port | 8001 |

## Running

### Development
```bash
python -m uvicorn app.main:app --reload --port 8001
```

### Production
```bash
uvicorn app.main:app --host 0.0.0.0 --port 8001
```

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | / | Home page |
| GET | /api/projects | List projects |
| POST | /api/projects | Create project |
| GET | /api/projects/{id} | Get project |
| DELETE | /api/projects/{id} | Delete project |

## Project Structure

```
<project>/
├── app/
│   ├── __init__.py
│   ├── main.py          # FastAPI application
│   ├── config.py        # Settings
│   ├── routes/          # API endpoints
│   └── services/        # Business logic
├── templates/           # Jinja2 templates
├── static/              # CSS, JS
├── requirements.txt
└── README.md
```

## Development

### Type Checking
```bash
mypy app/
```

### Testing
```bash
pytest
```

### Linting
```bash
ruff check app/
```

## Built With

- [FastAPI](https://fastapi.tiangolo.com/)
- [HTMX](https://htmx.org/)
- [Alpine.js](https://alpinejs.dev/)
- [Tailwind CSS](https://tailwindcss.com/)

## License

MIT
```

---

## Update Mode

When README already exists:
1. Read existing README
2. Identify sections to update
3. Preserve custom content (like badges, extra docs)
4. Update auto-generated sections
5. Mark with timestamp

```markdown
<!-- Auto-generated by Ralph Orchestrator: 2024-01-24 -->
```

---

## Checklist

- [ ] Scanned actual project files
- [ ] Detected tech stack correctly
- [ ] Setup instructions work (tested)
- [ ] Environment variables documented
- [ ] API endpoints extracted and listed
- [ ] Project structure matches reality
- [ ] README saved to project directory
