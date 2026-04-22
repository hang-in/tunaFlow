# tunaFlow Skills UI Phase 2 메타데이터 가시화 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 카테고리: skills / ui / phase2

```md
# tunaFlow Skills UI Phase 2 — snapshot 메타 + vendor/source 정교화

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

이번 작업 목표:
이미 구현된 Skills UI Phase 1을 바탕으로,
다음 메타데이터 가시화를 한 번에 마무리하라.

이번 단계에서 함께 가는 것이 맞는 범위:
1. runtime snapshot 메타데이터 표시
2. vendor/source 정보 정교화

중요:
- 검색/필터는 이번 단계에서 하지 않는다
- collections, registry, 추천 스킬 preset UI는 이번 단계에서 하지 않는다
- 과도한 구조 변경 없이 현재 SkillsPanel 중심으로 확장한다

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/skillsUiVisibilityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/how-to/skills-runtime-policy.md`
- `/Users/d9ng/privateProject/tunaFlow/CLAUDE.md`

먼저 확인할 파일:
- `/Users/d9ng/privateProject/tunaFlow/src/components/tunaflow/context-panel/SkillsPanel.tsx`
- `/Users/d9ng/privateProject/tunaFlow/src/components/tunaflow/ContextPanel.tsx`
- `/Users/d9ng/privateProject/tunaFlow/src/stores/slices/assetSlice.ts`
- `/Users/d9ng/privateProject/tunaFlow/src-tauri/src/commands/skills.rs`
- `/Users/d9ng/.tunaflow/skills/_snapshot.json`
- `/Users/d9ng/.tunaflow/skills/*/_meta.json`

## 현재 전제

Phase 1은 이미 다음을 포함한다고 본다:
- Skills 섹션 active count 표시
- vendor별 그룹핑
- vendor 라벨 표시
- skill name에서 vendor prefix 제거

이번 단계는 그 위에 메타데이터를 얹는 일이다.

## 이번 단계에서 할 일

### 1. snapshot 메타 표시

사용자가 최소한 아래를 볼 수 있어야 한다:
- 현재 runtime snapshot 발행 시각
- 총 스킬 수

표시 위치 예:
- SkillsPanel 하단
- 또는 Skills 섹션 헤더 아래 보조 정보 행

`_snapshot.json` 정보:
- `published_at`
- `total_skills`
- 필요 시 `source`

### 2. vendor/source 정교화

지금 vendor prefix를 이름 문자열에서 추론하고 있다면,
가능하면 `_meta.json`을 실제로 읽는 방향으로 개선하라.

권장 방향:
- backend `SkillDef`에 `vendor` 필드 추가
- 가능하면 `sourcePath`도 추가
- `list_skills()`에서 각 skill 폴더의 `_meta.json`을 읽어 메타를 포함

최소 목표:
- frontend가 더 이상 이름 split만으로 vendor를 추론하지 않게 한다

### 3. UI 반영

가능하면 각 vendor 그룹 또는 skill row에서 아래 중 일부를 보여라:
- vendor label
- source path tooltip 또는 작은 보조 텍스트

단, 너무 시끄럽지 않게 유지한다.

## 비목표

- 검색 입력
- vendor 필터 토글
- 추천 스킬 preset 버튼
- collections
- 다중 루트 skill loader

## 구현 원칙

- 현재 구조를 보존
- Zustand 개별 selector 유지
- 스킬 246개 기준으로 렌더링이 과하게 무거워지지 않게 한다
- snapshot 파일이 없거나 `_meta.json`이 없으면 graceful fallback 허용

## 완료 기준

1. Skills UI에서 snapshot published_at을 볼 수 있다
2. 총 스킬 수를 볼 수 있다
3. vendor 정보가 `_meta.json` 또는 backend 메타 기준으로 표시된다
4. 기존 Phase 1 그룹핑/active count가 유지된다

## 출력 형식

### A. Decision
### B. Files Changed
### C. Snapshot Metadata Display
### D. Vendor/Source Metadata Flow
### E. Verification
### F. Deferred Work
```
