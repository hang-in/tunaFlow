# 워크플로우 프롬프트 양식

> Status: draft
> Created: 2026-04-02
> 목적: 워크플로우 각 단계에서 에이전트에게 전달하는 프롬프트의 구조화된 양식 정의
> 사용자와 에이전트 모두의 가독성 + 누락 방지를 위한 통일된 포맷

---

## 원칙

1. **한눈에 파악** — 건수, 대상 파일, 완료 조건이 상단에
2. **체크리스트 형식** — 에이전트가 하나씩 처리하고 누락 방지
3. **파일 경로 우선** — 내용 복사 대신 파일 경로만 (에이전트가 직접 읽음)
4. **완료 조건 명시** — 뭘 해야 끝나는지 명확

---

## 1. Plan 승격 → 문서 작성 요청

**발생:** Chat에서 plan-proposal 승격 시
**수신:** Architect
**목적:** 계획서 + 작업 지시서 파일 작성

```
┌─ 문서 작성 요청 ────────────────────────┐
│                                         │
│ Plan: "{title}"                         │
│                                         │
│ 작성할 문서:                             │
│ □ docs/plans/{slug}.md — 전체 계획서     │
│ □ docs/plans/{slug}-task-01.md — {제목}  │
│ □ docs/plans/{slug}-task-02.md — {제목}  │
│ □ ...                                   │
│                                         │
│ 각 작업 지시서 포함 내용:                 │
│ • 대상 파일 및 경로                      │
│ • 구현 접근법 (단계별)                   │
│ • 의존성 (패키지, 다른 subtask)          │
│ • 리스크 및 주의사항                     │
│ • 완료 기준                             │
│                                         │
│ 완료 조건: 모든 문서 작성 후 알려주세요   │
└─────────────────────────────────────────┘
```

---

## 2. Subtask 수정 요청

**발생:** Subtask stage에서 [수정 요청 + 의견]
**수신:** Architect (슬라이더 Branch)
**목적:** 특정 subtask의 작업 지시서 수정

```
┌─ Subtask 수정 요청 ────────────────────┐
│                                         │
│ Subtask {N}: "{제목}"                   │
│ 파일: docs/plans/{slug}-task-{NN}.md    │
│                                         │
│ 검토 의견:                              │
│ {사용자 의견}                           │
│                                         │
│ 완료 조건: 파일 수정 후 변경 내용 요약   │
└─────────────────────────────────────────┘
```

---

## 3. Subtask 대화 시작

**발생:** Subtask stage에서 [대화하기]
**수신:** Architect (슬라이더 Branch)
**목적:** 특정 subtask에 대한 논의

```
┌─ Subtask 논의 ─────────────────────────┐
│                                         │
│ Subtask {N}: "{제목}"                   │
│ 파일: docs/plans/{slug}-task-{NN}.md    │
│                                         │
│ 이 subtask에 대해 논의합니다.            │
│ 질문하거나 검토 의견을 나눠주세요.       │
└─────────────────────────────────────────┘
```

---

## 4. Plan 문서 반영

**발생:** Subtask stage에서 [Plan 문서 반영]
**수신:** Architect (슬라이더 Branch)
**목적:** 수정된 task 파일들을 메인 plan 문서에 동기화

```
┌─ Plan 문서 반영 ───────────────────────┐
│                                         │
│ Plan: "{title}"                         │
│ 메인 문서: docs/plans/{slug}.md         │
│ 작업 지시서: docs/plans/{slug}-task-*.md│
│                                         │
│ 수정된 작업 지시서의 내용을              │
│ 메인 문서의 subtask 요약에 반영하세요.   │
│                                         │
│ 완료 조건: 메인 문서 업데이트 후         │
│ 변경 내용 요약                          │
└─────────────────────────────────────────┘
```

---

## 5. Dev 시작

**발생:** Approved stage에서 [Dev 시작]
**수신:** Developer (Implementation Branch)
**목적:** 전체 subtask 순차 구현

```
┌─ 구현 시작 ────────────────────────────┐
│                                         │
│ Plan: "{title}"                         │
│                                         │
│ 작업 지시서:                             │
│ □ docs/plans/{slug}-task-01.md          │
│ □ docs/plans/{slug}-task-02.md          │
│ □ ...                                   │
│ □ docs/plans/{slug}-task-{NN}.md        │
│                                         │
│ 규칙:                                   │
│ 1. 각 task 파일을 읽고 순서대로 구현     │
│ 2. 각 완료 시 <!-- subtask-done:N -->   │
│ 3. 전체 완료 시 <!-- impl-complete -->  │
└─────────────────────────────────────────┘
```

---

## 6. Review 요청

**발생:** Dev stage에서 [Review 시작]
**수신:** Reviewer (Review Branch)
**목적:** 구현 결과 검증

```
┌─ Review 요청 ──────────────────────────┐
│                                         │
│ Plan: "{title}"                         │
│                                         │
│ 검증 문서:                              │
│ • Plan: docs/plans/{slug}.md            │
│ • 결과: docs/plans/{slug}-result.md     │
│ • 지시서: docs/plans/{slug}-task-*.md   │
│                                         │
│ Plan과 작업 지시서를 기준으로            │
│ 구현 결과를 검증하세요.                  │
│                                         │
│ 완료 조건:                              │
│ <!-- tunaflow:review-verdict --> 제출   │
└─────────────────────────────────────────┘
```

---

## 7. Re-review 요청 (Rework 후)

**발생:** Rework 완료 후 [Review 시작]
**수신:** Reviewer (새 Review Branch)
**목적:** 이전 findings 수정 확인 + 전체 재검증

```
┌─ Re-review 요청 ───────────────────────┐
│                                         │
│ Plan: "{title}"                         │
│                                         │
│ 검증 문서:                              │
│ • Plan: docs/plans/{slug}.md            │
│ • 결과: docs/plans/{slug}-result.md     │
│ • 지시서: docs/plans/{slug}-task-*.md   │
│                                         │
│ 이전 Review Findings (수정 확인 필요):  │
│ □ 1. {finding 요약}                     │
│ □ 2. {finding 요약}                     │
│ □ ...                                   │
│                                         │
│ 위 사항이 수정되었는지 확인 후           │
│ 전체를 재검증하세요.                     │
│                                         │
│ 완료 조건:                              │
│ <!-- tunaflow:review-verdict --> 제출   │
└─────────────────────────────────────────┘
```

---

## 8. Rework 전달

**발생:** Review fail → [Developer에게 전달 + Rework]
**수신:** Developer (Implementation Branch)
**목적:** Review findings 기반 수정

```
┌─ Rework #{N} ──────────────────────────┐
│                                         │
│ 수정 항목 ({M}건):                      │
│                                         │
│ □ 1. {finding 요약}                     │
│   파일: {관련 파일 경로}                 │
│                                         │
│ □ 2. {finding 요약}                     │
│   파일: {관련 파일 경로}                 │
│                                         │
│ Recommendations:                        │
│ • {recommendation 요약}                 │
│                                         │
│ 완료 조건: 위 항목 모두 해결 후          │
│ <!-- tunaflow:impl-complete --> 포함    │
└─────────────────────────────────────────┘
```

---

## 구현 우선순위

| 순서 | 양식 | 현재 상태 |
|------|------|----------|
| 1 | Rework 전달 (#8) | 구현 필요 — 가독성 문제 가장 심각 |
| 2 | Review / Re-review (#6, #7) | 부분 구현 — findings 포함 개선 |
| 3 | Dev 시작 (#5) | 구현됨 — 양식 적용만 |
| 4 | Subtask 수정/대화 (#2, #3) | 구현됨 — 양식 적용만 |
| 5 | Plan 승격 문서 작성 (#1) | 구현됨 — 양식 적용만 |
| 6 | Plan 문서 반영 (#4) | 구현됨 — 양식 적용만 |
