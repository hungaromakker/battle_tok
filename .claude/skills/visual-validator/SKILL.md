---
name: visual-validator
description: "Visual validation plugin for AI workers to capture and analyze screenshots"
---

# Visual Validator Plugin

AI workers use this plugin to validate UI correctness by:
1. Starting the application
2. Capturing a screenshot
3. Analyzing with Claude vision
4. Reporting pass/fail

---

## Usage in Worker State

Add to `plugin_invocations` in your worker state file:

```json
{
  "plugin_invocations": [
    {
      "plugin": "visual_validator",
      "command": "validate_full",
      "args": {
        "command": "cargo run --release",
        "window_name": "My App",
        "startup_delay": 5
      }
    }
  ]
}
```

---

## Commands

| Command | Args | Description |
|---------|------|-------------|
| `validate_full` | `command`, `window_name`, `startup_delay`, `cwd` | Full flow: start, wait, capture, stop |
| `start_app` | `command`, `cwd` | Start application |
| `wait_for_window` | `name`, `timeout` | Wait for window to appear |
| `capture` | `window_name`, `filename` | Capture screenshot |
| `list_windows` | - | List visible windows |
| `stop_app` | - | Stop running application |

### Command Details

#### validate_full (Recommended)

The main command for workers. Performs the full validation flow automatically.

**Arguments:**
- `command` (required): Shell command to start the application (e.g., `cargo run --release`)
- `window_name` (required): Name/title of the window to capture
- `startup_delay` (optional, default: 5): Seconds to wait after starting app
- `cwd` (optional): Working directory for the command

**Example:**
```json
{
  "plugin": "visual_validator",
  "command": "validate_full",
  "args": {
    "command": "npm run dev",
    "window_name": "My React App",
    "startup_delay": 10,
    "cwd": "/path/to/project"
  }
}
```

#### start_app

Start an application process.

**Arguments:**
- `command` (required): Shell command to run
- `cwd` (optional): Working directory

#### wait_for_window

Wait for a window with matching name to appear.

**Arguments:**
- `name` (required): Window name to search for (partial match)
- `timeout` (optional, default: 30): Max seconds to wait

#### capture

Capture a screenshot of a window.

**Arguments:**
- `window_name` (required): Name of the window to capture
- `filename` (optional): Output filename (auto-generated if not provided)

#### list_windows

List all visible windows. Useful for debugging window names.

**Arguments:** None

#### stop_app

Stop the running application.

**Arguments:** None

---

## Results

Results appear in `plugin_results` in your worker state:

```json
{
  "plugin_results": [
    {
      "plugin": "visual_validator",
      "command": "validate_full",
      "status": "passed",
      "message": "Screenshot captured: /path/to/screenshot.png",
      "screenshots": ["/path/to/screenshot.png"],
      "metrics": {
        "screenshot_path": "/path/to/.magicm/screenshots/visual_My_App_20260127_120000.png",
        "window_found": true,
        "app_started": true,
        "window_id": "12345678"
      },
      "duration_ms": 8500
    }
  ]
}
```

### Result Fields

| Field | Description |
|-------|-------------|
| `status` | "passed", "failed", "error" |
| `message` | Human-readable summary |
| `screenshots` | Array of screenshot file paths |
| `metrics.screenshot_path` | Full path to the captured screenshot |
| `metrics.window_found` | Whether the window was found |
| `metrics.app_started` | Whether the app started successfully |
| `duration_ms` | Total execution time in milliseconds |

---

## Analyzing Screenshots

After receiving the screenshot path in `plugin_results`, use Claude's vision to analyze:

1. **Read the screenshot file** using Claude's Read tool
2. **Check for visual issues:**
   - UI renders correctly
   - No broken layouts or overlapping elements
   - Expected elements are visible
   - No error dialogs or blank screens
   - Colors and styling match expectations
3. **Report pass/fail** in your worker notes

### Example Worker Flow

```python
# Worker state after plugin invocation
{
  "current_step": "validating_ui",
  "notes_for_next_iteration": [
    "Received screenshot at /project/.magicm/screenshots/visual_MyApp_20260127.png",
    "Need to analyze the screenshot for visual correctness",
    "Check: login form visible, buttons styled correctly, no errors"
  ],
  "plugin_results": [
    {
      "plugin": "visual_validator",
      "status": "passed",
      "metrics": {
        "screenshot_path": "/project/.magicm/screenshots/visual_MyApp_20260127.png"
      }
    }
  ]
}
```

---

## Requirements

System dependencies (install if not present):

```bash
sudo apt install imagemagick xdotool
```

The plugin checks for these dependencies and will return an error message if missing.

---

## Integration with Worker Loop

### When to Use

Use visual validation when:
- Task involves UI changes (layouts, styles, components)
- Acceptance criteria mentions visual appearance
- Testing a desktop application's window

### Worker Steps

1. **Complete code changes** for the UI task
2. **Add plugin invocation** to your state file
3. **Plugin executes** (orchestrator processes invocations)
4. **Receive results** in `plugin_results`
5. **Analyze screenshot** using Claude vision
6. **Report validation result** in your notes

### Example Task Flow

```
Worker receives UI task
        |
        v
Makes code changes
        |
        v
Adds visual_validator invocation
        |
        v
[Orchestrator runs plugin]
        |
        v
Worker reads screenshot path from results
        |
        v
Worker uses Read tool to view screenshot
        |
        v
Worker analyzes with vision capabilities
        |
   +----+----+
   |         |
 PASS      FAIL
   |         |
   v         v
Mark task   Note issues,
complete    fix and retry
```

---

## Configuration

Default configuration in `.magicm/plugins.json`:

```json
{
  "active": {
    "visual_validator": {
      "enabled": true,
      "config": {
        "default_startup_delay": 5,
        "default_timeout": 30
      }
    }
  }
}
```

---

## Troubleshooting

### Window Not Found

If the window isn't found:
1. Use `list_windows` command to see available windows
2. Check the exact window title (it's a partial match)
3. Increase `startup_delay` if the app takes time to start

### Screenshot Capture Failed

If capture fails:
1. Ensure the window is visible (not minimized)
2. Check ImageMagick is installed: `which import`
3. Verify xdotool works: `xdotool search --name ""`

### App Fails to Start

If the application doesn't start:
1. Verify the command works manually in terminal
2. Check the `cwd` path is correct
3. Look at return codes in the error message
