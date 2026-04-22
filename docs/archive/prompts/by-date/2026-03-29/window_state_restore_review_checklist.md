# tunaFlow Window State Restore 검증 체크리스트

- 작성자: Claude
- 작성 시각: 2026-03-29

## 현재 구현 상태

### Backend (Rust)
- [x] `tauri-plugin-window-state` 플러그인 등록 (`lib.rs:45`)
- [x] `capabilities/default.json`에 `window-state:default` 권한 추가
- [x] `on_window_event`에서 `CloseRequested` 시 `save_window_state(StateFlags::all())` 호출 (`lib.rs:164-170`)

### 저장 위치
- `~/Library/Application Support/com.tunaflow.app/.window-state.json` (macOS)
- `%APPDATA%/com.tunaflow.app/.window-state.json` (Windows)

## 검증 절차

### 1. 기본 동작
- [ ] 앱 실행 → 창 이동/리사이즈 → 창 닫기(X 버튼)
- [ ] `.window-state.json` 파일 생성 확인
- [ ] 앱 재실행 → 이전 위치/크기 복원 확인

### 2. Edge Cases
- [ ] 최대화 상태에서 닫기 → 재실행 시 최대화 복원
- [ ] 듀얼 모니터에서 보조 모니터로 이동 후 닫기 → 재실행 시 보조 모니터에 복원
- [ ] dev 모드에서 Ctrl+C 종료 → `RunEvent::Exit` 발생 여부 (이 경우 저장 안 될 수 있음)

### 3. Dev 모드 주의
- `npm run tauri dev`에서 Ctrl+C로 종료하면 `CloseRequested` 이벤트가 발생하지 않음
- 반드시 **창의 X 버튼**으로 닫아야 상태가 저장됨
- hot-reload 시에도 `CloseRequested` 발생 안 함

## 알려진 제한

1. dev 모드 Ctrl+C 종료 시 상태 미저장 — 앱 자체 한계
2. `RunEvent::Exit`은 Tauri 프로세스 정상 종료 시에만 발생
3. `on_window_event` + `CloseRequested`가 가장 안정적인 저장 시점
