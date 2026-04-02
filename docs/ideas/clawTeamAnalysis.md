# ClawTeam 분석 — 레퍼런스 + 반면교사

> Status: idea
> Created: 2026-04-01
> 출처: github.com/HKUDS/ClawTeam (v0.2.0, 182 commits, Python 21k줄)
> 분석 기준: tunaFlow 코드베이스와의 비교, 프로덕션 실사용 관점

---

## 1. 프로젝트 개요

ClawTeam은 홍콩대학교 데이터과학 연구실(HKUDS)의 멀티에이전트 오케스트레이션 프레임워크. tmux + 파일시스템 기반으로 CLI 에이전트(Claude Code, Codex 등)를 스폰하고 조율.

**규모**: Python 103파일 21k줄, 테스트 35파일 7.5k줄
**평가**: 6.5/10 (코어 인프라는 실제 동작하나 lifecycle 관리 미흡, README 과장)

---

## 2. 레퍼런스로 삼을 점

### 2.1 파일 기반 태스크 스토어 (store/file.py)

**패턴**: JSON 파일 1개 = 태스크 1개, OS-level flock, atomic write(tmpfile + os.replace)

tunaFlow에 없는 것:
- **태스크 의존성 그래프 + 사이클 감지**: DFS로 순환 의존 탐지 → 에러 반환
- **자동 unblock**: 태스크 완료 시 의존하는 태스크들 자동으로 pending 전환
- **락 기반 소유권 + 죽은 에이전트 감지**: 락 보유자가 살아있는지 PID 체크 → 죽었으면 락 해제

**tunaFlow 적용 가치**: plan_subtasks의 `blocked_by` 필드가 존재하지만 자동 unblock 로직은 없음. 워크플로우 파이프라인에서 subtask 의존성 해소 자동화에 참고.

### 2.2 라우팅 정책 (routing_policy.py, 412줄)

**패턴**: 메시지 라우팅에 쓰로틀링 + 집계 + 재시도 + 감사 로그

- 동일 경로 30초 내 중복 메시지 → 집계하여 1건으로 전달
- 실패 시 지수 백오프로 재시도
- 최근 50건 이벤트 감사 로그 (source, target, action, reason)

**tunaFlow 적용 가치**: RT에서 에이전트 간 메시지 전달 시 쓰로틀링 패턴. 현재 tunaFlow RT는 쓰로틀 없이 순차/병렬 실행만 존재.

### 2.3 충돌 감지 (conflicts.py)

**패턴**: git diff의 hunk header를 파싱하여 라인 레벨 overlap 분석

- severity 3단계: high(같은 라인), medium(같은 파일 다른 라인), low(같은 디렉토리)
- 에이전트 쌍별 pairwise 비교
- rebase 필요 여부 휴리스틱 (commit 거리 + 파일 overlap)

**tunaFlow 적용 가치**: git worktree 연동 시 Implementation Branch 간 충돌 사전 감지에 직접 참고 가능.

### 2.4 워크스페이스 컨텍스트 주입 (context.py)

**패턴**: 다른 에이전트들의 작업 현황을 현재 에이전트의 context에 자동 주입

- 에이전트별 수정 파일/줄 수 추적 (git numstat)
- 파일 소유권 매핑 (어떤 에이전트가 어떤 파일을 수정 중인지)
- 관련 변경 요약 + 파일 overlap 경고

**tunaFlow 적용 가치**: ContextPack에 "다른 에이전트의 작업 현황" 섹션을 추가하는 참고. 특히 병렬 Implementation Branch 실행 시.

### 2.5 경로 보안 (paths.py)

**패턴**: identifier whitelist + path escape 감지

```python
_IDENTIFIER_RE = re.compile(r"^[A-Za-z0-9._-]+$")
resolved.relative_to(base)  # 탈출 시 ValueError
```

**tunaFlow 적용 가치**: tunaFlow의 `validate_identifier` 같은 경로 검증이 현재 부재. conversation_id, branch_id 등에 적용 고려.

---

## 3. 반면교사 — tunaFlow가 피해야 할 방향

### 3.1 Lifecycle 관리를 후순위로 미루지 마라

**ClawTeam의 실패**: lifecycle.py가 **104줄**. 멀티에이전트 시스템의 핵심인 에이전트 생사 관리가 메시지 래퍼 4개 함수로 끝남.

없는 것:
- 크래시 복구 (에이전트가 죽으면 in-progress 태스크 방치)
- 타임아웃 (shutdown 요청 후 응답 없으면 무한 대기)
- 강제 종료 (응답 없는 에이전트 kill 수단)
- 상태 머신 (shutdown 이중 요청, 레이스 컨디션 방어)

**교훈**: 정상 경로(happy path)만 구현하고 에러 경로를 미루면, 시스템이 커질수록 장애 복구가 불가능해진다.

**tunaFlow 적용**:
- 워크플로우 파이프라인에서 에이전트가 응답하지 않을 때의 처리를 **설계 시점에** 정의
- `agent:error` 이벤트 후 자동 재시도 or 사용자 알림 or phase 롤백
- Implementation Branch에서 Developer 에이전트가 크래시하면 → 태스크 상태 자동 복구
- 현재 `guardrailImprovementIdeas.md`의 에러 복구 아이디어(Phase 1-3)를 초기부터 적용

### 3.2 README에 검증 불가능한 수치를 넣지 마라

**ClawTeam의 실패**: "8 H100 GPU × 2430 실험, val_bpb 6.4% 개선"

이 코드가 하는 건 **tmux에 CLI를 띄우는 것**. GPU 오케스트레이션 코드는 단 한 줄도 없음. autoresearch가 별도로 필요하고 ClawTeam은 "띄워주는 역할"만. 하지만 README는 마치 ClawTeam이 이 성과를 달성한 것처럼 기술.

**교훈**: 프레임워크의 역할과 사용 사례의 성과를 명확히 구분해야 한다. "tunaFlow를 사용하여 X를 달성했다"와 "tunaFlow가 X를 달성한다"는 다르다.

**tunaFlow 적용**:
- CLAUDE.md, docs/reference에 기능 주장과 실제 구현 범위를 정확히 구분
- "워크플로우 파이프라인으로 코드 품질 2-3배 향상" 같은 주장은 **실측 데이터 확보 전까지 삼가**
- `implementationStatus.md`의 현황 테이블에 "검증됨/미검증" 칼럼 유지

### 3.3 가짜 프리셋/템플릿을 프로덕션으로 포장하지 마라

**ClawTeam의 실패**: presets.py에 `gpt-5.4`, `DeepSeek-V3.2` 등 **실제 존재하지 않는 모델명**. AI Hedge Fund, Agentic Engineering 템플릿은 TOML 구조만 있고 실제 금융/코드 로직 없음.

**교훈**: 테스트되지 않은 예시를 프로덕션 기능처럼 나열하면 신뢰를 잃는다.

**tunaFlow 적용**:
- `docs/agents/{architect,developer,reviewer}.md` 템플릿이 실제 워크플로우에서 검증되었는지 추적
- persona 7종 built-in이 실제 효과가 있는지 A/B 검증 계획 (후순위여도 인식은 필요)
- 스킬 스냅샷의 `published_at`처럼 "마지막 검증 일시" 메타 유지

### 3.4 다형성 모델로 모든 메시지를 하나의 클래스에 넣지 마라

**ClawTeam의 실패**: TeamMessage 1개 클래스에 10+ 메시지 타입 (join_request, plan_approval, shutdown, idle, broadcast...). 대부분 필드가 context-dependent null.

```python
class TeamMessage(BaseModel):
    type: MessageType
    sender: str
    content: str
    proposed_name: Optional[str] = None  # join_request에서만 사용
    plan_file: Optional[str] = None      # plan_approval에서만 사용
    feedback: Optional[str] = None       # review에서만 사용
    # ... 10+ optional fields
```

**교훈**: 메시지 타입이 늘어날수록 어떤 필드가 어떤 타입에서 유효한지 파악 불가. 타입 안전성 상실.

**tunaFlow 적용**:
- 현재 `PlanEvent`가 `event_type: string + detail: string(JSON)` 구조 → ClawTeam과 유사한 방향으로 갈 위험
- event_type별 detail 스키마를 **문서화하거나 타입으로 구분** (discriminated union)
- RT `RoundtableParticipant`의 `role?: string`도 enum으로 강화 고려
- 마커 파서의 `ParsedPlanProposal`, `ParsedImplPlan`, `ParsedReviewVerdict`가 이미 분리된 것은 올바른 방향 — 이 패턴 유지

### 3.5 `except Exception: pass` 패턴을 습관화하지 마라

**ClawTeam의 실태**: lifecycle.py, context.py, costs.py 등에서 `except Exception: pass` 반복

```python
# lifecycle.py
try:
    workspace.cleanup(agent)
except Exception:
    pass  # ← 파일 권한 에러? 디스크 풀? 전부 무시
```

**교훈**: 개발 초기엔 빠르지만, 디버깅이 불가능해지는 부채. 특히 멀티에이전트 시스템에서 한 에이전트의 silent failure가 전체 워크플로우를 오염시킴.

**tunaFlow 현재 상태**: CLAUDE.md에 "dev 단계에서 silent fallback 최소화" 규칙이 이미 있음 ✅. 이 원칙을 유지하고, 특히 workflowOrchestration.ts에서 fire-and-forget 패턴(`catch(() => {})`)이 늘어나지 않도록 주의.

**구체적 위험 지점**:
- `syncPlanDocument()`, `syncReviewReport()`, `syncResultReport()` — 모두 `catch { /* fire-and-forget */ }`
- 이 함수들이 실패하면 plan 문서가 생성 안 됨 → 사용자가 알 수 없음
- 최소한 `console.warn`이나 toast 알림은 필요

### 3.6 "에이전트가 알아서 한다"를 시스템 보장으로 착각하지 마라

**ClawTeam의 실패**: 에이전트에게 `clawteam task update {team} {id} --status completed` CLI 명령을 프롬프트로 알려주고, 에이전트가 이것을 "자발적으로" 실행할 것을 기대. 실제로는:

- 에이전트가 명령을 잊거나 형식을 틀리면 → 태스크가 영원히 in_progress
- 에이전트가 completed 보고 없이 다음 태스크를 시작하면 → 의존성 체인 깨짐
- **어떤 시스템 레벨 강제도 없음**

**교훈**: 에이전트의 프롬프트 준수는 "희망"이지 "보장"이 아니다. 핵심 상태 전환은 시스템이 감지하고 강제해야 한다.

**tunaFlow 현재 상태**: 마커 기반 감지(`plan-proposal`, `impl-complete`, `review-verdict`)로 에이전트 출력을 파싱하여 phase 전환 → ClawTeam보다 나은 접근. 하지만:

- 에이전트가 마커를 안 넣으면? → 현재 수동 fallback만 (사용자가 직접 phase 전환)
- 마커 형식이 틀리면? → `planProposalParser`가 null 반환 → silent skip

**개선 방향**:
- 마커 미감지 시 **타임아웃 후 사용자 알림** ("Developer가 실행계획을 보고하지 않았습니다. 재요청할까요?")
- 구조화된 Verdict 루브릭(rtAlgorithmEnhancementIdeas P0)이 이 문제를 근본적으로 해결 — 자유형식 → 스키마 강제

### 3.7 테스트에서 모킹을 과도하게 사용하지 마라

**ClawTeam의 실태**: test_spawn_backends.py 1,472줄 중 대부분이 `monkeypatch`로 subprocess를 mock. 실제 tmux 세션 생성, 에이전트 스폰, 메시지 전달을 검증하는 integration test는 없음.

**교훈**: 모킹 테스트는 "코드가 올바르게 호출하는가"를 검증하지만, "실제로 동작하는가"를 검증하지 못함. 특히 subprocess 기반 시스템에서 mock과 실제 동작의 괴리가 큼.

**tunaFlow 현재 상태**:
- Frontend 78 테스트 (대부분 mock 기반)
- Rust 60 테스트 (unit 위주)
- integration test **부재** (CLAUDE.md에 명시된 알려진 이슈)

**개선 방향**:
- 워크플로우 파이프라인의 E2E 테스트 (Chat → Plan 승격 → Approval → Implementation → Review → Done)를 mock 없이 실행하는 smoke test 1개라도 추가
- `workflowOrchestration.test.ts`의 9개 테스트가 invoke를 mock하고 있다면, 실제 DB + Tauri command를 사용하는 integration test도 병행

---

## 4. tunaFlow vs ClawTeam 비교표

| 차원 | ClawTeam | tunaFlow | tunaFlow 우위 |
|------|----------|----------|-------------|
| 에이전트 스폰 | tmux 세션 (외부) | Tauri subprocess (내장) | tunaFlow — 앱 내 통합 |
| 에이전트 간 통신 | 파일 기반 inbox | DB 메시지 + 이벤트 | tunaFlow — SSOT 보장 |
| 태스크 관리 | JSON 파일 + flock | SQLite + plan_subtasks | tunaFlow — ACID |
| 워크플로우 상태 | 없음 (에이전트 자율) | plan.phase + plan_events | tunaFlow — 시스템 추적 |
| 마커 기반 감지 | 없음 | plan-proposal 등 4종 | tunaFlow — 자동 파싱 |
| context 조립 | 없음 (에이전트 CLI에 위임) | ContextPack 4-engine parity | tunaFlow — 핵심 차별점 |
| git 격리 | worktree (구현됨) | Branch (대화 격리만, git 미연동) | ClawTeam — 코드 격리 |
| 충돌 감지 | 라인 레벨 diff | 없음 | ClawTeam — 참고 필요 |
| lifecycle | 104줄, 크래시 복구 없음 | `agent:error` 이벤트 + UI 표시 | tunaFlow — 최소한 감지는 됨 |
| 테스트 | 7.5k줄 (mock 과다) | 78+60 tests (mock 기반) | 비슷 — 둘 다 integration 부재 |

---

## 5. 핵심 교훈 요약

```
1. Lifecycle은 1순위다
   → 에이전트가 죽을 때의 처리를 설계 시점에 정의하라

2. 수치 주장은 검증 가능해야 한다
   → "X를 달성한다"와 "X를 달성하는 데 사용될 수 있다"를 구분하라

3. 프리셋/템플릿은 테스트된 것만 제공하라
   → 가짜 모델명은 신뢰를 파괴한다

4. 메시지 모델은 타입별로 분리하라
   → 10개 Optional 필드의 kitchen sink 클래스는 유지보수 지옥

5. Silent failure는 부채다
   → except pass는 빠르지만 디버깅을 불가능하게 만든다

6. "에이전트가 알아서"는 보장이 아니다
   → 핵심 상태 전환은 시스템이 감지하고 강제해야 한다

7. Mock 테스트만으로는 부족하다
   → Integration test 1개가 unit test 100개보다 신뢰를 준다
```
