# Contributing to tunaFlow

Thanks for your interest. tunaFlow is a multi-agent orchestration client built almost entirely by AI agents through a plan-driven workflow. Human contributors are welcome; please read this short guide first.

## Plan-driven workflow

tunaFlow development follows a four-step loop:

1. **Plan** — write a short spec in `docs/plans/<name>Plan.md` (TL;DR / Specification / Invariants / Rationale).
2. **Implement** — small, focused PRs. Each PR references a plan section or subtask.
3. **Review** — at minimum one reviewer. Large changes go through a Roundtable (RT) review.
4. **Merge** — squash merge with a Conventional Commit title.

For non-trivial changes, open a draft plan before writing code. This prevents wasted effort on designs that conflict with invariants.

## Development setup

See `INSTALL.md` for environment setup. Quick check:

```bash
npm install
npm run tauri dev
```

Build verification:

```bash
npx tsc --noEmit
npx vite build
cd src-tauri && cargo check
```

## Testing

```bash
npx vitest run                      # frontend
cd src-tauri && cargo test --lib    # Rust
```

New features require tests. Bug fixes should include a regression test.

## Commit style

Use Conventional Commits:

- `feat(scope): ...` — new user-facing feature
- `fix(scope): ...` — bug fix
- `refactor(scope): ...` — no behavior change
- `docs(scope): ...` — documentation only
- `chore(scope): ...` — tooling / infra

Example: `feat(search): add hybrid RRF across FTS and vector`

## Pull requests

- Keep PRs small. Prefer 200–500 changed lines. Split large changes.
- Link the related plan or issue in the description.
- Include a short test plan (what you ran, what you observed).
- CI must pass before review.

## Code style

- TypeScript: strict mode, no `any` unless justified.
- Rust: `cargo fmt` and `cargo clippy -- -D warnings` clean.
- Follow existing patterns in the file you edit. Consistency beats personal preference.

## Questions

Open a GitHub Discussion for design questions, or an Issue for bugs and concrete feature requests.

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0 (see `LICENSE`).
