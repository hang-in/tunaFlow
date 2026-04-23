---
title: User Worldview Injection — Identity 축 (Interface/Continuity 축은 projectIdentityAnalysisPlan 으로 이관)
status: partial (subtask-01 merged, subtask-02~04 superseded)
priority: P1
created_at: 2026-04-22
updated_at: 2026-04-23
related:
  - src-tauri/src/commands/worldview.rs                                       # Rust helper (subtask-01 구현물)
  - src-tauri/src/commands/agents_helpers/send_common/prompt_assembly.rs      # ContextPack worldview 주입 위치
  - src/components/tunaflow/settings/WorldviewSettings.tsx                    # Settings UI
  - docs/plans/projectIdentityAnalysisPlan.md                                 # subtask-02~04 를 대체하는 신규 plan
  - docs/reference/tokenPolicyReference.md                                    # 품질 우선 철학
superseded_subtasks:
  - subtask-02 (preference_events / preference_snapshots) → projectIdentityAnalysisPlan (artifacts 재사용)
  - subtask-03 (stance-conflict rule + model verify + modal) → 설계 폐기 (LLM 자연 능력에 맡김)
  - subtask-04 (background job skeleton) → projectIdentityAnalysisPlan subtask-02 (metaAgent trigger) 로 이관
---

# User Worldview Injection (축소판)

> **현재 범위**: 사용자가 직접 작성하는 철학 stance (`user_worldview.md`) 를 ContextPack 에 주입하는 **Identity 축** 만.
>
> 2026-04-22 초안에서 3축 번들 (Identity / Interface / Continuity) 로 설계됐으나, 2026-04-23 사용자 리마인드 + Codex 자문 결과:
> - **Interface 축 (stance-conflict)** 은 agent 에게 LLM 자연 능력으로 맡기는 게 옳음 → 별도 장치 불필요
> - **Continuity 축 (preference_timeline)** 은 Artifacts 의 원 설계 의도 (사용자 결정 + agent finding 누적 → 비정기 분석 → 정체성) 에 이미 포함 → `projectIdentityAnalysisPlan` 으로 이관
> - 결과: 본 plan 은 **Identity 축 (worldview 주입)** 만 유지, 나머지 축은 superseded
>
> Subtask-01 은 **PR #144 로 머지 완료** (2026-04-22 23:38 UTC). 본 plan 은 운영 참고용 기록.

---

## 적용 범위

본 plan 의 모든 로직 (worldview 주입) 은 **sdk-session 경로 (Branch chat) 한정**. RT (`-p` one-shot) 는 매 turn full ContextPack 을 재주입하는 것이 정상 동작이며 본 plan 대상 아님.

---

## 구현 완료 요약 (subtask-01)

1. **`user_worldview.md` 파일 체계**
   - Global: `~/.tunaflow/user_worldview.md`
   - Project override: `<project_path>/.tunaflow/user_worldview.md` (있으면 global 무시)

2. **ContextPack 주입**
   - `prompt_assembly.rs::assemble_prompt()` 가 `identity_fragment` 바로 앞에 `worldview_fragment` 삽입
   - 순서: `project → platform → agent-role → ... → worldview → identity → skills → recent_context → ... → user_prompt`

3. **Settings UI**
   - `src/components/settings/WorldviewSettings.tsx` — 편집기 + "기본 문구 로드" 버튼
   - 토큰 제한: Token Policy (`docs/reference/tokenPolicyReference.md`) 기준 — AGENTS.md 수준 **1,500~3,000 tokens 허용**. 2026-04-22 초안의 "500 tokens 상한" 은 토큰 절감 편향이었고, Token Policy 확정으로 상향 조정.

4. **토글**
   - Settings 에 "Worldview 주입 활성화" 체크박스. 기본 ON. 끄면 fragment 완전 생략.

---

## Invariants (남은 INV)

- **[INV-1]** ContextPack 조립 시 `user_worldview` fragment 는 반드시 `identity_fragment` **바로 앞** 에 위치한다. project / platform / agent-role 등 identity 앞 섹션들은 그대로 유지 — worldview 가 이들을 앞지르지 않는다. **이유**: agent 가 자신의 역할 (identity) 을 정의하기 전에 사용자 OS (worldview) 를 먼저 받아야 한다는 원 설계 의도. **검증**: `prompt_assembly.rs::assemble_prompt` 단위 테스트 — sections 순서에서 `"worldview"` 가 `"identity"` 바로 앞 인덱스.

- **[INV-2]** Worldview 콘텐츠는 사용자가 **자유롭게 작성** 한다. tunaFlow 는 기본 템플릿 외에는 내용에 개입하지 않는다 (placeholder 제공, stance 예시 제공 금지). **이유**: 사용자 철학에 대한 bias 주입 방지. **검증**: 기본 템플릿이 section 헤더만 포함하고 본문은 비어 있는지 확인.

- **[INV-3]** Worldview 는 **사용자 대면 stance 문서**이며, 분석 산출물 (`identity_summary`, `projectIdentityAnalysisPlan` 범위) 과 **별도의 ContextPack 섹션**으로 공존한다. 두 문서 충돌 시 worldview 가 우선 — 프롬프트 주입 시 "User-authored worldview takes priority over analysis-derived identity on conflict." 한 줄 명시. **이유**: 자동 분석 결과가 사용자 의식적 선언을 덮지 않도록. **검증**: `projectIdentityAnalysisPlan` 의 ContextPack selector 확장 후 integration test — 두 섹션이 각각 `"worldview"` / `"project_identity"` 로 분리 렌더.

(기존 INV-4~8 은 superseded subtask 소속이라 제거)

---

## Superseded subtasks

아래 3개는 **본 plan 에서 구현하지 않음**. 파일은 archive 또는 deprecated 표기 후 git history 로 보존:

### subtask-02: `preference_events` + `preference_snapshots` (migration v46)
- **Supersede 이유**: `artifacts` 테이블 재사용이 사용자 원 설계 의도. 별도 테이블 신설은 중복 memory 계층.
- **이관처**: `projectIdentityAnalysisPlan-task-01.md` (artifact 자동 생성 6 타입)

### subtask-03: Stance-conflict (rule + model verify + modal)
- **Supersede 이유**: 사용자 요청 vs 과거 선호 판정은 rule/model/modal 엔진보다 **LLM 자연 능력 + ContextPack 주입** 로 충분. Modal 은 UX 침습 과다. 필요 시 agent 가 응답 안에서 자연스럽게 현실 감각 제공.
- **이관처**: 없음 (폐기). 필요 시 `docs/ideas/stanceConflictStrongIdea.md` 로 idea 만 보존.

### subtask-04: Background insight worker skeleton
- **Supersede 이유**: metaAgent 구현과 묶어 진행하는 편이 자연스러움. 스켈레톤 단독 머지는 dead feature 리스크.
- **이관처**: `projectIdentityAnalysisPlan-task-02.md` (metaAgent trigger + analysis job)

---

## Rationale (간략)

### 축소의 근거

2026-04-22 초안은 Identity / Interface / Continuity 3축을 한 plan 에 묶었다. 이는 Gemini 의 "거부권" insight 와 검토 세션의 피드백을 합친 결과였으나, 2026-04-23 사용자 리마인드로 다음이 명확해졌다:

1. **tunaFlow 원 취지 = 2인 3각 협업 도구** (사용자 확인)
2. **"불편한 동반자" 는 tunaFlow 정체성 맞음** — 단 구현 수단은 rule + modal 이 아니라 **LLM 에게 context 를 주면 자연스럽게** 수행
3. **Artifacts 는 이미 사용자 결정 + agent finding 누적 → 분석 → 정체성 추출** 이라는 원 의도를 가진 인프라 — 별도 preference 테이블 신설 중복

결과적으로 본 plan 은 **사용자 수동 stance (worldview)** 에만 집중하고, 자동 분석 / 변곡점 timeline / 정체성 추출은 `projectIdentityAnalysisPlan` 이 담당.

### 토큰 정책 조정

초안의 worldview 500 tokens 상한은 `docs/reference/tokenPolicyReference.md` 의 "품질 우선" 원칙과 충돌. **1,500~3,000 tokens 로 상향**. Subtask-01 이 이미 머지된 이후의 후속 조정이며, 기존 짧은 worldview 는 그대로 작동 + 사용자가 길게 작성해도 ContextPack 에 full 주입.

---

## 관련 문서

- 구현 PR: #144 (merged 2026-04-22)
- Token Policy: `docs/reference/tokenPolicyReference.md`
- 후속 대체 plan: `docs/plans/projectIdentityAnalysisPlan.md`
- Archive 대상: `userWorldviewInjectionPlan-task-02.md`, `-task-03.md`, `-task-04.md` (Developer 가 git mv 로 `docs/archive/plans/superseded/` 이동)
