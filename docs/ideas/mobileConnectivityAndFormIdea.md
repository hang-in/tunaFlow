# 모바일 연결 경로 & 배포 형태 — Rust 네이티브 터널 + 단계적 설치형

> Status: idea
> Created: 2026-04-22
> Trigger: 현재 Cloudflare Tunnel 의존이 모바일에서 불안정 → Rust 네이티브 대안 + 모바일 형태 장기 방향 검토
> 관련: `docs/ideas/mobileArchitectureIdea.md`, `docs/ideas/onboardingMetaAgentIdea.md`, s30 (tunaflow-mobile scaffold)

---

## 1. 현재 상황

- `tunaflow-mobile/` 은 웹 기반 PWA 스캐폴드 (s30)
- 원격 접속 경로는 **Cloudflare Tunnel** 경유
- 사용자 체감: 모바일에서 간헐적 불안정
- 원인 분리 필요 — CF 지연·끊김인지, 앱 resilience 부족인지, 혼재인지

---

## 2. 결론

- **경유 터널 제거**: Cloudflare Tunnel 의존 끊고 Rust 네이티브 (`mDNS + WebRTC`) 로 전환
- **자체 호스팅 tunnel (VPS)** 선택지는 사용자 진입 장벽 때문에 제외
- **모바일 형태는 단계적**: PWA → Tauri Mobile → Standalone agent 디바이스
- 바이너리 증가 2~3MB 는 무시 가능

---

## 3. 연결 경로 설계 — mDNS + WebRTC

### 3.1 3-tier 시나리오

```
앱 시작 → 데스크톱이 mDNS 브로드캐스트 + WebRTC offer 준비
모바일 → QR 스캔 → 시나리오 판별:
  (a) 같은 Wi-Fi (mDNS)        → ws://<local>:19840    [즉시, ~100%]
  (b) 원격 (WebRTC P2P)        → data channel 성립     [3-10초, 85-95%]
  (c) 둘 다 실패               → "같은 Wi-Fi 로 연결하세요" 안내
```

### 3.2 커버리지 예상

| 환경 | 예상 성공률 |
|---|---|
| 같은 Wi-Fi | ~100% (mDNS 로 해결) |
| 일반 가정 원격 회선 | 85-95% (symmetric NAT 제외) |
| 기업/공용 Wi-Fi | 50-70% (UDP 차단 많음) |

실패 케이스는 **친절한 거부** 로 처리 — "현재 회선이 연결을 차단합니다. 같은 Wi-Fi 공유 시 즉시 연결됩니다". 절대 VPS 띄우라고 요구하지 않음.

### 3.3 고급 옵션 (숨김)

- 본인 TURN 서버 주소 입력 가능 필드 (파워 유저용)
- 기본값은 공용 STUN 만 사용

---

## 4. 구현 스케치

### 4.1 추가 모듈 (Rust)

```
src-tauri/src/tunnel/
├── mdns.rs        — LAN 발견/브로드캐스트 (crate: mdns-sd)
├── webrtc.rs      — data channel + ICE (crate: webrtc-rs)
├── signaling.rs   — SDP/ICE 인코딩 → QR 페이로드
└── mod.rs         — 시나리오 선택 로직
```

### 4.2 기존 자산 재사용

| 이미 있음 | 활용 방식 |
|---|---|
| `react-qr-code` 의존성 | QR payload 에 `{token + SDP offer + local IP 힌트}` 인코딩 |
| HTTP API token auth (`93308089-...`) | WebRTC 연결 후 첫 메시지로 검증 |
| HTTP API 19840 (axum) | mDNS 시 직접 접근. WebRTC 시 data channel → axum router 프록시 |

### 4.3 QR 페이로드 크기 대응

SDP 가 길어서 QR 버전이 높아짐 → 읽기 어려워질 수 있음.
- 옵션 A: gzip + base64 압축
- 옵션 B: short-code → 로컬에 잠시 HTTP 로 호스팅, 모바일이 fetch
- 권장: B — QR 에는 짧은 코드만, 실제 SDP 는 local HTTP 에서 받음 (같은 Wi-Fi 가정 성립 시 자연스러움)

### 4.4 바이너리 영향

- `webrtc-rs` + `mdns-sd`: 합쳐서 2~3MB 증가
- 데스크톱 현재 번들 대비 무의미한 수준

---

## 5. 앱 레벨 Resilience (CF 교체와 별개로 점검 필수)

현재 모바일 불안정이 CF 문제만은 아닐 가능성. 같은 Wi-Fi 에서 local IP 직접 접속해보고 **여전히 불안정**하면 아래가 근본 원인.

| 체크 항목 | 문제 패턴 | 대응 |
|---|---|---|
| WebSocket heartbeat | 5분 idle 시 연결 silently dead | 30s ping/pong |
| 재연결 로직 | disconnect 후 자동 복구 안 함 | exponential backoff reconnect |
| 모바일 백그라운드 복귀 | 앱 재개 시 stale connection 보유 | visibility API → 재연결 |
| 네트워크 전환 (4G↔Wi-Fi) | IP 바뀌며 WS 끊김 인지 못 함 | online/offline 이벤트 반응 |
| 메시지 순서 | 재연결 후 누락 이벤트 | sequence number + resume-from 쿼리 |
| 세션 복구 | 연결 복구되어도 상태 mismatch | `/api/v1/session/resume` 엔드포인트 |

→ 이 부분은 **네트워크 경로(CF/WebRTC/mDNS) 와 독립**된 문제. 어떤 경로로 가든 필요.

---

## 6. 모바일 배포 형태 로드맵

### Phase 1 (현재 ~ 베타): PWA
- 배포: URL 공유 → 즉시 사용, 앱스토어 심사 0
- 업데이트: 즉시 반영
- iOS/Android 동시 커버
- 한계:
  - **mDNS 웹에서 불가** — 브라우저가 `.local` resolve 못 함 → QR 에 local IP 인코딩하는 우회로만
  - iOS Safari 의 background persistence / notification / 파일시스템 제약
  - "진짜 앱" 인상 부족

### Phase 2 (베타 이후 3~6개월): Tauri Mobile
- Tauri 2 iOS/Android 베타 성숙도 평가
- 데스크톱 Rust 코어 (DB, http_api, ContextPack, embedder) **그대로 재사용** → 유지보수 1 코드베이스
- mDNS native 처리 → LAN UX 압도적 개선
- OS keychain, native notification, background sync
- 리스크: Tauri Mobile 플러그인 ecosystem 미성숙

### Phase 3 는 없음 — 데스크톱 필수

tunaFlow 의 정체성은 **상용 터미널 에이전트(Claude/Codex/Gemini CLI)를 잘 부리는 것**. 이건 데스크톱 없이 성립 안 함 (CLI 바이너리·파일시스템·PTY 접근). 따라서:

- **모바일은 데스크톱의 원격 컨트롤러** 역할에 항상 고정
- "폰 하나로 agent orchestration" 은 tunaFlow 의 목표가 **아님**
- 소형 LLM 을 폰에서 직접 돌리는 것은 다른 용도(예: RT 전용 보조 판정자, offline memo 보조) — 별도 프로덕트(`tunaMicro` 같은 가칭) 로 브랜치. tunaFlow 본류로 끌고 들어오지 않음

### Phase-independent 설계 원칙

PWA 단계부터 Tauri Mobile 전환이 쉽도록:
- 비즈니스 로직은 React Query/Zustand 로 분리 (훅 단위 재사용 가능)
- `@tauri-apps/api` 등가물 추상화 (웹에서는 fetch shim, native 에서는 실제 tauri command)
- PWA manifest + service worker 는 "설치형 체험" 근사치 (홈화면 설치, offline cache)

---

## 7. 실행 우선순위

1. **진단 먼저**: 같은 Wi-Fi 에서 local IP (`ws://192.168.x.x:19840`) 로 직접 접속 시도 → CF 문제 vs 앱 resilience 문제 분리
2. **앱 resilience 패치** (§5): 어떤 경로로 가든 필요. 먼저 해결
3. **mDNS + QR 페이로드 확장** (§3.1-(a), §4.3): LAN 에서 설정 0 연결
4. **WebRTC P2P** (§3.1-(b)): 원격 접속 CF 대체
5. **CF Tunnel 제거 + 옵션 유지**: CF 는 optional fallback 으로 남기고 기본 off
6. **Phase 2 Tauri Mobile PoC** 는 별도 plan 으로 승격 (베타 이후)

---

## 8. 검증 필요 항목

- `webrtc-rs` crate 의 Rust 통합 성숙도 (dev-test 결과)
- iOS Safari WebRTC data channel 안정성 (최소 iOS 15+)
- QR 페이로드 옵션 B (short-code HTTP fetch) 의 보안 (토큰이 잠시 노출되는 위험)
- Tauri Mobile 프로덕션 가용성 판정 기준 (pinning 할 tauri 버전)
- mDNS-sd crate 의 macOS/Windows/Linux 호환성
- 공용 STUN (Google) 가용성 SLA — 여러 STUN 서버 fallback 필요

---

## 9. 관련 문서

- `docs/ideas/mobileArchitectureIdea.md` — 모바일 아키텍처 일반
- `docs/ideas/litertLmIntegrationIdea.md` — Phase 3 에서 재등장
- `docs/ideas/onboardingMetaAgentIdea.md` — QR 페어링 UX
- s30 (`project_session_2026-04-13_s30`) — tunaflow-mobile 초안
- `src-tauri/src/http_api/ws.rs` — 쿼리파라미터 auth (s30)
