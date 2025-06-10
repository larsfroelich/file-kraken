# AGENT Guidelines

This repository uses AGENTS.md to capture reminders for Codex.

## Style
- Keep code changes short and concise.
- Avoid unnecessary variables; inline simple logic where possible.
- Add short comments to group code into logical blocks
- Don't combine `#[cfg(test)]` with `#[test]`; the `#[test]` attribute already implies `cfg(test)`.
- Skip a `mod tests` wrapper when all test functions use `#[test]` or `#[cfg(test)]`.
- Use positive conditions in `if` statements for better readability. For example,
  prefer `if collection.is_empty() { None } else { Some(collection) }` over
  checking with negation.

## Workflow
- Always run `cargo fmt` to ensure consistent formatting.
- Run `cargo test` to verify functionality before committing.
- Use `cargo clippy` to check for common mistakes and improve code quality.
- If feedback is received, incorporate it in this file if needed.

