# tunaFlow Window State Restore 수정 프롬프트

- 작성자: Claude
- 작성 시각: 2026-03-29
- 카테고리: infrastructure / window-state

```md
# tunaFlow Window State Restore 확인/수정

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

이번 작업 목표:
앱 재시작 시 창 위치와 크기가 복원되는지 확인하고, 안 되면 수정하라.

먼저 확인할 파일:
- `src-tauri/src/lib.rs` — window-state 플러그인 등록 + on_window_event
- `src-tauri/capabilities/default.json` — window-state:default 권한
- `src-tauri/Cargo.toml` — tauri-plugin-window-state 의존성
- `src-tauri/tauri.conf.json` — 윈도우 설정

체크 항목:
1. `tauri_plugin_window_state::Builder::new().build()` 플러그인 등록 여부
2. capabilities에 `window-state:default` 존재 여부
3. `on_window_event`에서 `CloseRequested` 시 `save_window_state` 호출 여부
4. `.window-state.json` 파일 생성 여부 (앱 종료 후)

검증:
- `~/Library/Application Support/com.tunaflow.app/.window-state.json` 존재 확인
- 앱 재실행 시 위치/크기 복원 확인

참고:
- dev 모드 Ctrl+C는 CloseRequested를 발생시키지 않음 — X 버튼으로 닫아야 함
- 검증 체크리스트: `docs/prompts/2026-03-29/window_state_restore_review_checklist.md`

출력 형식:
### A. Current Status
### B. Issues Found
### C. Fixes Applied
### D. Verification
```
