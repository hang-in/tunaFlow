# tunaFlow Agent Profile ↔ Chat Input 연결 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Agent Profile Chat Input Binding

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentProfileChatInputBindingPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentProfilesSettingsMvpPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentSkillPersonaIaPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/codexProjectReference_2026-03-29.md`

우선 짧게 의견부터 말하라:
- 지금 단계에서 profile 선택기와 기존 engine/model selector를 어떻게 공존시키는 게 현실적인지
- 이번 단계에서 꼭 연결해야 하는 필드와 나중으로 미뤄도 되는 필드를 구분
- 이번에 같이 묶지 말아야 할 것

그 다음 실제 작업을 진행하라.

## 목표

1. Chat input에서 `Agent Profile` 선택 가능
2. 선택 profile의 `engine / model / persona / default skills`가 실행 입력에 반영
3. conversation 단위로 현재 active profile을 유지

## 권장 방향

- profile selector를 입력창 상단 또는 mode bar 근처에 배치
- 기존 engine/model selector는 MVP 동안 override 용도로 유지 가능
- default skills는 profile 선택 시 activeSkills에 반영

## 수정 대상 후보

- `src/components/tunaflow/NewMessageInput.tsx`
- 입력 관련 하위 컴포넌트
- settings/store persistence 로직
- 필요 시 conversation state

## 중요

- 이번 단계는 `Profile → Chat Input 연결`이다
- persona 편집기 구현으로 범위를 넓히지 말 것
- auto skill selection은 하지 말 것
- Settings > Agents 재설계로 돌아가지 말 것
- full IA 재설계와 섞지 말 것

## 검증

- `tsc --noEmit`
- profile 선택 시 engine/model/default skills 반영 확인
- 실제 전송 시 선택된 profile 값이 사용되는지 설명
- 앱 재실행 또는 conversation 전환 후 active profile 유지 여부 확인

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Binding Model
### E. Verification
### F. Next Recommendation
```

