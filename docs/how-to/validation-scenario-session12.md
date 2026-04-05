# 실사용 검증 시나리오 — 세션 12

> 세션 10-11 기능을 실제 프로젝트(tunaInsight 또는 tunaFlow 자체)에서 검증.
> 각 항목 통과 시 ✅ 체크, 실패 시 이슈 기록.

---

## 시나리오 1: 풀사이클 워크플로우 (마커 도구 호출 포함)

**목적**: Chat → Plan 승격 → Approval → Implementation → Review → Verdict → Done 전체 경로 검증

### 준비
1. tunaFlow 앱 실행 (`npm run tauri dev`)
2. 테스트 프로젝트 선택 (tunaInsight 또는 신규)
3. 새 대화 생성

### 스텝

| # | 액션 | 예상 결과 | 체크 |
|---|---|---|---|
| 1 | Architect 프로필로 "사용자 인증 모듈 설계해줘" 전송 | `<!-- tunaflow:plan-proposal -->` 마커가 포함된 응답 | ☐ |
| 2 | PlanProposalCard에서 "승격" 클릭 | Plan 탭에 Plan 생성됨, subtask 표시 | ☐ |
| 3 | Approval Gate에서 엔진 선택 후 "승인" | Implementation Branch 자동 생성, 드로어 열림 | ☐ |
| 4 | Developer 프롬프트 자동 전송 확인 | `<!-- tunaflow:impl-plan -->` 마커 응답 | ☐ |
| 5 | ImplPlanCard에서 "구현 시작" 클릭 | Developer가 코드 구현 시작 | ☐ |
| 6 | **마커 도구 호출 확인**: 응답에 `<!-- tunaflow:tool-request:rawq:... -->` 또는 `<!-- tunaflow:tool-request:docs:... -->` 포함 | 자동 follow-up 전송됨 (tool-request 처리) | ☐ |
| 7 | `<!-- tunaflow:impl-complete -->` 마커 감지 | "Review RT 시작" 버튼 활성화 | ☐ |
| 8 | Review RT 시작 | 2-agent RT 실행, 참가자 상태 표시 | ☐ |
| 9 | `<!-- tunaflow:review-verdict -->` 감지 | ReviewVerdictCard 표시 (pass/fail/conditional) | ☐ |
| 10 | pass → Done 처리 | Plan phase=done, 브랜치 아카이브 | ☐ |

### 실패 시 확인
- Trace 패널에서 ContextPack mode/sections 확인
- 콘솔 에러 확인 (`tauri dev` 터미널)
- DB 직접 조회: `plans`, `plan_events`, `branches` 테이블

---

## 시나리오 2: 스킬팩 + 프로젝트 자동 감지

**목적**: 스킬 4-layer (A/B/C/D) 동작 확인

### 스텝

| # | 액션 | 예상 결과 | 체크 |
|---|---|---|---|
| 1 | 프로젝트 선택 후 Settings > Skills 확인 | Layer A: 프로젝트 스택 자동 감지 추천 스킬 표시 | ☐ |
| 2 | 추천 스킬 수락 | Layer B: activeSkills에 추가, 영속 | ☐ |
| 3 | "React 컴포넌트 최적화해줘" 전송 | Layer C: 프롬프트 키워드 매칭으로 추가 스킬 활성화 (Trace에서 확인) | ☐ |
| 4 | Persona에 recommendedSkills 설정 | Layer D: Persona 전환 시 스킬 자동 활성화 | ☐ |
| 5 | Trace 패널에서 skills 섹션 크기 확인 | 8k 미만, 선택적 주입 동작 | ☐ |

---

## 시나리오 3: code-review-graph (CRG) 통합

**목적**: CRG가 ContextPack에 자동 주입되는지 확인

### 전제
- 프로젝트에 CRG 바이너리 설치 (`code-review-graph`)

### 스텝

| # | 액션 | 예상 결과 | 체크 |
|---|---|---|---|
| 1 | CRG 바이너리 존재 확인 (`which code-review-graph`) | 경로 출력 | ☐ |
| 2 | 대화에서 코드 변경 요청 전송 | agent:completed 시 CRG auto-update 실행 (콘솔 로그) | ☐ |
| 3 | Trace 패널에서 context sections 확인 | `graph` 섹션 포함 (Standard+ 모드) | ☐ |
| 4 | `<!-- tunaflow:tool-request:graph:함수명 -->` 마커 포함 응답 | 자동 follow-up으로 impact 분석 결과 전송 | ☐ |

---

## 시나리오 4: 후속 플랜 인프라

**목적**: `plans.parent_plan_id` (DB v25) 기반 후속 플랜 생성

### 스텝

| # | 액션 | 예상 결과 | 체크 |
|---|---|---|---|
| 1 | Plan 완료 후 같은 대화에서 "후속 작업 계획해줘" 전송 | `<!-- tunaflow:plan-proposal -->` 마커 + `parent_plan_id` 참조 | ☐ |
| 2 | 후속 Plan 승격 | Plans 탭에서 부모-자식 관계 표시 | ☐ |
| 3 | `<!-- tunaflow:tool-request:plans:completed -->` 마커 | 완료된 이전 Plan 정보 자동 follow-up | ☐ |

---

## 시나리오 5: Rework Loop + Doom Loop

**목적**: Review 실패 → Rework → 재리뷰 + 3회 실패 시 에스컬레이션

### 스텝

| # | 액션 | 예상 결과 | 체크 |
|---|---|---|---|
| 1 | Review verdict = fail | phase → rework, rework 프롬프트 자동 전송 | ☐ |
| 2 | Rework 후 다시 Review | 이전 findings 포함된 review context | ☐ |
| 3 | 3회 연속 fail | `doom_loop_escalated` 이벤트 → subtask_review 단계 | ☐ |
| 4 | plan_events 확인 | review_failed × 3 + doom_loop_escalated 기록 | ☐ |

---

## 공통 확인 사항

- [ ] 앱 시작 시 rawq daemon 자동 시작
- [ ] 프로젝트 전환 시 rawq re-index
- [ ] 대화 12+ 메시지 후 compressed memory 자동 생성
- [ ] Trace 패널: tok/s 스파크라인 + context % 프로그레스바
- [ ] RuntimeStatusBar: context mode 배지 + memory 상태
- [ ] 콘솔 warning 0 (개발자 도구 확인)
