---
title: 마크다운 single newline 무시 — react-markdown remark-breaks 추가
status: ready-to-implement
priority: P1 (Beta 사용자 보고 — 채팅/로그 newline collapse 가시화)
created_at: 2026-04-26
related:
  - src/components/tunaflow/MessageItem.tsx              # REMARK_PLUGINS 정의 (line 56)
  - src/components/tunaflow/MetaFloatingChat.tsx         # 인라인 remarkPlugins (line 692)
  - src/components/tunaflow/ProjectOnboardingModal.tsx   # remarkPlugins (line 354)
  - src/components/tunaflow/RoundtableView.tsx           # ReactMarkdown 사용
  - package.json                                         # remark-breaks 의존성
canonical: true
owners:
  - architect (본 plan 작성)
  - developer (구현)
---

# 증상 (사용자 보고, 2026-04-26)

> "입력창이 마크다운 형식으로 들어가야 뉴라인으로 인식하는건지, 입력 후 보이는 채팅이 마크다운만 제대로 표현하는건지 모르겠는데 예를 들어 로그 여러줄을 넣으면 죄다 한줄로 표현되고 있고, 뉴라인도 무시되어 한줄로 표현되는게 좀 거슬리네요."

---

# 진단 (Architect 사전 분석)

- 입력 textarea 측은 `\n` 정상 보존 (DB 저장된 메시지에도 `\n` 그대로)
- **표시 측** `react-markdown` + `remark-gfm` 만 사용 — CommonMark spec 상 paragraph 안 single newline 은 공백으로 collapse 되는 게 정상 동작
- 채팅/로그 컨텍스트에서는 newline 의미가 살아야 자연스러움
- **해결**: `remark-breaks` 플러그인 추가 → single newline → `<br>` 변환. 마크다운 의도 (paragraph break / list / code block / table) 깨지지 않음

현재 코드 위치 (실측):
- `MessageItem.tsx:56` — `const REMARK_PLUGINS: any[] = [[remarkGfm, { singleTilde: false }]];`
- `MetaFloatingChat.tsx:692` — `<ReactMarkdown remarkPlugins={[[remarkGfm, { singleTilde: false }]]}>`
- `ProjectOnboardingModal.tsx:354` — `remarkPlugins={REMARK_PLUGINS}` (자체 정의 가능성, 본문 확인 필요)
- `RoundtableView.tsx:6` — `import ReactMarkdown` (사용 위치 추가 grep)

---

# Fix Scope

## Layer A — remark-breaks 도입

### A1. 의존성 추가
- `package.json` 에 `remark-breaks` (latest stable) 추가
- `npm install` 후 lockfile 갱신

### A2. SSOT 모듈 추출 + 일관 적용
- `src/lib/markdownPlugins.ts` 신규 — 단일 SSOT export
  ```ts
  import remarkGfm from "remark-gfm";
  import remarkBreaks from "remark-breaks";

  /** 채팅/로그 친화 마크다운 플러그인 셋. single newline → <br>. */
  export const REMARK_PLUGINS = [
    [remarkGfm, { singleTilde: false }],
    remarkBreaks,
  ] as const;
  ```
- 4 위치를 모두 새 SSOT 로 교체:
  - `MessageItem.tsx` — 자체 정의 제거, import 로 변경
  - `MetaFloatingChat.tsx` — 인라인 array 제거, import 사용
  - `ProjectOnboardingModal.tsx` — import 정리
  - `RoundtableView.tsx` — `<ReactMarkdown>` 호출부에 적용

### A3. 일관 적용 회귀 가드 (선택)
- ESLint custom rule 또는 grep 기반 sanity test (`tests/markdown-plugin-consistency.test.ts`) — `<ReactMarkdown>` 사용처가 모두 `REMARK_PLUGINS` 를 통해 가는지 검증

## Layer B — 회귀 방지 테스트

### B1. 단일 newline 분리 렌더링 테스트
- `src/components/tunaflow/MessageItem.test.tsx` (또는 동등) 에:
  - "여러 줄 로그" 입력 → 결과 DOM 에 `<br>` 분리 또는 line별 텍스트 노드 확인
  - 기존 `\n\n` paragraph break 동작 보존
  - list (`- a\n- b`) / code block (` ```...``` `) / table 정상 렌더 (회귀 가드)

### B2. (선택) snapshot 테스트
- 대표 메시지 패턴 1~2개로 snapshot 추가

---

# Invariants

- INV-1: 입력 메시지 안 single `\n` 이 표시 시 visible line break 으로 나타남
- INV-2: 기존 paragraph break (`\n\n`) 동작 보존
- INV-3: list / code block / table / heading / strikethrough 등 GFM 동작 보존
- INV-4: 4 위치 (MessageItem / MetaFloatingChat / ProjectOnboardingModal / RoundtableView) 모두 동일 SSOT 사용 — 향후 표시 차이 발생 차단
- INV-5: 채팅 메시지 외 (예: tool steps, plan document modal) 사용처도 일관 (확장 시 동일 SSOT 사용)

---

# 검증

## 자동
- `npx tsc --noEmit`
- `npx vitest run` (B1 신규 테스트 포함)
- `cd src-tauri && cargo check` (FE only 변경이라 영향 없음, sanity)

## 수동 smoke
1. 채팅에 여러 줄 로그 paste → 줄 단위 표시 확인
2. 마크다운 의도 (heading / list / code block / table / strikethrough) paste → 정상 렌더 (regression 가드)
3. MetaFloatingChat 동일 확인
4. ProjectOnboardingModal preview 동일 확인
5. RoundtableView 메시지 표시 동일 확인

---

# Developer 핸드오프 프롬프트

`docs/plans/markdownSingleNewlineBreaksPlan_2026-04-26.md` 의 Layer A/B 따라 작업.

**작업 절차**

1. **A1 의존성** — `npm install remark-breaks` (또는 package.json 수동 추가 후 npm install)
2. **A2 SSOT** — `src/lib/markdownPlugins.ts` 신규 작성, 4 위치 (MessageItem / MetaFloatingChat / ProjectOnboardingModal / RoundtableView) 를 import 로 교체. **추가 사용처 grep 으로 발견 시 같이 교체.**
3. **A3 (선택)** — 일관 적용 sanity test
4. **B1 회귀 테스트** — single newline 분리 + 기존 마크다운 의도 보존 테스트
5. 검증 (tsc / vitest / cargo check)
6. 커밋 분할:
   - `feat(markdown): single newline preserved via remark-breaks (SSOT plugin set)`
   - `test(markdown): regression coverage for newline + GFM features`
7. PR 생성

각 커밋 trailer:
```
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

**PR**:
- title: `feat(markdown): preserve single newlines in chat (remark-breaks)`
- body: 사용자 보고 인용 + 적용 위치 (SSOT 리스트) + 회귀 테스트 + 수동 smoke checklist

**주의사항**:
- ReactMarkdown 사용처 grep (`rg "ReactMarkdown" src/`) 으로 4 곳 외에 추가 발견 시 동일 SSOT 적용
- code block 내부의 newline 은 remark-breaks 가 건드리지 않음 (검증 필요)
- 만약 inline `code` 안에 newline 이 있는 케이스가 보고되면 별 plan 으로 분리

---

# 셀프 이슈 본문 초안

> ## feat: preserve single newlines in chat markdown rendering
>
> Beta 사용자 보고 (2026-04-26): 여러 줄 로그를 paste 하면 한 줄로 합쳐짐. CommonMark spec 상 paragraph 안 single newline 은 공백으로 collapse 되는 게 맞지만, 채팅/로그 컨텍스트에서는 newline 의미가 살아야 자연스러움.
>
> ### 진단
>
> - 입력 textarea / DB 저장 측은 `\n` 정상 보존
> - 표시 측 `react-markdown` 이 `remark-gfm` 만 사용 → single newline collapse
> - 해결: `remark-breaks` 플러그인 추가 (single newline → `<br>`)
>
> ### Plan
>
> `docs/plans/markdownSingleNewlineBreaksPlan_2026-04-26.md` — 4 사용처를 `src/lib/markdownPlugins.ts` SSOT 로 통일 + 회귀 테스트.
