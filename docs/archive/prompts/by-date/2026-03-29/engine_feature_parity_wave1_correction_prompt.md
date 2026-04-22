# tunaFlow 4-Engine Feature Parity Wave 1 보정 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow 4-Engine Feature Parity Wave 1 Correction

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/engineFeatureParityClassificationPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/skillsEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/contextPackEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/collaborationContextEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/rawqEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/implementationStatus.md`

우선 짧게 의견부터 말하라:
- 이번 보정에서 parity 완료 판정을 막는 핵심 이슈 2개
- 이 단계에서 반드시 고쳐야 하는 것

그 다음 실제 수정으로 들어가라.

## 이번 보정의 핵심 이슈

1. `stream_with_gemini()`가 아직 `build_lite_enriched_prompt()`를 사용한다
- 즉 Gemini streaming 경로는 normalized context parity에서 빠져 있다

2. `build_normalized_prompt()`의 rawq inclusion 조건이 아직 충분히 동등하지 않다
- 현재는 `ctx_mode >= Standard`일 때만 rawq가 들어간다
- 일반 메인 대화에서는 rawq가 빠질 수 있다
- rawq parity는 `branch 여부`가 아니라 실제 `prompt_needs_rawq()` / `build_rawq_section()` 기준으로 맞춰야 한다

## 목표

1. Gemini streaming 경로도 normalized prompt 사용
2. rawq section inclusion 조건을 4개 엔진에서 동일하게 정리
3. 그 후에만 provider parity 표기를 `equal`로 올릴지 판단

## 수정 대상

- `src-tauri/src/commands/agents.rs`
- `src-tauri/src/commands/agents_helpers/send_common.rs`
- 필요 시 parity 관련 plan 문서
- 필요 시 `docs/reference/implementationStatus.md`

## 중요

- 이번 작업은 Wave 1 보정이다
- Streaming/Token/Resume 전체 구현으로 범위를 넓히지 말 것
- rawq 내부 로직을 재구현하지 말 것
- 문서 표기는 실제 코드와 정확히 맞출 것

## 검증

- `cargo check`
- Gemini streaming 경로가 normalized prompt를 타는지 확인
- rawq inclusion 조건이 main chat / branch / non-Claude 경로에서 일관적인지 설명

## 결과 보고 형식

### A. Opinion
### B. Correction Applied
### C. Files Changed
### D. Verification
### E. Updated Wave 1 Status
### F. Next Recommendation
```

