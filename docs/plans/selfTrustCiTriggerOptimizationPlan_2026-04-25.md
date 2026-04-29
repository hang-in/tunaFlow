---
title: Self-trust CI trigger 최적화 — main 직접 push 시 CI skip (외부 PR + release 만 검증)
status: applied (2026-04-25, ci.yml 직접 수정)
priority: P1 (인지 부담 fragmentation 해소 — 모든 미래 작업 흐름에 영향)
created_at: 2026-04-25
related:
  - .github/workflows/ci.yml      # 본 변경 위치
  - .github/workflows/build.yml   # release tag (v*.*.*) trigger 담당
canonical: true
owners:
  - architect (사용자 + 본 plan 작성)
---

# 배경 — 인지 비용 분석

## 사용자 보고 (2026-04-25)

> "플랜이 누적된 건 CI 탓이 커. 하나 할 때마다 몇 분씩 잡아 먹으니까 PR 을 빨리 하면 되는데 그거 기다리다가 인간이 정체돼"

CI 5~6분 wait 의 진짜 비용은 시간 자체가 아니라 **사용자 working memory fragmenting**. 매 PR 마다 wait → 컨텍스트 끊김 → 다음 작업 시 재로딩 cost. 8~10 PR batch 면 1~2시간 wait + 누적 컨텍스트 재로딩 → "꼬이는 느낌" 의 root cause.

## CI 가치 평가

| 시나리오 | CI 가치 | 본 plan 결정 |
|---|---|---|
| 외부 contributor PR (베타 첫날 batmania52 4건) | ✅ 신뢰 못함, 검증 필수 | **유지** — pull_request trigger |
| Self-trust 작업 (Architect / Developer subagent batch) | 로컬에서 cargo check / tsc / cargo test / vitest 다 돌림 → CI redundant | **제거** — main push trigger 삭제 |
| Release tag push (v0.1.x → v0.2.x) | DMG/app 빌드 + 환경 차이 통합 검증 | **build.yml 이 이미 cover** — ci.yml 에 추가 불필요 |
| macOS 로컬 vs Linux 환경 차이 | 정당. 단 PR / release 시점에서 검출 가능 | (옵션) nightly cron 보류 — 필요 시 후속 plan |

# 변경 (이미 적용)

`.github/workflows/ci.yml` 의 `on:` 섹션:

```yaml
# Before
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
    paths-ignore: [...]

# After
on:
  pull_request:
    branches: [main]
    paths-ignore: [...]
  # main push trigger 삭제. release 는 build.yml (tags: v*.*.*) 이 담당.
```

# Invariants

- **[INV-1]** Self-trust main 직접 push (Architect / Developer subagent / 사용자 본인) 는 ci.yml CI 안 돔. PR 머지 후 자동 main push 도 마찬가지
- **[INV-2]** 외부 contributor PR (`pull_request`) 은 기존 그대로 CI 작동
- **[INV-3]** Release tag push (`v*.*.*`) 는 build.yml 이 cover (DMG 빌드 + 환경 검증)
- **[INV-4]** Self-trust 작업의 검증 책임은 **로컬** (cargo check / tsc / cargo test / vitest). 위배 시 main 빌드 깨질 위험

# 위험 + 보완

## 위험 1 — macOS 로컬 vs Linux 환경 차이

사용자 / Developer subagent 가 로컬 macOS 에서만 검증. Linux 빌드 (target triple `x86_64-unknown-linux-gnu`) 의 깨짐 가능성. 매 push CI 가 안 돔.

**검출 시점**:
- PR 시 (외부 contributor 가 다른 환경)
- Release tag 시 (build.yml)
- 사용자 / Developer 가 깨뜨린 채 release tag 시점에 발견 → 늦음

**보완 (옵션, 후속 plan 후보)**:

```yaml
on:
  schedule:
    - cron: '0 2 * * *'  # 매일 UTC 02:00 (KST 11:00)
  pull_request: ...
```

nightly cron 으로 main 환경 차이 자동 검출. 비용 적음 (self-hosted runner). **본 plan 은 보류** — release 시점 검증으로 일단 충분 평가, 깨진 사례 나오면 도입.

### 갱신 (2026-04-29) — cross-OS 예외 활성화

> **예외: OS 회귀는 redundant 가 아님.** cross-OS workflow (Linux + Windows) 는 본 plan 의 *단일 OS self-trust 가정* 의 한계라, 부분적으로 PR-trigger 외 (push to main / schedule) 로 보강 가능. 상세: [`docs/plans/windowsCiPipelinePlan_2026-04-29.md`](./windowsCiPipelinePlan_2026-04-29.md).

근거: 2026-04-29 Windows architect 가 path-separator 회귀 5건 발견 (PR #213 + escalate 4건) — 모두 Linux CI 통과 후 main 진입. 단일 OS self-trust 의 한계가 데이터로 확정. cross-OS 회귀 한정으로 PR-trigger 외 보강 (nightly cron + main push, 향후 self-hosted Windows runner) 채택. PR-level windows-latest 매트릭스는 working-memory 비용 우려로 채택 X (Phase 3 재검토).

## 위험 2 — 정책 인지 부재로 future architect 가 PR 모델 복원

미래 architect 세션이 "왜 main push CI 안 돔" 의문 시 본 plan 의 의도 / 근거 모르고 ci.yml 의 `on: push: branches: [main]` 다시 추가할 가능성.

**보완**: 본 plan SSOT + ci.yml 의 코멘트에서 본 plan 명시 reference (적용 완료).

## Revert 절차 (필요 시)

문제 발생 시:

```yaml
# .github/workflows/ci.yml on: 섹션에 한 줄 추가 복원
on:
  push:
    branches: [main]
  pull_request: ...
```

한 줄 추가 / 1 commit 으로 복원 가능. self-trust 모델이 깨지면 즉시 fallback.

# 후속 / Sibling

- **(보류) nightly cron 추가** — main 환경 차이 검출. release 시점 검증이 부족하다고 판단되는 시점 도입
- **사용자 / Developer subagent 의 로컬 검증 routine** — `npm run tauri build` 가 너무 무거우니 `cargo check` + `cargo test --lib` + `npx tsc --noEmit` + `npx vitest run` 4 종 충분. CLAUDE.md §12 에 이미 명시
