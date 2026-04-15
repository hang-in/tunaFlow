---
title: 메타에이전트가 프로젝트 초기 구성(에이전트·페르소나·스킬·설정)까지 세팅
status: planned
priority: P1   # metaAgentOnboardingPlan 다음 단계. 베타 전에 들어가면 좋음
created_at: 2026-04-16
related:
  - docs/plans/metaAgentOnboardingPlan_2026-04-16.md   # 전 단계 (에이전트 선택 + CLAUDE.md/reference 생성)
  - src/lib/defaultPersonas.ts
  - src/stores/slices/assetSlice.ts                     # agent_profiles 초기값
  - src/components/tunaflow/settings/AgentsSection.tsx
  - src-tauri/src/commands/project_onboarding.rs
---

# 메타에이전트 초기 구성 자동화

## 1. 배경

현재 온보딩의 메타에이전트 분석 결과물은 **CLAUDE.md + docs/reference/index.md** 두 파일에 한정. 이후 사용자가 수동으로:

- Agent Profile 선택/생성 (Architect Claude / Developer Codex 등)
- Persona 지정
- 관련 Skill 활성화
- Workflow track(Quick/Deep), RT 참가자, ContextPack 모드 등 앱 설정

각각을 Settings 돌며 만져야 함. 메타에이전트가 프로젝트 스택을 이미 **읽었으므로** 이 중 상당 부분을 **추천값으로 미리 세팅**할 수 있음.

## 2. 목표

온보딩 분석 단계에서 메타에이전트가 세 가지를 더 **제안**(사용자 승인 후 적용):

1. **Agent Profile 추천** — 프로젝트 스택 × 작업 성격 기반
2. **관련 Skill 자동 활성화** — rawq가 인덱스할 skills 중 프로젝트와 매칭되는 것
3. **Workflow 기본값** — Quick/Deep 트랙, RT 참가자, ContextPack mode

각 항목은 **사용자가 체크박스로 on/off 선택 가능**. 강제 적용 X.

## 3. 추천 로직

### 3.1 Agent Profile

메타에이전트가 CLAUDE.md 분석 후 다음 쌍(3-role)을 추천:

| 스택 | 추천 Architect | Developer | Reviewer |
|------|---------------|-----------|----------|
| Rust / Go / Swift / 시스템 | Claude Opus | Codex | Gemini (정적 분석 강점) |
| TS/React/Next | Claude Opus | Codex | Claude (type-level 추론) |
| Python ML | Claude Opus | Codex | Gemini |
| 1인 개인 프로젝트 | Claude Opus (3 역할 전부) | — | — |
| 로컬 LLM 선호(API 미설정) | Ollama | Ollama | Ollama |

→ 사용자가 선택한 메타에이전트(Claude or Ollama 등) 와 설치된 CLI 조합 기반. 설치 안 된 엔진은 후보에서 자동 제외.

### 3.2 Skill 활성화

기존 `skills/` 레지스트리에서 스택 키워드 매칭:

- 프로젝트 스택에 `rust` → `rust-review`, `cargo-test` 스킬 추천
- `react` → `react-testing-library`, `component-design`
- `tauri` → `tauri-plugin-audit`
- `python + ML` → `pytest-coverage`, `model-eval`

현재 사용자의 `~/.tunaflow/skills/` 안에 실제 로드 가능한 스킬 목록과 매칭.

### 3.3 Workflow 기본값

- Plan Review track: 소규모(`subtask ≤ 3`) → Quick / 중대규모 → Deep
- RT 참가자: 설치된 CLI 중 2~3개
- ContextPack mode: 프로젝트 파일 수 기반 Lite/Standard/Full

## 4. UI 흐름

```
Preview 단계(현재 CLAUDE.md + reference.md) 다음에
┌── 프로젝트 초기 구성 추천 ─────────────────────┐
│  메타에이전트가 이 프로젝트에 맞는 기본 구성을  │
│  제안합니다. 체크된 항목만 적용됩니다.         │
│                                              │
│  ▸ Agent Profiles                            │
│    ☑ Architect Claude   (claude-opus-4-6)   │
│    ☑ Developer Codex    (gpt-5-codex)       │
│    ☐ Reviewer Gemini    (gemini-2.5-pro)    │
│                                              │
│  ▸ Recommended Skills                        │
│    ☑ rust-review                             │
│    ☑ cargo-test                              │
│    ☐ tauri-plugin-audit                      │
│                                              │
│  ▸ Workflow Defaults                         │
│    Review track:   [Deep ▼]                  │
│    ContextPack:    [Auto ▼]                  │
│                                              │
│           [ 건너뛰기 ]  [ 선택 항목 적용 ]    │
└──────────────────────────────────────────────┘
```

## 5. 출력 형식 (메타에이전트 프롬프트 확장)

기존 `[CLAUDE_MD_START] ... [REF_INDEX_START]` 뒤에 새 섹션 추가:

```
[INITIAL_SETUP_START]
{
  "agent_profiles": [
    { "role": "architect", "engine": "claude", "model": "claude-opus-4-6", "persona_id": "persona_architect" },
    { "role": "developer", "engine": "codex",  "model": "gpt-5-codex",     "persona_id": "persona_implementer" },
    { "role": "reviewer",  "engine": "gemini", "model": "gemini-2.5-pro", "persona_id": "persona_reviewer" }
  ],
  "skills": ["rust-review", "cargo-test"],
  "workflow": {
    "review_track": "deep",
    "context_mode": "auto",
    "rt_participants": ["claude", "codex", "gemini"]
  },
  "rationale": "Rust + Tauri 2 프로젝트. 3-role 분리 권장. 시스템 프로그래밍에 익숙한 Codex 를 구현, 정적 분석 강한 Gemini 를 리뷰로 배치."
}
[INITIAL_SETUP_END]
```

Rust parse 후 Frontend 에 JSON 전달 → UI 체크박스 렌더 → 사용자 confirm 시 실제 store/DB 반영.

## 6. 변경 범위

| 파일 | 종류 | 변경 |
|------|------|------|
| `project_onboarding.rs` | 수정 | 프롬프트에 `[INITIAL_SETUP_START/END]` 출력 요구 + parse. payload JSON 을 `project:onboarding:preview` 이벤트에 추가 |
| `ProjectOnboardingModal.tsx` | 수정 | 프리뷰 탭에 "Initial Setup" 추가 + 체크박스 UI |
| 신규 `initialSetupApply.ts` (frontend) | 신규 | 선택된 agent profiles/skills/workflow 를 store/DB 에 반영 |
| `src/lib/skillsRegistry.ts` or 기존 | 수정 | 사용 가능 skill 목록 제공 API |
| `src-tauri/src/commands/projects.rs` | 수정 | agent_profiles 초기값을 프로젝트별로 저장하는 컬럼 (필요 시 migration) |
| `defaultPersonas.ts` | 참조 | persona_id 후보 제공 |

## 7. 안전 장치

- 메타에이전트 JSON이 손상되면 **건너뛰기** (CLAUDE.md/reference 는 이미 생성됨)
- 설치되지 않은 엔진이 추천되면 체크박스 disabled + `not installed` 표기
- 추천 skill 이 레지스트리에 없으면 자동 제외

## 8. 단계별 로드맵

1. Rust: 프롬프트 확장 + JSON 파서 + event payload 확장 (~1일)
2. Frontend: Initial Setup 체크박스 UI + apply 로직 (~1.5일)
3. 통합 테스트 — 5개 스택 샘플 프로젝트로 추천 품질 확인 (~0.5일)
4. 실패 케이스 처리 + 문서 정리 (~0.5일)

**합 ~3~4일**.

## 9. 의존 / 순서

- `metaAgentOnboardingPlan_2026-04-16.md` (구현 완료됨 — PR #23, #24) 다음 단계
- 베타 직전에 이 기능까지 들어가면 "프로젝트 연결 = 1분 내 완전 세팅" UX 달성 가능. **베타 차단 아님** (현재도 수동 설정 가능)

## 10. 남은 질문

- 추천 persona_id 는 `DEFAULT_PERSONAS` 에서만 고를지 vs 사용자 커스텀 persona 까지 고려할지
- workflow defaults 를 프로젝트별 영속화할 DB 컬럼 vs appSettings 전역 값
- "rationale" 문자열을 UI 에 표시할지 (사용자가 왜 이 추천이 나왔는지 알 수 있게) — 권장 표시
