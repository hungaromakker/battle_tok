---
name: progress
description: Display real-time progress dashboard for the Ralph orchestrator
---

# Progress Dashboard Skill

Display real-time progress of the currently running orchestration project.

## Usage

Run `/progress` to view the dashboard in the terminal.

## What It Shows

The dashboard displays:

1. **Project Status**: Current project ID and overall status
2. **Progress Bar**: Visual progress with percentage complete
3. **Current Phase**: Active phase and its task completion status
4. **Task Statistics**: Counts by status (completed, in-progress, validating, pending, failed)
5. **Active Workers**:
   - Worker ID
   - Current task being worked on
   - Iteration count
   - Current focus description
   - Notes left for next iteration
6. **Recent Commits**: Last 5 git commits with hash and message

## Implementation

The dashboard is implemented in `src/ralph_orchestrator/ui/cli_dashboard.ts`.

Key functions:
- `collectDashboardData()`: Gathers all state information
- `renderDashboard()`: Formats the data for terminal display
- `runDashboard()`: Runs the auto-refreshing loop (every 5 seconds)
- `getDashboardSnapshot()`: Gets a single point-in-time view

## Running the Dashboard

### Option 1: In Current Terminal

```typescript
import { runDashboard, createDashboardStopSignal } from './ui';

// Create stop signal for graceful shutdown
const stopSignal = createDashboardStopSignal();

// Handle Ctrl+C
process.on('SIGINT', () => {
  stopSignal.stop();
});

// Run dashboard (auto-refreshes every 5 seconds)
await runDashboard({
  stateDir: 'state',
  projectDir: process.cwd(),
  clearScreen: true,
  useColors: true,
  stopSignal,
});
```

### Option 2: Get Single Snapshot

```typescript
import { getDashboardSnapshot } from './ui';

const output = await getDashboardSnapshot({
  stateDir: 'state',
  useColors: true,
});
console.log(output);
```

### Option 3: Get Raw Data (JSON)

```typescript
import { collectDashboardData, getDashboardJSON } from './ui';

// Get structured data
const data = await collectDashboardData({ stateDir: 'state' });
console.log(data.progressPercent);
console.log(data.activeWorkers);

// Or get JSON string
const json = await getDashboardJSON({ stateDir: 'state' });
console.log(json);
```

## State Files Read

The dashboard reads from:
- `state/orchestrator_state.json` - Main orchestrator state
- `state/workers/{worker_id}.json` - Individual worker states
- Git log - For recent commits

## Display Refresh

The dashboard refreshes every 5 seconds by default. Configure with:

```typescript
await runDashboard({
  refreshIntervalMs: 2000,  // 2 seconds
});
```

## Color Support

Colors are enabled by default. Disable with:

```typescript
await runDashboard({
  useColors: false,
});
```

## Example Output

```
════════════════════════════════════════════════════════════
Ralph Orchestrator - Progress Dashboard
════════════════════════════════════════════════════════════

Project: my_awesome_app  |  Status: in_progress
Updated: 2024-01-15T10:30:00.000Z

[▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░] 45%

Current Phase
────────────────────────────────────────
  Backend (in_progress)
  Tasks: 3/7 completed

Tasks
────────────────────────────────────────
  completed              15
  in_progress             2
  validating              1
  pending                12
  failed                  0
  Total                  30

Active Workers
────────────────────────────────────────────────────────────
  worker_a8f2
    Task: US-015 - Create validator agent skill
    Iteration: 3  |  Status: working
    Focus: Implementing validation result aggregation
    Notes: Need to handle edge case for empty errors array

  worker_c4e1
    Task: US-016 - Implement linter check module
    Iteration: 7  |  Status: working
    Focus: Parsing ESLint JSON output
    Notes: Found biome support needed too

Recent Commits
────────────────────────────────────────────────────────────
  abc1234 [US-014] Implement worker iteration manager - compl...
  def5678 [US-013] Implement worker context loader - completed...
  ghi9012 [US-012] Create worker agent skill with Ralph loop...
  jkl3456 [US-011] Define worker state file schema - completed...
  mno7890 [Phase 4] Worker Agent complete

────────────────────────────────────────────────────────────
Press Ctrl+C to exit
```
