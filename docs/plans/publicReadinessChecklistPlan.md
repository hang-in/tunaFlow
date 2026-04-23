---
title: Public Release Readiness Checklist — OSS 공개 전 cleanup + hygiene
status: planned
priority: P0 (공개 전 필수)
created_at: 2026-04-23
related:
  - LICENSE                                 # 신규
  - CONTRIBUTING.md                         # 신규
  - SECURITY.md                             # 신규
  - CODE_OF_CONDUCT.md                      # 신규
  - .github/ISSUE_TEMPLATE/                 # 신규
  - .github/PULL_REQUEST_TEMPLATE.md        # 신규
  - README.md / README.ko.md                # 기존 — 스토리 보강
  - docs/plans/i18nPlan.md                  # 영문판 완료 후 공개
triggered_by:
  - 2026-04-23 Architect first-time visitor 시뮬레이션 — LICENSE 부재, tunaflow.db tracked, root debris 지적
  - 사용자 공개 시점 확정 — i18n 영문판 완료 시점 (~1주일 내 예상)
---

# Public Release Readiness

> tunaFlow 를 처음 만나는 외부 개발자에게 "짜파게티 코드 OSS" 가 아니라 "잘 만든 OSS" 로 전달되도록 준비.

---

## TL;DR for Developer

1. **Phase 0 (CRITICAL)**: `tunaflow.db` untrack + gitignore. 필요 시 `git filter-repo` 로 history 청소.
2. **Phase 1 (cleanup)**: Root debris 9 파일 정리, `_util/` 디렉토리 제거 (별도 repo 차용물), `coverage/` gitignore.
3. **Phase 2 (hygiene)**: LICENSE (Apache 2.0) + CONTRIBUTING + SECURITY + CODE_OF_CONDUCT + `.github/` templates 추가 — Architect 가 전문 제공.
4. **Phase 3 (스토리)**: README 에 Mission 보강 (터미널 복붙 배경), "Built with tunaFlow" 섹션, "References" 섹션 (_util 출처 감사), CLAUDE.md 성격 1줄, 배지 `Private` → `Beta`.
5. **Phase 4 (크리티컬 점검)**: build / test / 민감 데이터 grep / broken link 확인.
6. **Phase 5 (전환)**: i18n 영문판 완료 확인 → repo public 전환 → first release tag `v0.x.x-beta`.

공개 시점은 i18n Phase 1~3 완료 후. 현재 남은 실작업이 많지 않아 1주일 내 가능.

---

## Phase 0 — CRITICAL: `tunaflow.db` 처리

`src-tauri/tunaflow.db` 는 이미 gitignore. **root 의 `tunaflow.db` 는 tracked** — 개발자 개인 DB 라 공개 repo 에 절대 불가.

### Step 1: 현재 상태 확인

```bash
# 과거 history 에서 tunaflow.db 변경 시점 확인
git log --all --oneline -- tunaflow.db | head -10
```

### Step 2: DB 내용 민감성 검사 (사용자 직접)

다음이 있으면 **history 청소 필요**:
- API key / token (`sk-...`, `ghp_...`)
- 제 3자 대화 내용
- 개인 식별 정보
- 자주 쓰는 엔드포인트 / credentials

없으면 단순 untrack 으로 충분.

### Step 3A: 단순 untrack (민감 데이터 없을 때)

```bash
git rm --cached tunaflow.db
echo "/tunaflow.db" >> .gitignore
git add .gitignore
git commit -m "chore: untrack root tunaflow.db (dev-local DB only)"
```

### Step 3B: History 청소 (민감 데이터 있을 때)

```bash
# git-filter-repo 설치: brew install git-filter-repo (macOS)
git filter-repo --path tunaflow.db --invert-paths

# 이후 force-push 필요 (현재 repo 가 private 이므로 영향 적음)
git push --force origin main
```

**주의**: `git filter-repo` 는 history 재작성. 모든 로컬 clone 이 다시 clone 필요. Public 전이라 영향 최소.

---

## Phase 1 — Root Cleanup

### 파일별 처리

| 파일/디렉토리 | 조치 | 명령 |
|---|---|---|
| `_stash_recovery/` | 검토 후 archive 이동 or 삭제 | 내용 확인 후 결정 |
| `claude-new.png` | 삭제 (README 미참조) | `git rm claude-new.png` |
| `screenshot-2026-04-08-161654.png` | 삭제 (낡은 스크린샷) | `git rm screenshot-2026-04-08-*.png` |
| `screenshot-2026-04-14-175830.png` | 삭제 (낡은 스크린샷) | `git rm screenshot-2026-04-14-*.png` |
| `resume_claude.md` | `docs/prompts/archive/` 이동 | `git mv resume_claude.md docs/prompts/archive/` |
| `resume_claude_archtect.md` | 오타 (archtect). 이동 + rename or 삭제 | 검토 후 결정 |
| `resume_codex.md` | 이동 | `git mv resume_codex.md docs/prompts/archive/` |
| `resume_gemini.md` | 이동 | `git mv resume_gemini.md docs/prompts/archive/` |
| `_util/` | **repo 에서 제거** (별도 repo 차용물) | `git rm -r _util/` + `.gitignore` 에 `/_util/` |
| `coverage/` | gitignore | `.gitignore` 에 `/coverage/` |
| `evals/scripts/run-eval.mjs` | cleanup 기본값 전환 (opt-in → opt-out) | 아래 "Eval scratch cleanup 기본 활성화" 섹션 참고 |

### Eval scratch cleanup 기본 활성화

`evals/scripts/run-eval.mjs` 가 HTTP API `/api/v1/projects` 로 `[eval] <label>` 임시 프로젝트를 생성 (line 163 근처). 현재 cleanup 이 `--cleanup` / `EVAL_CLEANUP=1` **opt-in** 이라 잊으면 DB 에 찌꺼기 누적. 공개 후 외부 기여자가 돌릴 때 동일 문제 재발 방지를 위해 기본값 전환:

```diff
-  if (process.argv.includes("--cleanup") || process.env.EVAL_CLEANUP === "1") {
+  if (!process.argv.includes("--no-cleanup") && process.env.EVAL_CLEANUP !== "0") {
     console.log("\n[eval] cleaning up scratch projects");
```

- 기본: 항상 cleanup
- 디버깅용 보존: `--no-cleanup` 또는 `EVAL_CLEANUP=0` 으로 opt-out
- `evals/README.md` 사용법 섹션에 새 플래그 기재

이유: `projects` 테이블에 `kind` 컬럼을 신설하는 대안도 있으나 과잉 설계. 기본값 전환 한 줄로 같은 효과 + 마이그레이션 불필요.

### `_util/` 특별 처리

사용자 확인 (2026-04-23): `_util/` 은 별도 repo 의 차용물. 라이센스 충돌 및 중복 배포 방지를 위해 **repo 에서 완전 제거**. 차용 아이디어에 대한 **감사 + 원 repo 링크** 를 README 의 "References" 섹션으로 대체 (Phase 3).

### 결과

Root 에 남는 것:
- **주요 디렉토리**: `agents/` `docs/` `mcp-server/` `public/` `scripts/` `src/` `src-tauri/`
- **Markdown**: `CLAUDE.md` `INSTALL.md` `LICENSE` `README.md` `README.ko.md`
- **Config**: `package.json` `package-lock.json` `vite.config.ts` `tsconfig.json`
- **Rust**: `Cargo.lock` (src-tauri 안)
- **`.github/`** `.gitignore`

약 15개. 짜파게티 시그널 제거.

---

## Phase 2 — OSS Hygiene 파일 추가

Architect 가 전문 제공 (아래 "Architect 산출 파일" 섹션). Developer 는 복붙 + commit.

### 신규 파일 목록

| 파일 | 출처 |
|---|---|
| `LICENSE` | Apache License 2.0 표준 텍스트 + `Copyright 2026 d9ng, tunaflow.dev` |
| `CONTRIBUTING.md` | tunaFlow specific — Plan-driven methodology 소개 |
| `SECURITY.md` | vuln 제보 경로 (d9ng@outlook.com) |
| `CODE_OF_CONDUCT.md` | Contributor Covenant v2.1 표준 |
| `.github/ISSUE_TEMPLATE/bug_report.yml` | bug form |
| `.github/ISSUE_TEMPLATE/feature_request.yml` | feature form |
| `.github/PULL_REQUEST_TEMPLATE.md` | PR checklist |

### `CHANGELOG.md` 는 생성하지 않음

사용자 결정 (2026-04-23): Release 탭으로 대체. GitHub Release 에 버전별 노트 작성하는 방식.

---

## Phase 3 — README 스토리 보강 + 부가 설명

### 3-1. Mission 보강 — 터미널 복붙 탄생 배경

사용자 원점 공유 (2026-04-23): tunaFlow 는 "여러 CLI 에이전트를 터미널 (cmux/tmux/iterm2) 에서 복붙으로 반복하는 고통" 을 해소하고자 시작. 이 맥락이 "코드 안 쓰는 IDE" 포지셔닝과 결합해 설득력 있음.

**README.md (영문) "Who is this for?" 섹션 하단** 에 문단 추가:

```markdown
### Why it exists
tunaFlow started from a concrete pain: running Claude Code, Codex, and Gemini CLI side by side meant constant copy-pasting between tmux panes, iTerm tabs, or terminal multiplexers like cmux. Even when the individual engines were great, the workflow was manual stitching. tunaFlow bundles that stitching into a single surface so the user's attention stays on intent, not on terminal pane management.
```

**README.ko.md** 동일 문단 한국어:

```markdown
### 왜 만들어졌나
tunaFlow 는 구체적 고통에서 시작됐습니다 — Claude Code / Codex / Gemini CLI 를 동시에 쓸 때 tmux / iTerm / cmux 등 터미널에서 복붙으로 반복하는 작업이 많다는 것. 각 엔진은 강력한데 워크플로우는 수동 조립이었습니다. tunaFlow 는 그 조립을 한 화면 안에 묶어, 사용자의 주의가 "터미널 pane 관리" 가 아니라 "의도" 에 머물게 합니다.
```

### 3-2. "Built with tunaFlow" 섹션 신설

**README.md 의 "Known Constraints (Beta)" 섹션 직후** 에 추가:

```markdown
---

## Built with tunaFlow

tunaFlow itself and the following projects are developed entirely through the Plan → Dev → Review workflow of tunaFlow. No manual coding by the user.

| Project | Description | Repository |
|---|---|---|
| **tunaFlow** | This repo. The orchestration client itself. | (current) |
| **secall** | Korean/English hybrid search CLI library. | (TBD — link to be added) |
| **tunaReader** | Document reader with AI highlighting. | (TBD) |
| **tunaInsight** | Code analysis dashboard. | (TBD) |

All commits carry `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`. See the git log for proof of the 100% AI-authored workflow.
```

사용자 repo URL 확정 후 TBD 교체.

### 3-3. "References" 섹션 — `_util/` 감사

**README.md 하단**, "Contact" 위 또는 아래:

```markdown
---

## References & Acknowledgments

tunaFlow borrows ideas from several independent projects. They are not bundled in this repository, but their influence shaped parts of tunaFlow's design. Thanks to the following maintainers:

- **[\_util repo name]** ([URL]) — ideas adopted into tunaFlow's [concrete area, e.g. "context assembly logic"].
- (Add more as needed.)
```

사용자가 `_util/` 의 원 repo URL 과 **어떤 아이디어를 차용** 했는지 구체화 필요.

### 3-4. 배지 변경

**README.md / README.ko.md 상단 배지**:

```markdown
<!-- before -->
[![License](https://img.shields.io/badge/License-Private-9ca3af)](.)

<!-- after -->
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue)](./LICENSE)
[![Status](https://img.shields.io/badge/Status-Beta-f59e0b)](./docs/plans/betaReleaseReadinessPlan.md)
```

### 3-5. `CLAUDE.md` 상단 1줄 설명

**CLAUDE.md 첫 줄 앞에** 추가:

```markdown
> This file is the system-level handoff document for AI agents (Claude Code, Codex, Gemini) operating on tunaFlow. Human readers: see [README.md](./README.md) for the product overview and [docs/](./docs/) for architecture and design plans.
```

### 3-6. `docs/plans/index.md` 외부 독자 안내

**맨 위에 추가**:

```markdown
> **For external readers**: these are active development plans — artifacts of tunaFlow's self-hosting (Plan → Dev → Review) methodology. You do **not** need to read these to use tunaFlow. For product overview see [README](../../README.md). For architecture see [docs/reference/](../reference/).
```

### 3-7. `agents/` 디렉토리에 README 추가

**`agents/README.md`** (신규, 짧게):

```markdown
# tunaFlow bundled agent prompts

These are system prompts used by tunaFlow's internal agents (Architect, Developer, Reviewer, etc.). They are loaded at runtime by the persona system.

**Users**: you do not need to edit these files. To change an agent's behavior per project, use `Settings > Agents` or per-project `AGENTS.md`.

**Contributors**: see [CONTRIBUTING.md](../CONTRIBUTING.md) for how to propose changes to bundled prompts.
```

### 3-8. `INSTALL.md` 역할 명시

**INSTALL.md 맨 위에 추가**:

```markdown
> This file is an AI-agent-readable install guide (used by Claude Code / Codex / Gemini during onboarding). **Human users**: [README.md](./README.md) has everything you need.
```

---

## Phase 4 — 크리티컬 오류 점검

공개 직전 필수 확인. Developer 실행.

### Build 검증

```bash
# Rust
cd src-tauri && cargo build --release
cd .. && cargo check --manifest-path src-tauri/Cargo.toml

# Frontend
npm install
npm run build            # TypeScript + Vite
npm run tauri build      # Tauri desktop bundle (optional, 느림)

# Test
cargo test --lib --manifest-path src-tauri/Cargo.toml
npx vitest run
```

모두 녹색 확인.

### 민감 데이터 grep

```bash
# API key / token patterns
grep -rE "sk-[a-zA-Z0-9]{20,}|sk-proj-|sk-ant-|ghp_[a-zA-Z0-9]{36}|gho_[a-zA-Z0-9]{36}" \
  --include="*.rs" --include="*.ts" --include="*.tsx" --include="*.json" \
  --include="*.md" --include="*.toml" .

# 절대 경로 (개인 정보)
grep -rE "/Users/[a-zA-Z0-9]+/" \
  --include="*.rs" --include="*.ts" --include="*.tsx" --include="*.json" \
  --include="*.md" --include="*.toml" . | grep -v node_modules | grep -v ".git"

# Third-party 이메일 / URL (본인 외)
grep -rE "[a-zA-Z0-9._%+-]+@(?!outlook\.com)[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}" \
  --include="*.md" .
```

### Broken link 검사

docs/ 내 markdown link 의 존재 여부:

```bash
# 간단 grep (완전하진 않음)
grep -rnE "\]\((\./|\.\.\/|docs/|src/|src-tauri/)" docs/ README*.md CLAUDE.md \
  | while IFS=':' read -r file line content; do
      # 각 link 추출 후 파일 존재 확인 (수동 or 스크립트)
      echo "$file:$line $content"
    done
```

또는 `markdown-link-check` 같은 npm 패키지로.

### TODO/FIXME 현황 재확인

```bash
grep -rnE "TODO|FIXME|XXX|HACK" src/ src-tauri/src/ \
  --include="*.rs" --include="*.ts" --include="*.tsx"
# 현재 3건. 공개 전 내용 검토 — 민감 지점이면 fix, 아니면 그대로.
```

---

## Phase 5 — i18n 영문판 완성 + Public 전환

### 5-1. i18n 영문판 확인

`docs/plans/i18nPlan.md` Phase 1~3 완료 후:
- Settings > Language 드롭다운 동작
- chat / settings / workflow / branch / insight / dialog 주요 화면 영문 렌더
- 번역 누락 필드는 `fallbackLng: 'en'` 으로 영어 표시 (INV-4)

### 5-2. Repo visibility 전환

```bash
# GitHub CLI
gh repo edit --visibility public

# 또는 GitHub Settings > Danger Zone > Change visibility
```

### 5-3. First release tag

```bash
git tag v0.1.0-beta -m "First public beta release"
git push origin v0.1.0-beta

# GitHub Release 작성 (CHANGELOG 대체)
gh release create v0.1.0-beta --title "v0.1.0-beta" --notes "..."
```

Release notes 에 포함할 것:
- Vision / Mission (README 발췌)
- 지원 엔진
- 알려진 제약
- 설치 방법
- Built with tunaFlow 4 프로젝트

### 5-4. 공개 후 초기 대응

- GitHub Discussions 활성화 (Settings)
- Issue / PR 대응 SLA (초기 48시간 목표)
- README 상단에 "Star 부탁" 배너는 **지양** — 자연스러운 adoption 우선

---

## Architect 산출 파일 (이번 plan 과 함께 생성)

각 파일 전문은 별도 파일로 생성됨. Developer 는 존재 확인 후 commit:

- [x] `LICENSE` — Apache 2.0 전문 + Copyright
- [x] `CONTRIBUTING.md` — tunaFlow Plan-driven methodology
- [x] `SECURITY.md` — 제보 경로
- [x] `CODE_OF_CONDUCT.md` — Contributor Covenant v2.1
- [x] `.github/ISSUE_TEMPLATE/bug_report.yml`
- [x] `.github/ISSUE_TEMPLATE/feature_request.yml`
- [x] `.github/PULL_REQUEST_TEMPLATE.md`

---

## Developer 핸드오프 프롬프트

Phase 0-5 전부 실행 지시. 약 반나절~1일 작업:

```
tunaFlow Architect 산출. Public release 준비 전체 작업.

## Phase 0 — CRITICAL (먼저)

1. `git log --all --oneline -- tunaflow.db` 로 history 포함 여부 확인
2. DB 내용 검사:
   - sqlite3 tunaflow.db 에서 민감 데이터 grep (API key / 3자 대화)
   - 민감 데이터 있으면 `git filter-repo --path tunaflow.db --invert-paths`
3. 단순 untrack:
   - git rm --cached tunaflow.db
   - echo "/tunaflow.db" >> .gitignore

## Phase 1 — cleanup

1. Root debris 정리:
   - git rm claude-new.png screenshot-2026-04-08-*.png screenshot-2026-04-14-*.png
   - git mv resume_claude.md resume_claude_archtect.md resume_codex.md resume_gemini.md docs/prompts/archive/
2. `_util/` 제거:
   - git rm -r _util/
   - echo "/_util/" >> .gitignore
3. `coverage/` gitignore:
   - echo "/coverage/" >> .gitignore
4. `_stash_recovery/` 검토 후 삭제 or archive 이동
5. `evals/scripts/run-eval.mjs` cleanup 기본값 전환 (외부 기여자 안전장치):
   - 파일: `evals/scripts/run-eval.mjs` line 373 근처
   - 기존: `if (process.argv.includes("--cleanup") || process.env.EVAL_CLEANUP === "1")`
   - 변경: `if (!process.argv.includes("--no-cleanup") && process.env.EVAL_CLEANUP !== "0")`
   - 주석도 opt-in → opt-out 으로 수정
   - README (`evals/README.md`) 의 사용법 섹션에 `--no-cleanup` / `EVAL_CLEANUP=0` 옵션 추가 기재
   - 이유: 외부 기여자가 eval 돌릴 때 projects 테이블에 `[eval] <label>` 스크래치가 남지 않도록 안전 기본값 확보

## Phase 2 — hygiene 파일 commit

Architect 가 생성한 파일 확인 + 스테이지 + 커밋:
- LICENSE
- CONTRIBUTING.md
- SECURITY.md
- CODE_OF_CONDUCT.md
- .github/ISSUE_TEMPLATE/bug_report.yml
- .github/ISSUE_TEMPLATE/feature_request.yml
- .github/PULL_REQUEST_TEMPLATE.md

## Phase 3 — README 보강

publicReadinessChecklistPlan.md §3 의 7개 항목 반영 (Mission / Built with tunaFlow / References / 배지 / CLAUDE.md / docs/plans/index.md / agents/README.md / INSTALL.md).

단 "Built with tunaFlow" 4 프로젝트 URL + "References" _util 원 repo URL 은 사용자 확인 후 교체.

## Phase 4 — 크리티컬 점검

publicReadinessChecklistPlan.md §4 스크립트 실행:
- Build: cargo build --release + npm run build
- Test: cargo test --lib + npx vitest run
- 민감 데이터 grep
- Broken link 검사

## Phase 5 — 공개 (i18n 영문판 완료 후)

i18n Phase 1~3 완료 확인 → gh repo edit --visibility public → git tag v0.1.0-beta

## Commit 전략

Phase 별로 commit 분리:
- Phase 0 + 1: "chore: untrack dev DB + clean root debris"
- Phase 2: "docs: add OSS hygiene files (LICENSE Apache 2.0, CONTRIBUTING, SECURITY, CoC, templates)"
- Phase 3: "docs(readme): mission / built-with / references sections + badges"
- Phase 4: (점검만, commit 없음)
- Phase 5: 별도 (i18n 완료 시)

Phase 0~3 통합 PR 하나로 올리는 것도 가능 (한 번의 리뷰).

## 브랜치

docs/public-release-prep (또는 유사)
```

---

## Invariants

- **[INV-1]** Public 전환 이전에 `tunaflow.db` 가 tracked 상태로 남아있으면 **공개 금지**. DB 는 dev-local 전용. 위반 시 public 이후 history rewrite 는 타인 fork 에 영향 — 복구 불가 이슈 발생.
- **[INV-2]** `_util/` 은 repo 에서 제거된 상태로 공개. 별도 repo 차용물 — 라이센스 충돌 / 중복 배포 방지. 대신 README 의 References 섹션으로 감사 + 원 repo 링크만.
- **[INV-3]** LICENSE 파일은 Apache 2.0 전문 유지. MIT 가 아닌 이유 (patent 보호 + 기여자 식별) 는 Rationale 기록.
- **[INV-4]** 공개 시점은 i18n 영문판 완료 후. 한국어만 지원하는 상태에서 공개하면 첫인상이 "local project" 로 제한됨 — 글로벌 adoption 저해.
- **[INV-5]** First release tag 는 `v0.x.x-beta` 패턴. 실제 안정화 전까지 `-beta` suffix 유지.

---

## Rationale

### Apache 2.0 선택 이유

MIT 가 간결하나 **patent grant 조항이 없어** 기여자 / 사용자 간 patent 분쟁 시 보호 부족. Apache 2.0 은:
- 명시적 patent license grant (Section 3)
- 기여자 소유 patent 침해 시 자동 license termination (방어 매커니즘)
- NOTICE 파일을 통한 attribution 체계
- 기여자 identity 요구 (Section 5)

tunaFlow 가 AI 로 작성된 코드베이스라는 특수성 — **저작권 / patent 이슈가 명확히 규정돼야 법적 안정성** 확보. Apache 2.0 이 이 기준 충족.

사용자 메모 (2026-04-23): 과거 특정 사례 ("Claude Code 유출 관련" 의 patent grant 문제) 로 Apache 선호.

### `_util/` 제거 + References 로 대체 이유

원 repo 가 별도 존재 → 사본을 tunaFlow repo 에 포함하면:
- 업데이트 시 sync 비용
- 원 repo 의 LICENSE 와 tunaFlow LICENSE 간 호환성 검토 필요
- 외부인이 "이 코드가 정말 tunaFlow 것인가" 혼란

References 섹션 + 링크로 전환 시:
- 원 repo 가 변경되어도 tunaFlow 는 영향 없음
- 감사 (attribution) 은 명시적으로 유지
- 외부인이 관심 있으면 원 repo 로 이동 가능

### 공개 시점 = i18n 완료 이유

영문 랜딩 README 는 이미 적용됨 (README.md default). 하지만 **앱 내부 UI** 는 한국어. 외부 개발자가 앱 실행 후 한국어 UI 를 만나면 adoption 막힘 — i18n Phase 1~3 완료 후 영문 UI 렌더 가능.

### Phase 0 의 우선순위

`tunaflow.db` 는 공개 직전이 아니라 **지금** 처리. 이유:
- Public 전환 후 노출되면 복구 불가
- 민감 데이터 여부 확인에 시간 걸림 (DB 열어서 row 검사)
- Git filter-repo 는 로컬 작업이라 사전 실행 문제 없음

### 공개 이후

- GitHub Discussions 활성화 — issue 와 별개 대화 공간
- 적극 홍보 (HN / Reddit / Twitter) 는 **첫 1주 관찰** 후 결정 — 초기 crash bug 노출 방지
- LLM wiki 180/30 benchmark 대비 tunaFlow 의 강점 (multi-engine + dogfood + methodology) 을 community 대화로 전달

---

## 관련 문서

- first-time visitor 시뮬레이션 결과: 본 세션 Architect 답변
- LICENSE 선택 배경: 본 plan Rationale
- i18n 영문판 plan: `docs/plans/i18nPlan.md`
- README 구조: `README.md` (영문 default), `README.ko.md`
- Token Policy: `docs/reference/tokenPolicyReference.md`
