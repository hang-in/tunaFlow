# ContextPack Visibility UI Polish Plan

> 작성: 2026-03-30
> 선행: 4-engine context metadata parity (trace_log에 context_mode/sections/length/truncated 저장 완료)

## 목적

DB에 저장된 context metadata를 사용자가 읽기 쉬운 표면으로 다듬는다.
범위는 **UI 가시화**에만 두고, budget slider나 retrieval 구조 변경까지는 포함하지 않는다.

## 현재 상태

- `trace_log`에 `context_mode`, `context_sections` (JSON), `context_length`, `context_truncated` 저장 완료 (4-engine)
- TracePanel 히스토리 카드에서 기본 표시 존재 (8px 폰트, muted 색상 — 거의 안 보임)
- RuntimeStatusBar에는 context 정보 없음
- MessageMeta에는 context 정보 없음

## 변경 사항

### Step 1: TracePanel 히스토리 카드 context 섹션 가독성 개선

현재:
```
[claude-code] [agent.send]               [ok]
⏱ 2.1s  1,234 tok  💲$0.0034             14:23:05
Lite project context                     2.1k
```

목표:
```
[claude-code] [agent.send]               [ok]
⏱ 2.1s  1,234 tok  💲$0.0034             14:23:05
─────────────────────────────────────────────────
📦 Lite · project context plan rawq     2.1k chars
                                    ⚠ truncated
```

변경:
- context 섹션 영역을 `text-[9px]`로 키우고 border-t 강화
- context mode를 pill badge로 (Lite=회색, Standard=파랑, Full=보라)
- sections를 개별 pill로 (bg-accent 배경)
- truncated 경고를 amber pill로 (text + icon)
- context length를 "2.1k chars" 형식으로

### Step 2: Aggregate 영역에 최근 context mode 표시

Aggregate stats 3-col 그리드 아래에:
```
Last context: Standard · 5 sections · 3.2k chars
```
- 가장 최근 span의 context metadata 요약
- 한 줄, 작은 폰트, 클릭 시 히스토리 열기

### Step 3: RuntimeStatusBar에 context mode 요약

현재: `[spinner] claude-code | 0 jobs | $0.0034 | rawq ready`

추가: trace area에 마지막 context mode 표시
```
[spinner] claude-code | 1 job | Std · 5s | $0.0034 | rawq ready
```
- "Std" = Standard의 약어 (Lite=Lite, Standard=Std, Full=Full)
- 5s는 context sections 수
- 실행 중이 아닐 때도 마지막 사용된 mode 표시

## 수정 파일

| 파일 | 변경 |
|---|---|
| `src/components/tunaflow/context-panel/TracePanel.tsx` | Step 1 (카드 가독성), Step 2 (aggregate context 요약) |
| `src/components/tunaflow/RuntimeStatusBar.tsx` | Step 3 (status bar context mode) |

## 검증

1. `npx tsc --noEmit`
2. `npx vite build`
3. 수동: TraceModal 열어서 히스토리 카드의 context 영역 확인
4. 수동: RuntimeStatusBar에서 context mode 약어 확인
