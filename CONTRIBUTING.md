# Contributing to LettuceAI

LettuceAI is a private, local-first AI roleplay app for Android, iOS, Windows, macOS and
Linux. Changes can touch users' chats, characters, memories, API keys, local models,
backups and data synced between devices. A pull request must be based on the code that
is here, not on how a typical Tauri or chat application might work.

Bug fixes and small, self-contained improvements can go straight to a pull request.
Open an issue before starting a large feature, a new subsystem, a change to stored
data, or anything that can migrate, sync or delete user data.

## Community

Join the project's Discord server if you can, and pay attention to what users are
asking for and struggling with. Use that feedback to choose priorities and to check
your assumptions, instead of building only from what seems right locally.

## Rust owns application behavior

**Rust is the source of truth for LettuceAI's data and behavior.** Business logic,
validation, prompt assembly, provider requests, memory, sync, storage, file handling
and credentials belong in the Rust crates under `crates/`.

**The frontend displays the result and handles interaction.** It owns form drafts,
view state, animation and presentational formatting. It must not reimplement a rule
that decides what gets stored, sent to a model, synced, deleted or imported.

Follow the structure that is already there. Each crate has a README describing what it
owns; read the ones your change touches before starting, and put new code where the
existing code for that area lives instead of adding a parallel path.

A few rules matter everywhere:

- Text sent to a model belongs in the built-in prompt catalog
  (`crates/lettuce-app/resources/built-in-prompts/`), not in Rust code.
- API keys and tokens belong in the native secret store, never in the database,
  settings, logs or frontend state.
- `bash scripts/check-architecture.sh` checks the dependency rules between crates. If
  it fails, the change is in the wrong place; do not work around it.

## User data comes first

Users trust LettuceAI with long conversations and characters they wrote themselves.

- **Never lose data.** Imports, backups, restores, migrations and sync must keep every
  record or record exactly why one was skipped.
- Operations that replace user data need a recoverable sequence: validate first, write
  to a staging location, switch over atomically, and keep the old data until the new
  state is verified.
- Changes to stored or synced shapes need migrations, backward-compatible defaults, and
  the sync versioning rules described in the `lettuce-sync` README.

## Tests and checks

Run from the repository root before opening a pull request:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
bash scripts/check-architecture.sh
```

Match the existing tests:

- use in-memory databases and temporary directories, never real user data;
- test old and partially migrated data when changing persistence;
- test cancellation, interruption and rollback for multi-step operations;
- test the failure paths, not only the happy path;
- name tests after the behavior they prove.

UI changes need a manual pass in the running application, on mobile and desktop when
both are affected. State what you actually tested; do not imply coverage you did not
perform.

## Pull requests

Keep the change focused. Do not mix a feature with unrelated cleanup, formatting or
dependency updates. Review the complete diff for API keys, personal chats, model files,
build output and debug code.

**Commit messages and pull request titles follow Conventional Commits.** Use a type such
as `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build` or `chore`, add a scope when
it helps, and describe the change in the imperative mood:

```text
feat(chat): add message search
fix(settings): save the theme when the app restarts
docs: explain how to run the app on Android
```

The description explains the problem, the implementation and the observed result.
Call out data migrations, platform limits, compatibility behavior and follow-up work.
Expect requests for changes: long-term maintainability matters more than merging
quickly.

## AI and generated code

**We do not accept vibe-coded contributions.** LettuceAI is maintained by people, and
every line that is merged has to be understood, owned and maintained by the person who
submitted it.

Limited AI assistance is tolerated, carefully:

- An AI tool may help you read unfamiliar code, explain an API, suggest a small focused
  change, or review work you are actively directing.
- It must not replace your understanding or your judgment. **You** read the relevant
  code, choose the approach, review every generated line, test the real workflow, and
  can explain and defend the result in review.
- Treat AI output as untrusted code. Verify every API, file path and assumption it
  produces; tools invent them.
- **Disclose any material use of generated code in the pull request.** Autocomplete,
  spelling and formatting help do not need disclosure.

The following are not accepted:

- handing an issue to a tool and submitting what it produced;
- prompting repeatedly until the build or tests pass, without understanding why;
- large generated changes, broad churn, or generated tests that only restate the
  implementation;
- replying to review comments by passing them to a tool and pasting the answer back.

A pull request may be closed without further review when its author cannot explain the
code, when it relies on invented APIs or behavior, or when it shows any of the patterns
above. Repeated vibe-coded submissions may lead to a block from the repository.
Passing CI does not show that a change fits LettuceAI, preserves user data, or handles
failures.

## Reporting bugs

Search the existing issues first, then use the bug report form. Include what you did,
what happened, what you expected, the shortest reliable steps to reproduce it, the
LettuceAI version from Settings (or the commit), your platform and device, and the
provider and model when generation is involved.

**Remove API keys, tokens and any chat content you do not want to share** from logs and
screenshots before attaching them.

**Never report a security vulnerability in a public issue.** Contact the maintainers
privately instead.
