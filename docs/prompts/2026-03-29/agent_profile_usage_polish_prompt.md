# tunaFlow Agent Profile 사용성 보강 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Agent Profile Usage Polish

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentProfileUsagePolishPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentProfileChatInputBindingPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentProfilesSettingsMvpPlan_2026-03-29.md`

우선 짧게 의견부터 말하라:
- 지금 단계에서 profile selector 주변에 어떤 요약 정보를 보여주는 것이 가장 효과적인지
- `Profile ↔ Custom` 전환에서 반드시 명확히 보여줘야 하는 규칙이 무엇인지
- 이번 단계에서 persona editor, auto skill selection, settings 재구성으로 새면 안 되는 이유

그 다음 실제 작업을 진행하라.

## 목표

1. 현재 선택된 `Agent Profile`의 핵심 값이 입력창 근처에서 바로 보인다
2. `Custom`이 일반 profile과 무엇이 다른지 사용자가 이해할 수 있다
3. RT participant profile 표시가 profile 기반 구조와 더 일관되게 보인다

## 권장 방향

- selector는 compact하게 유지
- 대신 선택 직후 작은 summary row, chips, tooltip 중 하나로
  `profile / engine / model / persona / default skills`를 짧게 보여준다
- `Custom`은 `manual override` 경로임이 시각적으로 드러나야 한다
- 기존 동작을 크게 바꾸기보다 가시성과 설명을 보강한다

## 수정 대상 후보

- `src/components/tunaflow/NewMessageInput.tsx`
- 입력 관련 selector 하위 컴포넌트
- RT participant 표시 UI

## 중요

- 이번 단계는 `Agent Profile UX polish`다
- persona 편집기 구현 금지
- auto skill selection 금지
- Settings > Agents 구조 재설계 금지
- profile persistence 모델 변경은 최소화

## 검증

- `tsc --noEmit`
- profile 선택 시 어떤 값이 적용되는지 UI에서 설명 가능한지 확인
- `Custom` 전환 시 어떤 값이 유지/초기화되는지 명확해졌는지 설명
- RT participant가 profile 기반 구조를 더 잘 드러내는지 확인

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. UX Model
### E. Verification
### F. Next Recommendation
```
