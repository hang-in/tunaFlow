---
title: tunaFlow Beta — Announcement Draft
updated_at: 2026-04-20
canonical: true
status: draft
owner: tunaFlow-core
---

# tunaFlow Beta 공개 — 공지 초안

> GitHub Release, 블로그 포스트, 트위터/X 스레드에 재활용할 수 있도록 세 가지 톤으로 준비했습니다. 배포 직전에 버전/날짜를 확정하세요.

---

## A. GitHub Release 본문

```markdown
# tunaFlow v0.1.0-beta

> **Of the agent, By the agent, For the agent**
> 도메인 전문가가 여러 AI 에이전트를 하나의 작업 흐름 안에서 운영하기 위한 데스크톱 클라이언트

## 이런 분들께

- Claude Code / Codex / Gemini CLI 를 쓰지만 **대화 이상의 구조**가 필요한 개발자
- 에이전트에게 실행은 맡기되 **방향과 판단은 직접** 유지하고 싶은 사람
- AI 에이전트를 일상 워크플로우에 넣으려는 소규모 팀 또는 개인

## 핵심 기능

- **Plan → Develop → Review** 3-role 워크플로우 (Architect / Developer / Reviewer)
- **Branch / Roundtable** 대화 분기 + 다엔진 토론
- **ContextPack** 4개 엔진 공통 프롬프트 (Lite/Standard/Full 자동 Tiering)
- **Insight** 프로젝트 품질 분석 6개 카테고리
- **PTY Terminal** CLI 에이전트 인터랙티브 세션
- **모바일 클라이언트** HTTP + WS + 재연결 복구

## 설치 (macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash
xattr -cr /Applications/tunaFlow.app   # Gatekeeper 해제
```

## 알려진 제약

- macOS 전용 (Windows/Linux 추후)
- ad-hoc 서명 (Developer ID 인증서 취득 진행 중)
- RT 중간 스트리밍 미지원
- JSONL 완료 감지 실패 간헐 발생

전체 목록: [docs/beta/known-issues.md](./docs/beta/known-issues.md)

## 피드백

- **Bug**: https://github.com/hang-in/tunaFlow/issues
- **Email**: d9ng@outlook.com
- **크래시 리포트**: Settings > Help 패널에서 바로 확인

## 감사

tunaFlow 는 **100% AI 작성** 프로젝트입니다. Claude Code 가 코드를 쓰고, 사람은 방향만 결정했습니다. 같은 방식으로 일하는 모두에게 도움이 되길 바랍니다.

---

**Changelog**: [release-notes.md](./docs/beta/release-notes.md)
**Roadmap**: [refactorRoadmap_2026-04-20.md](./docs/plans/refactorRoadmap_2026-04-20.md)
```

---

## B. 블로그 포스트 초안

### 제목 후보

1. **tunaFlow 베타 — AI 에이전트 오케스트레이션 클라이언트**
2. **에이전트가 편해야 결과가 좋다 — tunaFlow 베타 공개**
3. **Claude Code, Codex, Gemini 를 한 흐름 안에 — tunaFlow 베타**

### 본문

**왜 만들었나**

Claude Code, Codex, Gemini CLI 를 매일 쓴다. 각자 강점이 다르고, 복잡한 작업 하나를 맡기려면 여러 에이전트를 번갈아 부르거나 교차 검증을 시켜야 한다. 그런데 터미널만 보고 있으면:

- 대화가 일자 흐름이라 실험과 본줄기가 섞이고
- 에이전트가 설계한 Plan 을 다른 에이전트가 검증하게 하기 어렵고
- 내가 원하는 도메인 컨텍스트를 매번 다시 설명하게 된다

tunaFlow 는 이 세 가지를 해결한다.

**에이전트가 편해야 결과가 좋다**

설계 원칙은 "agent-first". 에이전트에게 매번 필요한 컨텍스트를 토큰 효율적으로, 역할 혼동 없이, 정확한 형식으로 전달하는 데 집중했다.

- **ContextPack** 이 매 요청마다 프로젝트 문서 / 기억 / 도구 결과를 조립한다. Lite/Standard/Full 자동 Tiering 으로 토큰을 낭비하지 않는다
- **3-Role Workflow** — Architect 가 Plan 을 설계하고, Developer 가 구현하고, Reviewer 가 검증한다. 실패하면 findings 로 rev.N+1 Plan 을 자동 제안한다
- **Branch 와 Roundtable** — 본줄기 대화에서 분기해 독립 실험을 하고, 결과만 요약으로 삽입한다. RT 는 여러 엔진이 같은 주제로 토론한다

**이번 베타는**

지난 40여 번의 세션 동안 누적된 기능을 정리하고, 베타 공개를 앞두고 5-Phase 리팩토링을 거쳐 **테스트 basline 을 Rust 305 + Frontend 293 까지 올렸다**. API 는 `/api/v1/` 로 버저닝했고, WebSocket 은 재연결 시 missed event 를 replay 한다.

**100% AI 가 썼다**

이 앱의 코드는 전부 Claude Code 가 작성했다. 사람은 "무엇을 만들지 / 어떤 방향으로 갈지" 만 정했다. 리팩토링 라운드에서 잘못된 가정을 지적해준 것도, 베타 직전에 dead code 의 실제 원인을 재해석한 것도 에이전트였다.

**써보기**

```bash
curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash
```

macOS 전용, ad-hoc 서명이라 Gatekeeper 경고가 뜬다. `xattr -cr /Applications/tunaFlow.app` 로 해제.

**피드백은 GitHub Issues 로**. 앱 내부 Settings > Help 에서 단축키/기능/문제 해결을 확인할 수 있고, 크래시가 나면 같은 페이지에 리포트 파일 위치가 표시된다.

---

## C. 트위터/X 스레드 (짧은 버전)

### 버전 1 — 기술 중심

1/ tunaFlow 베타 공개 🎉
macOS 데스크탑 앱. Claude Code, Codex, Gemini CLI 를 하나의 작업 흐름 안에서 오케스트레이션.
100% AI 가 쓴 코드.

2/ 핵심은 "agent-first"
- ContextPack: 매 요청마다 프로젝트 컨텍스트를 토큰 효율적으로 조립
- 3-Role Workflow: Architect 가 Plan 설계 → Developer 구현 → Reviewer 검증
- Branch / Roundtable: 대화 분기 + 다엔진 토론

3/ 수치
- Rust 305 tests
- Frontend 293 tests
- WAL SQLite v41
- 4-engine parity (Claude/Codex/Gemini/Ollama)

4/ 설치 (macOS)
```
curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash
xattr -cr /Applications/tunaFlow.app
```

5/ 공개 채널
- GitHub: https://github.com/hang-in/tunaFlow
- Issues & 피드백 환영

### 버전 2 — 철학 중심

1/ "에이전트가 편해야 결과가 좋다"
tunaFlow 베타를 공개합니다. AI 에이전트 오케스트레이션 데스크탑 클라이언트.

2/ 설계 원칙은 한 줄:
에이전트가 불필요한 토큰 낭비 없이, 정확한 맥락으로, 역할 혼동 없이 작업할 수 있는가.

모든 기능은 이 기준으로 판단했습니다.

3/ 결과물:
- 3-role workflow (Architect/Dev/Reviewer)
- ContextPack 자동 조립
- Branch + Roundtable
- 4-engine parity

4/ 그리고 이 앱 자체의 코드도 100% AI 가 썼습니다. 베타까지 이끈 것은 agent-first 철학과, 리팩토링 라운드에서 잘못된 가정을 지적해준 리뷰어 에이전트들입니다.

5/ macOS, ad-hoc 서명:
https://github.com/hang-in/tunaFlow

---

## 배포 전 체크리스트

- [ ] 버전 번호 확정 (`v0.1.0-beta` 또는 RC 표기?)
- [ ] `install.sh` 가 최신 release 를 가리키는지 확인
- [ ] GitHub Release 태그 생성
- [ ] release-notes.md / known-issues.md 문서가 main 에 머지됐는지
- [ ] README 의 설치 섹션이 최신 경로와 일치
- [ ] 공개 후 피드백 수집 채널 준비 (Issue template?)
