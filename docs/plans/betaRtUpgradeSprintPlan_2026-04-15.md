---
title: 베타 진입 직전 RT 고도화 Sprint (Quick/Deep 2트랙)
status: planned
created_at: 2026-04-15
related:
  - docs/plans/roundtableBlindVerifierPhasePlan_2026-03-30.md
  - docs/ideas/rtAlgorithmEnhancementIdeas.md
  - docs/reference/geminiCriticReview_2026-04-15.md
  - docs/reference/knownIssues_2026-04-15.md
  - docs/plans/betaReleaseReadinessPlan.md
---

# 베타 진입 직전 RT 고도화 Sprint

> 새로 짜는 게 아니라 **기존 idea/plan을 베타 진입 sprint로 묶는 포인터 문서**.
> 작업 시작은 `fix/indexing-pipeline-recovery` 머지 후, secall 마무리 후.

---

## 배경

Gemini 외부 리뷰(2026-04-15)에서 Reviewer 허상 지적. README의 "2-agent 교차 검증" 주장과 코드 불일치 확인 (\`reviewWorkflow.ts:58\` Reviewer 실행 금지). 그 외 P0/P1 다수가 이미 발견됨.

기존 RT 고도화 계획이 충분히 있고, **"2트랙 진입(Quick/Deep)"**과 결합하면 README를 솔직 + 강한 약속으로 재정의 가능. 베타 직전 최소 sprint로 묶음.

---

## 베타 진입 직전 범위 (this sprint)

다른 phase 작업은 베타 후 plan으로 분리.

### S1. 구조화 Verdict 루브릭 (P0)

- **출처**: \`docs/ideas/rtAlgorithmEnhancementIdeas.md\` §3 P0
- **변경 위치**: \`src/lib/planProposalParser.ts\` (마커 schema에 \`scores:\` 필드 추가)
- **출력 예시**:
  \`\`\`
  scores:
    plan_coverage: 4/5
    code_quality: 3/5
    test_coverage: 2/5
    convention: 5/5
  total: 14/20
  threshold: 16/20
  \`\`\`
- **UI**: \`ReviewVerdictCard\`에 차원별 점수 시각화 (작은 바 차트 또는 숫자 그리드)
- **자동 판정**: 총점 기반 pass/fail/conditional

### S2. Agent-as-Judge 테스트 자동 주입 (P2 → 베타 직전 격상)

- **출처**: \`docs/ideas/rtAlgorithmEnhancementIdeas.md\` §3 P2 Agent-as-Judge
- **변경 위치**: \`src/lib/workflow/reviewWorkflow.ts\` \`startReviewRT\` 호출 직전에 \`run_project_tests\` 실행 → 결과를 \`testOutput\` 파라미터로 전달
- **이미 있는 것**: \`startReviewRT\`의 \`testOutput\` 파라미터 + 프롬프트 분기 (\`reviewWorkflow.ts:66\`)
- **새로 할 것**: 자동 실행 트리거 + 실패 시 fallback (테스트 설정 없는 프로젝트는 skip)
- **이게 핵심**: Reviewer가 Developer 자기 보고가 아닌 **실제 빌드/테스트 결과**를 직접 받음. README의 "교차 검증" 약속을 진짜로 만드는 핵심.

### S3. Quick/Deep 2트랙 진입 UI

- **출처**: 사용자 구두 (이전 세션 + 이번 세션)
- **변경 위치**: Plan 생성 모달 또는 워크플로우 진입 시점에 트랙 선택 토글
- **Quick (default)**: 단일 Reviewer 정적 검토 (현재 코드 유지) — 30초 내 sanity check
- **Deep**: Multi-engine RT + S1 루브릭 + S2 Agent-as-Judge — 정합성 + 실행 검증
- **저장**: plan 메타에 \`review_track: 'quick' | 'deep'\` 필드 추가

### S4. README 전면 재작성

- **출처**: \`docs/reference/geminiCriticReview_2026-04-15.md\` §5 (Reviewer 허상)
- **현재**: "2-agent 교차 검증으로 self-validation 한계 극복"
- **새 카피 초안**:
  > tunaFlow는 두 검증 트랙을 제공합니다.
  >
  > **Quick** — 단일 Reviewer가 코드와 Developer 보고를 정적 검토. 30초 내 sanity check.
  >
  > **Deep** — Multi-engine RT가 자동 테스트 결과를 직접 받아 구조화 루브릭 기반으로 채점. Developer의 자기 보고가 아닌 실제 실행 결과로 판정. 이종 모델 verdict + 투표(예정)로 self-validation 함정을 우회합니다.
- **언어**: ko / en / ja / zh 모두 동일 톤 (사용자 secall 작업 패턴 참조)
- **삭제**: 41.8%/36.9%/21.3% 같은 가짜 비율 (이미 베타 plan §4에서 삭제 완료)

---

## 베타 후 phase로 분리

기존 idea 문서의 P1/P2/P3 잔여:

| 항목 | 출처 | 분리 phase |
|------|------|-----------|
| 투표 메커니즘 (Self-Consistency 집계) | \`rtAlgorithmEnhancementIdeas.md\` P1 | Phase 2 |
| MoA Synthesizer (structured reducer) | 동 P1 | Phase 2 |
| Self-Refine 사전 검증 | 동 P2 | Phase 3 |
| Adaptive Stopping | 동 P3 | Phase 4 |
| Blind Verifier 명시화 | \`roundtableBlindVerifierPhasePlan\` | Phase 2 (투표와 같이 — sycophancy 방지가 투표와 시너지) |

---

## Sprint 작업량 추정

| Step | 작업 | 추정 |
|------|------|------|
| S1 | 루브릭 파서 + UI | 1~1.5일 |
| S2 | 테스트 자동 주입 + fallback | 0.5~1일 |
| S3 | 2트랙 UI 진입 + 메타 저장 | 0.5일 |
| S4 | README 4개 언어 재작성 | 0.5~1일 |

**합 ~3일**. 인덱싱 P0(\`fix/indexing-pipeline-recovery\`) 끝난 직후 진행.

---

## 의존 관계 / 순서

\`\`\`
secall 마무리 (사용자)
   ↓
fix/indexing-pipeline-recovery — Vector 복구 (인덱싱 I1)
   ↓
feat/rt-upgrade-deep-track — S1 + S2 + S3
   ↓
docs/readme-rewrite-honest — S4
   ↓
v0.1.0-beta.1 태그
\`\`\`

---

## 관련 문서 포인터

- 원본 RT 알고리즘 분석: \`docs/ideas/rtAlgorithmEnhancementIdeas.md\` (Opus 작성, 6개 논문 검토 + Codex 리뷰 반영)
- Blind Verifier 설계: \`docs/plans/roundtableBlindVerifierPhasePlan_2026-03-30.md\`
- 외부 리뷰 분석: \`docs/reference/geminiCriticReview_2026-04-15.md\`
- 세션 이슈 체크리스트: \`docs/reference/knownIssues_2026-04-15.md\`
- 베타 배포 plan: \`docs/plans/betaReleaseReadinessPlan.md\`
