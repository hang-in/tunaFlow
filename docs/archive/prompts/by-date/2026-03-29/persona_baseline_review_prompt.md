# tunaFlow Persona Baseline 검토 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Persona Baseline Review

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/how-to/tunaflow_persona_baseline_6.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/personaBaselineReviewPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentSkillPersonaIaPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentProfilesSettingsMvpPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentProfileChatInputBindingPlan_2026-03-29.md`

우선 짧게 의견부터 말하라:
- 현재 persona baseline 문서를 지금 바로 구현 기준으로 써도 되는지
- 무엇은 유지하고 무엇은 먼저 보정해야 하는지
- 이번 단계에서 persona 구현보다 review가 먼저 필요한 이유

그 다음 실제 작업을 진행하라.

## 목표

1. `tunaflow_persona_baseline_6.md`를 현재 제품 방향 기준으로 검토한다
2. 유지 / 수정 / 후순위 항목을 구분한다
3. persona 구현을 바로 시작할지, 문서 보정이 먼저인지 판단한다

## 반드시 검토할 쟁점

### 1. 기본 6종 구성

현재 6종:
- Architect
- Implementer
- Reviewer
- Debugger
- UX Critic
- Prompt Writer

검토:
- `Prompt Writer`를 유지할지
- `Tester`가 더 적절한지

### 2. systemPromptTemplate 책임

검토:
- persona가 최종 system prompt 전체를 책임지는 구조가 맞는지
- 아니면 persona는 policy fragment로 두고 runtime이 조립해야 하는지

### 3. recommendedSkills 의미

검토:
- `recommendedSkills`를 그대로 유지할지
- `default skills`와 `auto skill policy`를 분리해야 하는지

### 4. scope / 적용 위치

검토:
- 현재 `Agent Profile` 중심 구조에서 scope 정의가 충분한지
- 추가 적용 위치가 필요한지

## 중요

- 이번 단계는 review 문서 작업이다
- persona editor 구현 금지
- runtime prompt 조립 리팩토링 금지
- auto skill selection 구현 금지
- Agent Profile UI를 다시 건드리지 말 것

## 기대 산출물

다음 중 하나 또는 조합:

- baseline 문서 보정
- review memo 문서
- implementation recommendation

## 결과 보고 형식

### A. Opinion
### B. What Still Fits
### C. What Should Change
### D. What Should Wait
### E. Recommendation
```
