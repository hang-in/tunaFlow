## Summary

<!-- One or two sentences. What changed and why. -->

## Related plan / issue

<!-- e.g. docs/plans/searchPipelineFromSecallPlan-part2.md or #123 -->

## Changes

- <!-- bullet list of the main edits -->

## Test plan

- [ ] `npx tsc --noEmit`
- [ ] `npx vite build`
- [ ] `cd src-tauri && cargo check`
- [ ] `npx vitest run`
- [ ] `cd src-tauri && cargo test --lib`
- [ ] Manual smoke test of the affected feature

## Invariants touched

<!-- If the change modifies or establishes an invariant, note it here. -->

## Screenshots / logs

<!-- Only if relevant. Redact tokens and absolute paths. -->

## Checklist

- [ ] PR title follows Conventional Commits (`feat(scope): ...`)
- [ ] Tests added or updated
- [ ] Docs updated (plan, README, how-to) if behavior changed
- [ ] No secrets, tokens, or personal paths in the diff
