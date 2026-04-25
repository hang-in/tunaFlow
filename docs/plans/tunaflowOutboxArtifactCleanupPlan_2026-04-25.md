---
title: .tunaflow/outbox 폐기 잔재 정리 (4 stale .md 추적 제거 + .gitignore 추가)
status: ready-to-implement (dev 종료 후)
priority: P3 (cosmetic / housekeeping)
created_at: 2026-04-25
related:
  - .tunaflow/outbox/  # 폐기된 PTY 응답 수집 메커니즘 잔재
canonical: true
---

# 배경

`commit 9295062 feat: outbox 방식 폐기 → JSONL 세션 로그 polling으로 응답 수집` 시점에 PTY 응답 수집 메커니즘 (outbox) 자체는 폐기됐는데:

- **`.tunaflow/outbox/*.md`** 4 파일 (`1775854254733.md` ~ `1775854842763.md`) 이 git 에서 삭제 안 됨
- **`.gitignore`** 에 `.tunaflow/outbox/` 추가 안 됨

→ 폐기된 메커니즘의 dead artifact 가 그대로 추적 중. 미래 contributor 가 보면 "이게 왜 있지" 의문.

# 현재 상태 (사실 확인)

```
$ git ls-files .tunaflow/
.tunaflow/outbox/1775854254733.md
.tunaflow/outbox/1775854565931.md
.tunaflow/outbox/1775854713667.md
.tunaflow/outbox/1775854842763.md

$ grep -n 'tunaflow\|outbox' .gitignore
25:src-tauri/tunaflow.db
38:tunaflow.db
```

폐기 이후 outbox 디렉터리에 새 파일 생성 흔적 없음. 메커니즘 실제로 unused.

# 수정

## (1) 4 파일 git rm

```bash
git rm .tunaflow/outbox/1775854254733.md \
       .tunaflow/outbox/1775854565931.md \
       .tunaflow/outbox/1775854713667.md \
       .tunaflow/outbox/1775854842763.md
```

내용 자체는 폐기된 PTY 응답 fixture 라 보존 가치 없음.

## (2) `.gitignore` 에 추가

```
# .tunaflow/ 디렉터리 — 폐기된 outbox 잔재 + 향후 동적 산출물 차단
.tunaflow/outbox/
```

또는 더 보수적으로 `.tunaflow/` 전체 ignore (현재 outbox 외에 tracked 파일 없으면).

## (3) 빈 디렉터리 자동 정리

git rm 후 디렉터리가 비면 git 이 자동 정리. macOS 의 `.DS_Store` 같은 OS 메타파일 남으면 별도 처리 필요.

# Invariants

- **[INV-1]** 머지 후 `git ls-files .tunaflow/` 결과 0
- **[INV-2]** `.gitignore` 에 `.tunaflow/outbox/` 또는 `.tunaflow/` 명시 — 미래 outbox 메커니즘 부활해도 그 단계에서 재논의 강제

# 검증

- 머지 후 `find . -path ./.git -prune -o -name .tunaflow -print` 결과: 디렉터리 자체는 사라지거나 빈 상태 (untracked)
- `npx tsc --noEmit` / `cargo check` regression 0 (코드 영향 없음)

# Developer 핸드오프 프롬프트

```
[작업] .tunaflow/outbox 폐기 잔재 정리 (Plan tunaflowOutboxArtifactCleanup, P3 housekeeping)

[SSOT] docs/plans/tunaflowOutboxArtifactCleanupPlan_2026-04-25.md

[수정 범위]

1) git rm .tunaflow/outbox/*.md (4 파일)
2) .gitignore 에 .tunaflow/outbox/ 추가 (or .tunaflow/ 전체)
3) 검증: git ls-files .tunaflow/ → 0

[커밋]
chore(meta): remove stale outbox artifacts from polling-deprecated era

trailer: Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[PR 제목]
chore(meta): cleanup .tunaflow/outbox stale artifacts (post-9295062)
```

# 셀프 이슈 본문 초안

```markdown
## Summary

`.tunaflow/outbox/*.md` 4 files have been tracked since the now-deprecated PTY outbox response-collection mechanism (replaced by JSONL session log polling in commit `9295062`). The mechanism removal didn't include `git rm` for the existing artifacts or `.gitignore` entry.

## Cleanup

- `git rm .tunaflow/outbox/*.md` (4 files)
- Add `.tunaflow/outbox/` to `.gitignore`

Per `docs/plans/tunaflowOutboxArtifactCleanupPlan_2026-04-25.md`. P3 housekeeping.
```
