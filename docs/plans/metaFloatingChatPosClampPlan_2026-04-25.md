---
title: MetaFloatingChat stale pos clamp — drawer mount 시 ChatPanel 압축 race 차단
status: ready-to-implement
priority: P1 (사용자 가시 — drawer 열림 시 본문 영역 0px 압축)
created_at: 2026-04-25
related:
  - src/components/tunaflow/MetaFloatingChat.tsx       # 본 변경 위치
  - src/components/tunaflow/AppShell.tsx               # mount 부모 (main area)
  - docs/reference/flexboxConventions.md               # cascade SSOT
canonical: true
owners:
  - architect (본 plan 작성)
  - developer (구현)
---

# 증상 (사용자 보고, 2026-04-25)

> "브랜치(dev)에서 또 스크롤이 올라가면 풋터 아래가 말려 올라가서 빈공간이 생기는데?"
> "이젠 그냥 드로어 로딩만 되도 밀려 올라가네"
> "메타에이전트 아이콘을 살짝 옮기니깐 다시 정상으로 돌아왔어"
> "아까 살짝 옮겨진 상태에서 채팅 보내도 안말려올라가"

# 진단 (확정)

## 측정값 요약

콘솔 측정 결과 (drawer 열린 상태):

| element | top | height |
|---|---|---|
| viewport / shell / html | 0 | 1209 |
| body | 0 | 1209 |
| **chat (ChatPanel)** | — | **0** |
| drawer (BranchThreadPanel) | -59 | 1141 |
| mainArea | -65 | 1153 |
| TitleBar | -93 | 28 |

= shell/html 정상 + scrollY=0 인데 row flex 자식들이 음수 top 위치 + ChatPanel 0px 압축.

## Driver 확정 — MetaFloatingChat stale pos

`MetaFloatingChat.tsx:404-407` 의 root:

```jsx
<div className="absolute z-[60]" style={{ left: pos.x, top: pos.y }}>
```

- `pos` 는 `localStorage` 에서 복원 (`loadPos`, line 61)
- 부모 = main area `<div className="flex-1 min-w-0 min-h-0 h-full relative flex">`
- popup `style={{ height: Math.min(POPUP_H, containerH * 0.65) }}` — **render-time 측정** (line 446)

**시나리오**: 이전 세션에서 저장된 `pos.y` 가 현재 viewport 의 main area 안 bounds 밖 (예: 큰 viewport 에서 저장 → 작은 dev 창에서 복원). button + popup 이 main area 위쪽으로 빠지고 absolute 자식이지만 webview reflow 사이클이 row flex sibling (CenterPanel) 측정에 race 영향. ChatPanel root 가 0px 으로 압축.

**검증**: 사용자가 아이콘 살짝 드래그 → `handleButtonMouseDown` (line 351-391) 의 clamp (line 370-371) 가 pos 를 부모 안으로 끌어옴 → race 해소 → 정상 복귀 + 채팅 입력해도 재발 X.

# 옵션

## 옵션 A — mount 시 한 번 clamp + persist (채택)

`useEffect(() => { ... }, [])` 안에서 부모 size 측정 + clamp + localStorage 갱신.

- **Pros**: 단순, 한 번만 측정. mount 시 stale pos 정정.
- **Cons**: 이후 viewport resize 시 stale 재발 가능.

## 옵션 B — ResizeObserver 로 부모 size 추적 + 매번 clamp

부모 element 의 size 변경 시 자동 clamp.

- **Pros**: viewport resize 도 대응. 영구 정합.
- **Cons**: ResizeObserver 추가 (overhead 미미).

## 권장 — A + B 조합

mount 시 clamp + ResizeObserver. 단일 useEffect 로 통합. 코드 한 블록.

# 구현 (단일 경로)

`MetaFloatingChat.tsx:99` (existing useEffect 다음) 에 새 useEffect 추가:

```jsx
// Stale pos clamp: localStorage 복원값이 현재 부모 bounds 밖이면 sibling
// layout race (ChatPanel 0px 압축) 발생. mount 시 + viewport/parent resize
// 시 부모 안으로 clamp + persist. SSOT: docs/plans/metaFloatingChatPosClampPlan_2026-04-25.md
useEffect(() => {
  const wrapper = wrapperRef.current;
  const parent = wrapper?.parentElement;
  if (!parent) return;

  const clamp = () => {
    const pw = parent.clientWidth;
    const ph = parent.clientHeight;
    if (pw === 0 || ph === 0) return; // not laid out yet
    setPos((cur) => {
      const x = Math.max(0, Math.min(cur.x, pw - BUTTON_SIZE));
      const y = Math.max(0, Math.min(cur.y, ph - BUTTON_SIZE));
      if (x === cur.x && y === cur.y) return cur;
      localStorage.setItem("meta-float-pos", JSON.stringify({ x, y }));
      return { x, y };
    });
  };

  clamp(); // initial
  const ro = new ResizeObserver(clamp);
  ro.observe(parent);
  return () => ro.disconnect();
}, []);
```

# Invariants

- **[INV-1]** Mount 시 `pos` 가 부모 main area bounds 밖이면 즉시 clamp + localStorage 갱신
- **[INV-2]** 이후 부모 size 변경 (window resize, sidebar resize, drawer toggle 등) 시 자동 재clamp
- **[INV-3]** Clamp 후에도 사용자가 드래그로 의도적으로 위치 변경 가능 (`handleButtonMouseDown` 그대로 유지)
- **[INV-4]** ChatPanel 0px 압축 race 재현 안 됨 (사용자 검증: 아이콘 정상 위치 + 채팅 입력 → footer drift 없음)

# 검증

## 수동 Smoke (사용자 검증 패턴)

1. localStorage 에서 `meta-float-pos` 강제 변조: `localStorage.setItem("meta-float-pos", JSON.stringify({ x: 9999, y: 9999 }))`
2. 앱 reload → MetaFloatingChat mount → useEffect clamp → localStorage 다시 정상 값
3. drawer 열기 → ChatPanel 정상 height (`document.querySelector('[data-testid=chat-panel]').getBoundingClientRect().height > 0`)
4. 채팅 입력 → footer drift 없음

## 자동

- (선택) localStorage 변조 + render → pos clamp 검증 unit test (`react-testing-library`)
- ChatPanel.test 에 footer drift 회귀 가드 추가 (해당 axis 테스트 어려움 — 보류 가능)

# Developer 핸드오프 프롬프트

```
[작업] MetaFloatingChat stale pos clamp — drawer mount 시 ChatPanel 0px 압축 race 차단 (P1 사용자 가시)

[SSOT] docs/plans/metaFloatingChatPosClampPlan_2026-04-25.md

[배경 3줄]
- 사용자 보고: drawer 열리는 순간 본문 영역 위로 밀림. 메타에이전트 아이콘 살짝 옮기면 정상 복귀
- 진단: localStorage `meta-float-pos` 의 stale 값이 부모 bounds 밖 → sibling layout race → ChatPanel 0px
- Fix: mount 시 + ResizeObserver clamp + localStorage 재persist (한 블록)

[수정 범위]

1) src/components/tunaflow/MetaFloatingChat.tsx
   - line 99 (existing useEffect 다음) 새 useEffect 추가
   - plan 의 §구현 코드 그대로 복사 (clamp + ResizeObserver)
   - BUTTON_SIZE 상수 import 또는 same-file 참조 확인

2) docs/plans/index.md
   - 본 plan 등록 (alphabetical 위치)

[검증]
- npx tsc --noEmit / cargo check
- 수동: localStorage 변조 후 reload → ChatPanel height > 0 + drawer 열기 정상 + 채팅 입력 footer drift 없음

[커밋]
- fix(layout): clamp MetaFloatingChat pos to parent bounds — drawer mount race fix
- docs(plans): register metaFloatingChatPosClampPlan

trailer: Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[PR 제목 / 본문]
fix(layout): MetaFloatingChat stale pos clamp — drawer mount 시 ChatPanel 압축 race 차단

본문에 사용자 측정값 표 + 진단 요약 + 검증 step 포함
Closes #(셀프 이슈 — 작성 시 등록)
```

# 후속 / Sibling

- `mainChatBrandRunningGuardPlan_2026-04-25` — 같은 axis 의 다른 사용자 가시 issue (option B)
- (P3, 후순위) MetaFloatingChat 의 popup `containerH * 0.65` render-time 측정도 useLayoutEffect + ResizeObserver 로 옮기는 정합. 본 fix 로 trigger race 해소되면 후속 plan 후보로 보류
