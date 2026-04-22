# tunaFlow Chat Virtualization Plan

- 작성자: Claude
- 작성 시각: 2026-03-29
- 상태: 보류 — 긴 대화 성능 문제가 체감될 때 착수

## 목적

긴 대화(수백~수천 메시지)에서 렌더링 성능과 스크롤 안정성을 확보한다.
현재는 모든 메시지를 DOM에 직접 렌더링하며, 메시지 수가 적은 단계에서는 문제가 없다.

## 현재 상태

- `ChatPanel.tsx`: `messages.map()` → 전체 DOM 렌더링
- `MessageItem`: `React.memo` + custom `areEqual`로 불필요 리렌더 방지
- Auto-scroll: `scrollKey = length:id:status` 기반
- 성능 최적화: Zustand 개별 selector, memo 적용 완료

## 착수 기준

아래 중 하나라도 해당되면 착수:

1. 단일 대화에서 메시지 200개 이상 시 스크롤 버벅임 체감
2. 메시지 렌더링으로 인한 input lag 발생
3. 메모리 사용량이 대화 길이에 비례해 증가하는 것이 문제가 될 때

## 권장 방향

### Option A: react-virtuoso (권장)

tunaChat에서 검증된 라이브러리.

장점:
- 가변 높이 아이템 지원
- `atBottomStateChange` 콜백으로 auto-scroll 제어
- `followOutput` prop으로 streaming 중 하단 고정
- 역방향 스크롤(위로 더 로드) 지원

주의:
- 메시지 그룹핑(`grouped` prop)과의 호환성 확인 필요
- sticky input 영역과의 레이아웃 조정
- branch badge 등 hover 요소가 virtualized 영역 밖으로 나가지 않는지 확인

### Option B: 직접 intersection observer

라이브러리 없이 viewport 밖 메시지를 placeholder로 교체.

장점: 의존성 없음
단점: 가변 높이 계산 복잡, auto-scroll 구현 부담 큼

## 예상 변경 파일

- `src/components/tunaflow/ChatPanel.tsx` — 메시지 영역을 Virtuoso로 교체
- `src/components/tunaflow/MessageItem.tsx` — virtualized item으로 동작 확인
- `package.json` — `react-virtuoso` 추가

## 비목표

- 무한 스크롤 / 페이징 (메시지는 이미 전량 로드)
- DB 쿼리 최적화 (현재 `list_messages`는 전체 반환)
- 메시지 lazy loading

## 선행 조건

- Step 1-3 (Markdown/FileViewer/Grouping) 완료 — **완료**
- 긴 대화 성능 측정 데이터 확보 — 미완료
