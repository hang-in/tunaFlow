# 온보딩 + RT 메타에이전트 — 설치부터 프로젝트 설정까지

> Status: idea
> Created: 2026-04-10
> 원칙: "복잡한 설정은 에이전트가 대행, 판단은 Human"

---

## 1. 전체 온보딩 흐름

```
첫 설치 (1회)
  ↓
tunaFlow 앱 실행
  ↓
초기 설정 (에이전트 또는 수동)
  ↓
프로젝트 폴더 선택
  ↓
프로젝트 자동 분석 + 설정 제안
  ↓
사용 시작
```

---

## 2. 첫 설치 — 에이전트 기반 + 전통 방식 병행

### 2.1 에이전트 기반 설치 (권장)

**INSTALL.md 한 장을 읽고 에이전트가 알아서 설치해주는 방식.**

사용자가 Claude Code(또는 다른 코딩 에이전트)에게:
```
"이 INSTALL.md 읽고 tunaFlow 설치해줘"
```

INSTALL.md 내용:
```markdown
# tunaFlow 설치 가이드 (에이전트용)

## 전제조건 확인
1. Node.js 22+ (`node --version`)
2. Rust toolchain (`rustc --version`, 없으면 `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
3. macOS: Xcode CLI tools (`xcode-select --install`)

## 설치 순서
1. `git clone https://github.com/user/tunaFlow.git && cd tunaFlow`
2. `npm install`
3. rawq sidecar 빌드: `./scripts/build-rawq.sh`
4. 개발 실행: `npm run tauri dev`

## 검증
- `npm run tauri dev` 실행 후 앱 창이 뜨면 성공
- 에러 시: 아래 트러블슈팅 참조

## 트러블슈팅
| 에러 | 원인 | 해결 |
|------|------|------|
| `cargo: command not found` | Rust 미설치 | `curl ...rustup...` |
| `failed to bundle sidecar` | rawq 미빌드 | `./scripts/build-rawq.sh` |
| `node: command not found` | Node.js 미설치 | `brew install node` 또는 `fnm install 22` |
| `xcrun: error` | Xcode CLI 미설치 | `xcode-select --install` |
| `linking with cc failed` | macOS SDK 경로 문제 | Xcode 업데이트 후 재시도 |

## 의존성 목록
- Node.js 22+
- Rust (stable)
- Tauri CLI (`npm install` 시 자동)
- rawq sidecar (`./scripts/build-rawq.sh`)
```

**에이전트가 하는 것**:
1. INSTALL.md 읽기
2. 전제조건 확인 (`node --version`, `rustc --version`)
3. 없는 것 설치 (사용자 승인 후)
4. `npm install` + `./scripts/build-rawq.sh`
5. `npm run tauri dev`로 검증
6. 에러 시 트러블슈팅 테이블 참조하여 자동 해결

**에이전트가 못 하는 것 (Human 필요)**:
- Xcode 라이선스 동의 (GUI 필요)
- macOS 보안 설정 (시스템 환경설정)
- 특이한 환경 문제 (회사 프록시, VPN 등)
- **CLI 에이전트 최초 로그인** (아래 §2.3 참조)

### 2.2 전통적 수동 설치

INSTALL.md 동일 내용을 사람이 직접 따라함. 에이전트 없이도 가능.

```bash
# 1. 전제조건
node --version    # 22+
rustc --version   # stable

# 2. 클론 + 설치
git clone https://github.com/user/tunaFlow.git
cd tunaFlow
npm install

# 3. rawq sidecar
./scripts/build-rawq.sh

# 4. 실행
npm run tauri dev
```

### 2.3 CLI 에이전트 최초 로그인 (필수, Human만 가능)

**tunaFlow가 에이전트를 실행하려면 각 CLI가 터미널에서 최소 1회 로그인되어 있어야 합니다.** PTY 모드든 `-p` 모드든 동일.

| 엔진 | 최초 로그인 명령 | 인증 방식 | 상태 |
|------|----------------|----------|------|
| Claude Code | `claude` → 브라우저 OAuth | Anthropic 계정 | ✅ 지원 중 |
| Codex | `codex` → 브라우저 OAuth | OpenAI 계정 | ✅ 지원 중 |
| Gemini | `gemini` → Google OAuth | Google 계정 | ✅ 지원 중 |
| OpenCode | `opencode` → 설정 | 설정 파일 | ✅ 지원 중 |

**OpenAI 호환 API 모델 (현재 지원 가능)**:

`openai_compat.rs`의 `base_url` 교체로 이미 연결 가능하지만, **순수 채팅/추론만** 가능. Claude Code/Codex처럼 파일 편집, 터미널 실행 등 에이전트 기능 없음.

| 모델 | API base_url | 용도 |
|------|-------------|------|
| GLM (zhipu) | `https://open.bigmodel.cn/api/paas/v4` | 채팅, RT 토론 참가자 |
| DeepSeek | `https://api.deepseek.com/v1` | 채팅, RT 토론 참가자 |
| Kimi (Moonshot) | `https://api.moonshot.cn/v1` | 채팅, RT 토론 참가자 |
| Qwen (DashScope) | `https://dashscope.aliyuncs.com/compatible-mode/v1` | 채팅, RT 토론 참가자 |

→ **Developer로는 부적합** (파일 편집/터미널 실행 불가).
→ **Reviewer로는 사용 가능** — 코드 읽기는 ContextPack이 주입, 판정만 출력하면 되므로 API 채팅만으로 충분.
→ **RT 참가자, 일반 채팅**으로도 활용 가능.
→ API 키 필요 (각 서비스 가입 후 발급).
→ 온보딩 시 "추가 모델" 섹션으로 안내 (필수 아님).

**향후 지원 예정**: 새 터미널 에이전트 CLI (파일 편집 + 터미널 실행 가능)가 등장하면 동일 패턴으로 추가. tunaFlow의 엔진 아키텍처(`ENGINE_CONFIGS` + `openai_compat.rs`)가 plug-in 방식으로 지원하도록 설계되어 있음. 새 CLI 에이전트 추가 시 필요한 것:
- `agents/{engine}.rs` — CLI 래퍼 (~100줄)
- `ENGINE_CONFIGS`에 매핑 추가 (~10줄)
- 모델 디스커버리 (~30줄)
- 온보딩 로그인 안내에 항목 추가

**이건 tunaFlow가 대신할 수 없습니다.** 각 CLI가 브라우저 기반 OAuth를 요구하므로 사용자가 직접 해야 합니다.

**온보딩 Welcome 화면에서 안내**:

```
┌── 시작하기 전에 ─────────────────────────────────┐
│                                                  │
│ tunaFlow는 CLI 에이전트를 통해 작동합니다.         │
│ 사용할 엔진을 터미널에서 한 번 로그인해주세요:      │
│                                                  │
│  Claude Code:                                    │
│    터미널에서 `claude` 실행 → 브라우저 로그인      │
│    ✅ 로그인됨 / ❌ 미로그인                       │
│                                                  │
│  Codex:                                          │
│    터미널에서 `codex` 실행 → 브라우저 로그인       │
│    ✅ 로그인됨 / ❌ 미감지                         │
│                                                  │
│  Gemini:                                         │
│    터미널에서 `gemini` 실행 → Google 로그인        │
│    ☐ 미설치                                      │
│                                                  │
│ 최소 1개 엔진이 로그인되어 있으면 시작할 수 있습니다 │
│                                                  │
│               [로그인 확인 완료]                    │
└──────────────────────────────────────────────────┘
```

**로그인 상태 감지** (자동 체크 가능):

```rust
// Claude: ~/.claude/ 디렉토리 + 세션 파일 존재 여부
// Codex: ~/.codex/ 또는 OpenAI 토큰 파일 존재
// Gemini: ~/.config/gemini/ 또는 gcloud 인증 상태
// 각 CLI에 --version 또는 상태 확인 명령 실행으로 판단
```

정확한 감지가 어려우면 **사용자에게 체크박스로 확인**:
```
☑ Claude Code 로그인 완료
☑ Codex 로그인 완료
☐ Gemini (선택사항)
```

### 2.4 설치 실패 대응

```
에이전트 설치 시도
  ↓
성공 → 프로젝트 선택 화면으로
  ↓
실패 → 에러 분류:
  ├── 알려진 에러 (트러블슈팅 테이블에 있음) → 자동 해결 시도
  ├── 네트워크 에러 (npm install 실패) → "네트워크 확인 후 재시도" 안내
  ├── 권한 에러 (sudo 필요) → 사용자에게 권한 요청
  └── 미지 에러 → 에러 로그 + "수동 설치를 시도해주세요" + INSTALL.md 링크
```

---

## 3. 첫 실행 — 프로젝트 선택까지의 UX

### 3.1 Welcome 화면

```
┌────────────────────────────────────────────────────┐
│                                                    │
│              🐟 tunaFlow에 오신 것을 환영합니다      │
│                                                    │
│    에이전트와 함께 소프트웨어를 만드는 새로운 방법     │
│                                                    │
│                                                    │
│    ┌──────────────────────────────────────────┐    │
│    │  📂 프로젝트 폴더 선택                    │    │
│    │                                          │    │
│    │  프로젝트 폴더를 선택하면                  │    │
│    │  tunaFlow가 자동으로 분석하고              │    │
│    │  최적의 설정을 제안합니다.                  │    │
│    │                                          │    │
│    │       [폴더 선택하기]                     │    │
│    │                                          │    │
│    └──────────────────────────────────────────┘    │
│                                                    │
│    최근 프로젝트:                                   │
│    (없음 — 첫 실행입니다)                           │
│                                                    │
└────────────────────────────────────────────────────┘
```

### 3.2 프로젝트 선택 후 → 자동 분석 → 설정 제안

```
프로젝트 폴더 선택 (/Users/user/myProject)
  ↓
[자동, 사용자 대기 없음]
  ├── rawq 인덱싱 시작 (백그라운드)
  ├── code-review-graph 빌드 (백그라운드)
  ├── detect_project_stack() → 기술 스택
  ├── git branch/status 확인
  └── CLAUDE.md 존재 여부 확인
  ↓
[경량 LLM — 소넷 1회 호출, ~$0.01]
  입력: 기술 스택 + 파일 구조 + rawq map 요약
  출력: 추천 설정 JSON
  ↓
[설정 제안 화면]
┌────────────────────────────────────────────────────┐
│ 🔍 프로젝트 분석 완료                               │
│                                                    │
│ 📁 myProject                                      │
│ 감지: React + TypeScript + Zustand + Tailwind      │
│ 규모: 120 파일, 8,500줄                            │
│ 테스트: vitest ✅                                   │
│ Git: main 브랜치, clean                            │
│                                                    │
│ ── 추천 설정 ──────────────────────────────────    │
│                                                    │
│ 기본 엔진:   [Claude ▾]                            │
│ Persona:     [Senior Developer ▾]                  │
│ 스킬:        ☑ Frontend  ☑ Testing  ☐ API/SDK     │
│ 컨텍스트:    [Auto (Standard) ▾]                   │
│                                                    │
│ CLAUDE.md:   ☑ 자동 생성 (프로젝트 규칙 포함)       │
│                                                    │
│      [적용하고 시작] [수정 후 적용] [건너뛰기]       │
└────────────────────────────────────────────────────┘
```

### 3.3 에이전트 역할 (소넷 vs 하이쿠)

| 작업 | 하이쿠 | 소넷 | 선택 |
|------|--------|------|------|
| 기술 스택 분류 | ✅ 충분 | 과도 | 하이쿠 |
| 프로젝트 규모/복잡도 판단 | ✅ 충분 | 과도 | 하이쿠 |
| 추천 설정 JSON 생성 | ⚠️ 가능 | ✅ 적합 | 소넷 |
| CLAUDE.md 초안 작성 | ❌ 품질 부족 | ✅ 적합 | 소넷 |

**현실적 선택**: 소넷 1회 호출로 분류 + 설정 + CLAUDE.md 초안을 한번에 처리. 비용 ~$0.01. 하이쿠로 분류만 따로 하면 2회 호출이라 오히려 비효율.

### 3.4 엔진 추천 전략

**최소 요구사항**: Claude Code 또는 Codex **둘 중 하나**. 어느 쪽이든 단독으로 전체 워크플로우(Architect → Developer → Reviewer) 동작 가능.

**추천 조합 (설정 제안 시 기본값)**:

| 구성 | 엔진 | 역할 | 비용 예시 |
|------|------|------|----------|
| **입문 (1-engine)** | Claude (Pro $20) | 소넷으로 전체 (Architect/Dev/Reviewer) | $20/월 |
| **기본 A (1-engine)** | Claude (Max $100+) | 오퍼스+소넷 혼용 | $100/월 |
| **기본 B (1-engine)** | Codex (Pro) | 전체 | Codex Pro |
| **추천 (2-engine)** | Claude + Codex | Claude: 설계/리뷰, Codex: 구현 | Max $100 + Codex Pro |
| **추천 (2-engine)** | Codex + Claude | Codex: 구현/메인, Claude: 리뷰/RT | Codex Pro + Claude $20 |
| **추천 (2-engine)** | Claude + Gemini | Claude: 설계/구현, Gemini: RT/검증 | Max $100 + Gemini 무료 |
| **풀셋 (3-engine)** | Claude + Codex + Gemini | 역할 분리 최적 | 전부 |

**핵심**:
- **Claude $20 Pro 하나로도 시작 가능** — 소넷으로 Architect/Developer/Reviewer 전부 동작. 사용량 제한이 있으므로 대규모 워크플로우에는 Max 권장.
- 어떤 엔진이든 **하나만 있으면 시작 가능**. 두 번째 엔진은 역할 분리로 품질 향상.
- 같은 엔진(Claude)으로 3역할을 돌려도, `--model` 플래그로 모델은 지정 가능 (소넷/하이쿠/오퍼스).

**단일 엔진 3역할 구성 (Claude Pro $20 예시)**:

```
Architect: claude --model claude-sonnet-4-6     → Plan 설계
Developer: claude --model claude-sonnet-4-6     → 코드 구현
Reviewer:  claude --model claude-sonnet-4-6     → 코드 리뷰 판정

→ 전부 같은 claude CLI, 같은 소넷 모델
→ ContextPack + Persona + PLATFORM_TIER0가 역할을 분리
→ 모델이 같아도 역할별 프롬프트가 다르므로 행동이 다름
```

**Pro 플랜 주의사항**: 시간당 사용량 제한 있음. 워크플로우 풀사이클(Plan→Dev→Review) 1회에 ~3-5회 에이전트 호출. 빈번한 Rework 시 제한에 걸릴 수 있으므로 Max 플랜 또는 2-engine 구성 권장.

**온보딩 설정 제안 시 로직**:

```
사용자 환경 감지:
  claude CLI 있음? → 기본 엔진 후보에 Claude 추가
  codex CLI 있음?  → 기본 엔진 후보에 Codex 추가
  gemini CLI 있음? → 보조 엔진 후보에 Gemini 추가 (무료)
  
엔진 1개만 감지:
  → 해당 엔진을 기본으로 전체 역할 배정
  → "다른 엔진을 추가하면 역할 분리로 품질이 개선됩니다" 안내

엔진 2개 이상 감지:
  → 자동으로 역할 배정 추천 (설계/구현/리뷰 분리)

엔진 0개:
  → "Claude Code 또는 Codex를 먼저 설치해주세요" + 설치 링크
```

**설정 제안 화면에서**:

```
┌── 엔진 설정 ─────────────────────────────────┐
│                                              │
│ 감지된 엔진:                                  │
│  ✅ Codex (Pro)                              │
│  ✅ Claude Code ($20 플랜)                    │
│  ☐ Gemini (미설치 — `npm i -g @google/gemini-cli`) │
│                                              │
│ 추천 구성: Codex (구현) + Claude (설계/리뷰)  │
│                                              │
│ 워크플로우 역할 배정:                         │
│  Architect: [Claude ▾]                       │
│  Developer: [Codex ▾]                        │
│  Reviewer:  [Claude ▾]                       │
│                                              │
│ 💡 Gemini를 추가하면 RT 토론에 활용할 수 있습니다 │
│                                              │
└──────────────────────────────────────────────┘
```

### 3.4 CLAUDE.md 자동 생성

현재 `scaffold_project_dir()`가 기본 CLAUDE.md를 생성하지만 내용이 범용적. 소넷이 프로젝트를 분석한 결과로 **프로젝트 맞춤 CLAUDE.md** 생성:

```markdown
# myProject — Claude Code Handoff Document

## 프로젝트 개요
React + TypeScript + Zustand + Tailwind CSS 기반 웹 앱.
vitest로 테스트, Vite로 빌드.

## 기술 스택
| 계층 | 기술 |
|------|------|
| Frontend | React 18, TypeScript |
| 상태 관리 | Zustand 5 |
| 스타일 | Tailwind CSS 4 |
| 테스트 | vitest |
| 빌드 | Vite 6 |

## 빌드/테스트
```bash
npm run dev           # 개발
npm run build         # 빌드
npx vitest run        # 테스트
npx tsc --noEmit      # 타입 체크
```

## 코딩 컨벤션
- Zustand selector: 개별 필드 선택 (broad store 사용 금지)
- 한국어 응답, 코드/경로/식별자는 원문
```

사용자가 [적용하고 시작]을 누르면 이 CLAUDE.md가 프로젝트 루트에 생성됨.

---

## 4. RT 라운드 판정 — Human 제어 게이트

### 4.1 현재 문제

```
RT 시작 → Round 1 자동 → Round 2 자동 → ... → 전부 끝남
→ 사용자가 "충분한가?" 판단할 타이밍 없음
→ 불필요한 라운드에 토큰 낭비
```

### 4.2 제안: 라운드 완료 시 일시정지 + 판정 버튼

```
Round 1 실행
  ↓ 모든 참가자 응답 완료
  ↓
[자동 일시정지]
  ↓
드로워 하단에 판정 UI 표시:
┌──────────────────────────────────────────┐
│ Round 1 완료 (Agent A: Claude, Agent B: Gemini) │
│                                          │
│ [📋 정리 요청]  [🔄 다음 라운드]  [✅ 종료] │
│                                          │
│  💬 추가 지시: [________________] [전송]   │
└──────────────────────────────────────────┘
```

### 4.3 각 버튼의 동작

**📋 정리 요청**

```
현재 RT 대화 전체를 선택된 엔진에 전달:
  프롬프트: "지금까지 논의를 정리해주세요:
    1. 합의된 사항
    2. 대립된 의견 (각 입장 요약)
    3. 미결정 사항
    4. 추천 결론"
  ↓
정리 결과가 RT 채팅에 새 메시지로 표시 (별도 "정리" 라벨)
  ↓
다시 판정 버튼: [🔄 더 논의] [✅ 이 결론으로 종료]
```

**🔄 다음 라운드**

```
사용자가 선택적으로 추가 지시 입력 가능:
  예: "보안 관점에서 더 논의해줘"
  예: "A의 의견 방향으로 구체화해줘"
  ↓
다음 라운드 실행 (추가 지시가 있으면 라운드 프롬프트에 포함)
  ↓
완료 후 다시 일시정지 + 판정 버튼
```

**✅ 종료**

```
RT 완료 처리
  ↓
선택: [요약을 메인 채팅에 삽입] [Artifact로 저장] [그냥 닫기]
```

### 4.4 Synthesizer 대신 온디맨드 정리

| | 상시 Synthesizer | Human 판정 + 온디맨드 정리 |
|---|---|---|
| **토큰** | 매 라운드마다 요약 호출 | 사용자가 요청할 때만 |
| **품질** | 자동 판단 → 중요 포인트 누락 가능 | Human이 "충분한가" 판단 |
| **UX** | 자동이라 편하지만 제어감 없음 | 버튼 클릭 → 제어감 |
| **비용** | 높음 (라운드 × 요약) | 낮음 (요청 시에만) |
| **철학** | 에이전트가 판단 | Human이 판단 ✅ |

tunaFlow 철학 "Human with Agent"와 일치: **판정은 Human, 정리는 Agent.**

### 4.5 구현 위치

```
현재 흐름:
  sendThreadRoundtable() → start_roundtable_run
  → backend: execute_sequential/parallel() × total_rounds
  → 모든 라운드 자동 완료 → agent:completed

변경:
  sendThreadRoundtable() → start_roundtable_run
  → backend: execute_round() × 1 round만
  → roundtable:round_completed 이벤트 (새로 추가)
  → frontend: 판정 UI 표시 + 일시정지
  → 사용자 선택:
    "정리" → sendThreadMessage(정리 프롬프트) → 결과 표시 → 다시 판정
    "다음" → continue_roundtable_run(round + 1) (새 command)
    "종료" → complete_roundtable(conversation_id) (새 command)
```

**새 Tauri command**:
- `continue_roundtable_run(conversation_id, additional_prompt?)` — 다음 라운드 실행
- `complete_roundtable(conversation_id)` — RT 완료 처리

---

## 5. 구현 계획

### Phase 1: 온보딩 자동 분석 + 설정 제안

```
5-1. INSTALL.md 작성 (에이전트 설치용)
5-2. Welcome 화면 (첫 실행 감지)
5-3. 프로젝트 분석 파이프라인:
     detect_project_stack() + rawq map + git status
5-4. 소넷 1회 호출 → 추천 설정 JSON 생성
5-5. 설정 제안 화면 UI (적용/수정/건너뛰기)
5-6. CLAUDE.md 자동 생성 (프로젝트 맞춤)
```

### Phase 2: RT 라운드 판정

```
5-7. roundtable:round_completed 이벤트 추가
5-8. RT 드로워 하단 판정 UI (정리/계속/종료 버튼)
5-9. continue_roundtable_run Tauri command
5-10. complete_roundtable Tauri command
5-11. "정리 요청" → 구조화 요약 프롬프트
5-12. "종료" → 메인 채팅 삽입 / Artifact 저장 선택
```

### Phase 3: 설치 에이전트 (선택적)

```
5-13. INSTALL.md 트러블슈팅 테이블 확장
5-14. 설치 실패 시 자동 해결 로직 (알려진 에러 패턴 매칭)
```

---

## 6. 변경 범위 예측

### Phase 1 (온보딩)

| 파일 | 변경 | 규모 |
|------|------|------|
| 새 파일: `INSTALL.md` | 에이전트/수동 설치 가이드 | ~100줄 MD |
| `ProjectStartup.tsx` (또는 새 Welcome 컴포넌트) | Welcome 화면 + 분석 UI | ~200줄 FE |
| `projectSlice.ts` | 분석 + 설정 제안 로직 | ~50줄 FE |
| `project_tools.rs` | 분석 결과 → 추천 설정 생성 | ~80줄 Rust |
| 기존 `scaffold_project_dir()` 확장 | 소넷 호출 → CLAUDE.md 맞춤 생성 | ~30줄 Rust |

### Phase 2 (RT 판정)

| 파일 | 변경 | 규모 |
|------|------|------|
| `roundtable.rs` | `continue_roundtable_run` + `complete_roundtable` | ~60줄 Rust |
| `executor.rs` | 1라운드만 실행 후 이벤트 발행 | ~20줄 Rust |
| `RoundtableView.tsx` (또는 드로워 하단) | 판정 UI (3버튼 + 추가 지시 입력) | ~80줄 FE |
| `branchSlice.ts` | RT 상태 관리 (paused/running/completed) | ~20줄 FE |
| `lib.rs` | 새 command 등록 | ~5줄 |

---

## 7. 에지 케이스

### 온보딩

| 케이스 | 대응 |
|--------|------|
| 프로젝트에 매니페스트 없음 (순수 스크립트) | 파일 확장자 기반 언어 감지 → 범용 설정 제안 |
| 모노레포 (여러 package.json) | 루트 매니페스트 우선, 서브 프로젝트 감지 알림 |
| 빈 폴더 | "새 프로젝트입니다. 기본 설정을 적용합니다" |
| CLAUDE.md가 이미 있음 | "기존 CLAUDE.md를 유지합니다" (덮어쓰기 안 함) |
| rawq 빌드 안 됨 | rawq 없이도 기본 분석 가능 (detect_project_stack만) |
| 소넷 호출 실패 (API 에러) | detect_project_stack 기반 규칙 매핑으로 fallback |

### RT 판정

| 케이스 | 대응 |
|--------|------|
| 1라운드 후 바로 종료 | 정상 — 간단한 질문은 1라운드 충분 |
| 10라운드 넘어감 | "10라운드가 지났습니다. 정리를 권장합니다" 경고 |
| 정리 요청 중 에이전트 에러 | "정리에 실패했습니다. 다시 시도하거나 종료해주세요" |
| 사용자가 판정 안 하고 앱 닫음 | RT 상태를 "paused"로 저장, 다음 열림 시 판정 UI 복원 |
| 추가 지시 없이 "다음 라운드" | 이전 라운드 맥락만으로 다음 라운드 실행 |

---

## 8. Auto Fix — 메타에이전트의 핵심 서브루틴

> **현재 상태 (2026-04-12)**: Insight 탭에 `fix_difficulty: auto` 분류는 존재하지만, Auto Fix 버튼은 비활성화. 메타에이전트 도입 후 구현 예정.

### 8.1 왜 메타에이전트가 필요한가

Auto Fix는 단순히 에이전트에게 "이 파일 고쳐줘"를 보내는 것이 아니다. **수정 → 검증 → 실패 처리** 전체 루프를 자율적으로 관리해야 한다.

```
[현재 없는 것 — 메타에이전트가 담당해야 할 루프]

Finding (auto)
  ↓
코딩 에이전트에게 수정 지시 (Developer 역할)
  ↓
수정 완료 응답
  ↓
[메타에이전트 판단]
  ├── run_project_tests → 실패 → 롤백 + guided로 격상 + Human 게이트
  ├── rawq 재스캔 → 패턴 여전히 존재 → 수정 불완전 → 재시도 or 격상
  └── 모두 통과 → finding resolved + commit
```

Human이 이 루프에 개입하면 "auto"의 의미가 없다. Human 없이 루프를 돌릴 수 있어야 하므로, 이 판단 로직을 가진 메타에이전트가 필요하다.

### 8.2 현재 임시 처리 — manual 전환

Auto Fix 대신 현재 흐름:

```
Insight finding (auto/guided/manual)
  ↓
"Architect에게 전달" → Architect Review Branch 생성
  ↓
Architect가 자율 판단:
  - 여러 auto finding → 하나의 Plan subtask로 묶거나 / Plan 없이 메모로 남기거나
  - guided/manual → Plan subtask로 승격
  ↓
Human Approval Gate → Developer 실행
```

작은 auto finding들은 Architect가 "한 번에 처리할 수 있는 잡무 묶음"으로 판단해서 하나의 subtask로 처리. Plan 하나에 subtask 1개가 되는 낭비를 Architect 판단으로 방지.

### 8.3 메타에이전트 도입 시 변경 범위

| 항목 | 내용 |
|---|---|
| 메타에이전트 진입 | `fix_difficulty: auto` findings 선택 → Auto Fix 버튼 활성화 |
| 에이전트 선택 | Developer 역할 + 코딩 엔진 (Claude/Codex 중 선택) |
| 루프 관리 | 수정 → 테스트 → rawq 검증 → 성공/실패 분기 |
| 실패 처리 | 자동 롤백 + `fix_difficulty: guided`로 격상 + Insight UI 업데이트 |
| 성공 처리 | `finding.status = resolved` + git commit 메시지 자동 생성 |
| Human 게이트 | 실패 시에만 개입 (성공 경로는 완전 자동) |

### 8.4 구현 우선순위

메타에이전트 도입 순서 (의존성 기준):

1. **RT 판정 게이트** (§4) — 라운드 완료 시 Human 제어. 가장 단순한 메타에이전트 패턴.
2. **온보딩 분석** (§3) — 프로젝트 분석 + 설정 제안. 단방향 분석이라 부작용 없음.
3. **Auto Fix 루프** (§8) — 수정 + 검증 + 롤백. 부작용(파일 변경) 있으므로 마지막.

---

## 참고

- 메타 에이전트 Tier 1: `docs/ideas/projectMetaAgentIdea.md`
- 스킬 자동 감지: `src-tauri/src/commands/skills.rs` (detect_project_stack)
- 스킬 추천 배너: `src/components/tunaflow/context-panel/SkillsPanel.tsx`
- 프로젝트 스캐폴드: `src-tauri/src/commands/projects.rs` (scaffold_project_dir)
- RT 실행: `src-tauri/src/commands/roundtable_helpers/executor.rs`
- RT 뷰: `src/components/tunaflow/RoundtableView.tsx`
- mex GROW 패턴: `docs/ideas/mexContextScaffoldIdea.md` §2.3
- Code Agent Orchestra: `docs/ideas/codeAgentOrchestraReferenceIdea.md` (Reflection Loop)
