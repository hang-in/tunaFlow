# context-hub Search/Get UI Plan

> 작성: 2026-03-30
> 선행: contextHubMinimalIntegrationPlan (health/search/get backend 완료)

## 목적

context-hub의 search/get을 사용자가 실제로 사용할 수 있는 UI로 노출한다.
"공급층이 실제로 쓸 수 있다"를 보여주는 최소 단계.

## 현재 상태

- Backend: `context_hub_health`, `context_hub_search`, `context_hub_get` Tauri commands 완료
- Frontend: Settings > Runtime에 health 상태 카드만 있음
- context-hub CLI가 설치되지 않으면 "unavailable" graceful 표시

## 변경 사항

### Settings > Runtime context-hub 카드 확장

기존 health 카드를 interactive search/get UI로 확장:

1. **검색 입력**: query input + search 버튼
2. **결과 리스트**: title, source, snippet, score 표시. 클릭 시 get
3. **문서 미리보기**: 선택한 문서의 content를 모달 또는 확장 영역에 표시
4. **unavailable 상태**: 검색 비활성화, 설치 안내 표시

### UX 흐름

```
[context-hub card]
  Status: ready (v0.1.0)
  Policy: bundled/local/private only

  [Search: ____________] [Search]

  Results:
  ┌─ React Hooks Guide (local:docs) — 92%
  │  Custom hooks allow you to extract...
  ├─ API Reference (bundled:skills) — 87%
  │  The API supports...
  └─ (click to view full document)

  [Document Preview]
  ┌─────────────────────────────────┐
  │ React Hooks Guide               │
  │ Source: local:docs              │
  │                                 │
  │ (full content here)             │
  │                        [Close]  │
  └─────────────────────────────────┘
```

## 수정 파일

| 파일 | 변경 |
|---|---|
| `src/components/tunaflow/SettingsPanel.tsx` | context-hub 카드를 ContextHubPanel 컴포넌트로 교체 |

## 검증

1. `npx tsc --noEmit`
2. `npx vite build`
3. context-hub 미설치 시: "unavailable" + 검색 비활성화
4. 설치 시: 검색 → 결과 → get → 문서 내용 표시

## 비목표

- ContextPack 자동 삽입 (선택한 문서를 prompt에 넣기)
- Knowledge Sources settings nav 추가
- 새 파일 생성 (SettingsPanel 안에서 해결)
