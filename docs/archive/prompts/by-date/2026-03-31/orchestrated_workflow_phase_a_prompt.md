# Orchestrated Workflow Pipeline — Phase A: DB + Types + API

프로젝트: `/Users/d9ng/privateProject/tunaFlow`
모든 응답과 보고는 한국어로 작성하라.

---

## 사전 읽기 (필수)

1. `CLAUDE.md` — 프로젝트 구조, 코딩 컨벤션, 안전 규칙
2. `docs/plans/orchestratedWorkflowPipelinePlan.md` — **전체 파이프라인 설계** (이 프롬프트의 기반)
3. `src-tauri/src/db/migrations.rs` — 기존 마이그레이션 패턴 (v1-v17)
4. `src-tauri/src/db/models.rs` — Plan, PlanSubtask 모델
5. `src-tauri/src/commands/plans.rs` — 기존 Plan CRUD
6. `src/types/index.ts` — Plan, PlanSubtask TypeScript 타입
7. `src/lib/api/plans.ts` — 프론트엔드 API wrapper

---

## 배경

tunaFlow에 Chat → Plan → Implement → Review 오케스트레이션 파이프라인을 구현한다.
이 프롬프트는 Phase A (기반 인프라)만 다룬다.

**전체 5-Phase 구현 순서:**
- **Phase A: DB + 타입 + API** ← 이 프롬프트
- Phase B: Chat → Plan 승격 (마커 파서 + UI 카드)
- Phase C: Plan 승인 게이트 (3-way + 검토 Branch)
- Phase D: Developer 실행계획 + 구현
- Phase E: 테스트 러너 + RT 리뷰

---

## 작업 1: DB Migration v18

`src-tauri/src/db/migrations.rs`에 v18 추가.

```sql
-- plans 테이블 확장
ALTER TABLE plans ADD COLUMN phase TEXT NOT NULL DEFAULT 'drafting';
ALTER TABLE plans ADD COLUMN architect_engine TEXT;
ALTER TABLE plans ADD COLUMN developer_engine TEXT;
ALTER TABLE plans ADD COLUMN reviewer_engines TEXT;
ALTER TABLE plans ADD COLUMN implementation_branch_id TEXT REFERENCES branches(id);
ALTER TABLE plans ADD COLUMN review_branch_id TEXT REFERENCES branches(id);

-- plan_events 테이블 (이력 로그)
CREATE TABLE IF NOT EXISTS plan_events (
    id            TEXT PRIMARY KEY,
    plan_id       TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    event_type    TEXT NOT NULL,
    actor         TEXT,
    detail        TEXT,
    created_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_plan_events_plan_id ON plan_events(plan_id);
```

**주의:**
- `add_column_if_missing()` 멱등성 패턴 사용 (기존 마이그레이션과 동일)
- `phase` 기본값 "drafting" — 기존 plan은 자동으로 drafting 상태
- event_type 값: "promoted", "approved", "held", "review_requested", "review_merged", "impl_plan_submitted", "impl_approved", "impl_completed", "review_passed", "review_failed", "rework_requested"

### 검증
```bash
cd src-tauri && cargo check && cargo test --lib
```

---

## 작업 2: Rust 모델 업데이트

`src-tauri/src/db/models.rs`에 Plan 필드 추가 + PlanEvent 구조체 생성.

```rust
// Plan 구조체에 추가
pub phase: String,                        // "drafting" | "approval" | "implementation" | "review" | "done" | "rework"
pub architect_engine: Option<String>,
pub developer_engine: Option<String>,
pub reviewer_engines: Option<String>,     // JSON string: ["claude", "gemini"]
pub implementation_branch_id: Option<String>,
pub review_branch_id: Option<String>,

// 새 구조체
pub struct PlanEvent {
    pub id: String,
    pub plan_id: String,
    pub event_type: String,
    pub actor: Option<String>,
    pub detail: Option<String>,
    pub created_at: i64,
}
```

**주의:** 기존 Plan 구조체의 필드 순서/이름 변경 금지. 추가만.

### 검증
```bash
cd src-tauri && cargo check
```

---

## 작업 3: Tauri Commands 추가

`src-tauri/src/commands/plans.rs`에 새 commands 추가 (기존 함수 수정 금지).

### 3-1: Plan phase 업데이트
```rust
#[tauri::command]
pub fn update_plan_phase(
    id: String,
    phase: String,
    state: State<DbState>,
) -> Result<(), AppError>
```

### 3-2: Plan event 생성
```rust
#[tauri::command]
pub fn create_plan_event(
    plan_id: String,
    event_type: String,
    actor: Option<String>,
    detail: Option<String>,
    state: State<DbState>,
) -> Result<PlanEvent, AppError>
```

### 3-3: Plan events 조회
```rust
#[tauri::command]
pub fn list_plan_events(
    plan_id: String,
    state: State<DbState>,
) -> Result<Vec<PlanEvent>, AppError>
```

### 3-4: Plan engine 할당
```rust
#[tauri::command]
pub fn assign_plan_engines(
    id: String,
    architect_engine: Option<String>,
    developer_engine: Option<String>,
    reviewer_engines: Option<String>,  // JSON array string
    state: State<DbState>,
) -> Result<(), AppError>
```

**lib.rs에 commands 등록 필수.**

### 검증
```bash
cd src-tauri && cargo check && cargo test --lib
```

---

## 작업 4: TypeScript 타입 확장

`src/types/index.ts`에 추가 (기존 타입 수정은 최소).

```typescript
type PlanPhase = "drafting" | "approval" | "implementation" | "review" | "done" | "rework";

// Plan 인터페이스에 추가:
phase: PlanPhase;
architectEngine?: string;
developerEngine?: string;
reviewerEngines?: string[];
implementationBranchId?: string;
reviewBranchId?: string;

// 새 인터페이스:
export interface PlanEvent {
  id: string;
  planId: string;
  eventType: string;
  actor?: string;
  detail?: string;
  createdAt: number;
}
```

---

## 작업 5: Frontend API wrapper

`src/lib/api/plans.ts`에 추가 (기존 함수 수정 금지).

```typescript
export async function updatePlanPhase(id: string, phase: PlanPhase): Promise<void>
export async function createPlanEvent(planId: string, eventType: string, actor?: string, detail?: string): Promise<PlanEvent>
export async function listPlanEvents(planId: string): Promise<PlanEvent[]>
export async function assignPlanEngines(id: string, engines: { architect?: string; developer?: string; reviewers?: string[] }): Promise<void>
```

---

## 작업 6: list_plans 쿼리 업데이트

`plans.rs`의 `list_plans_by_conversation`과 `get_plan` 쿼리에 새 컬럼 포함.

**주의:** SELECT 문에 새 컬럼을 추가하되, 기존 컬럼 순서 변경 금지.

---

## 절대 하지 말 것

1. 기존 Plan CRUD API 시그니처 변경 금지 — 새 함수만 추가
2. 기존 plan_subtasks 테이블 변경 금지
3. 프론트엔드 UI 컴포넌트 수정 금지 (Phase B에서 함)
4. ContextPack 코드 수정 금지
5. Phase B-E 작업 시작 금지

---

## 검증 게이트 (최종)

```bash
cd src-tauri && cargo check
cd src-tauri && cargo test --lib     # 57+ tests
cd .. && npx tsc --noEmit
cd .. && npx vitest run              # 55+ tests
```

## 완료 후

- 커밋: `feat: workflow pipeline Phase A — plan phases, events, engine assignment`
- CLAUDE.md §5에 Phase A 완료 기록
- Phase B 프롬프트로 넘어갈 준비
