---
title: Windows CI 파이프라인 — PR-level / nightly / self-hosted 회귀 검출 정책
status: draft
priority: P0 (회귀 누적 차단 — 즉시 결정 필요)
created_at: 2026-04-29
calling_role: architect (Windows 머신)
related:
  - .github/workflows/ci.yml
  - .github/workflows/build.yml
  - docs/plans/selfTrustCiTriggerOptimizationPlan_2026-04-25.md   # CI trigger SSOT
  - docs/plans/windowsBuildPlan_2026-04-24.md                      # release artifact 빌드
  - docs/plans/windowsBetaHardeningPlan_2026-04-26.md
  - docs/plans/windowsDependencyBootstrapPlan_2026-04-29.md
canonical: true
---

# Windows CI 파이프라인 — PR-level / nightly / self-hosted 회귀 검출 정책

## 0. 요약 (1 단락)

현재 `.github/workflows/ci.yml` 은 **self-hosted Linux 단일 OS** 에서만 PR 검증을 돈다.
이는 `selfTrustCiTriggerOptimizationPlan_2026-04-25` 의 의도된 설계 — *"redundant CI"
가 아닌 외부 contributor PR + release tag 만 검증* — 이지만 **OS 차이로 인한 회귀는
redundant 가 아니다**. 2026-04-29 시점 *Windows architect 머신* 에서 작업 중 path-separator
회귀 5건이 발견됐다 (`conventions_sync.rs` 1건 = PR #213, `commands::files::tests` 4건 = T2
escalate). 모두 Linux CI 를 통과한 후 main 에 진입한 사례다. 이 plan 은 (1) Windows
회귀를 *반드시* 검출하는 메커니즘을 도입하고, (2) self-trust 모델의 working-memory
fragmenting 회피 가치를 깨지 않으며, (3) 현재 머신/runner 자원으로 가능한 단계적
roll-out 을 정의한다.

## 1. 배경 — 회귀 이력 (회귀 누적 증거)

| 회귀 | 파일 | 발견 경로 | 원인 |
|---|---|---|---|
| PR #213 | `src-tauri/src/commands/conventions_sync.rs:347` | Windows architect 가 dev 모드 cargo test 시 발견 | `Path::display()` 가 Windows backslash 출력 → `@` import 에서 escape 문제 |
| escalate-1 | `commands::files::tests::all_scope_skips_hidden_dirs` | T2 진행 중 cargo test --lib 에서 발견 | `paths.iter().any(|p| p.ends_with("docs/a.md"))` 가 Windows backslash path 와 미매치 |
| escalate-2 | `commands::files::tests::tunaflow_scope_includes_dot_github_and_skips_hidden_md` | 동일 | 동일 |
| escalate-3 | `commands::files::tests::all_scope_walks_recursively_and_respects_gitignore` | 동일 | 동일 |
| escalate-4 | `commands::files::tests::tunaflow_scope_returns_root_md_and_docs_only` | 동일 | 동일 |

**공통점**:
- Linux self-hosted CI 는 5건 모두 **녹색**으로 통과시킨 후 main 머지.
- `cfg(unix)` gate 가 Linux 도 매치하므로 Linux 환경에서 path-sep 비교가 정상 동작.
- Windows architect 가 *수동 cargo test* 로 발견.
- 본 회귀들이 사용자 가시성을 가질 가능성: `@` import 깨짐 (PR #213) 은 직접적, test-only 4건 은 간접적 (production 동작 검증 신뢰도 ↓).

**암묵적 가정**:
- 회귀는 5건이 끝이 아닐 가능성 높음 — 같은 패턴 grep audit (R-W-1) 로 추가 검출 필요.

## 2. 현재 CI/CD paradigm 정리

| Workflow | trigger | runs-on | 검증 항목 | 본 plan 변경 대상 |
|---|---|---|---|---|
| `ci.yml` | `pull_request: branches=main, paths-ignore=docs/**` | self-hosted Linux | rust-check + frontend-check (cargo check/test, tsc, vitest) | ✅ **본 plan 대상** |
| `build.yml` | `tags: v*.*.*` | matrix (macos-latest, windows-latest, etc.) | rawq sidecar build, tauri NSIS/DMG | (release artifact, 본 plan 무관) |
| `eval-regression.yml` | (별 검토 필요, 본 plan 범위 외) | — | — | — |

**self-trust 모델의 명문**:
> *"Self-trust 작업 (Architect / Developer subagent batch) — 로컬에서 cargo check / tsc / cargo test / vitest 다 돌림 → CI redundant"* (selfTrustCiTriggerOptimizationPlan §"CI 가치 평가")

이 가정의 한계:
- "로컬" = mac architect 의 mac 머신 또는 Windows architect 의 Windows 머신. **두 머신 동시 실행 안 함**.
- mac architect 가 PR 을 머지하면 Windows 측 회귀는 검출 안 됨. 반대도 동일.
- 회귀는 머지 후 *다른 환경 (Windows architect 의 머신)* 에서 작업하다가 발견 → 별 PR 생성 → 추가 working memory cost. self-trust 의 이득(CI wait 회피) 을 *상쇄하거나 초과*.

**결론**: self-trust 모델은 단일 OS 환경에선 합리적. 두 OS 환경에선 *OS 회귀에 한해* 부분적으로 깨야 함.

## 3. Invariants

| ID | 내용 |
|---|---|
| **INV-CI-1** | self-trust 모델의 working-memory 비용 회피 가치 보존. PR-level 으로 5분 wait 강제 부활 시키지 말 것. |
| **INV-CI-2** | OS 회귀는 *반드시* 검출되어야 함. 검출 시점이 PR 직후 / nightly / main push 후 어디든 OK, 단 회귀가 사용자에게 도달하기 *전*. |
| **INV-CI-3** | 자원 경제성. GitHub-hosted windows-latest 는 macos-latest 와 같은 비용 등급 (Linux 대비 2× minute charge). nightly 1회 정도는 허용, 매 PR 강제는 비용 부담. |
| **INV-CI-4** | docs-only / markdown-only / .github 템플릿 등은 제외 (현행 paths-ignore 유지). |
| **INV-CI-5** | self-hosted Windows runner 가능성 — Windows architect 머신을 runner 로 등록할 경우 비용 0 + 검출 즉시. 단 머신 가용성 (사용자 ON/OFF) 의 영향 명시. |
| **INV-1** (기존) | macOS 변경 0 — Windows job 추가는 macOS job 동작에 영향 없음. |

## 4. 옵션 비교

| 옵션 | trigger | 비용 | 검출 latency | working-memory 영향 | 권장도 |
|---|---|---|---|---|---|
| **A. nightly cron windows-latest** | `schedule: cron '0 18 * * *'` | low (1회/일 × ~10분) | 24h 까지 | low (사용자가 wait 안 함) | ⭐⭐⭐ Phase 1 P0 |
| **B. main push windows-latest** | `push: branches=main, paths-ignore=docs/**` | medium (머지마다 1회) | <30분 | low (백그라운드 통지) | ⭐⭐⭐ Phase 1 P0 (보강) |
| **C. PR-level windows-latest 매트릭스** | `pull_request: paths-ignore=docs/**` | high (PR 마다 ~10분 추가) | <30분 | **medium-high** (외부 contributor PR 에는 유지 필요, self-trust 작업에는 부담) | ⭐ Phase 3 후 재검토 |
| **D. self-hosted Windows runner** | 위 A/B/C 어느 trigger 와도 결합 가능 | 0 (Windows architect 머신 활용) | <5분 | 0 (배치 처리) | ⭐⭐⭐⭐ Phase 2 P1 |
| **E. 디벨로퍼 수동 검증 책임 (현행)** | 없음 | 0 | 디벨로퍼 다음 작업 때 | high (escalate fix 누적) | 본 plan 의 *대체* 대상 |

**조합 권장**: **A + B + D**. nightly 가 retroactive safety net, main push 는 빠른 검출, self-hosted 는 비용 0 + 즉시. PR-level (옵션 C) 은 self-trust 모델 정신과 충돌 가능성 있어 P2 이상으로 미룸.

## 5. Phase 분해

### Phase 1 — windows-latest GitHub-hosted 도입 (P0, 즉시)

#### W-CI-1 — `.github/workflows/ci.yml` 에 Windows job 추가 (main push + nightly)

```yaml
# 추가 안 (현재 ci.yml 에 append):
windows-check:
  runs-on: windows-latest
  if: github.event_name == 'push' || github.event_name == 'schedule'
  steps:
    - uses: actions/checkout@v5
    - name: Setup Rust
      uses: dtolnay/rust-toolchain@stable
    - name: Rust cache (Windows)
      uses: Swatinem/rust-cache@v2
      with: { workspaces: src-tauri }
    - name: Setup Node
      uses: actions/setup-node@v5
      with: { node-version: 22, cache: npm }
    - name: Install
      run: npm install --no-audit --no-fund
    - name: Sidecar placeholders
      shell: pwsh
      run: |
        New-Item -ItemType Directory -Force src-tauri/binaries
        $triple = "x86_64-pc-windows-msvc"
        foreach ($name in "rawq","crg","chub") {
          New-Item -ItemType File -Force "src-tauri/binaries/$name-$triple.exe"
        }
    - name: TypeScript
      run: npx tsc --noEmit
    - name: Vitest
      run: npx vitest run
    - name: Cargo check
      working-directory: src-tauri
      run: cargo check
    - name: Cargo test
      working-directory: src-tauri
      run: cargo test --lib
```

`on:` 섹션 보강:

```yaml
on:
  pull_request:
    branches: [main]
    paths-ignore: [...]
  push:
    branches: [main]                    # ← 신규 (Windows job 한정)
    paths-ignore: [...]
  schedule:
    - cron: '0 18 * * *'                # 매일 03:00 KST = 18:00 UTC 전날
```

**self-trust 충돌 회피**: `if: github.event_name == 'push' || github.event_name == 'schedule'` — pull_request trigger 에서는 windows-check 가 *skip*. 즉 외부 contributor PR 검증은 기존 Linux job 만, Windows 검출은 main 머지 후 또는 nightly. INV-CI-1 충족.

#### W-CI-2 — Windows job 실패 시 알림 통로

- main push 직후 Windows job 실패 → architect 인식 시점 = 다음 cargo test 또는 GitHub 알림 직접 확인.
- 권장: **별 plan 외 axis** — `gh notification`, slack, 또는 Push notification (현재 active 한 `nativeNotificationPlan_2026-04-29` 와 묶을 수 있음). 본 plan 은 *알림 메커니즘 구체화 안 함*. notification axis 는 별 plan.

#### W-CI-3 — path-separator 회귀 grep audit (회귀 5건 같은 패턴 추가 검출)

- 본 plan **밖** axis 지만 motivation 강함. fix PR 진행 시 같은 패턴 grep:
  ```
  rg '\.ends_with\("[^"]*\/[^"]*"\)' src-tauri/src/
  rg '\.contains\("\/[^"]+/[^"]+"\)' src-tauri/src/
  rg 'Path::new\("[^"]*\/[^"]*"\)' src-tauri/src/
  ```
- 결과는 별 fix PR 또는 windowsBetaHardeningPlan §C/§D 에 편입.

### Phase 2 — self-hosted Windows runner (P1)

#### W-CI-4 — Windows architect 머신 self-hosted runner 등록

- 등록 절차: GitHub repo → Settings → Actions → Runners → Add runner (Windows).
- Windows architect 가 작업 시작 시 runner 켜고, 종료 시 끔 (자동화 가능 — Task Scheduler).
- W-CI-1 의 `runs-on: windows-latest` 를 `runs-on: [self-hosted, Windows]` 로 분기 (동일 OS, 다른 호스팅).
- **장점**: 비용 0 + 검출 latency <5분 + Windows architect 의 dev 환경 그대로 (chub/crg 등 사전 설치).
- **단점**: 머신 OFF 시 Windows job pending. → fallback 으로 windows-latest 가 자동 잡히는 fail-over 정책 필요 (또는 OFF 시점 명시 정책).

#### W-CI-5 — runner OFF 시 fallback 정책

- 옵션 a: `runs-on: [self-hosted, Windows]` + queue. OFF 시 Windows architect 가 다음 켤 때 처리 (latency 길어짐).
- 옵션 b: `runs-on: ${{ vars.WINDOWS_RUNNER || 'windows-latest' }}` — repo variable 로 토글. 머신 OFF 시 사용자가 var 변경 → GitHub-hosted fallback.
- 옵션 c: matrix 로 둘 다 등록 후 fastest one wins (cancel-in-progress) — 복잡도 ↑.

**권장**: 옵션 a (단순). OFF 시 nightly cron 이 windows-latest 로 retroactive 잡음.

### Phase 3 — PR-level Windows 검증 (P2, 재검토 후)

W-CI-1/4 누적 운영 1~2주 후, working-memory 비용과 회귀 검출 즉시성 trade-off 재평가. PR-level 도입 가치가 명확하면 별 plan 으로 follow-up.

## 6. 작업 분해 — developer 인계용

| Task | 파일 | 검증 명령 | 예상 LOC | 우선순위 |
|---|---|---|---|---|
| **W-CI-1** | `.github/workflows/ci.yml` (windows-check job + on: schedule/push 추가) | workflow_dispatch 로 dry-run, main push 시 첫 회 통과 | +60 / -0 | P0 |
| **W-CI-2** | (별 plan 외 axis) — 본 plan 에서는 motivation 만 명시 | — | — | P1 (별 plan) |
| **W-CI-3** | (별 fix PR 또는 windowsBetaHardening §C 편입) | grep audit + path-sep fix | 가변 | P1 |
| **W-CI-4** | GitHub Settings UI + ci.yml runs-on 분기 | runner 등록 후 첫 job pickup 확인 | +5 / -3 | P1 |
| **W-CI-5** | 정책 문서화 (본 plan §5 옵션 a) + ci.yml 변경 없음 | docs only | — | P1 |

본 plan 의 즉시 PR 범위 = **W-CI-1**. W-CI-4/5 는 self-hosted runner 등록을 사용자가 마친 후 follow-up.

## 7. 회귀 가드 / 검증 시나리오

### 7.1 macOS 무영향성 (INV-1)

- W-CI-1 은 새 job 추가만, 기존 rust-check / frontend-check 스펙 미변경 → mac 측 (Linux self-hosted) 동작 0 변화.
- `if: github.event_name == ...` 으로 PR trigger 에서 Windows job skip → mac architect 의 PR wait 시간 증가 0.

### 7.2 Windows job 첫 도입 검증

| ID | 시나리오 | 기대 결과 |
|---|---|---|
| WCI-V1 | W-CI-1 머지 후 main push (다음 commit) → Windows job pickup | 정상 통과 (현재 main 의 baseline 통과 — 회귀 없는 상태에서 시작) |
| WCI-V2 | nightly cron 발화 (다음 18:00 UTC) → Windows job 정상 실행 | 정상 통과 |
| WCI-V3 | escalate 4건 회귀 fix PR 머지 *후* main push → Windows job 통과 | 회귀 0 |
| WCI-V4 | 의도적 Windows-breaking PR 시뮬레이션 (예: backslash 의존 코드) → main 진입 후 Windows job 실패 | 실패 + 사용자 인식 |
| WCI-V5 | docs-only PR → Windows job skip | paths-ignore 정상 |
| WCI-V6 | self-hosted runner OFF 상태 + Windows-only branch 작업 push | Windows job pending → 머신 ON 시 처리 (옵션 a) |

### 7.3 baseline 카운트

- W-CI-1 머지 시점의 main 기준 baseline:
  - macOS / Linux: cargo test --lib 568 passed (디벨로퍼 보고 569 - T2 추가 1, 추정. 정확한 값은 W-CI-1 PR 작성 시 측정)
  - Windows: cargo test --lib 569 passed + 4 failed (= 573, escalate 4건)
- escalate fix PR 머지 후 양 환경 모두 +N 또는 동일.

## 8. 리뷰어(Codex / mac architect) review 포인트

- **R-W-1** self-trust 모델 정신을 깨지 않는가 — pull_request trigger 에 Windows job 추가는 INV-CI-1 위반. W-CI-1 의 `if: ... push || schedule` 분기 정확성.
- **R-W-2** sidecar placeholders 가 Windows 에서 정상 동작 — `New-Item -ItemType File` 이 0 byte file 생성, Tauri build script 가 file existence 만 검사하므로 OK. 단 .exe 확장자 명시 누락 시 회귀.
- **R-W-3** Rust cache key (`Swatinem/rust-cache@v2`) 가 OS 별 분리되는지 — 디폴트로 분리됨, 그러나 명시 권장.
- **R-W-4** windows-latest 의 GitHub Actions cost — 사용자 GitHub plan 의 minute quota 초과 위험 평가. 현재 Linux self-hosted 라 quota 영향 0 → Windows job 도입 시 처음으로 quota 사용. nightly + main push 빈도로 월 ~20~50 runs × 10분 = ~300분/월 예상.
- **R-W-5** schedule cron 시간 — 18:00 UTC 가 Windows architect 의 EOD 시점과 겹치는지 (한국 03:00). nightly 결과를 다음 morning 에 보는 것이 자연스러움. 사용자 시간대 확인.
- **R-W-6** Windows job 실패 시 알림 통로가 본 plan 에서 미정의 — W-CI-2 에 motivation 만 적고 별 plan 으로 미루는 것이 옳은지, 아니면 본 plan 에 최소 GitHub email notification 명시는 포함할지.

## 9. 오픈 질문

| Q | 결정 필요한 사항 |
|---|---|
| **Q-WCI-1** | windows-latest 비용 — 월 quota 초과 시 self-hosted only 로 후퇴 정책 미리 합의할지. |
| **Q-WCI-2** | self-trust 모델 부분 수정 명문화 — `selfTrustCiTriggerOptimizationPlan_2026-04-25` 본문에 *"OS 회귀는 예외"* 한 줄 추가할지, 또는 본 plan 으로만 cross-reference 둘지. |
| **Q-WCI-3** | nightly cron 시간 (Q-W-5 와 묶음). |
| **Q-WCI-4** | self-hosted Windows runner 등록 시점 — Phase 2 P1 즉시 vs Phase 1 P0 검증 후. |
| **Q-WCI-5** | 알림 메커니즘 — 본 plan 포함 vs 별 plan (nativeNotificationPlan 과 묶음 가능성). |

## 10. 진행 메모

- 본 plan 의 motivation 은 디벨로퍼가 T2 진행 중 escalate 4건을 발견한 사건 (2026-04-29). 같은 패턴 회귀가 누적되는 paradigm 한계 명확.
- `windowsDependencyBootstrapPlan_2026-04-29` 의 §6.2 W-1~W-9 시나리오는 *수동* 검증 — 본 plan 의 W-CI-1 가 도입되면 일부 자동화 가능. 단 dialog UX 같은 UI 항목은 여전히 수동.
- self-hosted Windows runner 등록 (W-CI-4) 은 Windows architect 가 본 머신을 *대부분 켜놓는* 운용 패턴이면 매우 강한 옵션. 사용자 운용 패턴 확인 후 결정.
- 본 plan 작성 직후 mac architect review 권장 (Q-WCI-1~5 결정), 그 다음 W-CI-1 PR 진행.
