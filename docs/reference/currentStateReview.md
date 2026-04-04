# tunaFlow 현재 상태 리뷰

> **status: archived** — 세션 1 직전(2026-03-26) 시점의 스냅샷. 이후 10 세션의 대규모 변경으로 다수의 내용이 더 이상 유효하지 않음.
> 현행 상태는 CLAUDE.md §5, implementationStatus.md 참조.

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 08:24 KST
- 대상 프로젝트: `D:\privateProject\tunaFlow`
- 기준: 현재 코드, 현재 docs, 현재 테스트/CI 상태

---

## 요약

tunaFlow는 초기의 Tauri 기반 멀티엔진 채팅 도구 수준을 넘어서,
지금은 **계획, 브랜치, 라운드테이블, 추적, handoff, 테스트 체계**를 갖춘
작업형 멀티에이전트 IDE에 가까운 상태다.

실제 코드 기준으로 확인된 상태:

- plan backend + plan UI
- branch-scoped plan 생성 및 canonical conversation 처리
- OTel 스타일 span metadata를 포함한 trace 기록
- roundtable progress 이벤트
- roundtable brief 자동 저장
- Recent Agent Findings / Recent Artifacts ContextPack 주입
- plan ownership 메타데이터
- message / artifact 기준 follow-up handoff UX
- Rust + frontend 테스트 기반
- CI 기반 자동 검증

즉, 큰 기반 작업은 끝난 상태이고,
지금 남은 것은 구조 재설계가 아니라 **완성도와 polish**에 가깝다.

---

## 주요 발견사항

### 1. Plan ownership은 부분 구현 상태다

`plan_subtasks.owner_agent`와 `plan_subtasks.last_updated_by`는 스키마와 모델에 존재한다.
하지만 실제 일반 사용 흐름에서 확실히 갱신되는 것은 현재 `last_updated_by` 쪽이다.

확인 근거:

- `src-tauri/src/db/schema.rs`
- `src-tauri/src/db/models.rs`
- `src-tauri/src/commands/plans.rs`
- `src/components/tunaflow/context-panel/PlansPanel.tsx`

현재 상태:

- `last_updated_by`: subtask 상태 변경 시 기록됨
- `owner_agent`: UI 표시 필드는 있으나, 일반 사용 경로에서 명시적으로 설정하는 흐름은 아직 없음

영향:

- “누가 마지막으로 만졌는가”는 남지만
- “누가 맡고 있는가”는 아직 실사용 수준으로 완성되지 않음

심각도: 중간

### 2. Follow-up UX는 plan을 1급 source로 다루지 않는다

현재 follow-up handoff는 assistant message와 artifact에서는 동작한다.
하지만 plan/subtask에서 직접 다른 agent로 넘기는 UX는 아직 없다.

확인 근거:

- `src/stores/chatStore.ts`
- `src/components/tunaflow/MessageItem.tsx`
- `src/components/tunaflow/context-panel/ArtifactsPanel.tsx`
- `src/components/tunaflow/context-panel/PlansPanel.tsx`

영향:

- 현재 협업 흐름은 usable하다
- 하지만 plan 중심 작업 분배를 UI에서 직접 이어주는 경험은 아직 부족하다

심각도: 중간

### 3. Follow-up 메뉴는 기능은 되지만 UX polish는 남아 있다

메시지 기준 follow-up 메뉴는 토글 방식으로 동작한다.
현재 기준으로는 외부 클릭 시 닫힘 처리까지는 확인되지 않았다.

확인 근거:

- `src/components/tunaflow/MessageItem.tsx`

영향:

- 기능적 문제는 아님
- 반복 사용 시 UX 마찰 가능

심각도: 낮음

### 4. Roundtable participant span의 duration은 아직 placeholder 수준이다

roundtable root span은 실측 duration을 기록한다.
하지만 participant span은 아직 `duration_ms = 0`으로 들어간다.

확인 근거:

- `src-tauri/src/commands/roundtable.rs`
- `src-tauri/src/commands/roundtable_helpers/persist.rs`

영향:

- tracing 구조는 충분히 의미 있음
- participant 단위 latency 분석은 아직 불완전

심각도: 낮음

---

## 확인된 강점

### 구조

- `ContextPanel`이 서브패널로 분리됨
- agents/roundtable 로직이 helper 모듈로 분리됨
- `context_queries`, `agents_helpers`, `roundtable_helpers` 추출이 의미 있게 되어 있음
- frontend API wrapper가 생겨 `invoke(...)`가 덜 흩어짐

### Planning / Collaboration

- plans, plan_subtasks가 실제 저장됨
- branch-scoped plan이 canonical conversation 처리와 함께 동작함
- roundtable 결과가 `roundtable_brief` memo로 재사용 가능해짐
- Claude ContextPack에는 현재 다음 정보가 들어감
  - plan
  - recent findings
  - recent artifacts
  - rawq
  - context summary

### Tracing / Evaluation

- trace_log가 OTel 스타일 메타데이터를 기록함
- roundtable root/participant span이 parent-child 구조를 가짐
- evaluation 관련 테이블과 command가 존재함

### 테스트 / CI

현재 확인된 상태:

- Rust 테스트: 40개
- Frontend 테스트: 13개
- 총 테스트: 53개
- coverage 실행 가능
- GitHub Actions CI 존재

즉, E2E가 없더라도 지금은 “회귀를 막는 최소 경계”가 실제로 생겨 있는 상태다.

---

## 현재 리스크 수준

### 지금 당장 미뤄도 되는 항목

- follow-up 메뉴 UX polish
- participant trace duration 정밀화
- E2E 미도입

현재 테스트/CI 상태를 보면 이 셋은 즉시 해결 대상은 아니다.

### 다음으로 손대면 좋은 항목

- `owner_agent`를 실제로 설정하는 경로 추가
- plan/subtask 기준 follow-up UX 추가
- artifact handoff selection 기준을 최신순 외에 조금 더 다듬기

---

## 권장 다음 단계

### 1. Plan ownership 완성

가장 작은 단위로:

- `owner_agent`를 실제로 설정할 수 있는 command 또는 UI 경로 추가
- PlansPanel에서 최소한의 ownership assign/update 흐름 제공

### 2. Plan 기반 follow-up 추가

예:

- subtask → Claude refine
- subtask → Codex implement
- subtask → Gemini critique

이건 현재 Phase 1~3의 협업 모델을 가장 자연스럽게 완성하는 후속 작업이다.

### 3. Trace 품질 보강

새 tracing 체계를 도입할 필요는 없다.
다만 아래 정도는 보강 가치가 있다.

- participant duration 실측
- cancelled 상태를 span에 더 정확히 반영

### 4. E2E는 계속 보류 가능

지금의 E2E 보류 판단은 타당하다.
현재는 unit / integration / frontend / CI만으로도 꽤 강한 보호막이 형성돼 있다.

실제 flaky한 사용자 흐름이 확인될 때 smoke test 몇 개만 추가하는 방식이 더 적절하다.

---

## 최종 판정

tunaFlow는 현재 **좋은 상태**다.

지금 프로젝트는:

- 더 명확한 모듈 구조
- branch / plan / context 통합
- 실제 협업 handoff 매개체
- 실제 trace 메타데이터
- 실제 테스트와 CI

를 갖춘 상태다.

남은 것은 기반 공사가 아니라,
협업 UX와 ownership 완성도를 올리는 작업이다.

따라서 현재 판단은 다음과 같다.

- 협업 강화 기반 작업: 완료
- 제품 완성도 작업: 다음 단계
- 대규모 프레임워크 전환이나 구조 재설계: 불필요
