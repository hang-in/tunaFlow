# tunaFlow Harness Engineering 적용 실행 프롬프트

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\harnessEngineeringAdoptionPlan.md`

참고 에이전트 문서:
- `D:\privateProject\tunaFlow\agents\architect.md`
- `D:\privateProject\tunaFlow\agents\developer.md`
- `D:\privateProject\tunaFlow\agents\code-reviewer.md`
- `D:\privateProject\tunaFlow\agents\code-reviewerer.md`
- `D:\privateProject\tunaFlow\agents\repo-scout.md`
- `D:\privateProject\tunaFlow\agents\diff-summarizer.md`

이번 작업 목표는:
현재 `tunaFlow`의 plan / branch / RT branch / handoff / project 구조를 바탕으로,
Stavros식 harness를 **점진적으로 제품 기능으로 승격**하는 것이다.

중요:
- 실제 코드 기준으로만 작업
- 추측 금지
- 기존 동작을 한 번에 뒤엎지 말 것
- 기존 plan/branch/handoff 흐름을 최대한 재사용할 것
- 이번 단계는 "MVP harness 적용"까지만
- 모든 응답과 보고는 한국어로만 작성하라
- 일본어/영어 혼용 금지

---

# 전체 범위

이번 구현은 아래 6가지를 포함한다.

1. harness 관점의 artifact 표준화 1차
2. architect 승인 게이트 1차
3. developer lane 연결 1차
4. reviewer lane 연결 1차
5. 우측 패널(workspace panel) 재설계 1차
6. 상태/검증/문서 반영

중요:
- 실제 git/worktree 연동은 이번 단계에서 하지 말 것
- sidecar 도입 금지
- 완전한 RBAC 엔진까지는 가지 않아도 되지만, 역할 경계는 코드/상태/UI에 드러나야 한다

추가 UX 원칙:
- 메인 상단 탭은 채팅 객체만 가진다
- Plan / Reviews / Tests / Artifacts / Trace는 중앙 탭으로 올리지 말고 우측 패널에 둔다
- 단, 우측 패널은 단순 섹션 적층이 아니라 mode 전환형 workspace panel로 재설계해야 한다

---

# 1단계. 현재 구조 분석 및 적용 지점 확정

## 먼저 확인할 파일

### 백엔드
- `D:\privateProject\tunaFlow\src-tauri\src\db\schema.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\db\models.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\plans.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\branches.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\agents.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\memos.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\lib.rs`

### 프론트
- `D:\privateProject\tunaFlow\src\stores\chatStore.ts`
- `D:\privateProject\tunaFlow\src\components\tunaflow\context-panel\PlansPanel.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\context-panel\BranchesPanel.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\ChatPanel.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\MessageItem.tsx`

### 에이전트 문서
- `D:\privateProject\tunaFlow\agents\*.md`

## 1단계 목표

현재 코드에서 아래를 어디에 얹는 게 가장 자연스러운지 먼저 판단하라.

- `task-brief`
- `review-findings`
- `architect-decision`
- `test-report`
- 승인 게이트
- reviewer lane

중요:
- 새 시스템을 뜬금없이 만들지 말고
- 지금 있는 `plan`, `subtask`, `branch`, `artifact`, `message`, `memo` 중 어느 것을 재사용할지 먼저 결정할 것

## 1단계 산출물

아래를 작업 보고에 포함하라.

- 현재 구조에서 재사용 가능한 엔티티
- 새로 추가가 필요한 최소 엔티티/필드
- 이번 단계에서 하지 않을 것

1단계가 끝나면 바로 2단계로 진행하라.

---

# 2단계. Harness Artifact 표준화 1차

## 목표

아래 artifact를 현재 구조 안에서 1급으로 다룰 수 있게 하라.

- task-brief
- review-findings
- architect-decision
- test-report

## 구현 원칙

### 1. 최소 변경 우선

가능하면 기존 artifact/memo/message 테이블이나 구조를 재사용하라.

예:
- artifact type 추가
- memo type 추가
- 기존 message 메타 확장

중요:
- 새 테이블 남발 금지
- 그러나 기존 구조로는 정말 안 맞으면 최소한의 추가는 허용

### 2. 연결성 유지

artifact는 최소한 아래 중 일부와 연결 가능해야 한다.

- project
- conversation
- branch
- subtask

### 3. 진실원 역할

developer / reviewer 입력의 source of truth가 채팅 본문이 아니라 artifact가 되도록 구조를 정리하라.

## 완료 기준

- task 하나에 대해 task-brief / test-report / review-findings / architect-decision을 연결해 남길 수 있다

2단계가 끝나면 바로 3단계로 진행하라.

---

# 3단계. Architect 승인 게이트 1차

## 목표

문자열 `approved` 같은 취약한 signoff 대신,
현재 UI 흐름 안에서 최소한의 승인 게이트를 구조화하라.

## 최소 필요 게이트

1. Plan 승인
2. Task 시작 승인
3. Review 후 retry 또는 accept 판정

## 구현 방향

- PlanCard / PlansPanel 또는 관련 UI에서 승인 상태를 둘 수 있으면 재사용
- 버튼/상태 뱃지/작은 액션 형태면 충분
- 새 페이지/복잡한 workflow 디자이너 금지

## 중요

- 승인되지 않은 task는 developer lane 시작 금지
- architect decision artifact와 연결 가능하면 연결

## 완료 기준

- 사용자가 UI action으로 승인/보류를 명시할 수 있다
- architect 흐름이 텍스트 파싱에 의존하지 않는다

3단계가 끝나면 바로 4단계로 진행하라.

---

# 4단계. Developer Lane 1차 연결

## 목표

현재 branch/subtask 구조를 활용해 developer lane을 더 명확히 만들라.

## 구현 요구사항

1. subtask 또는 plan에서 developer branch를 시작할 수 있는 경로 검토
2. branch가 어떤 task-brief를 구현하는지 드러나게 하기
3. owner_agent와 branch/task 관계를 더 분명히 하기

## 권장 방향

- branch 메타에 task brief artifact id 또는 subtask id 연결
- `owner_agent`가 developer lane에 실질적으로 반영되게 할 것
- 기존 RT branch와 충돌하지 않게 할 것

## 하지 말 것

- 실제 git branch 생성
- worktree 도입
- branch 시스템 전면 재설계

## 완료 기준

- 특정 subtask에서 시작된 developer branch가 무엇을 구현 중인지 추적 가능하다

4단계가 끝나면 바로 5단계로 진행하라.

---

# 5단계. Reviewer Lane 1차 연결

## 목표

read-only reviewer 흐름을 제품 기능으로 더 명확히 만들라.

## 구현 요구사항

1. reviewer 입력은 최소한 아래를 포함해야 한다.
   - task-brief
   - diff 또는 변경 요약
   - test-report
2. reviewer 결과는 `review-findings` artifact로 구조화하라
3. architect가 findings를 보고 accept/retry 판단할 수 있어야 한다

## 권장 방향

- RT branch를 reviewer lane과 완전히 동일시하지 말 것
- review는 review artifact 중심, RT는 토론 중심으로 구분
- 다만 1차 구현에서 RT branch 재사용이 가장 작다면 허용 가능

## 완료 기준

- 리뷰 결과가 단순 채팅 텍스트가 아니라 추적 가능한 findings artifact로 남는다

5단계가 끝나면 바로 6단계로 진행하라.

---

# 6단계. 우측 패널(workspace panel) 재설계 1차

## 목표

harness 도입으로 우측 패널이 과적재되지 않도록,
기존 context panel을 **workspace panel** 성격으로 재설계하라.

## 핵심 원칙

1. 중앙 상단 탭은 채팅 객체만
   - Architect
   - Developer Branch
   - RT Branch
   - 필요 시 Reviewer Thread

2. 우측 패널은 작업 모드만
   - Plan
   - Reviews
   - Tests
   - Artifacts
   - Trace

3. 우측 패널은 긴 스크롤 적층 금지
   - 한 번에 하나의 주 모드 중심
   - 나머지는 count/badge/summary 정도만 노출

## 구현 요구사항

- 기존 우측 패널 구조를 실제 코드 기준으로 파악할 것
- 최소한의 모드 전환 UI를 추가하거나, 기존 섹션 구조를 재조합해 workspace panel처럼 보이게 만들 것
- 새 페이지/라우팅 금지
- 기존 기능 접근성이 떨어지지 않게 할 것

## 완료 기준

- 우측 패널이 Plan / Reviews / Tests / Artifacts / Trace 중 하나를 중심으로 보여주는 구조가 된다
- harness 정보가 우측 패널에 몰려도 정보 과적재가 줄어든다

6단계가 끝나면 바로 7단계로 진행하라.

---

# 7단계. 주의사항 점검

작업 중 반드시 아래를 점검하라.

## A. 에이전트 문서 오타/라우팅

`architect.md`에 reviewer 이름 오타(`@code-reviwerer`)가 섞여 있는지 확인하고,
실제 제품 라우팅과 충돌하면 최소한의 정리를 포함하라.

## B. 역할 권한

이번 단계에서 완전한 RBAC 엔진까지 못 가더라도,
적어도 architect / reviewer / developer의 의도된 경계가
상태/명령/UI에 드러나도록 할 것

## C. 과한 자동화 금지

- 자동 merge
- 자동 destructive action
- 승인 없는 developer 실행

은 이번 단계에서 하지 말 것

## D. artifact 우선

가능하면 채팅 메시지보다 artifact를 기준으로 다음 단계가 이어지게 할 것

---

# 8단계. 테스트와 검증

작업 후 반드시 아래를 실제로 검증하라.

## A. 타입/빌드 검증

- `cargo check`
- `cargo test`
- `tsc --noEmit`
- `vite build`

실패하면 수정 후 재실행하라.

## B. 기능 시나리오 검증

최소한 아래 시나리오를 점검하라.

1. plan 생성
2. 사용자 승인
3. task 시작
4. developer lane 생성 또는 연결
5. test-report artifact 생성/연결
6. reviewer lane 결과 생성
7. architect decision 기록

## C. 회귀 위험 확인

아래 기존 기능이 깨지지 않았는지 확인하라.

- plan owner assignment
- plan-based follow-up
- 자연어 handoff
- RT branch
- adopt summary
- project onboarding

---

# 9단계. 상태 문서 반영

## 확인할 파일

- `D:\privateProject\tunaFlow\docs\reference\implementationStatus.md`
- `D:\privateProject\tunaFlow\docs\reference\currentStateReview.md`

## 목표

이번 harness 적용 결과를 최소 범위로 반영하라.

반드시 포함할 것:

- 무엇이 실제 구현됐는지
- 무엇이 아직 MVP/1차 수준인지
- 아직 미구현인 핵심 항목
  - RBAC 강화
  - git/worktree
  - snapshot pinning
  - reviewer lane 고도화

---

# 최종 출력 형식

아래 형식으로 최종 보고하라.

### A. 전체 구현 요약
### B. 단계별 변경 내용
### C. 수정 파일 목록
### D. Harness Workflow 변화
### E. 테스트 및 검증 결과
### F. 남은 리스크
### G. 다음 우선순위

중요:
- 각 단계에서 무엇을 실제로 구현했고 무엇을 보류했는지 구분할 것
- 과장하지 말 것
- 실제 구현 범위만 정확히 적을 것

바로 실제 코드 수정까지 순차적으로 진행하라.
