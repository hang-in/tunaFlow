# tunaFlow Agent Profiles Settings MVP 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Agent Profiles Settings MVP

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentProfilesSettingsMvpPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentSkillPersonaIaPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/codexProjectReference_2026-03-29.md`

우선 짧게 의견부터 말하라:
- Agent Profile MVP에서 꼭 필요한 필드와 과한 필드를 구분
- 이번 단계에서 persona 편집을 왜 제외해야 하는지
- 이번에 같이 묶지 말아야 할 것

그 다음 실제 작업을 진행하라.

## 목표

1. `Settings > Agents`를 placeholder에서 실제 관리 UI로 바꾼다
2. profile 목록 / 선택 / 편집이 가능해야 한다
3. engine / model / default skills를 묶어 관리할 수 있어야 한다
4. settings 재실행 후 profile이 유지되어야 한다

## 권장 초기 필드

- id
- label
- engine
- model
- personaKey 또는 임시 persona 문자열
- defaultSkills[]

## 권장 초기 profile

- Architect Claude
- Reviewer Codex
- Tester Gemini
- General OpenCode

## 수정 대상 후보

- Settings 관련 컴포넌트
- appStore / settings persistence
- 필요 시 새 types/store slice

## 중요

- 이번 단계는 Settings > Agents MVP다
- chat input과 연결하지 말 것
- persona 관리 화면은 만들지 말 것
- auto skill selection은 하지 말 것
- project별 override도 하지 말 것

## 검증

- `tsc --noEmit`
- 새로고침/재실행 후 profile persistence 확인
- 기본 skills가 profile에 저장되는지 확인

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Agent Profile Model
### E. Verification
### F. Next Recommendation
```

