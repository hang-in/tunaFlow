---
title: Terminal-in-Chat Streaming
status: idea
created: 2026-04-12
---

## 아이디어

PTY 에이전트 실행 중 채팅 메시지 버블 대신 **xterm.js 터미널을 채팅 영역 인라인으로 표시**하고,
완료되면 터미널을 접고 최종 응답만 채팅 메시지로 남기는 방식.

## 현재 방식 (JSONL polling)

```
PTY 출력 → JSONL 파일 → 200ms 폴링 → 텍스트 파싱 → React DOM 업데이트
```

- 200ms × invoke × 파일 read × JSONL 파싱 = CPU 지속 사용
- React DOM 스트리밍 업데이트 = 빈번한 리렌더

## 제안 방식 (Terminal-in-Chat)

```
PTY 출력 → pty:output → xterm.js (GPU WebGL 렌더링)
완료 감지 → JSONL 1회 read → DB 저장 → 터미널 접힘
```

- JSONL 폴링 루프 완전 제거
- xterm.js WebGL 렌더링 = GPU, React DOM 업데이트 없음
- VTE 파싱은 동일 (완료 감지 필요)

## UX

- 실행 중: 채팅 메시지 위치에 xterm 터미널 표시 (Claude thinking/tool 실시간 확인)
- 완료 후: 터미널 자동 접힘 → 최종 응답 텍스트만 메시지로 표시

## 비용 비교

| 항목 | 현재 | 제안 |
|------|------|------|
| JSONL 폴링 | 200ms × N | 없음 |
| React 리렌더 | 청크마다 | 없음 |
| xterm 렌더링 | GPU (현재도 동일) | GPU |
| VTE 파싱 | 매 read | 매 read (동일) |

→ **제안 방식이 CPU 더 저렴**

## 구현 비용

- ChatPanel에서 streaming 중인 PTY 메시지에 xterm 컴포넌트 삽입
- 완료 시 터미널 → 텍스트 메시지 전환 애니메이션
- sendMessageViaPty 로직에서 JSONL 폴링 루프 제거

## 관련 이슈

- 현재 JSONL 폴링이 bge-m3 연속 실행과 함께 주요 CPU 원인
- xterm.js는 이미 pty:output 이벤트 수신 중 (TerminalPanel)
