---
title: Codex/Gemini 메타 분석 실패 원인 조사 + fix (#176/#189 sibling)
status: ready-to-implement (gray-box, Developer 가 audit 단계부터 진행)
priority: P2 (실패 자체는 onboarding cancel leak fix 로 UX 영향 흡수됨, 그러나 기능 완결성 측면에서 P2)
created_at: 2026-04-25
related:
  - https://github.com/dghong/tunaFlow/issues/176  # 원 제보 + follow-up
  - https://github.com/hang-in/tunaFlow/issues/189  # cancel leak fix
  - src-tauri/src/commands/project_onboarding.rs
canonical: true
owners:
  - architect (본 plan 작성)
  - developer (audit + 구현)
---

# 배경

커뮤니티 제보 (#176 → follow-up 댓글, M2 16GB 환경) 에서 확정된 재현 조건:

- **Claude 메타 선택**: 분석 성공 → preview 진입 (정상)
- **Codex / Gemini 메타 선택**: AI 정리 단계 (Step 3) 에서 **분석 실패** (`project:onboarding:error` 이벤트 emit)

UX 측면 (실패 후 "건너뛰기" → 메인창 freeze) 은 PR #190 에서 cancel leak 수정으로 해결. 하지만 **분석 자체가 왜 실패하는지** 는 별건이라 본 plan 으로 분리. 즉 onboarding 이 의도대로 작동하려면 Codex/Gemini 메타 선택 시에도 분석이 성공해야 함.

# 현재 상태 (사실 확인)

## (A) 엔진 분기 — 공통 호출 경로

`src-tauri/src/commands/project_onboarding.rs:586-601`:

```rust
let text = match engine {
    "claude" | "codex" | "gemini" => {
        call_cli_agent(engine, engine, prompt, model, cancel).await?
    }
    "ollama" => { ... call_ollama ... }
    "lmstudio" => { ... call_openai_compat ... }
    other => return Err(format!("지원하지 않는 엔진: {other}")),
};
parse_output(&text)
```

세 CLI 엔진이 **동일한 `call_cli_agent` 함수 호출**. 즉 dispatcher 레벨에서는 차이 없음 → 차이가 발생할 수 있는 지점은:

1. `call_cli_agent` 내부에서 engine name 으로 CLI flag / 명령 다르게 구성
2. CLI 의 `stdout` 응답 포맷 자체가 엔진별 다름
3. `parse_output` 이 strict 마커 (`[CLAUDE_MD_START]`, `[REF_INDEX_START]` 등) 요구 → Codex/Gemini 모델이 해당 마커를 빠뜨릴 가능성

## (B) parse_output 의 엄격성

`project_onboarding.rs:358 fn parse_output` 이 어떤 형식을 기대하는지 미확인 (Developer audit). 테스트 케이스 (line 716+) 가 4개 있는데 모두 "leagcy" / "with initial setup" / "missing claude_md errors" 로 **마커 누락 케이스가 error 처리** 되는 것을 명시. 즉 모델이 마커를 안 따라하면 무조건 fail.

## (C) build_prompt 의 모델 편향 가능성

`project_onboarding.rs:179 fn build_prompt` — 프롬프트 자체가 Claude 응답 패턴에 맞춰 설계됐을 가능성. Codex (코드 중심), Gemini (다른 instruction-following 패턴) 가 마커를 정확히 따라하지 않아 parse 단계에서 fail.

# 의심 시나리오 (가설, Developer 검증 필요)

1. **Gemini CLI 가 markdown wrapping 추가** — 응답을 ` ```markdown ... ``` ` 으로 감싸 마커 매칭 실패
2. **Codex 가 "Here's the plan:" 같은 introduction 추가** — 마커 앞에 설명 추가로 strict regex 실패
3. **모델별 출력 길이 / 토큰 제한 차이** — Codex 가 짧게 끝내고 마커 일부 누락
4. **call_cli_agent 의 CLI flag 차이** — engine 별 CLI 옵션이 응답 형식 영향 (예: `--text-only` vs `--json`)
5. **Tool 호출 인터페이스 차이** — Codex app-server 와 Claude stream-json 의 응답 시퀀스 차이로 raw text 파싱 시 문제

# 수정 방향 가설

## Layer A — Engine 별 raw 응답 capture + 분석

먼저 Developer 가 각 엔진으로 onboarding 분석 1 회씩 실행 → raw `text` (parse_output 전 raw 응답) 를 trace_log 또는 console 에 기록. 형식 차이 실측 후 fix 방향 결정.

## Layer B — parse_output 강건화

마커 매칭을 **looser**:

- `\b[CLAUDE_MD_START]\b` 식 word boundary 또는 multiline regex
- 마커 앞 introduction 허용 (예: `^.*?\[CLAUDE_MD_START\]` skip)
- Markdown code fence wrapping 자동 strip
- LLM 의 흔한 prefix ("Sure! Here's...", "## ...", etc.) 자동 제거 후 parse

## Layer C — build_prompt 의 engine-specific 변형

각 engine 별 instruction 미세 조정:

- Claude: 기존 그대로 (이미 잘 작동)
- Codex: "Output exactly the following format. No introduction, no markdown wrapping." 강조
- Gemini: similar 명시 + few-shot example 추가

또는 단일 prompt 에 모든 engine 이 잘 따르는 형식으로 일반화 (more rigid markers, JSON 응답 등).

## Layer D — JSON 응답 강제 (가장 robust)

마커 string 매칭 대신 **JSON 응답 강제**:

```json
{
  "claude_md": "...",
  "ref_index": "...",
  "initial_setup": {...}
}
```

각 엔진의 JSON mode (Codex: `response_format`, Gemini: `responseMimeType: application/json`, Claude: structured output) 활용. 마커 매칭 자체 제거. 가장 강건하지만 변경 표면 큼.

# Invariants

- **[INV-1]** Claude / Codex / Gemini 메타 선택 시 모두 분석 성공 (마커 누락 / wrapping / introduction 문제 흡수)
- **[INV-2]** parse_output 이 raw text 를 받아도 best-effort 로 추출 시도 (strict fail 대신 partial extraction + warning)
- **[INV-3]** build_prompt 가 engine-specific 차이를 흡수하거나 / 모든 engine 이 동일하게 따라할 단일 형식 사용
- **[INV-4]** 실패 시 raw 응답이 trace_log 에 보존 (사후 디버깅)

# 테스트

- 수동 (필수): 빈 프로젝트 또는 README 만 있는 프로젝트로 각 엔진 (claude/codex/gemini) 메타 분석 1 회씩 → 모두 success
- 자동 (가능 범위): parse_output 의 fixture 테스트 보강 (현재 4 케이스). Codex / Gemini 의 실측 raw 응답을 fixture 로 추가 → parse_output 통과 검증

# Developer 핸드오프 프롬프트

```
[작업] Codex/Gemini 메타 분석 실패 원인 조사 + fix (Plan codexGeminiOnboardingAnalysisFailureInvestigation / Issue #176 sibling)

[SSOT] docs/plans/codexGeminiOnboardingAnalysisFailureInvestigationPlan_2026-04-25.md

[배경 3줄]
- 빈 프로젝트 + Codex/Gemini 메타 선택 시 Step 3 (AI 정리 중) 실패. Claude 는 OK
- call_cli_agent 가 세 엔진 공통 호출이라 분기점은 더 깊은 곳 (CLI flag / 응답 형식 / parse_output)
- onboarding cancel leak (#189) 은 fix 됨. 분석 실패 자체가 별건

[수정 범위 — Step 1 은 audit, 실측 후 Layer 결정]

1) Audit (raw 응답 capture):
   - call_cli_agent 본문 확인 (project_onboarding.rs:line 미확인 — grep 으로 위치 찾기)
   - 각 엔진 (claude/codex/gemini) 으로 onboarding 분석 1 회씩 실행 후 raw text 를 console.log 또는 trace_log 에 dump
   - Claude vs Codex vs Gemini 응답 형식 차이 정리
   - 결과: docs/reference/codexGeminiOnboardingResponseAudit_2026-04-2X.md

2) Layer A — parse_output 강건화 (Step 1 결과 따라):
   - 마커 매칭을 looser regex (마커 앞 introduction skip, markdown fence strip, LLM prefix 제거)
   - 기존 4 fixture 테스트 + 신규 Codex/Gemini fixture 추가
   - INV-2 만족: strict fail 대신 best-effort partial extract

3) Layer B — build_prompt 강화 (필요 시):
   - "Output exactly this format. No introduction, no markdown wrapping." 강조
   - few-shot example 추가
   - 또는 engine-specific variant

4) Layer C — JSON 응답 강제 (선택, more robust):
   - 마커 대신 JSON schema 응답 강제
   - 각 엔진 JSON mode 활용 (Codex response_format / Gemini responseMimeType / Claude structured)
   - parse_output → serde_json deserialize 로 단순화
   - 변경 표면 크지만 강건성 최대

[검증]
- cargo check / cargo test --lib (parse_output 테스트 4 + 신규 fixture pass)
- 수동: 각 엔진 + 빈 프로젝트 + README only 프로젝트 + 작은 코드베이스 3 케이스 × 3 엔진 = 9 시나리오 모두 success

[커밋 분리]
- docs(ref): codex/gemini onboarding response audit (Step 1 결과)
- fix(onboarding): looser parse_output (Layer A)
- fix(onboarding): build_prompt engine-agnostic improvements (Layer B)
- (옵션) feat(onboarding): JSON-mode response for structured parsing (Layer C)

[셀프 이슈]
"bug: Codex/Gemini meta-agent analysis fails (parse_output strict, prompt biased toward Claude format)"
이슈 본문에 #176 follow-up + #189 sibling 명시
```

# 관련 기록

- Issue #176 (커뮤니티 제보, follow-up 댓글에서 engine-dependency 확정)
- Issue #189 / PR #190 (sibling, cancel leak fix)
- `asyncCancelPipelineAudit_2026-04-25` 와는 별 카테고리 (이건 응답 파싱 / 프롬프트 robust 영역)
