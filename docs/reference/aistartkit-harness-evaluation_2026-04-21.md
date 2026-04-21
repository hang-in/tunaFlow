---
title: aiStartKit Harness 적용 가능성 평가 — 2026-04-21
status: active
canonical: false   # 판단 근거 일부 주관
created_at: 2026-04-21
owner: architect
related:
  - docs/reference/harnessMaturityAudit_2026-04-16.md
  - docs/ideas/onboardingMetaAgentIdea.md
---

# aiStartKit 레퍼런스 평가

사용자가 보유한 **AIProject-Starterkit** (`/Users/d9ng/Downloads/aiStartKit`, v0.1.0) 를 tunaFlow 의 하네스로 적용할 수 있는지 판단한다.

## 0. 요약 (TL;DR)

| 적용 방향 | 가치 | 비용 | 우선순위 |
|-----------|:----:|:----:|:-------:|
| A. tunaFlow 내부 개발 하네스 이식 | ⚠️ 제한 | 중 | ❌ 스킵 |
| B. Pre-commit hook 설계 참조 | ✅ | 낮음 | **베타 직후** (1~2시간) |
| C. 온보딩 메타에이전트 통합 | ✅✅ | 중~상 | **v0.2 P2** (1~2주) |

> 핵심 결론: **aiStartKit 은 tunaFlow 가 "생성하는 사용자 프로젝트" 의 하네스로는 매우 유용, "tunaFlow 자체 개발 하네스" 로는 중복이라 가치 제한적.**

---

## 1. aiStartKit 정체

- 경로: `/Users/d9ng/Downloads/aiStartKit/` (사용자 본인 소유, `.git` 존재)
- 버전: `0.1.0`
- 목적: Claude Code 기반 **신규 프로젝트 부트스트랩 템플릿**
- 구성:
  - `CLAUDE.md` — placeholder 기반 골격
  - `.claude/skills/` — sprint-team, fix, docs-team, release, impl-check, create-prompt
  - `.claude/rules/` — architectures, commons, practices, **verifications**
  - `.claude/commands/` — `/init-project` (Step 0~9), `/tutorial`
  - `.claude/hooks/` + `HOOKS.md` + `settings.hooks.example.json` — PostToolUse 포맷터 등
  - `.claude/agents/`, `settings.json`

라이선스 제약 없음 (본인 소유).

---

## 2. 층위 차이

둘은 같은 층에 있지 않다. 이 점이 평가 전반을 지배한다.

```
aiStartKit  : Claude Code 단일 에이전트 프로젝트의 스캐폴드
tunaFlow    : 다엔진(Claude/Codex/Gemini/Ollama) 오케스트레이션 클라이언트
              + Claude Code 를 실행 대상이자 작성 주체로 포함
```

tunaFlow 는 **aiStartKit 이 만드는 산출물보다 한 층 위**에 있다. tunaFlow 에서 만드는 "사용자 프로젝트" 가 aiStartKit 이 타깃하는 층과 동일.

---

## 3. 방향별 상세 판단

### 3.1 A. tunaFlow 자체 개발 하네스 이식 → ❌ 제한적

tunaFlow 는 이미 같은 개념을 더 두껍게 갖고 있다:

| aiStartKit 자산 | tunaFlow 기존 대응 | 결론 |
|---|---|---|
| `CLAUDE.md` placeholder 구조 | 3-SSOT 기반 CLAUDE.md (dataModel / implementation / sessionHistory 분리) | 중복 |
| `skills/{sprint-team, fix, impl-check, docs-team, release, create-prompt}` | 4-layer skills (`~/.tunaflow/skills/`) + skill registry + plan-dev-review 워크플로우 | 중복/층위 다름 |
| `rules/{architectures, commons, practices}` | CLAUDE.md §15~17 + `docs/reference/` SSOT | 중복 |
| `rules/verifications/VERIFICATION.md` | CLAUDE.md §15 "작업 안전 규칙" | 겹침 |
| `/init-project`, `/tutorial` | 해당 없음 (tunaFlow 는 slash command 없음) | 개념만 참고 |

예외: `VERIFICATION.md` 의 "리소스 참조 ↔ 실제 파일 존재 검증" 섹션은 tunaFlow 에 대응 skill 이 없어 차용 여지. 다만 독립 skill 로 뽑을 정도인지는 의문.

### 3.2 B. Pre-commit hook 설계 참조 → ✅ 직접 가치

`harnessMaturityAudit_2026-04-16.md` §2.3 에서 이미 **"Pre-commit / lint 자동 훅 — 하(부족)"** 으로 판정. §5.1 즉시 투자 1순위.

aiStartKit 의 기여:

- `HOOKS.md` + `settings.hooks.example.json` 의 PostToolUse/PreToolUse 패턴
- placeholder `<<FORMAT_CMD>>` 치환 → 스택별 자동화 예시 표 (ts/py/go/rust/unity)
- `_README` / `_purpose` 언더스코어 키 주석 관례

활용 방식:

1. tunaFlow 의 `install.sh` 또는 첫 실행 훅에서 `.git/hooks/pre-commit` 스탬프 생성
2. 내용: CLAUDE.md 에 이미 정의된 `cargo check` / `tsc --noEmit` / `vitest run --changed` 조합
3. aiStartKit 의 스택별 표를 tunaFlow 의 **사용자 프로젝트 생성 시 템플릿** 으로도 재사용

구현 비용: 1~2시간. 베타 직후 착수 가능.

### 3.3 C. 온보딩 메타에이전트 통합 → ✅✅ 최대 ROI

**가장 큰 가치.** tunaFlow `ProjectOnboardingModal` 은 현재 "에이전트 감지 + 스택 추천" 까지만 동작하고 프로젝트 스캐폴드는 만들지 않는다. aiStartKit 의 `/init-project` Step 0~9 플로우는 **이미 설계된 레퍼런스 구현**.

현재 → 확장:

```
현재 tunaFlow                          aiStartKit 통합 후
─────────────────────────              ──────────────────────────────
ProjectStartup → 경로 선택             + aiStartKit 기반 스캐폴드 옵션
→ 스택 감지                            + Step 1 (경험 수준)
→ 에이전트 프로필                       + Step 3 (placeholder 치환)
→ 완료                                  + Step 4 (하네스 최적화)
                                        + Step 9 (튜토리얼)
```

통합 방식 제안:

1. aiStartKit 을 tunaFlow 저장소 내부 `templates/aiStartKit/` 로 복제 (또는 git submodule)
2. `ProjectOnboardingModal` 에 "Claude Code 템플릿으로 시작" 옵션 추가
3. 선택 시 템플릿 복사 + placeholder 를 onboarding 수집값으로 치환 + hook 자동 설치
4. 생성된 프로젝트의 `.claude/` 는 tunaFlow 의 skill snapshot (`~/.tunaflow/skills/`) 과 공존

다엔진 분기 필요:

- aiStartKit 의 `sprint-team`, `fix`, `docs-team` 등 skills 는 **Claude Code 슬래시 커맨드 전제**
- Codex / Gemini 에서는 무의미 → 템플릿 선택 시 "Claude Code 선호 프로젝트" 한정 또는 엔진-중립 Skills 변환 레이어 도입 필요

연관 문서: `docs/ideas/onboardingMetaAgentIdea.md` — P2 로 이미 로드맵에 있음. aiStartKit 가 구현 참조.

구현 비용: 1~2주. v0.2 시점 착수.

---

## 4. 도입 안 할 부분 (명시)

- aiStartKit 의 `skills/` 를 tunaFlow 내부 `.claude/skills/` 로 **직접 복사** — plan-dev-review 와 중복/층위 충돌
- aiStartKit 의 `rules/` 를 tunaFlow `CLAUDE.md` 로 **재편입** — 이미 §15~17 에 동등 이상 수록
- `/init-project` / `/tutorial` 슬래시 커맨드를 **tunaFlow 자체** 에 추가 — tunaFlow 는 slash command 체계가 없음 (앱 내 메뉴 구조)

---

## 5. 실행 체크리스트

### 단기 (베타 직후)

- [ ] `harnessMaturityAudit` §5.1 "Pre-commit hook 자동 생성" 작업에 aiStartKit `HOOKS.md` 스택 표 참조
- [ ] `install.sh` 에 `.git/hooks/pre-commit` 스탬프 단계 추가
- [ ] aiStartKit `VERIFICATION.md` 의 "리소스 존재 검증" 항목을 tunaFlow 의 workflow 검증 skill 에 추가 (선택)

### 중기 (v0.2)

- [ ] `docs/ideas/onboardingMetaAgentIdea.md` 를 `docs/plans/onboardingMetaAgentPlan_*.md` 로 승격
- [ ] plan 내에 aiStartKit 구조를 **레퍼런스 구현** 으로 명시
- [ ] `templates/aiStartKit/` 경로 설계 (복제 vs submodule)
- [ ] `ProjectOnboardingModal` 확장 단계 정의
- [ ] 엔진-중립 Skills 변환 방침 결정

---

## 6. 관련

- `docs/reference/harnessMaturityAudit_2026-04-16.md` — 이번 판단의 기준점
- `docs/ideas/onboardingMetaAgentIdea.md` — 코스 C 의 착수점
- aiStartKit 원본: `~/Downloads/aiStartKit/`
