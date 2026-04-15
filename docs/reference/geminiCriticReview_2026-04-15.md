---
title: Gemini 외부 리뷰 분석 (베타 공개 전 참고)
status: active
canonical: false
created_at: 2026-04-15
updated_at: 2026-04-15
owner: architect
related:
  - docs/plans/betaReleaseReadinessPlan.md
  - docs/reference/knownIssues_2026-04-12.md
  - src/lib/workflow/reviewWorkflow.ts
  - src/lib/planProposalParser.ts
  - src-tauri/src/guardrail.rs
---

# Gemini 외부 리뷰 분석 (2026-04-15)

외부 LLM(Gemini)에게 코드베이스를 넘겨 시니어 엔지니어 관점 리뷰를 요청한 결과와, 각 지적에 대한 자체 검증 결과. **베타 공개 전 반드시 검토할 것.**

---

## 요약 표

| # | 지적 | 타당성 | 근거 | 대응 |
|---|------|--------|------|------|
| 1 | HTML 마커 정규식 파싱 안티패턴 | 부분 타당 | `planProposalParser.ts` 마커 의존 확인 | WS 전환으로 일부 대체 가능 (§1) |
| 2 | 가짜 테스트 커버리지 (FE 20.5%, invoke mock) | 절반 타당 | Rust 259 tests 무시한 평가 | FE-BE 경계 테스트 보강 |
| 3 | 오케스트레이션 상태가 FE Zustand 종속 | **타당** | `runtimeSlice.ts` 휘발성 상태 확인 | Engine Server Mode (이미 P1) |
| 4 | 샌드박스 부재, `rm -rf` 위험 | **왜곡** | guardrail.rs는 프롬프트 크기 제한, CLI 자체 샌드박스 미인지 | 경고 UI 강화 정도 |
| 5 | Reviewer가 코드 실행 못 함 → self-validation 허상 | **타당** | `reviewWorkflow.ts:58` "빌드/테스트 실행 금지" 명시 | **README 수정 or Tester 분리 필수** |

---

## 1. HTML 마커 정규식 파싱

### Gemini 주장
> LLM이 마커 오타(`<!---`) 또는 닫는 태그 누락 시 오케스트레이션 파이프라인 전체 붕괴.

### 사실 확인
- `src/lib/planProposalParser.ts`에서 `<!-- tunaflow:plan-proposal -->`, `<!-- tunaflow:impl-complete -->`, `<!-- tunaflow:review-verdict -->` 등 HTML 주석 마커를 정규식으로 파싱하여 워크플로우 상태 전이.
- 방어 로직: `extractReviewVerdict` fail-safe, doom loop 감지, fallback phase 존재.

### 맥락 (Gemini가 빠뜨린 것)
- PTY/CLI subprocess 환경에는 **function calling / tool-use 프로토콜이 없음**. stdout 텍스트가 전부.
- 대안:
  - (a) SDK 직통 → 사용자 구독 모델 무력화, API 과금 부담
  - (b) 별도 JSON out-of-band 채널 → CLI가 지원 안 함
- 마커는 CLI-first 제약 하의 합리적 선택이었음.

### WS/Server Mode 전환 관점 (사용자 질문)

**현재 상태**:
- claude: `sdk-url` (WS 래퍼, s36 완료) — SDK tool-use 네이티브 지원 가능
- codex: `app-server` 계획 (s37 목표) — JSON-RPC 기반 구조화 프로토콜
- gemini: CLI만 (WS 없음)
- opencode: CLI만

**결론**: 마커 방식을 **부분 대체 가능**하지만 **완전 대체는 불가**.

| 엔진 | 구조화 tool-use | 마커 필요 |
|------|----------------|----------|
| claude (sdk-url WS) | ✅ 가능 | 선택 (호환성용 유지 권장) |
| codex (app-server WS) | ✅ 가능 (구현 후) | 선택 |
| gemini (CLI) | ❌ | 필수 |
| opencode (CLI) | ❌ | 필수 |

**4-engine parity 원칙** (`build_normalized_prompt_with_budget()` 단일 경로) 때문에, 엔진별로 워크플로우 제어 프로토콜을 분기시키면:
- 코드 복잡도 2배
- RT(roundtable)에서 claude-codex-gemini 혼합 시 상태 전이 규칙이 엔진별로 달라져 재앙
- ContextPack 통일성 깨짐

**권장 방향**:
1. **단기**: 마커 방식 유지. 관측성(파싱 실패 로깅, fallback 경로)만 강화.
2. **중기**: WS 엔진에 한해 **병행 검증** 추가 (마커 + 구조화 tool-use 둘 다 받아서 일치 확인). 불일치 시 마커 우선 (안전측).
3. **장기**: gemini/opencode가 WS 프로토콜 지원하면 그때 마커 deprecate 검토.

**지금 당장 WS로 갈아엎는 건 ROI 없음.** Engine Server Mode 안정화가 먼저.

### 개선 포인트 (수용)
- 마커 파싱 실패 시 **관측/경고** UI — 현재 silent fallback 위주. `extractReviewVerdict` 실패 시 사용자에게 "verdict 판독 실패, 수동 확인 요" 표시 필요.
- 마커 schema를 zod로 버전 관리해 회귀 방지.

---

## 2. 테스트 커버리지

### Gemini 주장
> FE 구문 커버리지 20.5%, `invoke`/`listen` 전부 mock → "껍데기 테스트".

### 사실 확인
- FE 커버리지 수치 맞음
- Tauri IPC mock 처리 맞음 (`src/tests/setup.ts`)

### 맥락 (Gemini가 빠뜨린 것)
- Rust 백엔드 **259 unit tests** 별도 운영 (ContextPack 조립, DB 마이그레이션, JSONL 파싱, vector search 등 핵심 로직은 Rust에서 검증)
- Tauri 앱의 FE-BE 경계 E2E는 CI 비용 큼 — Cursor/Zed도 동일 전략

### 타당한 부분
- FE에서 invoke happy/error path를 타는 테스트가 거의 없음
- 특히 runtimeSlice, threadSlice의 이벤트 핸들러는 복잡도에 비해 검증 약함

### 개선 방향
- Tauri `mockIPC` 헬퍼로 command별 happy/error/timeout 시나리오 테스트 추가
- 우선 대상: `sendWithEngine`, `adoptBranch`, `processReviewVerdict`

---

## 3. 오케스트레이션 상태의 FE 종속 ✅

### Gemini 주장
> 워크플로우 루프가 Zustand에 있어 UI 새로고침 시 증발.

### 사실 확인
- `runtimeSlice.ts`의 이벤트 리스너, 메시지 큐, 활성 RT 상태 등이 메모리 상주
- 앱 재시작 시 `runningThreadIds`가 항상 `[]`로 초기화됨 (`AppShell.tsx:61`)

### 맥락
- **DB = SSOT** 원칙으로 plan/phase/subtask/event는 영속화됨
- 재개 가능하지만 **진행 중인 스트리밍은 끊김**
- 이미 `docs/plans/` 내 **Engine Server Mode** 아키텍처 계획 문서(commit `d6ad489`)로 해결 로드맵 있음

### 대응
- P1 우선순위 유지
- 베타 공개 전 최소한: "앱 재시작 시 진행 중 plan은 멈춤, 재개는 DB에서 수동" 문구 UI 안내

---

## 4. 샌드박스 부재 ❌ (왜곡)

### Gemini 주장
> `guardrail.rs`가 샌드박스인 줄 알았는데 글자수 제한일 뿐. `rm -rf` 직결.

### 반박
- `guardrail.rs`는 파일명이 오해의 소지는 있으나 **ContextPack 크기 제한** 역할 (MAX_TOTAL_PROMPT 60,000자)
- 실제 샌드박스는 **CLI 엔진 자체가 제공**:
  - claude: `--permission-mode ask` / approval UI
  - codex: `--full-auto` / `--sandbox` 플래그 (OpenAI 샌드박스 규칙)
  - gemini: auto-approve 정책
- tunaFlow는 CLI를 subprocess로 실행 → CLI 자체의 approval layer가 작동

### 포지셔닝 차이
- OpenHands/Devin/Optio: **클라우드/SaaS형** → 컨테이너 격리 필수
- tunaFlow: **로컬 데스크톱 AOC** → "사용자가 이미 CLI 쓰고 있는 환경을 오케스트레이션"
- 위험 프로필이 다름. 비교 부적절.

### 남는 위험 (수용)
- PTY 세션에서 사용자가 `--full-auto` / `--dangerously-skip-permissions` 켜면 CLI 자체 approval이 비활성화됨
- 이 경우 tunaFlow가 경고 UI/approval-gate를 제공하면 좋음

### 개선 방향
- 위험 플래그 활성화 시 상단에 경고 배너
- `rm -rf`, `sudo`, `curl | sh` 등 위험 패턴 감지 시 확인 모달 (client-side guardrail)
- 이건 Phase H로 분리 가능

---

## 5. Reviewer Self-validation 허상 ✅ (가장 아픈 지적)

### Gemini 주장
> Reviewer에게 "빌드/테스트 실행 금지"를 명시. Developer가 환각으로 "테스트 통과"라고 거짓말하면 Reviewer가 그대로 pass.

### 사실 확인 (정확)
`src/lib/workflow/reviewWorkflow.ts:58`:
```ts
`당신은 코드 리뷰어입니다. **코드를 읽어서** 검증하세요. 빌드/테스트 명령을 직접 실행하지 마세요.`,
```

`reviewWorkflow.ts:71-73`:
```
2. **Verification 결과 확인**: Developer가 보고한 검증 결과를 확인하세요.
```

→ Developer 자기보고 텍스트만 보고 판정.

### 왜 이렇게 설계했는가
- Developer가 이미 돌린 테스트를 Reviewer가 또 돌리면 비용/시간 2배
- 다중 CLI 병행 실행 시 race condition (동일 파일시스템 writes)
- 초기 PoC에서 "빠른 루프"가 우선이었음

### 문제 (인정)
- README/마케팅: "2-agent 교차 검증으로 self-validation 한계 극복"
- 실제 코드: Reviewer는 Developer 텍스트 읽는 사람
- **주장과 구현이 불일치** — 기만 위험

### 수정 옵션

| 옵션 | 내용 | 비용 | 안전성 |
|------|------|------|--------|
| A | Reviewer도 읽기-only 검증 실행 (`cargo check`, `tsc --noEmit`) | 중 | 중 |
| B | Tester 에이전트 별도 단계 추가 (Dev → Test → Review 3-stage) | 고 | 고 |
| C | README 문구 수정 — "Reviewer=정적 리뷰어, 테스트 실행은 Developer 책임" 명확화 | 저 | 저 (기만 제거만) |
| D | Reviewer가 **랜덤 샘플링으로** test 일부 재실행하여 환각 여부 spot-check | 중 | 중-고 |

### 권장
- **즉시(베타 전)**: C (README 수정) — 기만 제거
- **다음 sprint**: D (랜덤 재검증) — 적은 비용으로 환각 탐지
- **장기**: B (Tester 분리) — 3-role 아키텍처 정립

---

## Gemini 리뷰의 한계 (교차검증용 메모)

외부 LLM 리뷰를 신뢰할 때 주의할 부분:

1. **파일명으로 기능 추정** — `guardrail.rs` 이름 보고 샌드박스로 단정 (실제 내용 안 읽음)
2. **CLI-first 아키텍처 맥락 무시** — 구독 모델/PTY 제약을 모름
3. **Rust 테스트 무시** — FE 커버리지만 보고 "가짜" 단정
4. **포지셔닝 차이 무시** — 클라우드 에이전트(OpenHands)와 로컬 AOC를 동일 선상 비교
5. **로드맵/계획 문서 미참조** — 이미 P1으로 올라간 이슈를 "치명적 미해결"로 단정

→ 외부 리뷰는 **자극 요소**로 쓰고, 각 지적을 **코드와 맥락 문서로 재검증**하는 절차가 필수.

---

## 베타 공개 전 체크리스트 (이 리뷰 반영)

- [ ] **P0**: README의 "2-agent 교차 검증" 문구 수정 (옵션 C) — 기만 제거
- [ ] **P0**: 마커 파싱 실패 시 사용자 경고 UI (silent fallback 제거)
- [ ] **P0**: 앱 재시작 시 진행 중 plan 안내 문구
- [ ] **P1**: 위험 CLI 플래그(`--full-auto`, `--dangerously-skip-permissions`) 활성화 시 경고 배너
- [ ] **P1**: Tauri mockIPC로 핵심 command 경계 테스트 추가
- [ ] **P2**: Reviewer 랜덤 재검증 (옵션 D) 시범 구현
- [ ] **P2**: Engine Server Mode 진행 (계획 문서 기반)
- [ ] **P3**: WS 엔진 한정 구조화 tool-use 병행 검증 (마커는 유지)

---

## 관련 문서

- `docs/plans/betaReleaseReadinessPlan.md` — 베타 체크리스트 원본
- `docs/reference/knownIssues_2026-04-12.md` — 알려진 이슈
- `docs/plans/engineServerModeArchitecturePlan.md` (commit `d6ad489`) — FE 종속 해결 로드맵
- 메모리 `project_marker_tool_calls.md` — 마커 설계 배경
- 메모리 `feedback_no_sdk.md` — CLI-first 아키텍처 근거
