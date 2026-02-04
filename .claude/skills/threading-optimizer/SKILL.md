---
name: threading-optimizer
description: "Analyze code for concurrency improvements. Identifies parallelization opportunities, race conditions, and recommends async/threading patterns."
---

# Threading Optimizer Skill

Analyze code for concurrency improvements and potential issues.

## Purpose

Review code to:
- Identify parallelization opportunities
- Detect race conditions and concurrency bugs
- Suggest async/await, threading, or multiprocessing improvements
- Recommend appropriate concurrency patterns

## Instructions

When analyzing code for threading optimization:

1. **Identify Bottlenecks**:
   - I/O-bound operations (network, file, database)
   - CPU-bound operations (computation, data processing)
   - Blocking calls that could be async
   - Sequential loops over independent items

2. **Detect Concurrency Issues**:

   **Race Conditions**:
   - Shared mutable state without locks
   - Check-then-act patterns
   - Non-atomic compound operations

   **Deadlocks**:
   - Multiple locks acquired in different orders
   - Circular wait conditions
   - Lock held during blocking I/O

   **Resource Contention**:
   - Lock granularity too coarse
   - Excessive synchronization
   - Thread pool exhaustion

3. **Recommend Solutions**:

   **For I/O-bound (Python)**:
   - `asyncio` with `async/await`
   - `aiohttp` for HTTP calls
   - `asyncpg` for database
   - `concurrent.futures.ThreadPoolExecutor`

   **For CPU-bound (Python)**:
   - `multiprocessing.Pool`
   - `concurrent.futures.ProcessPoolExecutor`
   - Consider: `numba`, `cython` for hot loops

   **For JavaScript/TypeScript**:
   - `Promise.all()` for parallel async
   - `Promise.allSettled()` when failures shouldn't stop others
   - Web Workers for CPU-bound

   **Synchronization Patterns**:
   - Locks/Mutexes for critical sections
   - Semaphores for resource limiting
   - Queues for producer-consumer
   - Read-write locks for read-heavy workloads

4. **Output Format**:

```
## Analysis Summary

**Code Type**: I/O-bound / CPU-bound / Mixed
**Current Approach**: Sequential / Threaded / Async
**Opportunity Level**: High / Medium / Low

## Issues Found

### Issue 1: [Description]
**Location**: file.py:123
**Severity**: Critical / Warning / Info
**Problem**: ...
**Fix**: ...

## Optimization Recommendations

### Recommendation 1: [Title]
**Impact**: High / Medium / Low
**Effort**: High / Medium / Low

**Before**:
```python
# current code
```

**After**:
```python
# optimized code
```

**Explanation**: ...
```

## Key Principles

- Concurrency adds complexity; only add it where there's measurable benefit
- Prefer async/await for I/O-bound over threads (simpler, more scalable)
- Use multiprocessing for CPU-bound work in Python (GIL limitation)
- Always consider: "What happens if this runs twice simultaneously?"
- Test concurrent code thoroughly; bugs are often intermittent
- Measure before and after; optimization should be data-driven
