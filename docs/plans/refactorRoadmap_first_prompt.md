# 새 세션 첫 프롬프트 — 베타 전 리팩토링

> 사용법: 아래 **>>> COPY BEGIN <<<** ~ **>>> COPY END <<<** 사이 텍스트를 복사해서 새 Claude 세션에 붙여넣으세요.

---

>>> COPY BEGIN <<<

# tunaFlow 베타 전 리팩토링 세션 시작

이 세션은 **tunaFlow 의 프로덕션급 베타 공개를 위한 리팩토링 및 안정화** 작업을 이어받습니다. 이전 세션에서 로드맵 + 핸드오프 문서가 준비되어 있습니다.

## 착수 전 필수 절차 (생략 금지)

**아래 문서를 순서대로 전부 읽으세요. 이해 완료 전까지는 어떤 코드 수정도 금지.**

```
1. CLAUDE.md (root)                                         — 프로젝트 개요 + 세션 핸드오프 규칙 + 안전 규칙 + 코딩 컨벤션
2. docs/plans/refactorRoadmap_2026-04-20.md                 — 이번 작업의 전체 5-Phase 설계 (필수 숙지)
3. docs/plans/refactorRoadmap_handoff_2026-04-20.md         — 이 세션의 재개 지점 + 프로젝트 철학 + 피할 함정
4. MEMORY.md                                                 — 이미 자동 로드됨. 사용자 선호/과거 결정 기록
```

위 1~3 번을 읽지 않고 추정으로 진행하는 것은 **금지**. 과거 세션에서 이로 인한 사고 사례가 있습니다.

## 이 세션의 범위

**Phase 1 - Finding 6: `src-tauri/src/lib.rs` 부트스트랩 분해**

- 로드맵 §2.1 `1-6` 섹션 참조
- 범위: Rust backend 만. Frontend 변경 없음
- 사용자 가시 동작: **동일해야 함**
- 예상 소요: 0.5일

**이 Finding 하나만** 처리합니다. 다른 Finding (1-1, 1-3, 1-5, 1-2, 1-4, Phase 2/3/4/5) 에 손대지 않습니다.

## 이 세션에서 금지되는 것

- Phase 1 Finding 6 외 다른 Finding 작업
- 새 기능 추가
- 사용자 가시 동작 변경 (refactoring 단계 규칙)
- 테스트 baseline 내려뜨리기 (Rust 295 unit / 25 integration / FE 222 vitest / TSC 0)
- `git stash drop/pop/clear` (사용자 메모리 규칙 — 절대)
- 자동 머지 (사용자 명시 지시 또는 명시적 예약 후에만)
- "사이드 이펙트가 있으니 리팩토링 범위를 넓혀도 되겠다" 식 스코프 확장

## 우선 할 일

1. 위 문서 4개 읽기
2. `src-tauri/src/lib.rs` 의 `run()` 함수를 읽고 11 단계 초기화 로직 파악
3. **작업 계획 제시** — 어느 bootstrap 모듈 (`env.rs`, `db.rs`, `services.rs`, `window.rs`) 에 어느 라인을 어떻게 옮길지 **구체 시퀀스** 를 사용자에게 먼저 보여주세요
4. 사용자 승인 후 브랜치 `refactor/lib-rs-bootstrap-split` 생성 → 작업 시작
5. `cargo check --lib` + `cargo test --lib` baseline 유지 확인 후 PR 생성
6. CI 녹색 확인 후 사용자에게 **머지 지시 대기** (자동 머지 금지)

## 성공 기준

- Finding 6 PR 오픈 + CI 녹색
- Rust 테스트 baseline 유지 (295 unit + 25 integration)
- 사용자 가시 동작 동일 (앱 시작 · DB 연결 · HTTP API 기동 모두 기존과 같음)
- 사용자가 리뷰 가능한 상태로 완료
- 다음 Finding (1-3 send pipeline) 은 **다음 세션에서**

## 주의

- 사용자는 `npm run tauri dev` 를 실제로 돌리고 있을 수 있습니다. 대량 파일 변경 (branch switch, merge) 전 사용자에게 알릴 것.
- 응답은 **한국어 존댓말** 필수. 반말 금지.
- 착수 계획 제시는 **간결 + 구체적 파일·라인 인용**. 모호한 선택지 나열 금지.

---

시작하세요. 먼저 문서 4개 읽고, 그 다음 Finding 6 착수 계획 제시부터.

>>> COPY END <<<

---

## 체크 — 프롬프트를 보낸 뒤 사용자가 확인할 것

새 Claude 가 다음 중 하나를 하면 정상:
- 위 문서 4개를 순서대로 읽는 tool call 수행
- 현재 상태 파악 완료 후 Finding 6 착수 계획을 **간결하게 제시**
- 사용자 승인을 **기다림**

다음 중 하나를 하면 **비정상 — 즉시 중단시키고 문서 읽기부터 다시**:
- 문서 읽지 않고 lib.rs 바로 수정 시작
- Finding 6 외 다른 영역 건드림
- 여러 Finding 묶어서 진행 제안
- 사용자 확인 없이 머지 / push
- stash 건드림

## 업데이트 이력

- 2026-04-20: 초안 작성
