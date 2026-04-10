# mex 컨텍스트 스캐폴드 — Drift Detection + 패턴 성장 참고

> Status: idea
> Created: 2026-04-10
> 출처: `_research/_util/mex/` (TypeScript 3,100줄, 에이전트 프로젝트 메모리 관리 도구)

---

## 1. mex란

AI 에이전트를 위한 **프로젝트 컨텍스트 스캐폴드 관리 도구**. 에이전트가 세션 간에 프로젝트를 "기억"할 수 있도록 구조화된 마크다운 문서를 유지하고, 문서와 실제 코드의 **drift(불일치)**를 자동 감지합니다.

핵심: "에이전트에게 넘기는 컨텍스트가 현재 코드와 맞는지 자동 검증"

---

## 2. tunaFlow에 참고할 패턴 (6개)

### 2.1 [P1] Drift Detection — 문서-코드 불일치 감지

**mex의 가장 독창적인 기능.** 마크다운에서 "주장(claim)"을 AST로 추출하고 실제 코드와 대조:

```
CLAUDE.md에 "src/stores/slices/runtimeSlice.ts" 기술됨
→ 실제 파일 존재 확인 → 없으면 ✗ MISSING_PATH

CLAUDE.md에 "npx vitest run" 기술됨
→ package.json scripts 확인 → 없으면 ✗ MISSING_COMMAND

CLAUDE.md에 "React 18" 기술됨
→ package.json dependencies 확인 → 버전 불일치 시 ⚠ VERSION_MISMATCH
```

8개 체커:

| 체커 | 검증 대상 |
|------|----------|
| **path** | 문서에 언급된 파일 경로가 실제로 존재하는지 |
| **command** | 문서에 언급된 npm/cargo 명령이 scripts에 있는지 |
| **dependency** | 문서에 언급된 의존성이 매니페스트에 있는지 |
| **cross-file** | 여러 문서 간 버전/이름 일관성 |
| **edge** | frontmatter의 cross-file 링크가 유효한지 |
| **staleness** | git 기준 문서 최종 업데이트 시점 (30일/50커밋 경고) |
| **index-sync** | 인덱스 문서와 실제 파일 목록 일치 |
| **script-coverage** | npm scripts가 문서에 설명되어 있는지 |

**tunaFlow 적용**:

CLAUDE.md가 프로젝트 성장과 함께 drift가 쌓이면 에이전트에게 잘못된 정보 전달. 메타 에이전트 Tier 1(`projectMetaAgentIdea.md`)의 알림 시스템에 drift check 통합:

```rust
// commands/meta.rs — drift 알림 추가
fn check_claude_md_drift(project_path: &str) -> Vec<Alert> {
    let claude_md = read_to_string(project_path.join("CLAUDE.md"))?;
    let claims = extract_path_claims(&claude_md);  // AST 기반 경로 추출
    
    let mut alerts = Vec::new();
    for claim in &claims {
        if !Path::new(project_path).join(&claim.path).exists() {
            alerts.push(Alert {
                level: "warning".into(),
                message: format!("CLAUDE.md에 기술된 '{}' 경로가 존재하지 않습니다", claim.path),
                action_hint: Some("CLAUDE.md 업데이트 필요".into()),
            });
        }
    }
    alerts
}
```

**규모**: ~100줄 Rust. DB 변경 없음. 기존 파일 시스템 + 마크다운 파싱만.

### 2.2 [P1] Git 기반 문서 신선도 추적

```
30일 경과 → 경고
90일 경과 → 에러
50 커밋 경과 → 경고
200 커밋 경과 → 에러
```

**tunaFlow 적용**:

Plan 문서, Idea 문서의 신선도를 git 히스토리로 자동 추적. 메타 에이전트 대시보드에 표시:

```rust
fn check_document_staleness(project_path: &str) -> Vec<Alert> {
    // git log -1 --format=%ct -- docs/plans/*.md
    // 현재 시간 - 마지막 커밋 시간 = 경과일
    // 30일 초과: warning, 90일 초과: error
}
```

**규모**: ~40줄 Rust. git CLI 호출.

### 2.3 [P2] Behavioral Contract (GROW 단계)

mex의 5단계 행동 계약:

```
1. CONTEXT: 라우팅 테이블에서 관련 파일 로드
2. BUILD: 작업 수행
3. VERIFY: 체크리스트 실행
4. DEBUG: 문제 시 디버그 가이드 참조
5. GROW: 작업 완료 후 스캐폴드 업데이트  ← 이것
```

현재 tunaFlow 워크플로우:

```
Plan → Dev → Review → Done
                       ↓
                    (여기서 끝. 문서 업데이트 없음)
```

**GROW 단계 추가**: Developer 완료 시 "CLAUDE.md 또는 Plan 문서를 업데이트할까요?" 제안.

```typescript
// workflowOrchestration.ts — processReviewVerdict() pass 후
if (verdict === "pass") {
    toast.info("작업 완료. 프로젝트 문서를 업데이트할까요?", {
        action: { label: "업데이트 제안", onClick: suggestDocUpdate }
    });
}
```

**규모**: ~30줄 FE. 자동 저장이 아니라 제안만.

### 2.4 [즉시] 부정 표현 섹션 (금지 목록)

mex의 "What we do NOT use" 섹션:

```markdown
## What we do NOT use
- Redux (Zustand 사용)
- MUI (Radix UI 사용)
- SDK 전환 (CLI subprocess가 최종 아키텍처)
```

에이전트가 이 섹션을 읽으면 해당 기술을 제안하지 않습니다.

**tunaFlow 적용**: CLAUDE.md §16 코딩 컨벤션에 명시적 금지 목록 추가.

```markdown
## 사용하지 않는 것
- MCP (토큰 낭비, 도입 계획 없음)
- SDK 직접 통합 (CLI subprocess가 최종 아키텍처)
- GraphRAG / LightRAG (복잡도 대비 가치 부족)
- sqlite-vec (현재 규모에서 brute-force로 충분)
- Python sidecar (Rust 단일 바이너리 유지)
```

**규모**: CLAUDE.md에 섹션 추가. 코드 변경 0.

### 2.5 [P2] 패턴 성장 (Emergent Pattern Growth)

작업 완료 후 발견한 패턴을 구조화 저장:

```markdown
# patterns/auth-middleware-change.md
## Steps
1. middleware.rs 수정
2. routes.rs 호출부 확인

## Gotchas
- ws_handler.rs가 비표준 방식으로 호출

## Verify
- [ ] cargo test 통과

## Debug
- middleware 에러 → trace_log에서 context_mode 확인
```

**tunaFlow 적용**: Artifacts 시스템과 결합. Developer가 작업 중 발견한 패턴을 type="pattern" Artifact로 저장. `artifactsTabDesignReviewIdea.md`의 순환 구조와 일치.

**규모**: Artifact 타입에 "pattern" 추가 (~5줄). UI 변경 없음 (기존 타입 드롭다운에 추가).

### 2.6 [P2] 라우팅 테이블

```markdown
# ROUTER.md
| Task Type | Load These Files |
|-----------|-----------------|
| API 추가 | context/architecture + patterns/api |
| 테스트 | context/conventions + patterns/test |
| 리팩토링 | context/decisions + context/architecture |
```

**tunaFlow 적용**: ContextPack의 auto mode가 이미 비슷한 역할이지만, 프로젝트 스캐폴드에 **명시적 라우팅 규칙**이 있으면 에이전트가 더 정확하게 필요한 맥락을 선택.

프로젝트별 `docs/ROUTER.md`에 태스크-컨텍스트 매핑을 기술하면 ContextPack 조립 시 rawq로 검색 → 관련 문서 자동 로드 가능.

**규모**: 문서 추가만. 코드 변경은 나중에 ContextPack 통합 시.

---

## 3. 참고하지 않을 것

| 패턴 | 이유 |
|------|------|
| mex CLI 자체 도입 | tunaFlow는 데스크톱 앱. CLI 스캐폴드 도구와 용도 다름 |
| AGENTS.md | CLAUDE.md + ContextPack이 이 역할 |
| 도구별 config 생성 | 단일 앱이므로 다중 도구 호환 불필요 |
| Sync prompt 생성 | ContextPack으로 직접 주입. 별도 프롬프트 생성 불필요 |
| YAML frontmatter edges | ContextPack의 cross-session + rawq가 이 역할 |

---

## 4. 구현 우선순위

| 항목 | 시점 | 위치 | 규모 |
|------|------|------|------|
| **부정 표현 섹션** | 즉시 | CLAUDE.md 수정 | 0줄 코드 |
| **Drift Detection** | 메타 에이전트 Tier 1 | `commands/meta.rs` | ~100줄 Rust |
| **Git 신선도 추적** | 메타 에이전트 Tier 1 | `commands/meta.rs` | ~40줄 Rust |
| **GROW 단계** | 워크플로우 확장 | `workflowOrchestration.ts` | ~30줄 FE |
| **패턴 Artifact 타입** | Artifacts 확장 | 타입 추가 | ~5줄 |
| **라우팅 테이블** | ContextPack 통합 시 | 문서 + 코드 | 검토 후 |

### 메타 에이전트 Tier 1과의 통합

`projectMetaAgentIdea.md`의 대시보드 + 알림에 drift check와 신선도 추적을 통합:

```rust
#[tauri::command]
pub fn get_project_dashboard(project_key: String, state: State<DbState>) 
    -> Result<ProjectDashboard, AppError> 
{
    // 기존 SQL 집계 (plan counts, cost, rework ratio)
    // + drift check (CLAUDE.md 경로 검증)
    // + staleness check (docs/ 문서 신선도)
}
```

---

## 5. mex의 컨텍스트 효율성 수치

```
mex 없이: ~3,300 tokens/세션
mex 있음: ~1,450-1,650 tokens/세션
→ ~60% 토큰 절감
```

tunaFlow는 ContextPack으로 이미 관련 데이터만 선별 주입하므로 동일한 수준의 효율성을 달성하고 있습니다. mex에서 가져올 것은 효율성이 아니라 **검증(drift detection)과 성장(pattern growth)** 패턴입니다.

---

## 참고

- mex 소스: `_research/_util/mex/` (TypeScript 3,100줄)
  - Drift 엔진: `src/drift/` (8개 체커)
  - Claim 추출: `src/drift/claims.ts` (AST 기반)
  - 스캐너: `src/scanner/` (코드베이스 사전 분석)
  - Sync: `src/sync/brief-builder.ts` (타겟 프롬프트 생성)
  - ROUTER: `ROUTER.md` (세션 부트스트랩 템플릿)
- tunaFlow 관련 문서:
  - 메타 에이전트: `docs/ideas/projectMetaAgentIdea.md`
  - Artifacts 탭: `docs/ideas/artifactsTabDesignReviewIdea.md`
  - 프로젝트 문서 RAG: `docs/ideas/projectDocumentRagIdea.md`
  - CI 피드백 루프: `docs/ideas/ciExecutionLoopIdea.md`
