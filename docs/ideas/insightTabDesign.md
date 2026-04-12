# Insight 탭 설계 — 프로젝트 전체 품질 분석 워크플로우

> 작성: 2026-04-06 (세션 14)
> 상태: 설계 단계

---

## 1. 동기

### 현재 문제
- Review/Test 탭이 Artifacts 탭의 필터링된 뷰에 불과 (데이터 중복)
- 프로젝트 전체를 대상으로 한 품질 분석 기능이 없음
- tunaInsight는 별도 서비스로 존재하지만 tunaFlow의 인프라(rawq, graph, lessons, memory)를 활용하지 못함

### 해결
- Review+Test 탭을 **Insight** 탭으로 통합/대체
- 프로젝트 전체 품질 분석 → 보고서 → 사용자 검토 → Architect Plan → 워크플로우 실행 → 보고서 업데이트
- tunaFlow 인프라를 적극 활용하여 토큰 효율적 타겟 분석

### 탭 구조 변경
```
Before: Chat | Plan | Artifacts | Review | Test
After:  Chat | Plan | Artifacts | Insight
```

---

## 2. tunaInsight vs tunaFlow Insight

| | tunaInsight | tunaFlow Insight |
|---|---|---|
| 분석 대상 | GitHub 레포 clone | 이미 열린 프로젝트 |
| 코드 탐색 | 에이전트가 전체 탐색 | rawq + code-graph로 타겟 영역 사전 추출 |
| 과거 지식 | 없음 | failure_lessons + conversation_memory |
| 결과 활용 | 보고서 파일 | 보고서 DB → Architect → Plan → 워크플로우 |
| 토큰 비용 | 수만~수십만 | 수천~수만 (타겟 주입) |

---

## 3. 워크플로우

```
Phase 1: 분석 실행
  |- run_project_tests → 테스트 결과 수집
  |- rawq 검색 → 품질 문제 후보 영역 (dead code, 복잡도, 에러 패턴)
  |- code-review-graph → 커플링/영향도 높은 모듈
  |- failure_lessons → 반복 결함 패턴
  +- 카테고리별 맞춤 컨텍스트 조립 → 에이전트 분석 실행

Phase 2: 보고서 도출
  |- 카테고리별 개별 보고서 (findings + severity + fix_difficulty)
  |- 메타 에이전트 종합 보고서
  +- insight_findings DB 저장 (항목별)

Phase 3: 사용자 검토
  |- Insight 탭에 보고서 항목 표시 (Quadrant 뷰)
  |- 사용자가 체크박스로 실행할 항목 선택
  |- Auto 등급은 "바로 실행" 가능
  +- Guided/Strategic 항목은 Architect에게 선택 전달

Phase 4: Architect → Plan
  |- 선택 항목 요약만 프롬프트에 포함 (전문 아님)
  |- Architect가 tool-request로 상세 검색 (rawq, graph, 보고서 DB)
  |- Plan proposal 생성
  +- 기존 Plan 워크플로우 (Approval → Implementation → Review)

Phase 5: 보고서 업데이트
  |- Plan 완료(done) 시 관련 보고서 항목 resolved 마킹
  +- 다음 Insight 실행 시 이전 보고서 참조 (개선/악화 추적)
```

---

## 4. 토큰 절감 전략

### 4.1 사전 추출 (Phase 1)
에이전트에게 "프로젝트 전체를 읽어봐"가 아니라 **시스템이 미리 추출한 영역만** 분석하게 함.

| 인프라 | 추출 내용 | 대상 카테고리 |
|---|---|---|
| rawq | `catch {}`, `unwrap`, `expect`, `todo!` 패턴 | 안정성, 보안 |
| rawq | 중복 코드 후보 (유사 snippet) | 기술 부채 |
| rawq | `.clone()`, `lock()`, 루프 내 쿼리 | 성능 |
| rawq | `innerHTML`, SQL 문자열 결합, `.env` | 보안 |
| rawq | `TODO`, `FIXME`, `deprecated` | 기술 부채 |
| code-graph | fan-in/fan-out 상위 모듈, 순환 의존성 | 아키텍처 |
| code-graph | 미참조 모듈 (dead module) | 기술 부채 |
| failure_lessons | 반복 결함 파일/패턴 | 안정성 |
| test runner | 실패 테스트 + 커버리지 갭 | 테스트 |
| conversation_memory | 기술 부채 맥락, 설계 결정 배경 | 아키텍처 |

### 4.2 Architect 자율 탐색 (Phase 4)
- 보고서 전문을 ContextPack에 넣지 않음
- 사용자가 선택한 항목의 **요약만** 프롬프트에 포함 (~500 토큰)
- Architect가 필요 시 `<!-- tunaflow:tool-request:insight:QUERY -->` 마커로 상세 검색
- tool-request handler가 insight_findings DB에서 관련 항목 조회 → 응답 주입

### 4.3 비교 (추정)

| 방식 | 토큰/분석 |
|---|---|
| tunaInsight (전체 탐색) | 50k~200k |
| tunaFlow Insight (타겟) | 5k~20k |
| Architect 자율 탐색 | 추가 2k~5k (필요시만) |

---

## 5. 데이터 모델

### 5.1 insight_sessions 테이블

```sql
CREATE TABLE insight_sessions (
  id          TEXT PRIMARY KEY,
  project_key TEXT NOT NULL,
  status      TEXT NOT NULL DEFAULT 'pending',  -- pending/analyzing/completed/failed
  categories  TEXT,                              -- JSON array of selected categories
  test_output TEXT,                              -- 테스트 실행 결과
  created_at  INTEGER NOT NULL,
  completed_at INTEGER
);
```

### 5.2 insight_findings 테이블

```sql
CREATE TABLE insight_findings (
  id              TEXT PRIMARY KEY,
  session_id      TEXT NOT NULL,
  project_key     TEXT NOT NULL,
  category        TEXT NOT NULL,     -- 'stability' | 'test' | 'architecture' | 'performance' | 'security' | 'debt'
  severity        TEXT NOT NULL,     -- 'critical' | 'major' | 'minor' | 'info'
  fix_difficulty  TEXT NOT NULL,     -- 'auto' | 'guided' | 'manual'
  title           TEXT NOT NULL,
  description     TEXT NOT NULL,
  file_path       TEXT,
  estimated_files INTEGER DEFAULT 1, -- 예상 수정 파일 수
  resolution      TEXT,              -- resolved 시 채움
  plan_id         TEXT,              -- 연결된 Plan (resolved 시)
  status          TEXT NOT NULL DEFAULT 'open',  -- open/selected/in_progress/resolved/dismissed
  created_at      INTEGER NOT NULL
);
```

### 5.3 insight_reports 테이블 (종합 보고서)

```sql
CREATE TABLE insight_reports (
  id          TEXT PRIMARY KEY,
  session_id  TEXT NOT NULL,
  project_key TEXT NOT NULL,
  type        TEXT NOT NULL,     -- 'category' | 'meta'
  category    TEXT,              -- category (nullable for meta)
  content     TEXT NOT NULL,     -- 마크다운 보고서 본문
  created_at  INTEGER NOT NULL
);
```

---

## 6. UI 설계

### 6.1 Insight 탭 레이아웃

```
+--------------------------------------------------+
| [Run Analysis v]  [Categories: All v]  Last: 2h   |
+--------------------------------------------------+
| +-- Summary ------------------------------------+ |
| | Critical: 2  Major: 5  Minor: 8  Info: 3     | |
| | Auto: 6  Guided: 8  Manual: 4                | |
| | Resolved: 12  Selected: 3                     | |
| +-----------------------------------------------+ |
|                                                    |
| -- Quick Wins (auto + high impact) ---- [Run All]  |
| [v] silent catch 35개 → console.error       [Fix]  |
| [v] 미사용 import 8개                       [Fix]  |
|                                                    |
| -- Strategic (guided + high impact) --------------- |
| [ ] auth.rs 에러 핸들링 누락           [Plan 생성]  |
| [ ] 테스트 커버리지 갭 3개 모듈         [Plan 생성]  |
|                                                    |
| -- Deprioritize (manual) -------------------------- |
| [ ] DB 커넥션 풀 아키텍처 변경              [Memo]  |
| [ ] 상태 관리 리팩토링                      [Memo]  |
|                                                    |
| [Send to Architect (2 selected)]                   |
|                                                    |
| -- History ---------------------------------------- |
| > 2026-04-06 12:00 -- 15 findings (8 resolved)    |
| > 2026-04-05 09:30 -- 12 findings (12 resolved)   |
+----------------------------------------------------+
```

### 6.2 핵심 인터랙션
- **Run Analysis**: 카테고리 선택 → 사전 추출 → 에이전트 분석 → 결과 표시
- **Quadrant 분류**: fix_difficulty + severity 자동 분류 → Quick Wins / Strategic / Fill-ins / Deprioritize
- **Auto Fix**: ⚠️ **메타에이전트 도입 후 구현** — 현재 비활성화. CodeCureAgent 패턴(실행→테스트→검증→롤백)은 에이전트를 자율적으로 오케스트레이션하는 메타에이전트 없이는 안전하게 동작하기 어렵다고 판단. `docs/ideas/onboardingMetaAgentIdea.md` §Auto Fix 참조.
- **Architect에게 전달**: `auto` 제외 findings(guided/manual) → Architect Review Branch 생성 → Architect가 자율 판단(Plan 승격 여부, 묶음/분리 결정)
- **History**: 이전 분석 세션과 비교 (개선/악화 추적)
- **Auto-resolve**: Plan done 시 관련 finding 자동 resolved

---

## 7. 카테고리 기반 분석 설계

### 7.1 기본 6-카테고리

페르소나가 아닌 **분석 목적(카테고리)**으로 에이전트에게 의뢰한다.
사용자가 카테고리를 선택해서 부분 분석 가능 ("안정성만 돌려볼게").

| 카테고리 | 분석 대상 | 사전 추출 |
|---|---|---|
| **안정성 (stability)** | 에러 처리, panic, silent catch, 경계 조건 | rawq: `catch {}`, `unwrap`, `expect` + failure_lessons |
| **테스트 (test)** | 커버리지 갭, 테스트 품질, 미테스트 경로 | test runner + rawq: 테스트 없는 모듈 |
| **아키텍처 (architecture)** | 의존성, 순환참조, 레이어 위반, 커플링 | code-graph + memory |
| **성능 (performance)** | 불필요한 복사, N+1, 블로킹 호출 | rawq: `.clone()`, `lock()`, 루프 내 쿼리 |
| **보안 (security)** | 인젝션, XSS, 인증 갭, 시크릿 노출 | rawq: `innerHTML`, SQL 결합, `.env` |
| **기술 부채 (debt)** | dead code, TODO/FIXME, deprecated API | rawq + graph 미참조 모듈 |

### 7.2 수정 난이도 자동 평가 (fix_difficulty)

각 finding에 에이전트가 수정 가능한지를 판단하는 난이도 등급을 부여한다.
**에이전트가 헤매면 무한버그지옥에 빠지므로** finding 단계에서 수정 가능성을 걸러야 한다.

#### 학술적 근거

**SWE-bench 계열 벤치마크** [1][2]:
- 수정 파일 수/라인 수가 에이전트 성공률과 강한 음의 상관관계
- SWE-bench Pro: 평균 107줄/4.1파일 문제에서 성공률 급격 저하
- 레포 특성(복잡도, 문서화)이 성공률의 결정적 변수
- 특정 레포에서 모든 모델 10% 미만, 다른 레포에서 50%+

**RepairAgent (ICSE 2025)** [3]:
- 단일 라인 버그에서 강하지만 multi-file 버그에서 성능 저하
- **파일 수가 난이도의 가장 좋은 프록시**

**CodeCureAgent (2025)** [4]:
- 정적 분석 경고 1,000개(SonarQube) 중 **96.8% 자동 수정 성공**
- 핵심 패턴: **Change Approver** — 수정 후 빌드 → 원래 경고 사라짐 확인 → 새 경고 없음 확인 → 테스트 통과 확인
- 실패 시 자동 롤백

#### 난이도 추정 공식

```
fix_difficulty = f(수정 예상 파일 수, 수정 예상 라인 수, 의존성 팬아웃)

auto:    1파일 + <20줄 + 낮은 팬아웃 (code-graph fan-out < 5)
guided:  2~5파일 + <100줄 + 중간 팬아웃
manual:  5+파일 또는 100+줄 또는 높은 팬아웃 (fan-out > 15)
```

에이전트에게 분석 시 같이 판정하도록 지시:

```
각 finding에 fix_difficulty를 부여하세요:
- auto: 단일 파일, 기계적 수정, 검증 명확 (에이전트 1회 성공 확률 90%+)
- guided: 소수 파일, 로직 변경 있음, task 파일 필요 (성공 확률 70%+)
- manual: 구조적 변경, 사람 판단 필요 (성공 확률 70% 미만)
```

#### Auto 수정 파이프라인 (CodeCureAgent 패턴 적용 [4]) — ⚠️ 메타에이전트 보류

> **현재 상태**: 비활성화. UI에서 Auto Fix 버튼 제거.
>
> **이유**: CodeCureAgent 패턴은 에이전트를 자율적으로 오케스트레이션해야 안전하다.
> - 수정 실행 → 테스트 → 검증 → 롤백을 에이전트가 자율 판단해야 하는데
> - 현재 tunaFlow는 Human이 중간에 개입하는 구조 (Plan → Approval Gate → Developer → Review)
> - 메타에이전트 없이 Auto Fix만 도입하면 리뷰 없이 코드가 변경되는 위험 경로가 생김
>
> **향후 설계**: 메타에이전트 도입 시 CodeCureAgent를 메타에이전트의 서브루틴으로 편입.
> 메타에이전트가 "auto finding → 수정 실행 → 검증 → 실패 시 guided로 격상 → Human 게이트" 전체를 관리.

```
[메타에이전트 도입 후 계획]
1. finding 기반 수정 실행 (코딩 에이전트 — Developer 역할)
2. run_project_tests → 기존 테스트 통과 확인
3. rawq 재스캔 → 원래 패턴 사라졌는지 확인
4. 새 경고/에러 없는지 확인
5. 성공 → finding 자동 resolved + git commit
6. 실패 → 자동 롤백 + guided로 등급 상향 + Human 게이트 트리거
```

### 7.3 우선순위 결정 — SQALE + Quadrant 방법론

#### SQALE (Software Quality Assessment based on Lifecycle Expectations) [5][6]

각 finding에 두 가지 비용을 추정:
- **Remediation cost** (수정 비용): fix_difficulty 등급 x 파일 수 x 복잡도
- **Non-remediation cost** (방치 비용): 변경 빈도 x 의존 모듈 수 x 과거 실패 횟수

```
우선순위 = non_remediation_cost / remediation_cost (ROI)

non_remediation_cost 시그널:
  - git log 변경 빈도 (자주 바뀌는 파일 = 방치 비용 높음)
  - code-graph fan-in (많이 참조되는 모듈 = 영향도 높음)
  - failure_lessons 이력 (반복 실패 = 방치 비용 높음)

remediation_cost 시그널:
  - fix_difficulty 등급
  - 수정 예상 파일/라인 수
```

연구에 따르면 SonarQube의 수정 시간 추정치 대비 실제 개발자 수정 시간은 50% 이하 [7].
에이전트 수정은 단순 패턴에서 더 빠를 가능성이 높다.

#### Quadrant Method (Impact x Cost) [8]

```
              High Impact
                  |
   Quick Wins ----+---- Strategic
   (auto 실행)    |    (guided → Plan)
                  |
------------------+------------------
                  |
   Fill-ins ------+---- Deprioritize
   (여유 시 auto) |    (manual 또는 dismiss)
                  |
              Low Impact
       Low Cost <---> High Cost
```

- **Quick Wins** → auto + 높은 우선순위 → "바로 실행" 버튼
- **Strategic** → guided → Architect에게 Plan 생성 요청
- **Fill-ins** → auto + 낮은 우선순위 → 목록에 보여주되 사용자 선택
- **Deprioritize** → manual → 메모만, 또는 dismiss

### 7.4 분석 프롬프트 구조

```
## 프로젝트 분석 요청: {category_name}

### 분석 대상
{project_name} ({project_path})

### 사전 조사 결과
{카테고리별 rawq/graph/lessons/test 사전 추출 결과}

### 분석 지침
- 위 사전 조사 결과를 기반으�� 분석하세요
- 필요 시 코드를 직접 읽어 확인하세요
- 각 finding에 아래 항목을 부여하세요:
  - severity: critical / major / minor / info
  - fix_difficulty: auto / guided / manual
  - 예상 수정 파일 수 및 설명
- 파일 경로와 구체적 설명을 포함하세요

### 출력 형식
<!-- tunaflow:insight-findings -->
[
  {
    "title": "...",
    "severity": "critical|major|minor|info",
    "category": "{category}",
    "fixDifficulty": "auto|guided|manual",
    "filePath": "...",
    "estimatedFiles": 1,
    "description": "..."
  }
]
<!-- /tunaflow:insight-findings -->
```

---

## 8. Architect 자율 탐색 (tool-request 확장)

기존 tool-request 4종(docs/rawq/graph/plans)에 `insight` 추가:

```
<!-- tunaflow:tool-request:insight:security findings -->
```

→ insight_findings에서 category='security' 검색 → 관련 항목 주입

Architect가 보고서 전문 없이도 필요한 부분만 자율적으로 탐색 가능.

---

## 9. 기존 기능과의 관계

| 기존 기능 | Insight와의 관계 |
|---|---|
| Plan 워크플로우 | Insight → Plan 생성 (findings에서 파생) |
| failure_lessons | Insight 분석 입력 + Insight에서 발견한 문제도 lesson화 가능 |
| Artifacts | Insight 보고서도 artifact로 저장 (plan_id 연결) |
| RT | Insight 분석 = 카테고리별 에이전트 실행 (RT 또는 단일 에이전트) |
| rawq/graph | Insight 분석의 사전 탐색 엔진 |
| Review 탭 (삭제) | Artifacts Harness 필터로 대체 |
| Test 탭 (삭제) | Artifacts test-report + Insight 테스트 결과로 대체 |
| Evaluation (Test 하위) | Insight 하위 서브뷰로 이동 또는 별도 유지 |

---

## 10. 구현 우선순위

| Phase | 내용 | 공수 |
|---|---|---|
| **A** | DB migration (insight_sessions + insight_findings + insight_reports) | small |
| **B** | Rust commands + TS types/API | small |
| **C** | 카테고리별 사전 추출 파이프라인 (rawq/graph/test/lessons → 카테고리별 컨텍스트) | medium |
| **D** | 분석 실행 + findings 마커 파서 + fix_difficulty 판정 + DB 저장 | medium |
| **E** | Insight 탭 UI (Quadrant 뷰, 체크박스, "Architect에게 전달" 버튼) | medium |
| **F** | ~~Auto 수정 파이프라인~~ → **메타에이전트 도입 후 재설계** (현재 보류) | — |
| **G** | Review/Test 탭 삭제 + CenterPanel 4탭 전환 | small |
| **H** | Architect tool-request:insight 핸들러 | small |
| **I** | Plan 완료 시 finding auto-resolve | small |
| **J** | 분석 이력 비교 + 우선순위 계산 (SQALE ROI) | medium |

---

## 11. 참고 연구 및 방법론

### 학술 논문

[1] C. E. Jimenez et al., "SWE-bench: Can Language Models Resolve Real-world Github Issues?", 2024.
    https://github.com/SWE-bench/SWE-bench
    - 수정 파일/라인 수와 에이전트 성공률의 강한 음의 상관관계 확인
    - 레포 특성이 성공률의 결정적 변수

[2] Scale AI, "SWE-bench Pro: Can AI Agents Solve Long-Horizon Software Engineering Tasks?", 2025.
    https://static.scale.com/uploads/654197dc94d34f66c0f5184e/SWEAP_Eval_Scale%20(9).pdf
    - 10줄 ���상 수정 필요 문제만 포함, 평균 107줄/4.1파일
    - multi-file 문제에서 에이전트 성능 급격 저하

[3] I. Bouzenia et al., "RepairAgent: An Autonomous, LLM-Based Agent for Program Repair", ICSE 2025.
    https://software-lab.org/publications/icse2025_RepairAgent.pdf
    - 단일 라인 strong, multi-file weak
    - 39개 버그를 기존 SOTA 도구가 못 고친 것을 수정

[4] "CodeCureAgent: Automatic Classification and Repair of Static Analysis Warnings", 2025.
    https://arxiv.org/pdf/2509.11787
    - SonarQube 경고 1,000개 중 96.8% 자동 수정
    - Change Approver 패턴: 빌드 → 경고 사라짐 확인 → 테스트 통과

[5] J.-L. Letouzey, "The SQALE Method for Evaluating Technical Debt", MTD 2012.
    https://dl.acm.org/doi/abs/10.5555/2666036.2666042
    - remediation cost + non-remediation cost = ROI 기반 우선순위
    - 계층적 품질 모델: Testability → Reliability → Security → ...

[6] "SQALE, the ultimate Quality Model to assess Technical Debt", Sonar.
    https://www.sonarsource.com/blog/sqale-the-ultimate-quality-model-to-assess-technical-debt/
    - SonarQube의 SQALE 구현 설명

[7] "On the Technical Debt Prioritization and Cost Estimation with SonarQube tool", ResearchGate.
    https://www.researchgate.net/publication/345632101
    - SonarQube 추정치 대비 실제 수정 시간 50% 이하

[8] vFunction, "How to Prioritize Tech Debt: Strategies for Effective Management", 2025.
    https://vfunction.com/blog/how-to-prioritize-tech-debt-strategies-for-effective-management/
    - Quadrant Method (Impact x Cost) 프레임워크

### 추가 참고

[9] "A Survey of LLM-based Automated Program Repair", 2025.
    https://arxiv.org/pdf/2506.23749
    - LLM 기반 자동 프로그램 수정 전체 서베이

[10] "LLM-based Agents for Automated Bug Fixing: How Far Are We?", 2024.
     https://arxiv.org/html/2411.10213v2
     - 에이전트 기반 버그 수정의 현재 한계와 가능성

[11] "LLM-Based Agentic Systems for Software Engineering", 2026.
     https://arxiv.org/pdf/2601.09822
     - SE 에이전트 시스템의 패러다임 비교 (fine-tuning/prompting/pipeline/agentic)

---

## 12. 하지 않는 것

- GitHub 레포 clone (tunaFlow는 로컬 프로젝트만)
- 레퍼런스 레포 비교 분석 (단일 프로젝트 분석만)
- 보고서 파일 시스템 저장 (DB가 SSOT)
- Evaluation 패널 삭제 (Insight 하위로 유지)
- 페르소나 기반 분석 (카테고리 기반으로 대체)
