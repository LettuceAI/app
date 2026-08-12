# Contributing

This repository values changes that are easy to review, consistent with the existing architecture, and maintainable after merge.

## Community

- If possible, join the project's Discord server and stay aware of what the community is asking for and experiencing.
- Pay attention to recurring pain points, common workflows, and feature requests.
- Do not build only from local assumptions. Use community feedback to guide priorities and validate decisions.

## Commit Messages

- Use Conventional Commits format for commit messages.
- Prefer clear, scoped messages such as:
  `feat: add session export support`
  `fix: prevent duplicate save requests`
  `refactor: simplify settings state handling`
  `docs: clarify architecture guidelines`
  `test: cover message serialization`

## AI Usage

- AI tools may be used as an assistive tool, but not as a substitute for engineering judgment.
- Contributors must understand the code they submit, be able to explain it, and verify that it is correct.
- Do not submit code produced through vibe coding, blind prompting, or trial-and-error generation without real understanding of the implementation.
- Treat AI output like untrusted code until it has been reviewed and validated properly.
- Do not submit large AI-generated changes that you cannot maintain yourself.

## Before Opening a Pull Request

- Keep the change focused. Do not combine unrelated fixes or refactors in one pull request.
- If the work is large, invasive, or changes product behavior significantly, discuss it first.
- Update all affected flows when a feature touches more than one surface.
- Test your changes before opening a pull request.
- Update documentation when behavior, architecture, or developer expectations change.

## After Opening a Pull Request

- Expect requests for changes. Review quality and long-term maintainability matter more than getting a feature merged quickly.
- Call out tradeoffs, assumptions, and incomplete areas clearly.
- Keep the branch up to date if the pull request becomes stale.

## Coding Guidelines

- Follow existing patterns before introducing new abstractions.
- Prefer the simplest change that fits the codebase cleanly.
- Avoid broad refactors unless they are necessary for the work being done.
- Keep functions focused and reasonably small.
- Write code that is easy to trace during review.
- Add comments only when they explain non-obvious intent.
- Avoid dead code, placeholder code, and temporary local-environment workarounds in production changes.

## Architecture Guidelines

- Core logic should live on the Rust side.
- While adding features, prefer implementing business logic, computation, normalization, validation, and decision-making in Rust.
- The frontend should primarily handle presentation, user interaction, view state, and displaying results.
- Do not move feature behavior or business rules into the frontend when they belong in the backend.
- If the same rule or transformation would otherwise be duplicated across multiple screens, move it into a shared layer or the Rust side.

## Frontend Guidelines

- Keep frontend code focused on rendering, event handling, and local UI state.
- Avoid embedding business rules or heavy computation in React components and hooks unless there is a clear UI-only reason.
- Prefer clear, composable components over large components with mixed responsibilities.

## Backend Guidelines

- Preserve data and configuration intentionally. Normalize only when there is a clear reason.
- Avoid one-off exceptions in shared code paths unless they are necessary and well-contained.
- Keep backend boundaries and integrations consistent with existing conventions.
- Prefer explicit behavior over hidden fallback logic.

## Platform Scope

- Consider desktop and mobile impact when changing shared behavior.
- Separate platform-specific fixes from feature work unless they are strictly required.
