# Architect 고도화 Phase 2 — 병렬 그룹 + 피드백 판단 + 메타에이전트

> Status: in_progress
> Created: 2026-04-04
> 관련: `docs/ideas/architectEnhancementIdea.md`

---

## Phase 2-1: 서브태스크 병렬 그룹

### 목적

독립적인 서브태스크를 동시 실행하여 구현 시간 단축.

### 변경

**DB**: `plan_subtasks` 테이블에 2개 컬럼 추가 (migration v24)
```sql
ALTER TABLE plan_subtasks ADD COLUMN depends_on TEXT DEFAULT '[]';  -- JSON array of idx
ALTER TABLE plan_subtasks ADD COLUMN parallel_group TEXT DEFAULT NULL;  -- group label
```

**Rust**: `models.rs` PlanSubtask 구조체에 `depends_on: Option<String>`, `parallel_group: Option<String>` 추가

**TypeScript**: `PlanSubtask` 타입에 `dependsOn?: number[]`, `parallelGroup?: string` 추가

**Architect 프롬프트**: plan-proposal 마커에 `depends_on` + `parallel_group` 포함 규칙
```json
{
  "subtasks": [
    { "title": "DB 마이그레이션", "parallel_group": "A" },
    { "title": "API 엔드포인트", "parallel_group": "B", "depends_on": [1] },
    { "title": "UI 컴포넌트", "parallel_group": "B", "depends_on": [1] },
    { "title": "통합 테스트", "parallel_group": "C", "depends_on": [2, 3] }
  ]
}
```

**Plan Hints 강화**: `build_plan_section`에 병렬 그룹 표시
```
**Subtasks:**
- ✅ Task 01: DB 마이그레이션 [Group A]
- 🔧 Task 02: API 엔드포인트 [Group B, depends: 01]
- ⬜ Task 03: UI 컴포넌트 [Group B, depends: 01]  ← 02와 동시 실행 가능
- ⬜ Task 04: 통합 테스트 [Group C, depends: 02,03]
```

**실행 엔진**: Phase 2-1에서는 UI 표시 + 프롬프트 힌트만. 실제 자동 병렬 실행은 후속 작업.

### 파일

| 파일 | 변경 |
|------|------|
| `src-tauri/src/db/migrations.rs` | v24: depends_on, parallel_group 컬럼 |
| `src-tauri/src/db/models.rs` | PlanSubtask 필드 추가 |
| `src-tauri/src/commands/plans.rs` | create/replace subtask에 새 필드 포함 |
| `src/types/index.ts` | PlanSubtask 타입 확장 |
| `src-tauri/src/commands/agents_helpers/context_pack/section_builders.rs` | Plan Hints에 그룹 표시 |
| `src-tauri/src/commands/agents_helpers/identity.rs` | PLATFORM_TIER0에 병렬 그룹 작성 규칙 |

---

## Phase 2-2: 리뷰 피드백 → 재설계 vs rework 자동 판단

### 목적

doom loop 3회 도달 전에 "구현 오류 vs 설계 오류"를 판단하여 불필요한 반복 방지.

### 접근

review_failed 2회 시점에서 **이전 2회의 findings를 비교**:
- **같은 파일, 같은 포인트 반복** → 설계 오류 가능성 → Architect 재설계 제안
- **다른 파일, 다른 포인트** → 단순 구현 누락 → rework 계속

### 변경

`processReviewVerdict()`에서 2회 실패 시 자동 분석:

```typescript
if (failCount === 2) {
  // Compare findings from 1st and 2nd failure
  const prevFindings = extractFindingsFromEvent(failEvents[0]);
  const currFindings = verdict.findings;
  const overlap = countFileOverlap(prevFindings, currFindings);

  if (overlap > 0.5) {
    // Same files failing → likely design issue
    detail += "\n⚠️ 동일 파일에서 반복 실패 — 설계 재검토를 권장합니다.";
  }
}
```

UI에서 사용자에게 "설계 재검토로 전환하시겠습니까?" 선택지 표시.

### 파일

| 파일 | 변경 |
|------|------|
| `src/lib/workflowOrchestration.ts` | `processReviewVerdict`에 findings 비교 로직 |
| `src/components/tunaflow/context-panel/DevProgressView.tsx` | 설계 재검토 권장 UI |

---

## Phase 2-3: 메타에이전트 (경량 버전)

### 목적

Architect에게 프로젝트 컨텍스트를 자동 제공. 처음부터 full 메타에이전트가 아닌 **"프로젝트 분석 프리패스"**로 시작.

### 접근

plan-proposal 요청 시 **자동으로 프로젝트 분석 결과를 Architect 프롬프트에 주입**:

```
## 프로젝트 분석 (자동 생성)
- 기술 스택: React + TypeScript + Zustand + Tailwind (package.json 기반)
- 추천 스킬: anthropic-frontend-design, microsoft-zustand-store-ts
- 최근 변경: 5개 파일 수정 (detect-changes 기반)
- 그래프: 147 nodes, 523 edges (status 기반)
```

이미 있는 인프라를 조합:
- `detect_project_stack()` → 기술 스택
- `mapKeywordsToSkills()` → 추천 스킬
- `code-review-graph status` → 그래프 상태
- `code-review-graph detect-changes` → 최근 변경

### 변경

`requestPlanRevision()`의 system prompt에 프로젝트 분석 섹션 추가.
일반 대화에서의 plan-proposal은 ContextPack이 이미 커버.

### 파일

| 파일 | 변경 |
|------|------|
| `src/lib/workflowOrchestration.ts` | `requestPlanRevision`에 프로젝트 분석 주입 |
| `src/lib/api/plans.ts` | (선택) `getProjectAnalysis()` 헬퍼 |

---

## 구현 순서

| # | 항목 | 난이도 | 의존성 |
|---|------|--------|--------|
| 1 | **병렬 그룹 DB + 타입** | 중간 | 없음 |
| 2 | **병렬 그룹 Plan Hints + 프롬프트** | 낮음 | #1 |
| 3 | **피드백 판단 (findings 비교)** | 낮음 | 없음 |
| 4 | **메타에이전트 경량 (프로젝트 분석 주입)** | 낮음 | 없음 |

#1-2는 순차, #3-4는 독립 — 병렬 가능.
