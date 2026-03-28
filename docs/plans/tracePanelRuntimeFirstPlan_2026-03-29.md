# tunaFlow TracePanel Runtime-First 재설계 계획

- 작성자: Claude
- 작성 시각: 2026-03-29
- 상태: Phase 1 완료

## 목적

TracePanel을 "trace 이력 뷰어"에서 "런타임 상태 대시보드"로 전환한다.
사용자가 가장 먼저 봐야 할 정보: 지금 뭐 하는 중인지, 얼마나 걸렸는지, 어떤 context가 적용 중인지.

## 이전 상태 (trace-history-first)

- Live status (Running/Idle) 인디케이터가 작음
- Active jobs 카드가 보조적
- Span 리스트가 전체 영역의 80%를 차지
- rawq/skills 상태가 부각되지 않음
- 실행 경과 시간 표시 없음

## Phase 1 (완료)

### 변경 사항

1. **Runtime 섹션 최상단 배치**
   - Running/Idle 인디케이터 확대 (2.5px dot, 12px text, font-semibold)
   - Active jobs: 카드 형태로 확대, 경과 시간 실시간 표시
   - rawq 상태: 배지 형태로 ready/error/unavailable 색상 구분
   - active skills 카운트 표시

2. **Trace History 접기/펼치기**
   - 기본 접힌 상태
   - 헤더에 span 수 + refresh 버튼
   - 펼치면 기존 span 리스트 표시

3. **Auto-refresh**
   - 실행 중일 때 2초 간격으로 jobs 자동 갱신

### 변경 파일

- `src/components/tunaflow/context-panel/TracePanel.tsx`

## Phase 2 (후순위)

- [ ] 실행 중 경과 시간 실시간 카운터 (현재는 갱신 시점 기준)
- [ ] span 그룹핑 (trace_id 기반)
- [ ] 엔진별 aggregate 분리 표시
