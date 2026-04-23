---
title: Work Safety Rules
updated_at: 2026-04-24
canonical: true
status: active
owner: tunaFlow-core
---

# Work Safety Rules

변경을 할 때 따라야 하는 안전 규칙. CLAUDE.md 에서 분리 (컨텍스트 절약). **코드/UI/스토어를 바꾸는 작업 시작 전에** 읽는다.

## 1. 실행 경로 검증 우선

- **UI 진입점을 변경하기 전에** 대체 경로가 완전히 동작하는지 반드시 확인한다.
- 기존 동작을 제거/교체할 때는 새 동작이 end-to-end 로 작동하는 것을 먼저 증명한다.
- "나중에 구현" 을 전제로 기존 기능을 제거하지 않는다.

## 2. 단일 경로 수정 원칙

- 한 번에 여러 실행 경로를 동시에 바꾸지 않는다.
- 하나의 경로를 수정 → 검증 → 다음 경로 순서로 진행한다.
- 특히 RT / Branch / Thread 같이 여러 모드가 얽힌 기능은 모드별로 분리 수정한다.

## 3. 사이드 이펙트 체크

- 컴포넌트를 교체할 때 해당 컴포넌트가 사용하던 **모든 기능 경로**를 나열하고, 새 컴포넌트가 동일하게 커버하는지 확인한다.
- Store 상태를 바꿀 때 해당 상태를 읽는 **모든 컴포넌트 / 훅**을 grep 으로 확인한다.
- dead code 제거는 기능 검증 완료 후에만 한다.

## 4. 병렬 세션 격리 (git worktree)

여러 Developer / Architect 세션이 **동시에** 돌아갈 때는 반드시 독립된 working directory 를 쓴다.

### 규칙

- **세션 하나당 브랜치 하나 + worktree 하나**. 같은 repo 체크아웃을 세션끼리 공유하지 않는다.
- 새 병렬 세션 시작 전:
  ```bash
  git worktree add ../tunaFlow-<slice> feat/i18n-pr-<slice>
  cd ../tunaFlow-<slice>
  # 여기서 세션 작업
  ```
- 세션 종료 + 머지 후 정리:
  ```bash
  git worktree remove ../tunaFlow-<slice>
  ```
- **Architect** 는 Developer 가 작업 중인 `feat/*` 브랜치에 **절대 직접 push 하지 않는다**. 별도 `docs/*` 브랜치에서만 작업하고 PR 경로로 머지한다.
- **Developer uncommitted 작업이 있는 브랜치를 Architect 가 checkout 하지 않는다** — stash/복구 과정에서 손실 위험.

### 왜

- 같은 working directory 를 세션이 공유하면 한 세션의 `git checkout` / `git stash` / `git reset` 이 다른 세션의 uncommitted 작업을 건드린다.
- i18n 병렬 스프린트 중 실제로 Architect 가 Developer 세션 브랜치에 커밋을 쌓았다가 revert 하면서 stash 복구로 복잡해진 전례가 있다 (세션 41, 2026-04-24).

## 5. 과거 사고 사례

- **2026-03-29**: RT branch 를 드로어로 전환하면서 드로어에 RT 지원이 없는 상태에서 full view 진입점 제거 → RT 기능 전체 사라짐. **대체 경로가 없는데 기존 경로를 제거한 것이 원인.**
- **2026-04-22**: `finalize_engine_run` 이 write lock hold 중 `app.emit` 으로 동기 listener 를 트리거 → `broadcast_event` 가 같은 lock 재진입 → deadlock. **스크롤 lock / mutex 의 re-entrant 여부는 항상 확인**.
- **2026-04-24**: 병렬 i18n 스프린트 중 Architect 가 Developer 의 `feat/i18n-pr-a2g-chat` 브랜치에 직접 push 하려다 remote 거부 + stash/reset 복구 경로로 진입. **§4 병렬 세션 격리 규칙 신설의 직접 계기.**

## 6. 세션 핸드오프 규칙

세션이 끝나거나 context 압축이 발생할 때:

1. **완료된 것과 안 된 것을 구분해서 기록** — "X 완료, Y는 미완 (이유: Z)" 형식. 모호한 "대부분 완료" 금지.
2. **변경한 파일 목록을 명시** — 파일 경로 + 변경 내용 요약. 다음 세션에서 grep 없이 파악 가능하도록.
3. **미완성 작업의 구체적 재개 지점** — "A 파일의 B 함수에서 C 를 추가해야 함" 수준. "이어서 하면 됨" 금지.
4. **사이드이펙트 경고** — 변경으로 인해 다른 부분에 영향 가능성이 있으면 명시. "X 를 바꿨으므로 Y 를 확인해야 함".
5. **테스트 상태** — `cargo test` / `vitest` 결과. 실패한 것이 있으면 원인 + 재현 방법.
6. **`sessionHistory.md` 는 과거 맥락 필요할 때만** — 매 세션 시작 시 전체를 읽지 않음. 특정 과거 결정이 필요하면 그때 참조.
