# tunaFlow Skills UI 가시화 계획

- 작성자: Claude
- 작성 시각: 2026-03-28
- 상태: 제안

## A. Current Gaps

### 지금 UI에서 보이는 것

- **SkillsPanel** (ContextPanel > Artifacts 탭 > Skills 접기/펼치기):
  - 스킬 이름 + description + on/off 토글
  - 활성 스킬은 Zap 아이콘이 primary 색상으로 표시
- **ContextBadges** (NewMessageInput 상단):
  - 활성 스킬 이름을 작은 배지로 표시
- **ContextPack 주입**: `activeSkills`에 포함된 스킬의 `SKILL.md` 내용이 Claude system prompt에 삽입됨

### 지금 UI에서 안 보이는 것

1. **vendor/source 정보** — 스킬이 어느 vendor(`anthropic`, `microsoft`, `openai` 등)에서 왔는지 표시 없음
2. **snapshot 메타데이터** — 현재 runtime snapshot이 언제 발행됐는지(`_snapshot.json`) 표시 없음
3. **스킬 개수/분류** — 246개가 flat list로 나열. vendor별 그룹핑이나 필터링 없음
4. **활성 스킬 카운트** — Artifacts 탭 배지처럼 Skills 섹션 접기 상태에서 활성 수를 보여주지 않음
5. **추천 스킬** — 작업 유형별 추천 세트(CLAUDE.md §15)가 UI에 반영되지 않음

## B. UX Goals

1. **vendor 그룹핑** — 스킬을 vendor별로 접기/펼치기 그룹으로 정리
2. **active skill 카운트** — Skills 접기 헤더에 `(3 active)` 같은 배지
3. **vendor/source 표시** — 각 스킬 항목에 vendor 라벨 또는 색상 dot
4. **snapshot 상태** — Skills 섹션 하단에 "Published: 2026-03-28T10:41:48Z · 246 skills" 한 줄
5. **검색/필터** — 246개를 다 보여주지 않고 검색 또는 vendor 필터

## C. Data Sources

| 데이터 | 현재 소스 | 필요한 변경 |
|---|---|---|
| 스킬 목록 + 내용 | `list_skills()` → `SkillDef { name, description, content }` | 없음 |
| vendor 정보 | `_meta.json` (각 스킬 폴더) | backend: `list_skills()`에 `vendor` 필드 추가 |
| snapshot 메타 | `_snapshot.json` (skills root) | backend: 새 command `get_skills_snapshot_info()` 또는 `list_skills()` 응답에 포함 |
| 활성 스킬 | `activeSkills: string[]` (store) | 없음 |

### Backend 확장점

`SkillDef` 구조체에 `vendor: Option<String>` 추가:
- `list_skills()` 시 `_meta.json`이 있으면 `vendor` 필드 파싱
- 없으면 스킬 이름에서 첫 `-` 앞을 vendor로 추정 (fallback)

`SkillsSnapshotInfo` 새 구조체:
- `published_at: Option<String>`
- `total_skills: u64`
- `source: Option<String>`

## D. Phased Plan

### Phase 1: 현재 UI 내 최소 가시화

목표: 코드 변경 최소화, 기존 데이터만 활용

- [x] Skills 접기 헤더에 활성 스킬 카운트 배지 추가
- [x] 스킬 이름에서 vendor prefix 추출하여 색상 라벨 표시 (frontend only)
- [x] vendor별 접기/펼치기 그룹 + 알파벳 정렬 + vendor별 활성 카운트

변경 파일: `SkillsPanel.tsx`, `ContextPanel.tsx`

### Phase 2: vendor 그룹핑 + 메타데이터 표시

목표: backend에서 `_meta.json` 파싱, vendor 기반 그룹 UI

- [x] `SkillDef`에 `vendor: Option<String>`, `source_path: Option<String>` 추가
- [x] `list_skills()`에서 `_meta.json` 읽기 (`read_meta()` 헬퍼)
- [x] `SkillsPanel`에서 backend vendor 메타 기반 그룹핑 (name split fallback 유지)
- [x] `get_skills_snapshot` command + `SkillsSnapshotInfo` 구조체 추가
- [x] Skills 섹션 하단에 snapshot published_at + total_skills 표시
- [x] 각 skill row에 source_path tooltip 추가

### Phase 3: 검색 + 필터 + 추천

목표: 246개 스킬을 효율적으로 탐색

- [x] 검색 입력 필드 (name/description 필터) + 클리어 버튼
- [x] vendor 필터 토글 (pill 형태, 단일 선택/해제)
- [x] 작업 유형별 추천 프리셋 버튼 5개 (Frontend/Review/OpenAI/Claude/MCP — CLAUDE.md §15 기반)
- [x] 필터링 시 결과 카운트 표시 (`N / 246 skills`)

### Phase 4: preset persistence + active visibility

목표: 반복 사용성 + 현재 상태 이해도 향상

- [x] active skill 조합 persist (`lastActiveSkills` → appStore settings.json)
- [x] 앱 재시작 시 복원 (snapshot에 존재하는 스킬만 필터)
- [x] preset 재클릭 → 해제 토글
- [x] preset 버튼 active styling 강화 (font-semibold, 강조 배경)
- [x] preset hover tooltip (포함 스킬 목록)
- [x] ContextBadges에서 active skill 이름 최대 3개까지 표시 (초과 시 +N)

### Phase 5: registry/collections 연결 (후순위)

목표: skillRegistryPlan과 합류

- [ ] 스킬 컬렉션 (저장된 스킬 조합)
- [ ] 대화별 적용된 스킬 이력
- [ ] applied skill visibility (ContextPack에 실제 주입된 스킬 표시)

## E. Deferred Work

- 다중 루트 스킬 로더 (system + user 경로 분리)
- 외부 skill registry 연동
- 원격 업데이트/동기화
- 스킬 자동 추천 (프롬프트 분석 기반)
