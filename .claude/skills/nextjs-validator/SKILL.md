---
name: nextjs-validator
description: "Automated visual and console validation for Next.js applications. Captures console logs, checks mobile/desktop responsiveness, validates UX/UI rules, and creates fix tasks."
---

# Next.js Validator Skill

Automated validation for Next.js applications using **agent-browser** (NOT Playwright). This skill checks:
- Console errors and warnings
- React hydration mismatches
- Element overflow (mobile responsive)
- Touch target sizes
- General layout issues
- Accessibility basics

---

## When to Use

This validator should run:
1. **After task completion**: Validate changes before marking task complete
2. **Before merging**: Final check that everything works
3. **During development**: Catch issues early
4. **On demand**: When debugging visual/responsive issues

---

## Configuration

Create `.nextjs-validator.json` in project root:

```json
{
  "enabled": true,
  "base_url": "http://localhost:3000",
  "dev_server_command": "npm run dev",
  "startup_delay": 8,
  "pages": ["/", "/about", "/products"],
  "viewports": ["mobile", "desktop"],
  "check_console_errors": true,
  "check_hydration": true,
  "check_overflow": true,
  "check_touch_targets": true,
  "min_touch_target_size": 44,
  "check_responsive": true,
  "auto_detect_pages": true,
  "auto_create_fix_tasks": true,
  "fail_on_critical": true,
  "fail_on_high": false
}
```

---

## Viewports Tested

| Name | Size | Description |
|------|------|-------------|
| mobile | 375x812 | iPhone X / standard mobile |
| tablet | 768x1024 | iPad / standard tablet |
| desktop | 1920x1080 | Standard 1080p monitor |

---

## Issue Categories

### CRITICAL (Must Fix)
- **Console Errors**: JavaScript runtime errors
- **Hydration Mismatches**: Server/client HTML mismatch
- **Page Load Failures**: Page doesn't load

### HIGH (Should Fix)
- **Overflow Issues**: Elements extending beyond viewport (especially mobile)
- **Touch Targets**: Buttons/links smaller than 44x44px on mobile

### MEDIUM (Nice to Fix)
- **Console Warnings**: Non-critical warnings
- **Accessibility**: Missing alt text, etc.

### LOW (Optional)
- **Performance Suggestions**: Optimization opportunities

---

## Integration with Worker Loop

### Pre-Check (Before Starting Task)

```
Worker Loop Start:
1. Run nextjs-validator on affected pages
2. Document current issues in notes_for_next
3. Proceed with task implementation
```

### Post-Check (After Completing Task)

```
Worker Loop End:
1. Run nextjs-validator on affected pages
2. Compare with pre-check results
3. If new issues introduced → Fix before completing
4. If all clear → Mark task complete
```

---

## Invoking from Worker State

Workers can request validation by adding to their state file:

```json
{
  "plugin_invocations": [
    {
      "plugin": "nextjs_validator",
      "command": "validate",
      "args": {
        "pages": ["/", "/products"],
        "viewports": ["mobile", "desktop"],
        "start_server": true
      }
    }
  ]
}
```

Results appear in `plugin_results`:

```json
{
  "plugin_results": [
    {
      "plugin": "nextjs_validator",
      "status": "completed",
      "passed": false,
      "report_id": "nextjs_myproject_20260128_120000",
      "summary": {
        "total_pages": 4,
        "total_issues": 5,
        "critical_issues": 0,
        "high_issues": 3,
        "medium_issues": 2
      },
      "issues": [
        {
          "category": "overflow",
          "severity": "high",
          "message": "Element overflows by 50px: button.add-to-cart",
          "url": "/products",
          "viewport": "mobile",
          "suggested_fix": "Use 'w-full max-w-[X]' instead of fixed widths."
        }
      ]
    }
  ]
}
```

---

## Automatic Fix Task Creation

When issues are found, the validator can automatically create fix tasks:

```json
{
  "id": "FIX-001",
  "title": "Fix overflow issues on /products",
  "description": "Validation found 3 overflow issues...",
  "acceptance_criteria": [
    "Fix all overflow issues listed above",
    "Test on mobile (375px) viewport",
    "Test on desktop (1920px) viewport",
    "No console errors",
    "Run Next.js validator to confirm fix"
  ],
  "status": "pending",
  "priority": "high",
  "source": "nextjs_validator"
}
```

---

## Common Issues and Fixes

### Overflow Issues

**Problem**: Element extends beyond viewport width on mobile.

**Fix patterns**:
```jsx
// ❌ Bad - fixed width
<div className="w-[500px]">

// ✅ Good - responsive width
<div className="w-full max-w-[500px]">

// ❌ Bad - flex without wrap
<div className="flex gap-4">

// ✅ Good - flex with wrap
<div className="flex flex-wrap gap-4">
// Or stack on mobile
<div className="flex flex-col sm:flex-row gap-4">
```

### Touch Target Issues

**Problem**: Button/link smaller than 44x44px on mobile.

**Fix patterns**:
```jsx
// ❌ Bad - small icon button
<button className="p-1">
  <Icon />
</button>

// ✅ Good - touch-friendly size
<button className="min-h-[44px] min-w-[44px] p-2 touch-manipulation">
  <Icon />
</button>
```

### Hydration Mismatches

**Problem**: Server HTML doesn't match client HTML.

**Fix patterns**:
```jsx
// ❌ Bad - browser-only code
function Component() {
  const width = window.innerWidth; // Fails on server
  return <div>{width}</div>;
}

// ✅ Good - useEffect for browser-only
function Component() {
  const [width, setWidth] = useState(0);
  useEffect(() => {
    setWidth(window.innerWidth);
  }, []);
  return <div>{width || 'Loading...'}</div>;
}

// ✅ Good - use client directive
'use client';
function Component() {
  // Now safe to use browser APIs
}
```

### Console Errors

**Problem**: JavaScript errors in console.

**Common causes**:
- Undefined variables
- Missing API responses
- Type mismatches
- Failed network requests

**Fix**: Check error message, add proper error handling:
```jsx
// ❌ Bad - no error handling
const data = await fetch('/api/data').then(r => r.json());

// ✅ Good - with error handling
try {
  const res = await fetch('/api/data');
  if (!res.ok) throw new Error('Failed to fetch');
  const data = await res.json();
} catch (error) {
  console.error('Error:', error);
  // Handle gracefully
}
```

---

## CLI Usage

Run validation manually:

```bash
# From project directory
python -c "
import asyncio
from pathlib import Path
from app.services.nextjs_validator import run_nextjs_validation

async def main():
    report = await run_nextjs_validation(
        project_id='myproject',
        project_dir=Path('.'),
        start_server=True
    )
    print(f'Status: {report.status}')
    print(f'Issues: {len(report.all_issues)}')
    for issue in report.all_issues:
        print(f'  [{issue.severity.value}] {issue.message}')

asyncio.run(main())
"
```

---

## Requirements

Install agent-browser CLI:

```bash
npm install -g agent-browser
agent-browser install  # Downloads Chromium
```

**IMPORTANT:** Do NOT use Playwright MCP tools (`mcp__playwright__*`). Always use agent-browser CLI or the MagicMv2 plugins (BrowserTestingPlugin, AgentBrowserPlugin, VisualValidatorPlugin).

---

## Integration with Design Critic

The validator works with the Design Critic agent:

1. Validator finds issues
2. Design Critic reviews screenshots
3. Design Critic debates severity and fixes with orchestrator
4. If disagreement → escalate to user
5. If agreement → create fix tasks and assign to workers

---

## Checklist for Workers

Before marking a UI task complete:

- [ ] Run Next.js validator on affected pages
- [ ] Check mobile viewport (375px) - no overflow
- [ ] Check desktop viewport (1920px) - layout correct
- [ ] No console errors
- [ ] No hydration warnings
- [ ] Touch targets ≥ 44x44px on mobile
- [ ] All issues resolved or documented
- [ ] **Used agent-browser CLI (NOT Playwright MCP tools)**

## Browser Automation Example

```bash
# ✓ CORRECT: Use agent-browser CLI
agent-browser open http://localhost:3000
agent-browser snapshot -i --json  # Get accessibility tree with refs
agent-browser screenshot mobile.png
agent-browser console --level error
agent-browser close

# ❌ WRONG: Do NOT use Playwright MCP tools
# mcp__playwright__browser_navigate  ← PROHIBITED
# mcp__playwright__browser_snapshot  ← PROHIBITED
```
