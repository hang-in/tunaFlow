---
title: i18n Track 1 — A2-D Settings subpanels
created_at: 2026-04-24
parallel_track: 1 of 3
ssot: docs/plans/i18nCompletionPlan_2026-04-24.md §"Slice A2-D"
---

# Developer Session Prompt — A2-D Settings subpanels

새 Claude Code 세션에 아래 블록 전체를 붙여넣는다.

```
[작업] i18n PR A2-D — Settings subpanels 전환

[SSOT]
- docs/plans/i18nCompletionPlan_2026-04-24.md §"Slice A2-D" 먼저 읽기
- docs/plans/i18nPlan.md (원 plan, 3계층 키 컨벤션)
- 현재 main 이 기준. 2026-04-24 세션에서 PR #161~#169 머지 완료 상태 (A1~A3 모두 main).

[브랜치]
main 에서 feat/i18n-pr-a2d-settings 생성.

[범위 — 9 파일 / ~156 Korean lines / ~120 keys 예상]
대상:
- src/components/tunaflow/SettingsPanel.tsx (2 lines, shell)
- src/components/tunaflow/settings/HelpSection.tsx (28 lines)
- src/components/tunaflow/settings/MobileSection.tsx (11 lines)
- src/components/tunaflow/settings/IdentityAnalysisSettings.tsx (5 lines, 대부분 주석)
- src/components/tunaflow/settings/ProfileSection.tsx (21 lines)
- src/components/tunaflow/settings/RuntimeSection.tsx (30 lines)
- src/components/tunaflow/settings/ConventionsSection.tsx (32 lines)
- src/components/tunaflow/settings/PersonasSection.tsx (1 line — desc only)
- src/components/tunaflow/settings/AgentsSection.tsx (23 lines)
- src/components/tunaflow/settings/WorldviewSettings.tsx (3 lines — default template 상수)

실측은 `grep -c "[가-힣]" <파일>` 또는 Grep 도구 활용.

[Namespace] settings.* 확장 (기존 profile/worldview/identity 는 이미 존재):
- settings.help.*
- settings.mobile.*
- settings.profile.* 확장 (section title, save button, saved msg, field hints)
- settings.runtime.* (신규)
- settings.conventions.* (신규)
- settings.personas.* (신규)
- settings.agents.* (신규)
- settings.worldview.* 확장 (default_template 상수)
- settings.identity.* (기존 유지)

기존 ko/settings.json 참고 — profile/worldview/identity 섹션 구조 유지.

[패턴 준수 — 기존 A2-C Sidebar PR #161 / A2-G #167 참고]
1. `useTranslation("settings")` 훅 추가
2. 하드코딩 한국어 → `t("<section>.<key>")` 교체
3. interpolation: `t("...", { count, error, name })`
4. 대형 멀티라인 template (예: WorldviewSettings 의 DEFAULT_WORLDVIEW_TEMPLATE) 은 JSON 에 `\n` 포함 단일 키로
5. ko 가 SSOT. en 은 ko 기반 번역
6. src/locales/index.ts / src/types/i18next.d.ts 변경 없음 — settings namespace 이미 등록됨

[INV]
- INV-1 (agent 시스템 프롬프트 영어 고정). 본 슬라이스는 UI 라벨만.
- INV-5 (namespace.section.action 3계층 키).
- INV-7 (다른 Track 과 파일 경로 겹침 없음 — settings/* 는 본 Track 전용).

[검증]
- npx tsc --noEmit
- npx vitest run (기대: 322 passed 유지)
- npx vite build (번들 성공)
- 수동: Settings 각 탭 ko ↔ en 전환, 컬럼 폭 깨짐 없음, interpolation 정상

[커밋 규약]
- feat(i18n): 접두어
- Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[PR]
제목: feat(i18n): PR A2-D — Settings subpanels (~120 keys)
설명에 포함:
- 파일별 변환 키 수 표
- 수동 확인 checklist
- 후속: A3-ext (lib/ + stores/ 서비스 계층)

[주의 — 다른 Track 과 충돌 방지]
- src/locales/{ko,en}/settings.json 은 본 Track 전용. 다른 Track 이 추가하지 않음
- src/locales/{ko,en}/common.json 에 키 추가 필요하면 3계층 엄수
- 본 Track 은 src/lib/* / src-tauri/* 건드리지 않음
```

## 참고 — 이 Track 이 아닌 것

- Insight/Identity/Workflow UI (완결됨)
- Chat/Branch/Input/Common UI (Track 3)
- Context panel tabs (Track 2)
- lib/stores services (A3-ext, 후속)
