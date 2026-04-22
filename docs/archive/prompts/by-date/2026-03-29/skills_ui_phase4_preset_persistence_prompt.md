# tunaFlow Skills UI Phase 4 preset persistence 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 카테고리: skills / ui / phase4

```md
# tunaFlow Skills UI Phase 4 — preset persistence + active state visibility

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

이번 작업 목표:
이미 구현된 Skills UI Phase 1-3 위에,
반복 사용성을 높이는 두 가지를 한 번에 마무리하라.

이번 단계에서 함께 가는 범위:
1. preset persistence
2. active state visibility 개선

중요:
- collections 전체 기능은 이번 단계에서 하지 않는다
- 대규모 skill registry 확장은 하지 않는다
- 현재 구조(`activeSkills`, `SkillsPanel`, `ContextBadges`, settings 저장`)를 최대한 재사용한다

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/skillsUiVisibilityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/CLAUDE.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/how-to/skills-runtime-policy.md`

먼저 확인할 파일:
- `/Users/d9ng/privateProject/tunaFlow/src/components/tunaflow/context-panel/SkillsPanel.tsx`
- `/Users/d9ng/privateProject/tunaFlow/src/components/tunaflow/input/ContextBadges.tsx`
- `/Users/d9ng/privateProject/tunaFlow/src/components/tunaflow/NewMessageInput.tsx`
- `/Users/d9ng/privateProject/tunaFlow/src/stores/slices/assetSlice.ts`
- `/Users/d9ng/privateProject/tunaFlow/src/lib/appStore.ts`

## 현재 전제

Phase 1-3은 이미 다음을 포함한다고 본다:
- vendor 그룹핑
- active count
- snapshot footer
- 검색/필터
- vendor filter
- 추천 preset 버튼

이번 단계는 "반복 사용"과 "현재 상태 이해"를 더 좋게 만드는 작업이다.

## 이번 단계에서 할 일

### 1. preset persistence

목표:
- 사용자가 마지막으로 적용한 preset 또는 active skill 조합이 다음 진입에서도 유지되게 한다

권장 방향:
- `appStore`를 사용해 현재 active skill 조합 저장
- 최소 범위:
  - `lastActiveSkills` 저장/복원
- 가능하면:
  - 마지막 사용 preset 이름도 저장

중요:
- conversation/branch 전역 설계를 크게 바꾸지 말 것
- 우선은 앱 수준 또는 현재 작업 흐름에 맞는 가벼운 persistence로 충분하다

### 2. active state visibility 개선

목표:
- 사용자가 지금 어떤 preset/skill 조합이 적용된 상태인지 더 빨리 이해하게 만든다

권장 항목:
- 현재 적용 중인 preset 표시 (가능하면)
- preset 버튼 active styling 강화
- `ContextBadges`에서 active skill 표시를 조금 더 읽기 쉽게 개선
- preset 적용 시 어떤 스킬이 켜졌는지 UI에서 자연스럽게 드러나게 함

### 3. UX 디테일

가능하면 아래도 포함:
- preset 재클릭 시 해제 또는 유지 정책 명확화
- 검색/필터 상태와 preset 상태가 충돌하지 않게 정리
- active skill이 0개일 때와 많을 때 표시 톤 구분

## 비목표

- skill collections 저장/관리 UI
- 공유 preset
- 원격 registry
- 자동 추천 엔진

## 완료 기준

1. active skill 조합이 재진입 시 유지된다
2. 사용자가 현재 어떤 preset/조합을 쓰는지 더 쉽게 알 수 있다
3. 기존 Phase 1-3 기능이 깨지지 않는다
4. 과도한 상태 복잡도 증가 없이 마무리된다

## 출력 형식

### A. Decision
### B. Files Changed
### C. Persistence Model
### D. Active State Visibility
### E. Verification
### F. Deferred Work
```
