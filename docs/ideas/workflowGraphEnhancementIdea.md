# 워크플로우 파이프라인 × 코드 구조 그래프 고도화

> Status: idea
> Created: 2026-04-02
> 선행: `knowledgeLayerArchitectureIdea.md` (ContextPack 지식 주입), `rawqGraphEvolutionStrategyIdea.md` (rawq+graph 통합 전략)

---

## 1. 현재 워크플로우에서 빠진 것: 코드 구조 인식

현재 워크플로우 파이프라인:

```
Chat (Architect) → Plan → Approve → Implementation (Developer) → Review RT → Done/Rework
```

각 단계에서 에이전트가 받는 컨텍스트:
- **Architect**: 사용자 요구사항 + ContextPack (일반)
- **Developer**: Plan 문서 + subtask 파일 (docs/plans/)
- **Reviewer**: Plan context + impl summary + test 결과

**공통적으로 없는 것**: 코드 구조 정보.

에이전트는 "이 함수를 고쳐라"라는 지시를 받지만, 그 함수를 누가 호출하는지, 어떤 테스트가 커버하는지, 어떤 모듈이 의존하는지 모른다. 사람이라면 IDE에서 "Find Usages"를 누르겠지만, 에이전트에게는 그 수단이 없다.

---

## 2. 워크플로우 단계별 graph 활용

### 2.1 Plan 단계: 영향 범위 추정

**현재**: Architect가 subtask를 만들 때 경험(LLM 학습 데이터)에만 의존

**graph 보강 시**:

```
사용자: "인증 미들웨어를 JWT에서 OAuth로 변경하고 싶어"
                ↓
Architect → graph.query("auth middleware") 
         → callers: [api/routes.rs, api/guards.rs, tests/auth_test.rs]
         → imports: [jsonwebtoken, cookie]
         → dependents: 14 files
                ↓
Architect: "영향 범위 14파일. subtask를 다음과 같이 나눕니다:
  1. 핵심 미들웨어 교체 (auth/middleware.rs)
  2. Route 어댑터 수정 (api/routes.rs, api/guards.rs) 
  3. 의존성 교체 (jsonwebtoken → oauth2 crate)
  4. 테스트 업데이트 (tests/auth_test.rs + 신규)"
```

**구현**:

```typescript
// workflowOrchestration.ts — buildPlanContext() 확장
async function buildPlanContextWithStructure(plan: Plan): Promise<string> {
  const base = await buildPlanContext(plan);
  
  // Plan의 주요 키워드로 구조 탐색
  const keywords = extractKeyTerms(plan.title + " " + plan.description);
  const structure = await invoke("graph_expand", { 
    projectPath, 
    symbols: keywords,
    depth: 1,        // 1-hop만
    includeTests: true,
  });
  
  if (structure.nodes.length > 0) {
    return base + "\n\n### 코드 구조 (자동 탐색)\n" + formatStructure(structure);
  }
  return base;
}
```

### 2.2 Developer 실행: 변경 영향 파악

**현재**: Developer가 Plan 문서만 보고 구현. 의존성 파악은 에이전트의 능력에 의존.

**graph 보강 시**:

Developer pre-implementation report (`<!-- tunaflow:impl-plan -->`)에 구조 정보 자동 삽입:

```
## 작업 지시서 파일
메인 Plan: docs/plans/auth-oauth-migration.md

## 코드 구조 (자동 제공)
### auth/middleware.rs
  호출자: api/routes.rs:42, api/guards.rs:18
  테스트: tests/auth_test.rs (3 test cases)
  의존: jsonwebtoken(0.9), cookie(0.18)

### api/routes.rs
  호출자: main.rs:setup_routes()
  테스트: tests/api_test.rs (12 test cases)
```

**구현**:

```typescript
// workflowOrchestration.ts — approveAndStartImplementation() 확장
export async function approveAndStartImplementation(
  plan: Plan,
  developerEngine: string = "claude",
): Promise<CreateBranchResult & { prompt: string }> {
  // ... 기존 로직 ...

  // Graph 정보 추가 (있으면)
  let structureBlock = "";
  try {
    const subtasks = await planApi.listSubtasks(plan.id);
    const keywords = subtasks.flatMap(st => extractKeyTerms(st.title));
    const structure = await invoke("graph_expand", { 
      projectPath: pp, 
      symbols: keywords,
      depth: 1,
    });
    if (structure.nodes.length > 0) {
      structureBlock = `\n## 코드 구조 (자동 제공)\n${formatGraphForDeveloper(structure)}`;
    }
  } catch { /* graph 없으면 무시 */ }

  const prompt = [
    `"${plan.title}" 구현을 시작합니다.`,
    "",
    `## 작업 지시서 파일`,
    `메인 Plan: \`docs/plans/${slug}.md\``,
    taskFileList,
    structureBlock,  // ← graph 정보 (있으면)
    "",
    `## 작업 규칙`,
    // ... 기존 규칙 ...
  ].join("\n");

  return { branch, shadowConvId, prompt };
}
```

### 2.3 Review RT: 변경 커버리지 검증

**현재**: Reviewer가 Plan + impl summary + test 결과를 받지만, "변경된 코드의 영향 범위가 테스트에 포함되는가"를 구조적으로 판단할 수 없음.

**graph 보강 시**:

```
## 변경 파일 영향 분석 (자동 제공)

auth/middleware.rs (변경됨)
  ├── 호출자 3곳: routes.rs, guards.rs, ws_handler.rs
  │   ├── routes.rs    → 테스트 있음 (api_test.rs)
  │   ├── guards.rs    → 테스트 있음 (auth_test.rs)  
  │   └── ws_handler.rs → ⚠ 테스트 없음
  └── 의존성 변경: jsonwebtoken → oauth2

커버리지 경고:
  - ws_handler.rs가 변경된 middleware를 사용하지만 테스트 미확인
```

Reviewer에게 **구조적 근거**를 제공. "plan_coverage" 판단이 감이 아니라 데이터 기반이 됨.

**구현**:

```typescript
// workflowOrchestration.ts — startReviewRT() 확장
export async function startReviewRT(
  plan: Plan,
  implMessages: Message[],
  testOutput?: string,
  reviewerEngines?: string[],
): Promise<CreateBranchResult> {
  // ... 기존 로직 ...

  // 변경 파일에서 구조 분석
  let coverageBlock = "";
  try {
    const changedFiles = extractChangedFiles(implMessages);  // impl 결과에서 파일 추출
    const coverage = await invoke("graph_coverage_check", {
      projectPath: pp,
      changedFiles,
    });
    if (coverage.warnings.length > 0) {
      coverageBlock = `\n## 변경 영향 분석 (자동 제공)\n${formatCoverageForReviewer(coverage)}`;
    }
  } catch { /* graph 없으면 무시 */ }

  const prompt = [
    `당신은 코드 리뷰어입니다.`,
    "",
    `## Plan (원래 요구사항)`,
    planContext,
    "",
    `## Implementation (Developer 구현 결과)`,
    implSummary.slice(0, 6000),
    "",
    coverageBlock,  // ← 구조 분석 (있으면)
    testOutput ? `## 테스트 결과\n${testOutput.slice(0, 3000)}\n` : "",
    // ... 기존 리뷰 기준 ...
  ].filter(Boolean).join("\n");

  // ...
}
```

### 2.4 Rework: 실패 원인 추적

**현재**: Review fail 시 Developer에게 findings만 전달. 어디를 고쳐야 하는지는 Developer가 다시 파악.

**graph 보강 시**:

```
## Rework 지시
Review 실패 사유:
  - ws_handler.rs의 인증 호환성 미검증 (finding #2)

관련 코드 구조:
  ws_handler.rs:23 → auth::middleware::validate()
  └── 변경된 함수. validate()의 시그니처가 바뀌었으므로 ws_handler 수정 필요.

제안 수정 범위:
  1. ws_handler.rs:23 — validate() 호출 업데이트
  2. tests/ — ws_handler 테스트 추가
```

---

## 3. graph 없이도 동작하는 구조 (Graceful Degradation)

모든 graph 보강은 **있으면 추가, 없으면 무시**:

```typescript
// 패턴: graph는 항상 optional
let structureBlock = "";
try {
  const structure = await invoke("graph_expand", { ... });
  if (structure.nodes.length > 0) {
    structureBlock = formatStructure(structure);
  }
} catch { /* graph 미설치 또는 인덱스 없음 → 무시 */ }
```

| 상태 | Developer | Reviewer | Plan |
|------|-----------|----------|------|
| graph 없음 | Plan 문서만 (현재와 동일) | plan + impl + test (현재와 동일) | 사용자 요구사항만 |
| graph 있음 | + 호출자/의존성/테스트 | + 변경 커버리지 분석 | + 영향 범위 추정 |

---

## 4. 필요한 Tauri Commands

```rust
// graph 관련 commands (Phase 2에서 추가)

/// 심볼에서 1-hop 확장
#[tauri::command]
async fn graph_expand(
    project_path: String,
    symbols: Vec<String>,
    depth: Option<u32>,        // 기본 1
    include_tests: Option<bool>, // 기본 true
) -> Result<GraphExpansion, AppError>;

/// 변경 파일의 테스트 커버리지 체크
#[tauri::command]
async fn graph_coverage_check(
    project_path: String,
    changed_files: Vec<String>,
) -> Result<CoverageReport, AppError>;

// 반환 타입
pub struct GraphExpansion {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

pub struct GraphNode {
    pub path: String,
    pub symbol: String,
    pub kind: String,           // "function", "class", "module"
    pub has_tests: bool,
}

pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub relation: String,       // "calls", "imports", "tests"
}

pub struct CoverageReport {
    pub covered: Vec<String>,   // 테스트가 있는 변경 파일
    pub uncovered: Vec<String>, // 테스트가 없는 변경 파일
    pub warnings: Vec<String>,  // "X uses changed Y but has no test"
}
```

---

## 5. Knowledge Layer와의 관계

`knowledgeLayerArchitectureIdea.md`의 `GraphSource`는 **ContextPack에 구조 정보를 주입**:
- 에이전트가 일반 대화에서 코드를 논의할 때 구조 맥락 제공
- `is_relevant()`: 코드 관련 쿼리 + 워크플로우 중

이 문서의 워크플로우 graph 활용은 **워크플로우 프롬프트 자체를 보강**:
- `workflowOrchestration.ts`의 각 단계별 프롬프트에 구조 정보 삽입
- Developer/Reviewer/Architect가 구조적 근거로 판단

```
Knowledge Layer (ContextPack)     ← 일반 대화 시 구조 맥락
  └── GraphSource

Workflow Orchestration            ← 워크플로우 프롬프트 보강
  ├── buildPlanContextWithStructure()
  ├── approveAndStartImplementation() + graph
  ├── startReviewRT() + coverage check
  └── rework prompt + structural trace
```

둘 다 같은 graph 바이너리를 사용하지만, 호출 시점과 목적이 다르다.

---

## 참고 자료

- 워크플로우 파이프라인: `src/lib/workflowOrchestration.ts`
- code-review-graph: `_research/_util/code-review-graph/`
- Knowledge Layer: `docs/ideas/knowledgeLayerArchitectureIdea.md`
- rawq + graph 전략: `docs/ideas/rawqGraphEvolutionStrategyIdea.md`
- RT 알고리즘: `docs/ideas/rtAlgorithmEnhancementIdeas.md`
- 워크플로우 설계: `docs/plans/orchestratedWorkflowPipelinePlan.md`
