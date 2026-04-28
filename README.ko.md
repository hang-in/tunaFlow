<div align="center">

# tunaFlow

**Claude Code · Codex · Gemini · OpenCode 를 한 화면에서 — plan · branch · review 까지.**

[![CI](https://github.com/hang-in/tunaFlow/actions/workflows/ci.yml/badge.svg)](https://github.com/hang-in/tunaFlow/actions/workflows/ci.yml)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![React 18](https://img.shields.io/badge/React-18-61DAFB?logo=react&logoColor=white)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue)](./LICENSE)
[![Status](https://img.shields.io/badge/Status-Beta-f59e0b)](./docs/plans/publicReadinessChecklistPlan.md)

[![🇺🇸 English](https://img.shields.io/badge/🇺🇸-English-9ca3af)](./README.md)
[![🇰🇷 한국어](https://img.shields.io/badge/🇰🇷-한국어-2563eb)](./README.ko.md)

> **Of the agent, By the agent, For the agent**
> *— 인간지능 × 인공지능, 2인3각.*

여러 CLI 를 번갈아 쓰는 피로를 줄이는 데스크톱 앱입니다. Claude Code / Codex / Gemini / OpenCode 를 하나의 Plan → Dev → Review 워크플로우에서 함께 운영합니다.

</div>

![tunaFlow screenshot](./docs/assets/screenshot-main.png)

> 📺 워크플로 데모 (6분)

https://github.com/user-attachments/assets/69cdc5b3-2456-4873-9599-3c2c3e0f6f13

---

## 누구를 위한 도구인가

- Claude Code / Codex / Gemini CLI 를 쓰면서 단순 채팅을 넘어선 **구조화된 워크플로우** 가 필요한 분
- 실행은 에이전트에게 맡기되 **방향과 판단은 직접 잡고** 싶은 분
- AI 에이전트를 일상 개발 흐름에 끼워 넣고 싶은 소규모 팀이나 개인

### 왜 만들었나

작은 불편에서 출발했습니다. Claude Code / Codex / Gemini CLI 를 같이 쓰다 보면 tmux / iTerm / cmux 같은 터미널을 오가며 복붙이 반복됩니다. 엔진 자체는 훌륭한데, 그걸 하나의 흐름으로 엮는 일은 결국 사람이 손으로 했습니다. tunaFlow 는 그 엮는 일을 한 화면 안으로 옮겨서, 사용자가 "터미널 창 관리" 가 아니라 "무엇을 시킬지" 에 집중할 수 있도록 만들었습니다.

---

## 설계 특징

### Engine Parity — 엔진을 바꿔도 프롬프트를 다시 쓰지 않는다
Claude · Codex · Gemini · Ollama 네 엔진 모두 하나의 조립 함수 `build_normalized_prompt_with_budget()` 을 거칩니다. identity · 최근 맥락 · 장기 기억 · 스킬 · 도구 결과가 엔진에 상관없이 같은 ContextPack 으로 조립되므로, 엔진 교체는 한 줄 토글로 끝납니다.

### Blind Cross-verification — 설계 단계에서 결함을 잡는다
Plan 은 Architect(Claude Opus) 가 작성하고, 독립적인 Reviewer(Codex, blind) 가 `invariant_checks` 와 4 차원 루브릭(plan_coverage · code_quality · test_coverage · convention) 으로 검증합니다. 구현 전에 BLOCKER 를 라운드 단위로 걸러내면, 구현 이후에 드러나는 큰 재작업을 막을 수 있습니다.

### Branch-adopt 모델 — 대화가 곁가지로 흐트러지지 않는다
같은 주제를 **Branch** 로 분기해 여러 에이전트에게 실험시키고, 결과가 마음에 들면 **adopt** 로 요약만 main 대화에 가져옵니다. 곁가지 전체가 main 에 섞이지 않아서 대화는 결론의 흐름만 남습니다. Roundtable(RT) 도 이 Branch 의 확장 모드입니다.

### CLI-first — 기존 구독을 최대한 활용
기본 경로는 Claude Code / Codex / Gemini **CLI** 입니다. SDK(API 과금) 는 보조 경로로만 씁니다. 이미 구독 중인 분이 별도 API 요금 없이 모든 기능을 쓸 수 있도록 설계했습니다.

### 품질 우선 — tunaFlow 는 토큰 절약 도구가 아니다
결과물 품질이 먼저입니다. identity / worldview / 분석 요약 같은 맥락 자료는 에이전트 응답에 실제로 도움이 된다면 **AGENTS.md 수준 (1,500~3,000 tokens)** 으로 풍부하게 써도 괜찮습니다. 대신 피하려는 낭비는 **중복** 입니다 — claude 세션 버퍼에 이미 있는 컨텍스트를 또 주입하거나, 오래된 압축본이 현재 요청에 섞이거나, 같은 정보가 여러 섹션에 중복되는 것. 여기서 말하는 "간결함" 은 "극단적으로 줄이기" 가 아니라 **"중복 없애기"** 에 가깝습니다.

---

## 주요 기능

### Orchestration Workflow

Architect → Developer → Reviewer 3 역할 구조입니다.
Architect 가 Plan 을 세우면 Developer 가 구현하고, Reviewer 가 교차 검증합니다.
리뷰에서 문제가 나오면 그 findings 를 분석해 다음 버전(rev.N+1) Plan 을 자동으로 제안합니다.

### Quick / Deep Review

- **Quick**: Reviewer 한 명이 빠르게 검증합니다.
- **Deep**: 여러 엔진이 Roundtable 로 교차 검증하고, 테스트 결과도 자동으로 주입합니다. 4 차원 루브릭(plan_coverage · code_quality · test_coverage · convention) 과 `invariant_checks` 로 평가하며, Reviewer 는 Architect 와 다른 엔진(blind) 으로 배정합니다.

### Interactive Session

`-p` 같은 일회성 플래그 없이 CLI 에이전트와 **세션을 살려둔 채** 대화합니다. 파일 수정, 명령 실행 등 에이전트가 가진 도구를 그대로 쓸 수 있고, 세션이 살아있는 동안은 tunaFlow 가 컨텍스트를 다시 주입하지 않습니다 (claude `--sdk-url` WebSocket 기본 경로 + PTY 레거시 폴백).

### Roundtable (RT)

여러 엔진의 에이전트가 하나의 주제로 토론합니다. Sequential(순차) / Deliberative(동시) 두 가지 모드가 있으며, 모든 RT 는 Branch 의 확장 모드입니다.

### ContextPack

지원 엔진 (Claude / Codex / Gemini / Ollama / LM Studio) 이 공유하는 프롬프트 조립 엔진입니다. Lite / Standard / Full 로 자동 티어링되며, rawq 코드 검색 · 장기 기억 · 실패 학습 · 역할 문서까지 한 번에 묶어 보냅니다.

### Insight

rawq 와 code-review-graph 가 미리 뽑아둔 데이터를 에이전트가 분석합니다. 안정성 · 테스트 · 아키텍처 · 성능 · 보안 · 기술 부채 6 개 카테고리. 간단한 수정은 Quick Wins 로 자동 반영까지 지원합니다.

### 메타 에이전트 온보딩

프로젝트를 처음 설정할 때 현재 설치된 에이전트 CLI 를 자동 감지해서, 해당 스택에 어울리는 에이전트 구성을 추천해줍니다.

---

## 지원 엔진

| 엔진 | 연동 방식 |
|------|----------|
| Claude (Anthropic) | CLI subprocess + WebSocket sdk-session (지속 세션) |
| Codex (OpenAI) | CLI subprocess + app-server (stateful thread) |
| Gemini (Google) | CLI subprocess |
| Ollama / LM Studio / vLLM | HTTP SSE (OpenAI-compatible) |

---

## 설치 및 실행

### 사전 준비

- macOS (현재 macOS 전용)
- **Node.js 20+**
- **Rust stable** — 설치 안 돼있으면 rustup 한 줄로 설치:

  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source "$HOME/.cargo/env"
  ```

  (`npm run tauri dev` 에서 `cargo metadata ... No such file or directory` 로 막히면 원인이 이것입니다. Tauri 는 Rust / cargo 가 필요합니다.)

- 에이전트 CLI 1 개 이상:

```bash
npm install -g @anthropic-ai/claude-code   # Claude
npm install -g @openai/codex               # Codex
npm install -g @google/gemini-cli          # Gemini
```

### 개발 실행

```bash
git clone https://github.com/hang-in/tunaFlow.git
cd tunaFlow
npm install
npm run tauri dev
```

### 빌드

```bash
./scripts/build.sh
```

### 베타 설치 (macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash
```

> macOS ad-hoc 서명이므로 Gatekeeper 경고가 뜰 수 있습니다.
> `xattr -cr /Applications/tunaFlow.app` 으로 해제합니다.

---

## 기술 스택

Tauri 2 + React 18 + TypeScript + Zustand 5 + Tailwind CSS 4 + Rust + SQLite (WAL, v46)

코드 검색: rawq sidecar (bge-m3 임베딩) · code-review-graph · context-hub
외부 연동: HTTP API + WebSocket · MCP 서버 (`tunaflow-mcp`)

---

## 문서

| 문서 | 내용 |
|------|------|
| [CLAUDE.md](./CLAUDE.md) | 아키텍처, 컨벤션, 핸드오프 |
| [Architecture Detail](./docs/reference/architecture-detail.md) | RT 흐름, Store 구조, DB 스키마 |
| [Implementation Status](./docs/reference/implementationStatus.md) | 기능별 구현 현황 |
| [Beta Release Plan](./docs/plans/betaReleaseReadinessPlan.md) | 배포 준비 체크리스트 |
| [Dev History](./docs/reference/devHistory.md) | 프로젝트 계보 + 개발 이력 |
| [Session History](./docs/reference/sessionHistory.md) | 세션별 상세 이력 (최근 설계 결정 추적용) |

### 📖 개발기 — 10부작 기술 시리즈

tunaFlow 를 만들면서 Claude Opus 가 쓴 개발기입니다. 설계 결정 / 트레이드오프 / 실제로 부서진 것들을 1인칭으로 솔직하게 기록.

- **[tunaFlow Wiki](https://github.com/hang-in/tunaFlow/wiki)** — 본편 10편 + side 시리즈

본편 목차:

1. 에이전트에게 프로세스를 줘라 — 왜 오케스트레이션 레이어가 필요한가
2. Plan → Dev → Review 워크플로우 파이프라인 구현기
3. 대화를 분기한다 — Branch 설계와 활용
4. 에이전트끼리 토론시키기 — Roundtable 설계와 한계
5. 대화가 길어지면 — 에이전트 장기 메모리 구현기
6. Claude $20 으로 워크플로우 돌리기 — 엔진 아키텍처
7. 코드 구조를 에이전트에게 알려주기 — rawq + code-review-graph
8. 246 개 스킬 중 필요한 것만 — 스킬 자동 적용 구현기
9. 에이전트가 같은 실수를 반복하면 — 품질 보증 설계
10. tunaFlow 로 풀사이클 돌려보기 — 워크플로우 실전 테스트 회고

---

## 알려진 제약 (Beta)

### 해결 예정 (P0 / P1)

- **PTY 터미널 — 작업 중** — 인앱 터미널 패널은 Beta 번들에서 일시적으로 비활성화되어 재구성 중입니다. 후속 릴리즈에서 복원되기 전까지는 외부 터미널 (iTerm2 / Terminal.app / Warp) 을 병행 사용하세요.
- **JSONL 완료 감지 실패 (P1)** — PTY 세션에서 응답이 UI 에 반영되지 않는 경우 간헐적 발생 (sdk-session WebSocket 경로로 이동 중).
- **Windows / Linux 빌드** — 미지원. 패키징 파이프라인 준비 중.

### 설계상 / Beta 단계

- **ad-hoc 서명** — Beta 에서는 Apple Developer ID 서명 없음. Gatekeeper 경고 해제 필요 (`xattr -cr /Applications/tunaFlow.app`). DMG 를 drag-install 만 하면 `.app` 에 quarantine 속성이 붙어 번들 안 사이드카 (rawq) 가 조용히 차단됩니다 — 증상/원인 표는 [INSTALL.md → "rawq 가 인식 안 될 때"](./INSTALL.md#rawq-가-인식-안-될-때-footer-rawq-sidecar-없음) 참조.
- **rawq 는 번들 sidecar 전용 (PATH 가 아님)** — tunaFlow 는 로컬 패치된 rawq 빌드를 `.app` 번들 안 (`Contents/MacOS/rawq`) 에 포함하고, 그 경로에서만 호출합니다. `cargo install rawq` 로 시스템 PATH 에 깔아도 tunaFlow 동작에는 영향 없습니다 (의도적 — 버전 드리프트 방지). 직접 빌드 시 `./scripts/build.sh` (사이드카 자동 빌드 wrapper) 사용 권장. `npm run tauri build` 직접 실행 시에는 `./scripts/build-rawq.sh` 가 사전 필요하며 (`binaries/rawq-aarch64-apple-darwin doesn't exist` 에러 회피), upstream 은 https://github.com/auyelbekov/rawq.
- **RT 라운드 중간 개입 어려움** — 참가자 별 토큰 스트리밍 자체는 실시간으로 나오지만, 라운드가 진행 중일 때 사용자가 방향을 틀기는 어렵습니다. 라운드 사이에 피드백을 주는 방식으로 운영합니다.
- **최초 인덱싱 지연** — 대규모 프로젝트 최초 1회 수 분 소요 (ONNX 스레드 제한 + 세마포어 + 점진적 인덱싱 적용 후 CPU 스파이크는 완화됨).

자세한 목록: [CLAUDE.md §5](./CLAUDE.md)

---

## 도움말 / 단축키

앱 내부 `Settings > Help` 패널에 주요 단축키, 기능 요약, 문제 해결 팁이 정리되어 있습니다.

---

## tunaFlow 로 만든 프로젝트

tunaFlow 의 멀티 에이전트 오케스트레이션 워크플로우로 개발한 프로젝트:

- **[secall](https://github.com/hang-in/secall)** — AI 대화 전체를 하이브리드 검색으로 뒤지는 "second brain". Andrej Karpathy 의 LLM wiki 개념을 한국어·일본어·중국어 환경에 맞춰 변형한 프로젝트입니다.

---

## References & Acknowledgments

tunaFlow 는 여러 오픈소스 프로젝트의 아이디어와 코드를 참고했습니다. 각 메인테이너에게 감사드립니다.

### 번들 사이드카 (앱과 함께 배포)

- **[rawq](https://github.com/auyelbekov/rawq)** (MIT) — 코드 검색 사이드카. 로컬 패치를 얹은 빌드를 번들로 포함합니다 ([upstream 에 clamp 패치 PR #11 제출](https://github.com/auyelbekov/rawq/pull/11)).
- **[code-review-graph](https://github.com/tirth8205/code-review-graph)** (MIT) — CRG 사이드카 (Full 트랙). 그래프 기반 코드 리뷰 분석.
- **[context-hub](https://github.com/andrewyng/context-hub)** (MIT) — 컨텍스트 공유 사이드카. 첫 실행 시 자동 설치됩니다.

### 설계 / 아키텍처 영향

- **[abtop](https://github.com/graykode/abtop)** (MIT) — AI 코딩 에이전트의 런타임 관측성 / 진단. Trace 패널과 상태바 디자인에 영향.
- **[hermes-agent](https://github.com/NousResearch/hermes-agent)** (MIT) — memory / toolset / iteration-budget 패턴.
- **[larksuite-cli](https://github.com/larksuite/cli)** (MIT) — CLI action layering / shared-rule / async-contract 패턴.
- **[chops](https://github.com/Shpigford/chops)** (MIT) — ContextPack code-slice 주입 아이디어.
- **[codex](https://github.com/openai/codex)** (Apache 2.0) — CLI 에이전트 프로토콜 참조 구현.
- **[xterm.js](https://xtermjs.org/)** (MIT) — PTY 패널 터미널 렌더링.
- **[react-markdown](https://github.com/remarkjs/react-markdown)** (MIT) — 채팅 마크다운 렌더링.
- **[D2Coding](https://github.com/naver/d2codingfont)** (OFL 1.1) — 번들된 고정폭 폰트.
- **[Tauri](https://tauri.app/)** (MIT / Apache 2.0) — 데스크탑 셸 프레임워크.

전체 참고 프로젝트 25+ 개 목록은 **[ACKNOWLEDGMENTS.md](./ACKNOWLEDGMENTS.md)** 에서 확인할 수 있습니다. 제3자 라이선스 표기 전문은 [NOTICE](./NOTICE) 참조.

### 철학 / 아티클

- **[Code Agent Orchestra](https://addyosmani.com/blog/code-agent-orchestra/)** — Addy Osmani. tunaFlow 의 멀티 에이전트 오케스트레이션 철학에 영향을 줬습니다.
- **[How I write software with LLMs](https://www.stavros.io/posts/how-i-write-software-with-llms/)** — Stavros Korokithakis. `Plan → Dev → Review` 파이프라인의 출발점.

---

## 연락처

- Email: d9ng@outlook.com
- Issues: https://github.com/hang-in/tunaFlow/issues
- Security: [SECURITY.md](./SECURITY.md) 참조

---

*100% AI-authored codebase — Claude Code 가 모든 줄을 썼고, 사람은 아키텍처와 방향만 정했습니다.*

---
🇺🇸 [English](./README.md) · 🇰🇷 한국어
