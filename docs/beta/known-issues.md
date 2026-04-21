---
title: tunaFlow Beta — Known Issues
updated_at: 2026-04-20
canonical: true
status: draft
owner: tunaFlow-core
---

# tunaFlow Beta — Known Issues

베타 시점에 알려진 제약입니다. 각 항목에 **회피 방법**과 **후속 계획**을 같이 표기합니다. 새 문제가 발견되면 [GitHub Issues](https://github.com/hang-in/tunaFlow/issues) 로 알려주세요.

심각도 기준:

- **P0** — 베타에서 사용 불가. 현재 없음.
- **P1** — 주요 기능에서 간헐적으로 발생. 회피 가능.
- **P2** — 특정 조건에서만 재현. 사용성 불편.
- **P3** — 시각적/문구 수준.

---

## 플랫폼 / 배포

### 🔸 P1 — macOS 전용

Windows / Linux 빌드는 아직 제공하지 않습니다. Tauri 2 + Rust 빌드 파이프라인이 크로스 컴파일을 지원하지만 PTY / cloudflared sidecar / 키체인 의존성을 다른 OS 에서 검증하지 못했습니다.

- **회피**: 소스에서 빌드 (`npm run tauri dev`) — 작동은 하지만 보증하지 않음
- **계획**: 0.2 시점에 Windows 빌드 우선 확장

### 🔸 P1 — ad-hoc 서명 (Gatekeeper 경고)

공식 Developer ID 인증서가 아니라 self-signed 로 배포합니다.

- **회피**: `xattr -cr /Applications/tunaFlow.app`
- **계획**: Developer ID 인증서 취득 후 notarization

---

## Roundtable / Streaming

### 🔸 P1 — RT 중간 스트리밍 미지원

Roundtable(RT) 은 라운드 단위로만 결과를 표시합니다. 각 참가자의 응답은 완료 후 한 번에 UI 에 반영됩니다.

- **영향**: RT 진행 중 "에이전트가 살아있는가" 확인이 어려움
- **회피**: RT 대신 단일 Branch 에서 대화를 진행
- **계획**: streaming aggregator 재설계 필요 — 0.2 대상

### 🔸 P1 — JSONL 완료 감지 실패 (간헐적)

PTY 세션에서 에이전트 응답이 DB 에는 기록되지만 UI 에 반영되지 않는 경우가 있습니다. 원인은 JSONL 빠른 완료 메시지 감지 타이밍 문제.

- **빈도**: 세션당 1~2회 수준
- **회피**: 메시지 전송 후 응답이 오지 않으면 대화를 닫았다 다시 열기 → DB 에서 최신 메시지 복구됨
- **계획**: PTY parser 재검토 (0.1.x 패치)

---

## Performance / Resource

### 🔸 P2 — 대규모 프로젝트 최초 인덱싱 시 CPU 스파이크

bge-m3 임베딩 + code-review-graph 초기화가 동시에 돌면 수 분간 CPU 가 높습니다. s35 에서 ONNX 스레드 제한 + 세마포어 + 점진적 인덱싱으로 일부 완화.

- **증상**: 프로젝트 전환 직후 수 분간 팬 속도 ↑
- **회피**: Settings > Runtime 에서 "증분 인덱싱 전용" 모드 전환 가능
- **계획**: 백그라운드 우선순위 조정 + 진행 UI 표시

### 🔸 P2 — 매우 긴 대화에서 compression 소요 시간

480 메시지 기준 compression 에 약 6.1초. 1000 메시지 초과 시 10초대로 증가할 수 있습니다.

- **회피**: 주기적으로 새 대화로 checkpoint
- **계획**: incremental compression + 백그라운드 처리

---

## 모바일 클라이언트

### 🔸 P2 — iOS Safari 에서 WS 연결이 PWA 설치 후에만 안정

브라우저 탭에서는 상태바 켜짐 → 꺼짐 전환 시 WS 가 끊기는 경우가 있음. PWA 로 설치하면 유지됨.

- **회피**: iOS Safari 공유 메뉴 > 홈 화면에 추가
- **계획**: heartbeat 간격 조정

### 🔸 P3 — cloudflared tunnel URL 이 재시작마다 변경

Free tier 사용 중에는 매번 새 URL 이 할당됩니다.

- **회피**: 유료 named tunnel 사용 또는 재시작 후 Settings > Mobile 에서 URL 재복사
- **계획**: named tunnel 설정 가이드 문서 보강

---

## UI / UX

### 🔸 P2 — dev 모드에서 Ctrl+C 종료 시 window state 미저장

`npm run tauri dev` 중 터미널에서 Ctrl+C 로 종료하면 사이드바/드로어 너비가 저장되지 않습니다. 정식 빌드에서는 발생하지 않음.

- **회피**: 앱 창을 먼저 닫고 터미널 종료
- **계획**: tauri-plugin-window-state 업스트림 이슈

### 🔸 P3 — 다크/라이트 모드 일부 구간 대비 부족

다크 테마의 `--prose-muted`, `--prose-faint` 가 작은 폰트에서 WCAG AA 4.5:1 기준을 일부 구간에서 미달합니다.

- **영향**: Lighthouse Accessibility 점수 90+ 달성을 막는 주 원인
- **회피**: 해당 없음 (사용에는 지장 없음)
- **계획**: Phase 4 accessibility-audit.md §2-4 참조, 0.1.x 에서 튜닝

---

## 에이전트 연동

### 🔸 P2 — Codex CLI 에서 모델명이 `models_cache.json` 에 없으면 400

과거 tunaFlow 가 하드코딩된 fallback(`gpt-5-codex` 등) 을 주입해 발생했던 문제. 현재는 명시적 에러로 표면화하며, 에러 메시지에 실제 허용 모델 목록을 포함합니다.

- **회피**: Settings > Agents 에서 Reviewer / Developer 프로필의 model 을 명시 선택
- **계획**: 모델 자동 감지 → 기본값 제안 UX

### 🔸 P2 — Claude CLI resume_token 주입 타이밍

세션 이어가기 시 `~/.tunaflow/api-token` 을 저장/재주입하는데, 첫 메시지 직후 resume_token 이 아직 파일에 쓰이기 전인 상태에서 두 번째 메시지를 보내면 새 세션으로 시작될 수 있습니다.

- **영향**: 컨텍스트 유실
- **회피**: 첫 응답이 완료된 후 다음 메시지 전송
- **계획**: write-back 동기화 보강

---

## 데이터 / 마이그레이션

### 🔸 P2 — 단일 DB 구조

현재 모든 프로젝트가 단일 SQLite DB 를 공유합니다. 베타 공개는 이 구조로 진행하되, 프로덕션은 프로젝트별 DB 분리가 예정되어 있습니다.

- **영향**: 프로젝트 간 데이터 격리가 논리적 (project_key 필터) 수준
- **회피**: 프로젝트별 tunaFlow 인스턴스 별도 실행 불가
- **계획**: 0.2 시점에 Project-per-window 아키텍처 검토 — `docs/ideas/projectPerWindowIdea.md`

### 🔸 P3 — 삭제된 문서의 DB chunks/edges 잔존 정리 미구현

Document RAG 에서 파일을 삭제해도 DB 의 chunk/edge 가 즉시 정리되지 않습니다.

- **영향**: 검색 결과에 stale chunk 가 포함될 수 있음
- **회피**: rawq reindex 실행
- **계획**: fs watcher 에 삭제 이벤트 hook

---

## 보고 방법

버그를 발견하면:

1. **크래시 리포트 첨부** — `~/.tunaflow/crash-reports/` 폴더의 최근 `.log` 파일
2. **재현 순서** — 클릭 순서와 입력 내용
3. **환경** — macOS 버전, tunaFlow 버전, 사용 에이전트
4. **GitHub Issue 생성** — https://github.com/hang-in/tunaFlow/issues

Settings > Help 패널에도 최근 크래시 리포트 목록이 표시됩니다.
