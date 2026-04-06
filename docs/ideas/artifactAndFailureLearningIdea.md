# Artifact 재정의 + 실패 학습 시스템

> 작성: 2026-04-06 (세션 13)
> 상태: 아이디어 단계

---

## 1. 현재 Artifact 시스템 현황

| 타입 | DB 테이블 | 실제 사용 |
|------|-----------|-----------|
| review-findings | artifacts | Reviewer fail 시 생성 |
| architect-decision | artifacts | Architect 의사결정 기록 |
| test-report | artifacts | 미사용 (testOutput이 프롬프트에만 주입) |

### 이미 별도 시스템으로 존재하는 것 (artifact 중복 불필요)

- **Plan**: `plans` 테이블 + `docs/plans/{slug}.md`
- **Review Verdict**: `plan_events` (review_passed/failed/conditional)
- **Implementation Brief**: `{slug}-task-NN.md` 파일이 이 역할
- **Findings**: `plan_events` detail에 JSON으로 저장

---

## 2. 실제로 빠져있는 것

### 2.1 Test Summary 영속화

- 현재: `run_project_tests()` → 결과가 프롬프트에만 주입, DB에 안 남음
- 문제: rework 시 이전 테스트 결과 참조 불가
- 방향: test 결과를 artifact 또는 plan_events에 영속화

### 2.2 실패 학습 (Failure Learning)

- 현재: review fail findings가 `plan_events.detail`에 JSON으로 저장되지만 검색 불가
- 문제: 같은 프로젝트에서 같은 패턴의 실수 반복 (e.g., FTS5 rowid 혼동, 에러 무시)
- 방향: 실패 이력을 검색 가능하게 저장 → rework 시 유사 실패 자동 검색

---

## 3. 실패 학습 시스템 설계

### 3.1 핵심 원칙

- **1차 구현 시에는 실패 이력 불필요** (검색 비용 0)
- **Rework 시에만 유사 실패 검색** → 관련 이력을 rework 프롬프트에 자동 포함
- 에이전트가 스스로 검색하지 않음 — 시스템이 자동 주입

### 3.2 데이터 흐름

```
Review fail
  → findings 추출 (file path, 결함 설명)
  → failure_lessons 테이블에 저장 (project_key, plan_id, finding, tags)

Rework 프롬프트 생성 시
  → 현재 findings의 파일/패턴으로 유사 실패 검색
  → 관련 이력을 rework 프롬프트에 "이전 유사 사례" 섹션으로 포함
```

### 3.3 DB 스키마 (안)

```sql
CREATE TABLE failure_lessons (
  id TEXT PRIMARY KEY,
  project_key TEXT NOT NULL,
  plan_id TEXT,
  file_path TEXT,          -- 결함 파일 경로
  pattern TEXT,            -- 결함 패턴 요약 (e.g., "FTS5 rowid vs logical index")
  finding TEXT NOT NULL,   -- 원본 finding 텍스트
  resolution TEXT,         -- 해결 방법 (rework 후 pass되면 자동 채움)
  created_at INTEGER NOT NULL
);
```

### 3.4 검색 방식

- **파일 경로 매칭**: 같은 파일에서 이전 실패가 있었는지
- **패턴 키워드**: finding 텍스트의 키워드 유사도 (FTS5 또는 단순 LIKE)
- **프로젝트 범위**: 같은 project_key 내에서만 검색

### 3.5 토큰 효율

| 시점 | 토큰 비용 |
|------|-----------|
| 1차 구현 | 0 (검색 안 함) |
| Rework | 유사 실패 N건 × ~100토큰 = ~300토큰 |
| 컨텍스트팩 상시 주입 대비 | 매 요청 ~200토큰 절약 |

---

## 4. UI/UX 고려사항

### 4.1 Plan 탭 — 실패 이력 표시

- DevProgressView rework 상태에서 "이전 유사 실패" 섹션 표시
- 접힘/펼침 가능 (기본 접힘)
- 각 항목: 파일 경로 + 패턴 + 해결 여부

### 4.2 Settings — 실패 학습 관리

- 프로젝트별 failure_lessons 목록 조회
- 불필요한 항목 삭제
- 패턴 수동 편집 (자동 추출이 부정확할 때)

### 4.3 Rework 프롬프트 표시

- rework 메시지에 "📚 유사 실패 사례 (N건)" 섹션 추가
- 사용자가 전송 전 확인 가능

### 4.4 Resolution 자동 채움

- Rework 후 Review pass → 해당 failure_lesson의 resolution을 Developer의 수정 내용으로 채움
- "이 패턴은 이렇게 해결됨" 이력 축적

---

## 5. Test Summary 영속화

### 5.1 현재 문제

- `run_project_tests()` 결과가 `startReviewRT`의 `testOutput`으로 전달 (방금 수정)
- 하지만 DB에 저장되지 않아 이후 참조 불가

### 5.2 방향

- plan_events에 `test_completed` 이벤트로 저장 (detail에 요약)
- 또는 artifacts 테이블에 type: "test-summary"로 저장
- ContextPack의 plan_document에 자동 포함

---

## 6. 구현 우선순위

| 순서 | 항목 | 공수 |
|------|------|------|
| 1 | failure_lessons 테이블 + DB migration | small |
| 2 | processReviewVerdict fail 시 자동 저장 | small |
| 3 | DevProgressView rework 시 유사 검색 + 프롬프트 주입 | medium |
| 4 | Resolution 자동 채움 (pass 후) | small |
| 5 | UI: rework 프롬프트에 유사 사례 표시 | small |
| 6 | Test Summary 영속화 | small |

---

## 7. 하지 않는 것

- 컨텍스트팩 상시 주입 (토큰 낭비)
- 에이전트가 스스로 실패 이력 검색 (tool-request 의존은 불확실)
- 프로젝트 간 실패 공유 (프로젝트별 코드베이스가 다름)
- Knowledge Note (context-hub 채택 기준이 모호)
