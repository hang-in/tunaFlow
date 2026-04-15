---
title: 메타에이전트 온보딩 UX (Ollama/LMStudio 추가, OpenCode 제거, 세부 모델 선택)
status: ready_for_impl
priority: P0   # 베타 진입 전 필수
created_at: 2026-04-16
related:
  - docs/ideas/onboardingMetaAgentIdea.md
  - src-tauri/src/commands/project_onboarding.rs
  - src-tauri/src/lib.rs (ENGINE_CONFIGS 기반 등록)
  - src/lib/engineConfig.ts
  - src/components/tunaflow/ProjectStartup.tsx
---

# 메타에이전트 온보딩 UX

## 1. 배경

현재 온보딩은 `project_onboarding::analyze_project_for_onboarding` 이 **claude CLI 를 무조건 호출**. claude 없거나 GUI 번들 PATH 이슈면 "분석 실패"로 끝나고 사용자는 "건너뛰기"로 빈 템플릿 진입. 몇 가지 문제:

- **Claude 고정**: 다른 에이전트(Codex/Gemini/Ollama/LMStudio) 있어도 못 씀
- **세부 모델 선택 불가**: Opus vs Sonnet, Ollama의 llama3 vs qwen2 등
- **건너뛰기 시 맥락 안내 없음**: 사용자가 차이를 모름 → 그냥 진행
- **OpenCode는 실사용 적은 데 엔진에 포함** (`openai_compat.rs` 있어 Ollama/LMStudio가 실제로 더 유용)

## 2. 목표

- 프로젝트 생성 시 **메타에이전트 선택 UI**
- 사용 가능한 에이전트 자동 감지(PATH + HTTP endpoint)
- **세부 모델 드롭다운** (CLI: 캐탤로그 / Ollama: `/api/tags` / LMStudio: `/v1/models`)
- 건너뛰기 시 **차이 안내 + 기본 스캐폴딩 유지**
- OpenCode 제거, Ollama/LMStudio 일등시민화

## 3. 엔진 재구성

### 3.1 전후

| 이전 | 이후 |
|------|------|
| claude | claude |
| codex | codex |
| gemini | gemini |
| ~~opencode~~ | ~~(제거)~~ |
| ollama | ollama |
| — | **lmstudio (신규)** |

### 3.2 LMStudio

- OpenAI 호환 서버 → 기존 `openai_compat.rs` 재사용. 신규 어댑터 불필요
- 기본 endpoint: `http://localhost:1234/v1`
- `/v1/models` 로 로컬 설치된 모델 enumerate

### 3.3 Ollama 확장

- 기본 endpoint: `http://localhost:11434`
- `/api/tags` 로 로컬 설치된 모델 enumerate
- 이미 `openai_compat` 지원 중, 모델 선택 UI만 추가

## 4. Rust: 에이전트 감지 커맨드

`src-tauri/src/commands/agent_detect.rs` (신규)

```rust
#[derive(serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentDetection {
    pub engine: String,                // claude / codex / gemini / ollama / lmstudio
    pub installed: bool,
    pub version: Option<String>,       // CLI: `<cmd> --version`
    pub path: Option<String>,          // CLI: which; HTTP: endpoint
    pub endpoint: Option<String>,      // HTTP 엔진만
    pub models: Vec<String>,           // HTTP: /api/tags 또는 /v1/models
    pub note: Option<String>,          // 에러/힌트
}

#[tauri::command]
pub async fn detect_available_agents(
    ollama_endpoint: Option<String>,   // UI에서 사용자 편집 가능
    lmstudio_endpoint: Option<String>,
) -> Vec<AgentDetection>;
```

- CLI 엔진: `which <bin>` + `--version` (병렬 실행)
- HTTP 엔진: reqwest GET, timeout 1.5s. 실패는 `installed=false`로만 기록 (에러 레이어 노출 X)
- 세부 모델은 CLI 엔진의 경우 `engine_models` (이미 있는 카탈로그) 사용, HTTP는 라이브 조회

## 5. Rust: 온보딩 일반화

`project_onboarding.rs` 의 `call_claude` 를 `call_agent(engine, model, endpoint)` 로 교체:

- `engine = "claude" | "codex" | "gemini"`: 기존 `agents::{claude,codex,gemini}::run(input)` 호출
- `engine = "ollama" | "lmstudio"`: `openai_compat::run(input)` + model/endpoint 파라미터
- 실패 시 동일 에러 메시지 (현재 UX 유지)
- `analyze_project_for_onboarding(input: OnboardingInput)` 시그니처 확장:
  ```rust
  struct OnboardingInput { project_path, engine, model, endpoint: Option<String> }
  ```

## 6. Frontend UI — `MetaAgentSelector`

### 6.1 모달 구조

```
┌── 메타 에이전트 선택 ──────────────────────────┐
│  프로젝트 탐색과 기본 문서 생성에 사용할          │
│  에이전트를 선택하세요.                         │
│                                              │
│  ┌──────────────────────────────────────┐   │
│  │ ◉ Claude                  [detected] │   │
│  │   Model:  claude-opus-4-6     ▼      │   │
│  │   Path:   ~/.nvm/.../claude          │   │
│  └──────────────────────────────────────┘   │
│  ┌──────────────────────────────────────┐   │
│  │ ○ Ollama                  [detected] │   │
│  │   Endpoint: http://localhost:11434   │   │
│  │   Model:    llama3.1:70b   ▼         │   │
│  └──────────────────────────────────────┘   │
│  ┌──────────────────────────────────────┐   │
│  │ ○ LMStudio                [detected] │   │
│  │   Endpoint: http://localhost:1234/v1 │   │
│  │   Model:    qwen2.5-coder    ▼       │   │
│  └──────────────────────────────────────┘   │
│  ┌──────────────────────────────────────┐   │
│  │ ✗ Codex                   [not found]│   │
│  │   [설치 안내 보기]                    │   │
│  └──────────────────────────────────────┘   │
│                                              │
│  [ 건너뛰기 ]            [ 확인 (진행) ]  ←비활성↔활성 │
└──────────────────────────────────────────────┘
```

### 6.2 상호작용 규칙

- **Endpoint 필드**: 기본값 표시 + 사용자가 편집 가능. 변경 시 재감지(debounce) 로 모델 목록 갱신
- **Model 드롭다운**: 해당 엔진 detected=true 일 때만 활성
- **`[확인]` 버튼**: **에이전트(+모델)가 1개라도 선택된 상태일 때만 활성**. 아무것도 선택 안 했으면 disabled
- **`[건너뛰기]` 버튼**: 언제나 클릭 가능. 클릭 시 **2차 확인 다이얼로그** 표시 (§6.3)

### 6.3 건너뛰기 2차 확인

```
┌── 메타에이전트 없이 진행 ─────────────────────┐
│  메타에이전트는 프로젝트 구조를 자동 분석해      │
│  다음을 생성합니다:                            │
│    • CLAUDE.md 프로젝트 요약                  │
│    • docs/agents/{architect,developer,       │
│      reviewer}.md 역할 지침                   │
│    • 초기 plans 폴더 스캐폴드                 │
│                                              │
│  건너뛰면 **기본 템플릿**만 생성됩니다:         │
│    • 비어있는 CLAUDE.md (수동 채우기)         │
│    • 기본 agents/*.md (샘플)                  │
│  나중에 Settings → Project 에서 재실행         │
│  가능합니다.                                  │
│                                              │
│          [ 취소 ]       [ 건너뛰고 진행 ]     │
└──────────────────────────────────────────────┘
```

- "건너뛰고 진행": 기존 `ensure_project_workflow_templates` (이미 존재) + 기본 CLAUDE.md 스캐폴드만 실행
- "취소": MetaAgentSelector 로 복귀

## 7. 진행 플로우

```
ProjectStartup (프로젝트 경로 선택)
   ↓
detect_available_agents 호출 → 상태 저장
   ↓
MetaAgentSelector 모달
   ├─ [확인]  → selected {engine, model, endpoint?} 저장
   │         → analyze_project_for_onboarding(selected)
   │         → apply_project_onboarding(결과 반영)
   └─ [건너뛰기] → 2차 확인
              ├─ [취소] → 모달 복귀
              └─ [건너뛰고 진행] → ensure_project_workflow_templates
                                 + 기본 CLAUDE.md 스캐폴드만
```

## 8. 변경 범위

| 파일 | 종류 | 변경 |
|------|------|------|
| `src-tauri/src/commands/agent_detect.rs` | 신규 | 감지 커맨드 (+models 포함) |
| `src-tauri/src/commands/project_onboarding.rs` | 수정 | `call_claude` → `call_agent(engine, model, endpoint)` |
| `src-tauri/src/commands/mod.rs` + `lib.rs` | 수정 | `agent_detect` 모듈 등록 + invoke_handler 추가 |
| `src-tauri/src/agents/mod.rs` | 수정 | opencode 모듈 제거 (또는 deprecate) |
| `src/lib/engineConfig.ts` | 수정 | opencode 제거, lmstudio 추가 |
| `src/components/tunaflow/MetaAgentSelector.tsx` | 신규 | 모달 UI |
| `src/components/tunaflow/ProjectStartup.tsx` | 수정 | 프로젝트 경로 선택 후 MetaAgentSelector 호출 |
| `src/stores/slices/onboardingSlice.ts` or 기존 | 수정 | `selectedMetaAgent`, `agentDetections` 상태 |
| `docs/posts/01-*.md`, README | 후속 | "메타에이전트 선택 가능" 반영 (별도 PR) |

## 9. 구현 순서 (한 PR 내)

1. **Rust agent_detect** — CLI `which` + HTTP endpoint check (병렬, 1.5s timeout)
2. **Rust onboarding 일반화** — `call_agent(engine, model, endpoint)` + 테스트 (opencode 제거 이관)
3. **engineConfig.ts** — opencode → lmstudio 교체
4. **MetaAgentSelector 컴포넌트** — UI, endpoint 편집, model 드롭다운, 활성화 조건
5. **ProjectStartup 통합** — 모달 호출 + 건너뛰기 2차 확인
6. **스토어 연결**
7. **통합 테스트** — claude/ollama/lmstudio/codex/gemini 5가지 시나리오 + 건너뛰기 2종

예상 4~6시간.

## 10. 베타 빌드 체크리스트 (구현 완료 후)

구현 끝나면 **한 번만** 빌드:

```bash
./scripts/build.sh --wipe-sandbox --remove-previous-app
```

- `--wipe-sandbox`: release DB 초기화 → 실제 첫 실행 온보딩 재현
- `--remove-previous-app`: `/Applications/tunaFlow.app` 제거 → AppCleaner 안 써도 덮어쓰기 깨끗
- 확인:
  - [ ] 메타에이전트 감지 결과 `[claude, ollama, lmstudio]` 중 사용자 환경 기반 표시
  - [ ] 엔드포인트 편집 시 모델 목록 리프레시
  - [ ] 에이전트 선택 전까지 `[확인]` 버튼 disabled
  - [ ] 건너뛰기 시 2차 확인 → 기본 스캐폴딩만 진행
  - [ ] 확인 진행 시 analyze + apply 성공, CLAUDE.md + agents/*.md 생성

## 11. 관련 문서

- `docs/ideas/onboardingMetaAgentIdea.md` — 메타에이전트 원본 아이디어 (설치 감지까지 포함)
- `docs/reference/knownIssues_2026-04-15.md` — 이번 작업으로 해소될 항목 (claude CLI PATH 이슈의 UX 완화)
