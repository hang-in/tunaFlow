---
title: Flexbox Conventions (Tailwind)
updated_at: 2026-04-25
canonical: true
status: active
owner: tunaFlow-core
related:
  - src/components/tunaflow/AppShell.tsx
  - src/components/tunaflow/CenterPanel.tsx
  - src/components/tunaflow/ChatPanel.tsx
  - docs/plans/chatPanelMinHeightCascadePlan_2026-04-25.md  # 본 문서의 동기 (#191/#192 사고 cascade)
---

# Flexbox Conventions

> tunaFlow 의 모든 React 컴포넌트는 본 규약을 따라 Tailwind flex 컨테이너를 작성한다.

## 핵심 invariant

**`flex flex-col` + `flex-1` 자식이면 그 자식에 `min-h-0` 필수.**

`min-w-0` 만 챙기는 흔한 함정. row 와 column 의 default min-size 가 다르기 때문에 axis 방향에 맞춰 명시해야 한다.

| 컨테이너 방향 | 자식이 `flex-1` 일 때 필요한 min 토큰 |
|---|---|
| `flex flex-col` | `min-h-0` (필수)  |
| `flex flex-row` (default `flex`) | `min-w-0` (필수) |

## 왜 필요한가

브라우저 default 로 flex item 의 `min-width` / `min-height` 는 `auto` — 즉 자식의 content min-size 까지 stretch. 이 default 가 깨져야 `flex: 1` 비율이 부모 사이즈를 정확히 분할한다.

- `flex-row` 자식에 `min-w-0` 누락 → 긴 텍스트가 자기 width 를 강요 → 옆 형제 squish
- `flex-col` 자식에 `min-h-0` 누락 → 긴 자식이 자기 height 를 강요 → **컨테이너가 부모 height 를 초과 → 푸터 / 상태바 화면 밖으로 밀림**

후자는 발생 시점이 mount/unmount 또는 phase 전이 같은 비교적 늦은 시점이라 dev 중 못 잡고 사용자 빌드에서 표면화되기 쉽다 (#191, #192 cascade).

## 적용 패턴

### Pattern A — 기본 column scroll

```tsx
// ✅ Good
<div className="flex flex-col h-full">
  <Header className="shrink-0" />
  <div className="flex-1 min-h-0 overflow-y-auto">
    {/* content */}
  </div>
  <Footer className="shrink-0" />
</div>
```

### Pattern B — column 내 또 다른 column

```tsx
// ✅ Good — 안쪽 column 도 자체 min-h-0
<div className="flex flex-col flex-1 min-h-0">
  <div className="flex flex-col flex-1 min-h-0 overflow-hidden">
    <div className="flex-1 min-h-0 overflow-y-auto">{/* ... */}</div>
  </div>
</div>
```

### Pattern C — Master-detail (row 안에 column)

```tsx
// ✅ Good
<div className="flex flex-1 min-h-0">  {/* row container */}
  <div className="w-[40%] shrink-0 overflow-y-auto">{/* list */}</div>
  <div className="flex-1 min-w-0 flex flex-col overflow-hidden">  {/* detail = column */}
    <Header className="shrink-0" />
    <div className="flex-1 min-h-0 overflow-y-auto">{/* content */}</div>
  </div>
</div>
```

핵심: row 자식 (`flex-1 min-w-0 flex flex-col`) 안의 column 자식 (`flex-1 overflow-y-auto`) 도 `min-h-0` 필요. cascade 가 자동으로 안 내려간다.

## 안티패턴

```tsx
// ❌ Bad — column 자식 min-h-0 누락
<div className="flex flex-col flex-1 min-w-0 overflow-hidden">
  <div className="flex-1 overflow-hidden">{/* virtuoso */}</div>
</div>

// ❌ Bad — overflow-hidden 만으로 방어 시도 (브라우저별 일관성 X)
```

## 검증 방법

### 수동 audit

```bash
# flex-col + flex-1 자식 중 min-h-0 누락 추적
rg -n "flex-1\b" src/components/ --type tsx | grep -v "min-h-0" | grep -v "shrink"

# 부모가 flex-col 인지 확인 후 누락이면 추가
```

### 회귀 트리거 (수동 smoke)

플랜/대화 phase 전이 시 푸터 위치가 흔들리면 십중팔구 column 의 어느 자식에 `min-h-0` 누락. ChatPanel / CenterPanel / 그 안의 Workflow tab / InsightPanel / ReviewPanel / ArtifactDetailPanel 의 column flow 부터 확인.

## 사고 이력

| 날짜 | 이슈 | 원인 |
|---|---|---|
| 2026-04-24 | #191 | AppShell main flex `min-h-0` 누락 → 푸터 밀림 |
| 2026-04-25 | #191 follow-up | `min-h-0` cascade 가 ChatPanel 내부 (line 190 / 205 / 215) 까지 못 도달 + ArtifactDetail / Insight 우측 column 안의 `flex-1 overflow-y-auto` 들도 동일 함정 |

## Related

- `docs/plans/chatPanelMinHeightCascadePlan_2026-04-25.md` — 본 규약의 동기
- `docs/reference/coding-convention.md` §2 Frontend — 참조 진입점
- (후속 후보) `lintFlexMinHeightAutomationPlan` — ESLint custom rule 자동 검출
