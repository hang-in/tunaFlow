---
title: Token Policy
updated_at: 2026-04-23
canonical: true
status: active
owner: tunaFlow-core
---

# Token Policy

tunaFlow 는 토큰 절감 도구가 아닙니다. 목표는 품질 좋은 결과물이며, 토큰은 이를 뒷받침하는 자원입니다. 단 **중복 / stale / doubling** 은 품질 저하 요인이므로 회피 대상입니다.

## 허용 (품질 우선 — 길이 제약 걸지 않음)

- `user_worldview.md`, `AGENTS.md`, `persona` 등 context 문서: **1,500~3,000 tokens** 허용
- identity_summary / 프로젝트 정체성 분석 산출물: 동일 수준
- recent_context 주입은 필요한 본문 길이를 자르지 않음
- Deep Review / 분석 LLM 출력은 구체성 우선, 짧게 강제하지 않음

## 회피 (품질 저하 = 토큰 낭비)

1. **중복 재주입** — 이미 claude 세션 buffer 에 있는 내용을 tunaFlow 가 ContextPack 으로 또 주입. Session Continuity Fix Plan INV-1 참조.
2. **Stale context** — 1년 전 대화 요약이 현재 요청에 그대로 붙음. compression + decay 원칙 유지.
3. **Doubling** — 같은 정보가 worldview + preference + persona 여러 곳에 복사되는 경우.
4. **불필요한 tool 호출 반복** — rawq 검색 결과가 이미 context 에 충분한데 agent 가 같은 쿼리를 되풀이.

## 실전 기준

- 새 기능 설계 시 "이거 길어도 되나?" 고민할 때: **품질에 기여하면 OK**. 중복인지만 점검.
- "토큰 아끼려고 자르자" 는 기각. "정보를 중복 없이 정제하자" 는 허용.

## 관련 문서

- `docs/plans/sessionContinuityFixPlan.md` — 중복 재주입 방지 INV
- `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md` — compressed memory decay
- `docs/reference/coding-convention.md` — 전체 코드 관례
