# Insight 워크플로우 고도화 — 파일 저장 + Plan 승격 + Finding 생명주기

> Status: idea
> Created: 2026-04-09
> 관련: Insight 탭 (세션 15 구현, DB v29), 워크플로우 파이프라인 (세션 5-14)

---

## 현재 문제

1. Insight 분석 결과가 DB에만 있고, **에이전트가 접근할 수 없음** — 아키텍트에게 "Insight 탭 볼 수 있어?"라고 물으면 "볼 수 없다"고 답함
2. Finding에서 Architect로 넘기는 **경로가 없음** — Auto Fix(CodeCureAgent)는 단순 수정만 가능
3. 해결된 finding의 **이력(과정/결과) 관리가 없음** — 해결 후 추적 불가

---

## 제안 1: Insight 리포트 파일 저장 + 에이전트 자율 접근

### 해결: 파일 저장 + 에이전트 자율 읽기

ContextPack에 데이터를 넣는 대신, **프로젝트 내 파일로 저장하여 에이전트가 직접 읽게** 한다. CLI 에이전트의 파일시스템 접근 능력을 활용.

```
Insight 분석 완료 시:
  → docs/insight/latest-report.md        (전체 리포트)
  → docs/insight/findings/SEC-001.md     (개별 finding)
  → docs/insight/findings/ARCH-003.md

ContextPack Tier 0에 한 줄만 추가:
  "프로젝트 분석 리포트: docs/insight/ 참조"

아키텍트가 필요하면:
  → 파일 직접 읽기 (CLI 에이전트의 자연스러운 능력)
  → ContextPack 토큰 비용 = 0
```

### Finding 생명주기

```
open → in_progress → done (읽기 전용)

open:
  - Insight 분석이 발견한 문제
  - docs/insight/findings/SEC-001.md 에 저장
  - 내용: 문제 설명, 심각도, 파일 경로, 코드 스니펫

in_progress:
  - 아키텍트/개발자가 해결 작업 중
  - finding 파일에 해결 과정 append

done:
  - 해결 완료
  - finding 파일에 결과 append:
    - 해결 방법
    - 변경된 파일 목록
    - 검증 결과
  - UI에서 읽기 전용 표시 (접힘/펼침, 편집 불가)
  - 아키텍트에게 "이미 해결됨"으로 인식
```

### Finding 파일 구조 예시

```markdown
# SEC-001: SQL injection in search query

- **Category**: security
- **Severity**: high
- **Fix Difficulty**: auto
- **Status**: done
- **File**: src-tauri/src/commands/context_queries.rs:351

## Description
FTS5 쿼리에 사용자 입력이 직접 삽입됨. 특수 문자 이스케이프 불완전.

## Snippet
\```rust
let query = format!("messages_fts MATCH '{}'", user_input); // 위험
\```

## Resolution
- **Method**: 파라미터 바인딩으로 전환
- **Files Changed**: context_queries.rs
- **Verified**: cargo test --lib 통과
- **Resolved At**: 2026-04-09
```

### 장점

1. **ContextPack 토큰 0** — 파일 경로 한 줄이면 충분
2. **에이전트 자율 접근** — CLI의 파일 읽기 능력 그대로 활용
3. **Git 추적 가능** — insight 결과가 프로젝트 히스토리에 남음
4. **다른 도구와 호환** — IDE, 리뷰어, CI에서도 읽을 수 있음
5. **이력 관리** — 과정/결과가 파일에 누적, done 후 읽기 전용

### 구현 범위

- `run_insight_analysis` 완료 시 → findings/report를 파일로 export
- `insight_findings` 테이블의 status 필드 활용 (open/in_progress/done)
- done 전환 시 resolution 필드 필수 입력 → 파일에 append
- InsightPanel UI에서 done findings를 접힘/읽기 전용으로 표시
- ContextPack identity block에 `docs/insight/` 경로 안내 한 줄 추가

---

## 제안 2: Insight → Architect Plan 승격 UX

### 흐름

```
InsightPanel finding 카드:
  [Auto Fix]    → CodeCureAgent (단순 수정, 기존)
  [Plan 승격]   → Architect에게 전달 (설계 판단 필요 시, 신규)
  [무시]        → status: dismissed

Plan 승격 시:
  1. 선택된 finding(s) → 프롬프트 자동 생성
  2. Chat 탭 전환 + Architect에게 자동 전송
  3. Architect: <!-- tunaflow:plan-proposal --> 마커로 응답
  4. 이후 기존 워크플로우: Approval → Implementation → Review
  5. Plan 완료 시: 연결된 findings 자동 done 처리
```

### 복수 선택

여러 findings를 묶어서 하나의 plan으로 승격 가능. 관련된 문제를 한 번에 해결하는 plan이 효율적.

### 프롬프트 자동 생성

```markdown
## Insight 분석 결과 — Plan 요청

다음 findings를 해결하는 plan을 제안해주세요.

### SEC-001: SQL injection in search query
- **심각도**: high
- **위치**: context_queries.rs:351
- **설명**: FTS5 쿼리에 사용자 입력 직접 삽입

각 finding의 원본 파일을 확인하고 구체적인 subtask로 분해해주세요.
```

### 구현 범위

| 항목 | 인프라 | 작업 |
|------|--------|------|
| Finding 표시 | ✅ InsightPanel | 선택 체크박스 + Plan 승격 버튼 추가 |
| 프롬프트 생성 | - | 🆕 `buildInsightPlanPrompt(findings[])` 함수 |
| Architect 전달 | ✅ `sendWithEngine` | Chat 탭 전환 + 자동 전송 |
| Plan 제안~리뷰 | ✅ 전체 워크플로우 | 변경 없음 |
| Plan 완료 → finding done | - | 🆕 `plan.status=done` 시 연결 findings 업데이트 |
| done finding 표시 | - | 🆕 읽기 전용 접힘 표시 + 해결 과정/결과 열람 |

### Finding ↔ Plan 연결

`insight_findings.plan_id` 필드가 **이미 존재** (DB v29). plan 승격 시 이 필드에 plan ID 설정 → plan 완료 시 역참조로 findings 일괄 done 처리.

---

## 구현 우선순위

| 순서 | 항목 | 효과 | 난이도 |
|------|------|------|--------|
| 1 | **Finding 파일 export** | 에이전트 접근 가능, 즉시 효과 | 낮 |
| 2 | **Plan 승격 버튼 + 프롬프트 생성** | Insight → 워크플로우 연결 | 중 |
| 3 | **Finding 생명주기 UI** (done 읽기 전용) | 이력 관리 | 중 |
| 4 | **Plan 완료 → findings 자동 done** | 자동화 | 낮 (인프라 있음) |
