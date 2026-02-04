# Project Rules

## Required Before Completion
- [ ] README.md exists
- [ ] All tasks complete
- [ ] Builds with `cargo build`
- [ ] Runs without panics

## Worker Guidelines
- Always check context/ folder first
- Follow Rust conventions (snake_case, etc.)
- Update progress file with learnings
- Keep dependencies minimal

## Code Standards
- Run `cargo fmt` before commits
- Run `cargo clippy` and fix warnings
- Use safe Rust where possible
- Document public APIs

## Performance Rules
- Profile before optimizing
- Prefer GPU compute for heavy calculations
- Maintain 60fps target
