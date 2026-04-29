# Prompts

> 갱신: 2026-04-22 (docs/reorg-phase-a 작업)
> 대부분의 one-time 실행 프롬프트는 `docs/archive/prompts/` 로 이동.

## 📂 구조

- `docs/prompts/` — 재사용 가능한 템플릿/마스터 핸드오프
- `docs/archive/prompts/by-date/YYYY-MM-DD/` — 세션 날짜별 one-time 프롬프트
- `docs/archive/prompts/one-time/` — 플랜별 one-time 실행 프롬프트 (완료된 plan 대응)

## 🔁 재사용 템플릿

- [handoffMaster](./handoffMaster.md): **tunaFlow는 Tauri 2 + React + Rust + SQLite 기반의 3패널 멀티에이전트 오케스트레이션 IDE다.**

## 🟢 활성 핸드오프

- [windowsBetaHardeningArchitectHandoff_2026-04-29](./windowsBetaHardeningArchitectHandoff_2026-04-29.md) — Plan: `windowsBetaHardeningPlan_2026-04-26`. **Windows 환경 architect 세션용** (사용자 본인 머신). 오늘 작업: A v0.1.4-beta Windows 자산 빌드 + C DB path stale fix(option A) + B startup race 진단 + D watchdog kill compat. INV-1~4, PR + CI watch 필수.

## ✅ 완료된 Developer 핸드오프 (recent)

- [communityFollowupBatchDeveloperHandoff_2026-04-29](./communityFollowupBatchDeveloperHandoff_2026-04-29.md) — **MERGED 5 PR + F1** (2026-04-29). batmania52 #1/#3/#4/#5/#6/#7 + Plan B follow-up F1. PR #215~#220 + #222. baseline FE 381 / Rust 564 (559+5 v48 신규).
- [watchdogAndReviewerReadGuardDeveloperHandoff_2026-04-29](./watchdogAndReviewerReadGuardDeveloperHandoff_2026-04-29.md) — Plan: `watchdogAndReviewerReadGuardPlan_2026-04-29`. **MERGED PR #212 / 8aa944c** (2026-04-29). claude.rs watchdog RAII guard + REVIEWER_TEMPLATE `*-result.md` read 금지.
- [resultMdContaminationFixDeveloperHandoff_2026-04-29](./resultMdContaminationFixDeveloperHandoff_2026-04-29.md) — Plan: `resultMdContaminationFixPlan_2026-04-29`. **MERGED PR #211 / bc34b53** (2026-04-29). reviewer ContextPack 의 result.md 자동 첨부 제거 + truncation/self-include 가드 + i18n 정리. FE 381 / Rust 559 통과.

## 📦 Archive — one-time 프롬프트 (23개)

주로 완료된 plan 대응 실행 프롬프트 + 세션 핸드오프 문서.
[docs/archive/prompts/one-time/](../archive/prompts/one-time/)

## 📅 Archive — by-date 프롬프트 (4개 폴더)

- [2026-03-28](../archive/prompts/by-date/2026-03-28/): 6개 문서
- [2026-03-29](../archive/prompts/by-date/2026-03-29/): 24개 문서
- [2026-03-30](../archive/prompts/by-date/2026-03-30/): 65개 문서
- [2026-03-31](../archive/prompts/by-date/2026-03-31/): 4개 문서

