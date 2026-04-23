---
title: Windows 빌드 지원 — macOS 이후 가장 빠른 공개 타겟
status: ready-to-implement
priority: P1 (베타 이후 가장 강한 우선순위, 사용자 요청 2026-04-24)
created_at: 2026-04-24
related:
  - .github/workflows/build.yml
  - src-tauri/tauri.conf.json
  - src-tauri/binaries/
  - scripts/build.sh
  - scripts/build-rawq.sh
---

# Windows 빌드 지원

## TL;DR

현재 `tunaFlow v0.1.0-beta` 는 **macOS aarch64 (Apple Silicon) 전용**. 사용자 2026-04-24 요청: 윈도우 지원 "빠르게". Tauri 2 가 Windows 를 지원하므로 아키텍처 변경 없이 **빌드 파이프라인 확장 + 사이드카 크로스 컴파일 + UI 경로 정규화** 3가지로 해결 가능. 예상 1~2일.

## Scope

- **타겟**: Windows x86_64 (MSVC). ARM64 Windows 는 후속.
- **결과물**: `tunaFlow_0.1.0-beta_x64-setup.nsis` (NSIS installer) + 포터블 `.exe` 옵션.
- **릴리즈 채널**: 같은 `v0.1.0-beta` Release 에 Windows asset 을 추가 업로드.

## 주요 작업 영역

### 1. `.github/workflows/build.yml` 확장

- `rawq` 빌드 matrix 에 `windows-latest` + `x86_64-pc-windows-msvc` 추가
- `tauri-lite` job 을 matrix 로 변환 (macos-latest + windows-latest)
- 타겟별 번들 포맷 분기 (`.dmg` / `.app.tar.gz` vs `.nsis` / `.exe`)

### 2. `src-tauri/tauri.conf.json`

`bundle.targets` 에 `"nsis"` 추가. WebView2 embed bootstrapper 포함으로 Windows 10 초기 빌드도 지원.

### 3. rawq 사이드카 크로스 컴파일

`scripts/build-rawq.ps1` 신규 (PowerShell). 또는 `scripts/build-rawq.sh` 에 `--target` 옵션.

**주의**: rawq 로컬 patch (`_research/_util/rawq/src/search/engine.rs:995-1001` clamp) 는 Windows 빌드에도 반영.

### 4. UI 경로 정규화 (최대 리스크)

Windows `\` vs Unix `/`. audit 대상:
- `src-tauri/src/commands/projects.rs` — project path
- `src-tauri/src/commands/vector_search/index.rs` — chunk.source_path
- `src-tauri/src/commands/agents/` — CLI spawn cwd
- 하드코딩 `split("/")` / `str.split('/')` grep

대부분 `PathBuf` 사용이면 OS 자동 처리.

### 5. CLI agent 가용성

- Claude Code CLI: Windows 공식 지원
- Codex / Gemini CLI: Windows 동작 확인 필요
- PTY: `portable-pty` 크레이트가 ConPTY 이미 지원 — 코드 변경 0 예상

### 6. Installer UX

- NSIS 설치 경로: `%LOCALAPPDATA%\tunaFlow`
- Start Menu shortcut 자동
- 사용자 데이터: `%APPDATA%\tunaFlow\`

### 7. 서명 (후속)

Beta 는 unsigned. SmartScreen 경고 뜸.
- 당장: README 에 "SmartScreen → 추가 정보 → 실행" 우회 한 줄
- 후속: Azure Trusted Signing 또는 Authenticode certificate

## Invariants

- **[INV-1]** Windows 빌드는 macOS 와 **동일한 `v*.*.*` Release** 에 asset 추가 (별도 release 분리 금지).
- **[INV-2]** rawq 로컬 patch 는 Windows 빌드에도 반영.
- **[INV-3]** 경로 처리는 `PathBuf` 통해 OS 자동 처리. 하드코딩 `/` split 발견 시 수정.
- **[INV-4]** CLI agent 감지 로직은 OS 무관 — PATH lookup 만 사용.

## Developer 핸드오프 프롬프트

```
[작업] Windows x64 빌드 지원 — macOS 와 동일 Release 에 asset 추가

[SSOT] docs/plans/windowsBuildPlan_2026-04-24.md 먼저 읽을 것

[순서]

1. .github/workflows/build.yml — rawq matrix + tauri-lite matrix 에 windows-latest 추가
2. src-tauri/tauri.conf.json — bundle.targets 에 "nsis" 추가 + windows 블록
3. scripts/build-rawq.ps1 신규 — Windows 타겟 rawq 빌드
4. Path handling audit — PathBuf 사용 여부 확인 (projects.rs / vector_search / agents/)
5. README / README.ko — "Windows / Linux builds 미지원" 문구 업데이트
6. INSTALL.md — Windows 설치 섹션 추가 (SmartScreen 우회 포함)
7. GitHub Actions 에서 windows-latest runner 로 실제 빌드 검증
8. 빌드 성공 시 v0.1.0-beta Release 에 asset 자동 업로드 확인

[검증]
- cargo check --target x86_64-pc-windows-msvc (CI 기준)
- Windows VM / 실기에서 설치 + Chat 1회 + Branch 생성 + Engine 전환 smoke test

[커밋]
- feat(ci): add Windows x64 to Release Build workflow
- feat(rawq): Windows cross-compile script
- chore(tauri): NSIS installer bundle config
- fix(path): ensure PathBuf usage across commands
- docs(install): Windows installation + SmartScreen bypass

[PR 제목]
feat(windows): x64 build + NSIS installer + rawq cross-compile
```

## Rationale

### 왜 Windows 를 "빠르게"

사용자 요청 (2026-04-24). 커뮤니티 공개 후 Windows 지원 여부는 즉시 들어오는 질문. macOS 만 배포하면 사용자 베이스 절반+ 차단.

### 왜 x64 만 먼저

Windows ARM64 는 사용자 베이스 아직 소수. x64 안정화 후 후속.

### 왜 NSIS 만 (MSI 아님)

MSI 는 WiX Toolset 설치 필요 + 빌드 시간 증가. NSIS 가 Tauri 기본이며 대부분 요구사항 충족.
