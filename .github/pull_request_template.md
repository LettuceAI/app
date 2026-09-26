<!--
Use a Conventional Commit title, for example:
feat(chat): add message search
fix(settings): save the theme when the app restarts
-->

## Summary

<!-- What changed? Keep this focused on the behavior reviewers need to understand. -->

## Why

<!-- What problem does this solve, and why is this the right approach for LettuceAI? Link the issue when there is one. -->

## Testing

<!-- List the commands and manual workflows you used. Include platforms, providers or local models, and devices when relevant. -->

- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `bash scripts/check-architecture.sh`

## Visual changes

<!-- Add before and after screenshots or a short recording, on mobile and desktop when both are affected. Remove this section when the change has no visible effect. -->

## AI disclosure

<!--
Required. LettuceAI does not accept vibe-coded contributions; see CONTRIBUTING.md.
State "None" or describe exactly which parts were produced with an AI tool, which tool,
and how you reviewed and verified them. Autocomplete, spelling and formatting help do
not need to be listed.
-->

## Checklist

- [ ] I reviewed the complete diff and removed unrelated changes.
- [ ] I tested the affected workflow in the application.
- [ ] I followed the existing structure and patterns described in CONTRIBUTING.md and the affected crates' READMEs.
- [ ] Existing user data keeps working after this change.
- [ ] I added or updated tests where the changed behavior can be tested reliably.
- [ ] I updated documentation when setup, behavior or contributor expectations changed.
- [ ] I did not include API keys, tokens, personal chats, models, build output or debug code.
- [ ] I wrote or directed every change myself, understand all of it, and can explain it in review. This is not vibe-coded work.
- [ ] Any AI-generated code was reviewed line by line, its APIs and assumptions verified, and it is disclosed above.
- [ ] I read CONTRIBUTING.md.
