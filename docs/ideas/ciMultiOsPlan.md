# CI 멀티 OS 빌드 계획

> 작성: 2026-04-07
> 상태: 계획

---

## 현재 상태

- `.github/workflows/ci.yml`: windows-latest 단일, Node 20, actions v4
- Windows에서 빌드 실패 중 (Tauri 플러그인 / sidecar 관련 추정)
- Node.js 20 deprecation 경고 (2026-06-02부터 Node 24 강제)

## 즉시 수정 (이번 세션)

- `windows-latest` → `macos-latest` (현재 개발 환경)
- `actions/checkout@v4` → `@v5`
- `actions/setup-node@v4` → `@v5`
- `node-version: 20` → `22`

## 향후 멀티 OS (릴리스 준비 시)

```yaml
strategy:
  matrix:
    include:
      - os: macos-latest
      - os: ubuntu-latest
      - os: windows-latest
```

### OS별 주의사항

| OS | 추가 설정 |
|---|---|
| macOS | 없음 (현재 환경) |
| Linux | `libwebkit2gtk-4.1-dev`, `libappindicator3-dev` 등 apt 패키지 |
| Windows | WebView2 (기본 포함), rawq sidecar `.exe` 확장자 |

### Job 분리

| Job | 실행 조건 | OS |
|---|---|---|
| `check` | 모든 push/PR | macos-latest 단일 (빠른 피드백) |
| `build` | main push 또는 release tag | matrix 3 OS |
| `release` | tag push (`v*`) | matrix 3 OS + artifact 업로드 |

### rawq sidecar

- 바이너리명이 OS별로 다름: `rawq-aarch64-apple-darwin`, `rawq-x86_64-unknown-linux-gnu` 등
- CI에서 빌드하거나 pre-built binary 다운로드 필요
- `scripts/build-rawq.sh` / `build-rawq.ps1` 활용
