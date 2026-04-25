# PTY 세션 정책 — 채팅=세션 1:1 매핑

> Status: archived (superseded by `docs/reference/branchSessionPolicy.md`)
> Created: 2026-04-11
> Archived: 2026-04-25 (s40, sdk-url WS 전환 후 일반화)
> Branch: `feature/pty-interactive`
> 연관: `ptyFullIntegrationPlan.md`, `ptyInteractiveIdea.md`
> Superseded by: `docs/reference/branchSessionPolicy.md` — PTY 의존 제거하고
> "interactive session backbone (PTY → sdk-url WS / codex app-server)" 로
> 일반화한 SSOT.

> **본 문서의 원래 의도 (`Branch | 부모 채팅의 PTY 세션 공유`, line 164) 는
> branchSessionPolicy.md 의 INV-1~INV-5 로 옮겨졌다.** PTY → WS 전환 후
> 6 세션 (s36~s40) 동안 의도가 task 파이프라인으로 surface 되지 못해 코드와
> divergence 가 발생했고, `docs/plans/branchInheritsMainSessionPlan_2026-04-25.md`
> 가 회복 작업을 추적한다.

---

## 핵심 원칙

1. **채팅 = Claude 세션 1:1** — tunaFlow의 Conversation 하나가 Claude Code의 Session 하나에 대응
2. **세션 재사용** — 앱 재시작 시 `--resume <sessionId>`로 기존 세션 이어감. 불필요한 세션 생성 없음
3. **`/clear`로만 리셋** — 사용자의 명시적 요청 시에만 새 세션 생성
4. **`-p` 모드는 fallback** — PTY가 정상 동작하면 `-p`는 긴급 복구용으로만 유지
5. **세션 간 맥락 전달** — 격리된 세션 간에는 ContextPack(cross-session, compressed memory)으로 맥락 공유

---

## 데이터 모델

### 기존 활용 (변경 없음)

| 필드 | 테이블 | 용도 |
|------|--------|------|
| `resume_token` | `conversations` | Claude 세션 ID (이미 `-p` 모드에서 사용 중) |
| `resume_token_engine` | `conversations` | 토큰 소유 엔진 |

### 새로 추가

| 필드 | 테이블 | 용도 |
|------|--------|------|
| `jsonl_path` | `conversations` | 해당 세션의 JSONL 파일 경로 (PTY 응답 polling용) |

> `resume_token`에 Claude 세션 ID가 이미 저장되므로, JSONL 경로는 `~/.claude/projects/{encoded-path}/{resume_token}.jsonl`로 **유도 가능**. `jsonl_path` 컬럼 없이 규칙 기반으로도 동작 가능 — 구현 시 판단.

---

## 생명주기

### 프로젝트

| 이벤트 | 동작 |
|--------|------|
| 프로젝트 선택 | 현재 선택된 대화의 PTY 세션 spawn (`--resume` or 새 세션) |
| 프로젝트 전환 | 이전 PTY kill → 새 프로젝트의 현재 대화 PTY spawn |
| 프로젝트 삭제 (soft) | PTY kill, 세션 데이터는 DB에 보존 (hidden=1) |
| 프로젝트 복원 | 다시 프로젝트 선택 시 기존 `resume_token`으로 복원 |

### 채팅 (Conversation)

| 이벤트 | 동작 |
|--------|------|
| 채팅 생성 | PTY spawn (새 세션, `--resume` 없음) → `resume_token` 저장 |
| 채팅 전환 | 현재 PTY kill → 대상 채팅의 `resume_token`으로 `--resume` spawn |
| 채팅에서 첫 메시지 | JSONL 감지 (스냅샷 비교) → `resume_token` 저장 |
| `/clear` | 현재 PTY kill → `resume_token` 초기화 → 새 PTY spawn |
| 채팅 삭제 | PTY kill (해당 세션만) |

### PTY 프로세스

| 이벤트 | 동작 |
|--------|------|
| 앱 시작 | 마지막 선택 대화의 PTY spawn (`--resume`) |
| 앱 종료 | 모든 PTY kill (프로세스만, 세션 데이터 보존) |
| PTY 비정상 종료 | `pty:exit` 이벤트 → 자동 재spawn (`--resume`) |
| HMR (dev) | `pty_kill_all` → 재spawn |

---

## 채팅 전환 흐름

```
Chat A (active, PTY running)
  → 사용자가 Chat B 클릭
  → PTY kill (Chat A 세션)
  → Chat B의 resume_token 확인
    → 있으면: claude --resume <token> --permission-mode bypassPermissions
    → 없으면: claude --permission-mode bypassPermissions (새 세션)
  → PTY spawn, 응답 대기 상태
```

**비용**: 채팅 전환 시 PTY 재시작 ~1-2초. Claude Code가 `--resume`으로 세션을 복원하므로 컨텍스트 손실 없음.

---

## JSONL 식별

**규칙**: `resume_token`을 알면 JSONL 경로는 결정적:
```
~/.claude/projects/{projectPath.replace('/', '-')}/{resume_token}.jsonl
```

- 첫 메시지 전송 시: 스냅샷 비교로 JSONL 감지 → `resume_token` 추출 (파일명 = 세션 ID)
- 이후: `resume_token`에서 경로 유도 → 직접 polling
- 별도 `jsonl_path` 컬럼 불필요 (규칙 기반)

---

## `/clear` 구현

```
사용자: /clear
  → 현재 PTY kill
  → UPDATE conversations SET resume_token = NULL WHERE id = ?
  → 새 PTY spawn (--resume 없이)
  → 첫 메시지 시 새 resume_token 저장
```

Claude Code 측에서도 `/clear`가 새 세션을 시작하므로 양쪽이 일관됨.

---

## `-p` 모드 Fallback

| 조건 | 동작 |
|------|------|
| PTY spawn 실패 | `-p` 모드로 자동 전환, 사용자에게 toast 알림 |
| PTY 3회 연속 비정상 종료 | `-p` 모드로 전환, 설정에서 수동 복원 가능 |
| 사용자 명시적 선택 | Settings > Runtime에서 PTY/CLI 전환 토글 |

`-p` 모드에서도 `resume_token`은 동일하게 동작 (기존 구조 그대로).

---

## 프로젝트당 1 PTY vs 채팅당 1 PTY

**결정: 채팅당 1 PTY (세션)**

이유:
- 채팅별 컨텍스트 격리 (다른 목적의 대화가 섞이지 않음)
- `-p` 모드가 매 메시지마다 새 세션을 만드는 것에 비해 세션 수가 오히려 줄어듦
- PTY의 핵심 가치 = stateful 멀티턴 → 채팅 단위 세션이 자연스러움
- 세션 간 맥락은 ContextPack이 이미 해결 (cross-session, compressed memory)

런타임에 **활성 PTY 프로세스는 항상 1개** (현재 선택된 채팅의 세션만 실행).

---

## 구현 순서

### Phase 1: 채팅=세션 1:1 기반 구축
1. `resume_token` 기반 PTY spawn/resume 로직 (projectSlice → conversationSlice 이동)
2. 채팅 전환 시 PTY kill → resume 흐름
3. JSONL 경로를 `resume_token`에서 유도
4. 앱 시작 시 마지막 대화의 PTY 자동 resume

### Phase 2: /clear + fallback
5. `/clear` 커맨드 구현 (PTY 리셋 + resume_token 초기화)
6. PTY 실패 시 `-p` fallback 자동 전환
7. Settings에 PTY/CLI 토글 추가

### Phase 3: 안정화
8. PTY 비정상 종료 시 자동 재spawn
9. 콘솔/toast 로깅 정리
10. 실사용 검증 + 버그 수정

---

## Branch / RT 세션 정책 (확정)

| 모드 | 세션 정책 | 이유 |
|------|----------|------|
| **Branch** | 부모 채팅의 PTY 세션 공유 | 분기→adopt/폐기, 같은 맥락에서 작업 |
| **RT** | `-p` 모드 유지 | 1턴 발언, stateful 불필요, 맥락은 ContextPack 주입, 1회성 |

---

## 엔진별 파서 (확정)

각 엔진의 JSONL/JSON 구조가 모두 다르므로 **엔진별 별도 파서** 구현. 불필요한 추상화 금지.

| 엔진 | 파서 | 위치 | 형식 |
|------|------|------|------|
| Claude | `pty_poll_jsonl` (구현 완료) | `~/.claude/projects/{encoded}/{id}.jsonl` | JSONL |
| Codex | `pty_poll_codex` (미구현) | `~/.codex/sessions/{y/m/d}/rollout-{id}.jsonl` | JSONL |
| Gemini | `pty_poll_gemini` (미구현) | `~/.gemini/tmp/{project}/chats/session-{id}.json` | 단일 JSON |

참고: seCall `crates/secall-core/src/ingest/{claude,codex,gemini}.rs`

---

## ContextPack × PTY (검토 필요)

### 현재 `-p` 모드
- 매 메시지마다 `build_normalized_prompt_with_budget()`로 전체 ContextPack을 system prompt에 주입
- 에이전트가 매번 identity, plan, memory, skills 등 전체 맥락을 받음

### PTY 모드의 차이
- PTY는 **stateful** — 에이전트가 이전 대화를 기억
- 매 메시지마다 전체 ContextPack을 보내면 **중복** (이미 세션에 축적된 맥락)
- 하지만 plan 변경, memory 갱신 등 **동적 맥락**은 갱신 필요

### 검토 포인트
1. **초기 세션**: 첫 메시지 시 전체 ContextPack 주입 → CLAUDE.md 또는 system prompt로?
2. **후속 메시지**: 변경된 섹션만 delta로 전달? 아니면 매번 전체?
3. **CLAUDE.md 동적 갱신**: PTY 세션의 프로젝트 CLAUDE.md에 tunaFlow context 섹션을 동적으로 갱신?
4. **토큰 절감**: stateful이므로 반복 주입 불필요 → 초기 1회 + delta만으로 충분?

---

## 미결 사항

- **ContextPack × PTY 구체 설계**: 위 검토 포인트 기반으로 구현 방향 결정
- **세션 정리**: 오래된 JSONL 파일 자동 삭제 정책 (CLI 자체 관리에 위임?)
