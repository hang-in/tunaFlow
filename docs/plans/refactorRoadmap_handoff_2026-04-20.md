# 베타 전 리팩토링 — 새 세션 핸드오프 (2026-04-20)

> **이 문서를 읽고 있는 Claude 에게**
>
> 당신은 tunaFlow 의 **프로덕션급 베타 공개 직전 리팩토링 작업** 을 이어받았습니다. 이 문서는 당신이 맥락을 잃지 않고 올바른 방향으로 진행하도록 준비된 핸드오프입니다. **먼저 끝까지 읽고 명확해진 뒤에** 작업에 착수하세요.

---

## 0. 당신이 반드시 지켜야 할 최우선 규칙

1. **로드맵 밖 일 하지 말 것**: `docs/plans/refactorRoadmap_2026-04-20.md` 에 있는 것만 한다. 없는 것 (신기능, 임의 리팩토링) 은 사용자 지시 없이 추가하지 않음
2. **스코프 혼합 금지**: 하나의 Finding / 작업 = 하나의 PR. 다른 영역이 눈에 거슬려도 이번 PR 에 섞지 않음
3. **사용자 가시 동작 변경 없음** (refactoring Phase 기준): 외부 관찰 동일해야 함. 버그가 있어 보여도 그건 별도 PR
4. **테스트 baseline 유지**: Rust 295 unit + 25 integration / FE 222 vitest / TSC 0. 내려가면 즉시 revert
5. **Stash 절대 건드리지 말 것**: `git stash drop/clear/pop` 금지 (사용자 메모리 rule)
6. **머지 자동화 금지**: 각 PR 은 CI 녹색 확인 후 사용자 지시 또는 명시적 예약에 따라서만 머지
7. **HMR 주의**: 사용자가 `npm run tauri dev` 돌리고 있을 수 있음. 대량 파일 변경 (branch switch, merge) 전 상태 확인 권장

---

## 1. 반드시 먼저 읽을 것 (순서 준수)

```
1. CLAUDE.md (root)
   → 프로젝트 개요, 기술 스택, 세션 핸드오프 규칙 §15, 작업 안전 규칙, 코딩 컨벤션

2. docs/plans/refactorRoadmap_2026-04-20.md          ← 이번 작업의 전체 설계
   → 5 Phase · 16~19일 · Phase 별 근거 / 순서 / 완료 기준

3. 본 문서 (refactorRoadmap_handoff_2026-04-20.md)   ← 지금 읽고 있는 것
   → 재개 지점 / 프로젝트 철학 / 함정

4. docs/api-inquiry-gamma-delta.md
   → γ/δ API 실사 응답 + stability 태그 (Phase 2 작업 시 참조)

5. MEMORY.md (자동 로드됨)
   → 사용자 선호, 과거 결정, 실수 사례 누적
```

**아래 는 해당 작업 영역 진입 시에만**:
- `docs/reference/architecture-detail.md` — RT/Branch/Store 상세
- `docs/reference/sessionHistory.md` — 특정 과거 결정 맥락 필요 시만
- `docs/reference/implementationStatus.md` — 기능별 현재 상태
- `docs/reference/multiAgentContextStrategy.md` — ContextPack 관련 작업 시

---

## 2. 프로젝트 철학 (반드시 이해)

### 2.1 Of the agent, By the agent, For the agent
tunaFlow 는 **다중 에이전트 오케스트레이션 클라이언트 (AOC)** 입니다.
- 사용자는 **도메인 지식과 방향을 결정**하는 주체
- 에이전트는 그 결정을 **최적 조건에서 실행**
- **에이전트가 편해야 결과가 좋아진다** — ContextPack, identity, memory, retrieval 등 모든 설계가 "에이전트가 불필요한 토큰 낭비 없이, 정확한 맥락으로, 역할 혼동 없이" 를 기준으로 판단됨

### 2.2 역할 분리 (PR #81 에서 확정)
- **Architect** (`docs/agents/architect.md`) — 설계 전담, 사용자 명시 요청 시만 Plan 작성. 시스템이 자동 호출하지 않음
- **Developer** (`docs/agents/developer.md`) — 구현 전담
- **Reviewer** — 검토 전담
- **Meta** (`docs/agents/meta.md`) — 프로젝트 조망 + 제안. "제안하되 결정하지 않는다". 이벤트 수집 허브
- **Synthesizer** — RT 종합

절대 하지 말 것: 예를 들어 "Plan 완료 후 다음 우선순위 제안" 같은 걸 Architect 에게 자동 전송. 이는 Meta 역할이고 PR #81 에서 제거된 안티패턴이다.

### 2.3 CLI-first
- CLI (claude, codex, gemini, opencode) 가 기본 경로
- SDK 는 fallback (compression 등 특수 경로)
- **Rust 백엔드 API 직접 호출 비권장** — 구독 사용자 중심

### 2.4 Project-centric
- 모든 데이터는 Project 소속
- 프로젝트 삭제는 soft-hide (hidden=1)
- Store 는 선택된 프로젝트 데이터만 보유

### 2.5 Background execution
- `start_*` 커맨드 = DB 준비 후 즉시 반환 → background thread subprocess → 이벤트 통지
- DB = SSOT, 이벤트 놓쳐도 `list_messages()` 로 복구

---

## 3. 현재 상태 (2026-04-20 기준)

### 3.1 최근 머지된 PR (리팩토링과 관련)
- **PR #80** (2026-04-20): 모바일 γ/δ API 지원 — plan detail 확장, Meta HTTP, WS broadcast, B' 계층 inspection 도구 (probe/fetch_slice/full_message)
- **PR #81** (2026-04-20): Architect/Meta 역할 분리 — Plan 완료 시 Architect 자동 호출 제거, Meta 인박스가 단일 알림 채널
- **PR #82** (2026-04-20): Plan drafting 게이트 — 'Subtask 검토' 버튼이 모든 subtask details 채워진 후에만 노출 + "아키텍트 문서 작성 중..." 스피너
- **PR #76~#79** — s38 sprint (19 작업묶음), 메모리 품질 업그레이드, compression eager trigger

### 3.2 테스트 baseline
- **Rust**: 295 unit + 25 integration = 320 passed
- **FE**: 222 passed / 18 files
- **TSC**: 0 errors
- **Clippy**: 3 pre-existing warnings (무관, 건드리지 않음)

### 3.3 DB 버전
- **v39** (최신). Phase 2 의 `2-2. Branch detail` 에서 **v40 마이그레이션** 추가 예정 (`adopted_message_id` 컬럼)

### 3.4 현재 브랜치
- main 기준 진행. 작업 시 각 Finding 별 `refactor/<finding-name>` 또는 `feat/<feature>` 브랜치 생성

### 3.5 첫 작업 진입점
**Phase 1 Finding 6: `lib.rs` 부트스트랩 분해 (0.5일)** — 가장 작은 단위. 로드맵 §2.1 `1-6` 섹션 참조

---

## 4. 프로젝트 구조 핵심 (한 눈)

```
tunaFlow/
├── src-tauri/              # Rust backend
│   ├── src/lib.rs          # ★ Phase 1-6 작업 대상 (부트스트랩 분해)
│   ├── src/agents/         # CLI agent adapters (claude, codex, gemini, opencode, rawq)
│   ├── src/commands/       # Tauri commands + http_api/ 모듈
│   ├── src/db/migrations.rs # v1~v39, Phase 2 에서 v40 추가 예정
│   └── src/http_api/       # axum REST + WS bridge
├── src/                    # React frontend
│   ├── components/tunaflow/  # UI
│   ├── stores/slices/      # ★ Phase 1-1/-3/-4 작업 대상
│   ├── lib/workflow/       # ★ Phase 1-5 작업 대상 (서비스화)
│   └── types/index.ts
└── docs/
    ├── plans/              # 실행 계획 (40+)
    ├── reference/          # SSOT
    ├── how-to/             # 운영 가이드
    └── agents/             # 에이전트 role.md
```

---

## 5. 피해야 할 함정 (과거 사례)

### 5.1 "대체 경로 없이 기존 제거"
- 2026-03-29 사고: RT branch 를 드로어로 전환하면서 드로어에 RT 지원 없는 상태로 full view 진입점 제거 → RT 기능 전체 사라짐
- **규칙**: UI 진입점 / 기존 동작 제거 전에 **대체 경로가 end-to-end 로 동작** 함을 먼저 증명

### 5.2 "단일 경로 수정"
- 한 번에 여러 실행 경로 동시 변경 금지
- RT/Branch/Thread 같이 여러 모드 얽힌 기능은 모드별로 분리 수정 → 검증 → 다음

### 5.3 "사이드이펙트 미체크"
- 컴포넌트 교체 시 해당 컴포넌트의 **모든 기능 경로** 나열하고 새 컴포넌트가 동일 커버 확인
- Store 상태 변경 시 해당 상태를 읽는 **모든 곳** grep 으로 확인

### 5.4 "Dead code 제거는 기능 검증 후"
- 순서: 신기능 추가 → 검증 → 구기능 제거. 역순 금지

### 5.5 "silent fallback"
- dev 단계에서 에러 조용히 삼키지 말 것 — toast/console 로 표면화
- 메모리 규칙: `feedback_error_visibility.md` 참조

---

## 6. 사용자 상호작용 스타일

### 6.1 항상 존댓말 (한국어)
- user-facing 답변은 반드시 한국어 존댓말
- 반말 절대 금지

### 6.2 작업 승인 모델
- **큰 변경 / 파괴적 작업**: 반드시 사용자 확인 후 진행
- **destructive 조작**: git reset --hard, branch -D, force push 등 — 사용자 명시 승인 필수
- **stash 은 절대 drop/pop/clear 금지** (반복 강조)
- **자동 머지 금지** — CI 녹색 + 사용자 "머지" 지시 또는 명시적 예약

### 6.3 착수 전 제안
- Phase 내 각 Finding 진입 시: **먼저 구체 작업 계획 (어느 파일 어떻게) 을 사용자에게 제시** 하고 승인 후 착수
- 사용자가 "바로 진행" 이라고 하면 스스로 단일 PR 원칙 지켜가며 수행

### 6.4 소통 톤
- 간결, 구체적, 확정적. 모호한 선택지 나열 금지
- "파일명:줄번호" 형식으로 코드 위치 인용
- 긴 배경 설명 지양. 결론 먼저

---

## 7. 진행 체크리스트 (이 세션에서 할 일)

Phase 1 Finding 6 단일 작업:

- [ ] 로드맵 + 본 문서 + CLAUDE.md 읽기 완료
- [ ] `src-tauri/src/lib.rs` 읽고 `run()` 함수 11 단계 파악
- [ ] 작업 계획 (어느 모듈 만들고 어느 라인 어떻게 이전할지) 사용자에게 제시
- [ ] 사용자 승인 후 브랜치 `refactor/lib-rs-bootstrap-split` 생성
- [ ] `src-tauri/src/bootstrap/{env,db,services,window}.rs` 생성
- [ ] `lib.rs` 에서 각 단계 호출로 치환
- [ ] `cargo check --lib` → 0 errors
- [ ] `cargo test --lib` → 295 pass + 25 integration pass (baseline 유지)
- [ ] commit + push + PR 생성
- [ ] CI 녹색 확인 → 사용자에게 보고 → 머지 지시 대기

Finding 6 완료 후 **사용자에게 다음 Finding (1-3) 착수 여부 질의**. 자동으로 넘어가지 말 것.

---

## 8. 예상 함정 (Finding 6 특화)

- `lib.rs` 의 `run()` 함수 안에서 `app` 변수를 여기저기 공유. 모듈로 분리 시 시그니처 맞추기 주의
- DB 경로 결정 로직이 dev vs release 분기 있음 — 둘 다 유지
- `tauri_plugin_*` 초기화는 `tauri::Builder::default()` 체인에 있음 — 이건 `run()` 에 남기고 내부 setup 만 분리
- `inherit_shell_path()` 가 OS-specific (`#[cfg(target_os = "macos")]`) — 그대로 유지
- WAL mode 적용, stale message cleanup, agent_jobs cleanup 은 DB bootstrap 의 일부
- HTTP API token 은 `services::http_api` 에서 초기화 후 `.manage()` 로 등록

자세한 건 `lib.rs` 직접 읽고 판단.

---

## 9. 성공 판단

이 세션이 성공적으로 끝났다는 건:
- Finding 6 PR 이 열리고 CI 녹색
- 사용자가 내용을 리뷰할 수 있는 상태
- 테스트 baseline 유지
- 나머지 Phase 1 진행은 다음 세션에서

**과소 성공 주의**: 빠르게 끝나도 스코프 폭주하지 않음. "다음 Finding 도 하자" 욕심 금물.

---

## 10. 문의·의심이 들면

- 로드맵 `docs/plans/refactorRoadmap_2026-04-20.md` 내용과 본 문서가 **충돌하면 로드맵이 우선**
- CLAUDE.md 의 안전 규칙이 다른 문서와 충돌하면 **CLAUDE.md 가 우선**
- 그래도 불확실하면 **사용자에게 물어보고 진행**. 추측으로 결정하지 말 것

---

**끝.** 이제 위의 "1. 반드시 먼저 읽을 것" 을 순서대로 읽은 뒤, §7 체크리스트 첫 항목부터 시작하세요.
