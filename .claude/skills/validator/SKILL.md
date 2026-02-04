---
name: validator
description: "Validator agent that checks completed work for errors. Runs typecheck, linter, and relevant tests, then reports results to the state file."
---

# Validator Agent

The Validator Agent is triggered after each task completion to verify the work meets quality standards. It runs typecheck, linter, and relevant tests, then reports results.

---

## The Job

The validator performs these checks in order:

1. **Typecheck**: Run TypeScript compiler to catch type errors
2. **Linter**: Run project linter (eslint, biome, etc.) on changed files
3. **Tests**: Run tests related to the changed files
4. **Build**: (Only on phase completion) Verify the project builds

---

## When Triggered

The validator is triggered by the Agent Manager when:
- A worker completes a task (status = "completed")
- A phase completes (all tasks in phase done)

---

## Step 1: Load Completion Report

First, load the task completion report to understand what was changed:

```typescript
import { readCompletionReport } from '../agents/completion_reporter';

const completionReport = await readCompletionReport(taskId, { stateDir: 'state' });
if (!completionReport) {
  throw new Error(`No completion report found for task ${taskId}`);
}

const filesChanged = completionReport.files_changed;
console.log(`Validating ${filesChanged.length} changed files for task ${taskId}`);
```

---

## Step 2: Run Typecheck

Run TypeScript compiler to catch type errors:

```typescript
import { runTypecheck } from '../validation/typecheck';

const typecheckResult = await runTypecheck({ projectDir: process.cwd() });

if (!typecheckResult.passed) {
  console.log(`Typecheck FAILED with ${typecheckResult.errors.length} errors`);
  for (const error of typecheckResult.errors) {
    console.log(`  ${error.file}:${error.line} - ${error.message}`);
  }
}
```

---

## Step 3: Run Linter

Run the project linter on changed files only:

```typescript
import { runLinter } from '../validation/linter_check';

const linterResult = await runLinter({
  files: filesChanged,
  projectDir: process.cwd(),
});

if (!linterResult.passed) {
  console.log(`Linter FAILED with ${linterResult.errors.length} errors`);
}
```

---

## Step 4: Run Relevant Tests

Run tests related to the changed files:

```typescript
import { runTests } from '../validation/test_runner';

const testResult = await runTests({
  relatedFiles: filesChanged,
  projectDir: process.cwd(),
});

if (!testResult.passed) {
  console.log(`Tests FAILED: ${testResult.failed_tests.length} test failures`);
}
```

---

## Step 5: Aggregate Results

Combine all validation results:

```typescript
import { aggregateValidationResults, ValidationSummary } from '../agents/validator_agent';

const summary: ValidationSummary = aggregateValidationResults({
  typecheck: typecheckResult,
  linter: linterResult,
  tests: testResult,
});

const allPassed = summary.passed;
console.log(`Validation ${allPassed ? 'PASSED' : 'FAILED'}`);
```

---

## Step 6: Update Orchestrator State

Report results to the orchestrator state:

```typescript
import { updateValidationResult } from '../agents/validator_agent';

await updateValidationResult(taskId, summary, { stateDir: 'state' });

// If validation passed, task moves to 'completed'
// If validation failed, task moves to 'failed' and worker gets error context
```

---

## Validation Result Schema

```typescript
interface ValidationSummary {
  /** Whether all validations passed */
  passed: boolean;
  /** Timestamp of validation */
  validated_at: string;
  /** Individual validation results */
  typecheck: ValidationResult;
  linter: ValidationResult;
  tests: ValidationResult;
  build?: ValidationResult;
  /** Aggregated errors across all validations */
  all_errors: ValidationError[];
  /** Total duration in milliseconds */
  total_duration_ms: number;
}

interface ValidationResult {
  passed: boolean;
  type: 'typecheck' | 'linter' | 'test' | 'build';
  errors: ValidationError[];
  warnings?: ValidationWarning[];
  duration_ms?: number;
}

interface ValidationError {
  file: string;
  line?: number;
  column?: number;
  message: string;
  code?: string;
}
```

---

## On Validation Pass

When all checks pass:

```
=== Validation Passed ===
Task: US-005
Checks:
  [x] Typecheck - 0 errors
  [x] Linter - 0 errors, 2 warnings
  [x] Tests - 12 passed, 0 failed

Status: VALIDATED - Ready for commit
```

The orchestrator will:
1. Mark task as 'completed'
2. Trigger git commit for the task
3. Check if phase is complete

---

## On Validation Failure

When any check fails:

```
=== Validation Failed ===
Task: US-005
Checks:
  [x] Typecheck - 0 errors
  [ ] Linter - 3 errors
  [x] Tests - 12 passed, 0 failed

Errors:
  src/components/PriorityBadge.tsx:15:5 - 'unused' is defined but never used
  src/components/PriorityBadge.tsx:23:10 - Expected indentation of 2 spaces
  src/components/TaskCard.tsx:42:1 - Missing semicolon

Status: FAILED - Returning to worker with error context
```

The orchestrator will:
1. Mark task as 'failed'
2. Inject error context into worker's state
3. Worker continues with error information
4. Track retry count (max 3 retries)

---

## Build Verification (Phase Completion Only)

When all tasks in a phase are completed, run build verification:

```typescript
import { runBuildCheck } from '../validation/build_check';

const buildResult = await runBuildCheck({ projectDir: process.cwd() });

if (!buildResult.passed) {
  console.log('Build FAILED - cannot advance to next phase');
  // Orchestrator will need to identify which task broke the build
}
```

---

## Checklist

For each validation run:

- [ ] Load task completion report
- [ ] Get list of changed files
- [ ] Run typecheck on project
- [ ] Run linter on changed files
- [ ] Run related tests
- [ ] Aggregate all results
- [ ] Update orchestrator state with validation result
- [ ] If passed: signal ready for commit
- [ ] If failed: inject errors into worker state for retry

---

## Integration with Orchestrator

The validator integrates with the orchestration loop:

```
Worker completes task
        |
        v
Completion Report written
        |
        v
Validator triggered
        |
   +----+----+
   |         |
 PASS      FAIL
   |         |
   v         v
Commit    Inject errors
task      to worker
   |         |
   v         v
Phase     Worker
check     retries
```
