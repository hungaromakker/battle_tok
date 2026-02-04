---
name: setup-deps
description: "Install system dependencies required for project builds. Handles apt packages, Rust toolchain, Node.js, Python packages, and common development libraries."
---

# Setup Dependencies Skill

Install system and language-specific dependencies required for building projects.

## When to Use

- At the start of a new project (before first build)
- When build/test fails due to missing libraries
- When linking errors mention missing `-l<library>`
- When `command not found` errors occur

## Common Dependency Patterns

### Rust Projects

**Cargo not found:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
```

**X11/Windowing (for GUI/graphics):**
```bash
sudo apt update && sudo apt install -y \
    libx11-dev \
    libxcursor-dev \
    libxrandr-dev \
    libxi-dev \
    libgl1-mesa-dev \
    libwayland-dev
```

**Audio:**
```bash
sudo apt install -y libasound2-dev
```

**Common Rust dev dependencies:**
```bash
sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    cmake
```

### Python Projects

**Python not found:**
```bash
sudo apt install -y python3 python3-pip python3-venv
```

**Common dev tools:**
```bash
pip install ruff mypy pytest
```

### Node.js Projects

**Node not found:**
```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs
```

**Build tools for native modules:**
```bash
sudo apt install -y build-essential
```

### C/C++ Projects

**Compiler not found:**
```bash
sudo apt install -y build-essential gcc g++ cmake
```

## Detection from Error Messages

| Error Pattern | Dependency Needed |
|--------------|-------------------|
| `unable to find library -lX11` | `libx11-dev` |
| `unable to find library -lGL` | `libgl1-mesa-dev` |
| `unable to find library -lssl` | `libssl-dev` |
| `unable to find library -lasound` | `libasound2-dev` |
| `unable to find library -lwayland` | `libwayland-dev` |
| `unable to find library -lpthread` | (usually included, try `build-essential`) |
| `cc: command not found` | `build-essential` |
| `cmake: command not found` | `cmake` |
| `pkg-config: command not found` | `pkg-config` |
| `cargo: command not found` | Rust toolchain (rustup) |
| `node: command not found` | `nodejs` |
| `python3: command not found` | `python3` |

## Worker Integration

When a worker encounters a build or link error:

1. **Identify the missing dependency** from error message
2. **Install using sudo apt** (workers have sudo access)
3. **Retry the build/test command**
4. **Record in notes_for_next** what was installed

Example worker iteration:
```
Previous notes: "Cargo build failed - missing X11"

Action this iteration:
1. Installed libx11-dev: sudo apt install -y libx11-dev
2. Re-ran cargo build - SUCCESS
3. All criteria now passing

Notes for next: "X11 dependency installed. Build passes. Ready to verify in browser."
```

## Full Rust Development Setup

For a complete Rust development environment:

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# Common system libraries
sudo apt update && sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    cmake \
    libx11-dev \
    libxcursor-dev \
    libxrandr-dev \
    libxi-dev \
    libgl1-mesa-dev \
    libasound2-dev \
    libwayland-dev \
    libxkbcommon-dev

# Verify
cargo --version
rustc --version
```

## Full Python Development Setup

```bash
# Python
sudo apt install -y python3 python3-pip python3-venv

# Create venv
python3 -m venv venv
source venv/bin/activate

# Dev tools
pip install ruff mypy pytest fastapi uvicorn

# Verify
python3 --version
pip --version
```

## Full Node.js Development Setup

```bash
# Node.js 20.x LTS
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs

# Verify
node --version
npm --version
```

## Checklist

When setting up a new project:

- [ ] Check project type from `Cargo.toml`, `package.json`, `pyproject.toml`
- [ ] Install language toolchain if missing
- [ ] Install system libraries based on project features (GUI, audio, network, etc.)
- [ ] Run build command to verify setup
- [ ] Document what was installed in project README or notes
