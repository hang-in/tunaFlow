---
title: rawq sidecar release gap — 사용자 환경 미인식 진단/수정
status: ready-to-implement
priority: P1 (Beta 첫 사용자 보고 — rawq "없음" 표시 3 케이스 수렴)
created_at: 2026-04-26
related:
  - src-tauri/tauri.conf.json                # externalBin: ["binaries/rawq"]
  - .github/workflows/build.yml              # build-rawq + build-tauri-lite
  - scripts/build-rawq.sh                    # 로컬 sidecar 빌드
  - scripts/build.sh                         # 올인원 wrapper
  - install.sh                               # macOS 자동 설치 (xattr -cr 처리)
  - README.md                                # 사용자 가이드
canonical: true
owners:
  - architect (본 plan 작성)
  - developer (구현)
---

# 증상 (사용자 보고, 2026-04-26)

> "rawq 가 계속 없다고 나와요.
> - 배포 release 받아 실행해도 없다고 나옴
> - rawq 공식 `cargo install rawq` 해도 인식 안 됨
> - 직접 빌드해보니 `binaries/rawq-aarch64-apple-darwin doesn't exist` 라고 뜨며 중단됨"

3 케이스 모두 동일한 표면 메시지("rawq 없음")로 노출되어 사용자가 어느 경로가 맞는지 판단 불가.

---

# 진단 (Architect 사전 분석)

| # | 케이스 | 진단 | 비고 |
|---|---|---|---|
| A | DMG release 설치본에서 미인식 | `build.yml` 의 `build-tauri-lite` job 이 `build-rawq` 결과를 download artifact 로 받아 `src-tauri/binaries/rawq-{triple}` 에 배치 후 tauri-action 빌드 → 정상이면 DMG 안에 sidecar 들어가야 함. **실측 검증 필요** — release DMG `.app/Contents/MacOS/` 또는 `Resources/` 안에 sidecar 실재/실행권한/quarantine 여부 | 사용자 첫 인상 → P1 핵심 |
| B | `cargo install rawq` 후에도 미인식 | tunaFlow 는 Tauri sidecar 만 호출 (`tauri.conf.json:40` `externalBin: ["binaries/rawq"]`). 시스템 PATH 의 rawq 무시는 의도된 디자인. **README 명시 부재** | UX 문서 문제 |
| C | 직접 빌드 시 `binaries/rawq-aarch64-apple-darwin doesn't exist` | 사용자가 `npm run tauri build` 직접 실행했을 가능성. `scripts/build.sh` wrapper 가 사이드카 빌드 자동 처리하지만 README 의 빌드 가이드 부족 | 빌드 흐름 안내 부족 |

---

# Fix Scope

## Layer A — Release DMG 검증/수정 (P1 핵심)

### A1. Audit — release DMG 안 sidecar 실재 확인 (선행)

1. `gh release download v0.1.1-beta -p '*.dmg'` 로 DMG 다운로드
2. `hdiutil attach tunaFlow_0.1.1-beta_aarch64.dmg` mount
3. mount 된 볼륨의 `tunaFlow.app/Contents/MacOS/` 또는 `Contents/Resources/` 안에 `rawq-aarch64-apple-darwin` 존재 + `chmod +x` 권한 확인
4. `xattr -p com.apple.quarantine` 으로 quarantine 부착 여부 확인
5. 결과를 `docs/reference/rawqSidecarReleaseAudit_2026-04-26.md` 에 기록 (스크린샷·`ls -la`·`xattr` 출력 포함)

audit 결과에 따라 분기:
- **sidecar 누락** → A2 (build pipeline 결함 수정)
- **sidecar 존재 + quarantine 부착 / 실행권한 누락** → A3 (install.sh / README 우회 안내 보강)
- **sidecar 정상** → 코드측 호출 경로 점검 (`src-tauri/src/agents/rawq.rs` `Command::new_sidecar("rawq")` 실패 사유 로깅 강화)

### A2. Build pipeline 보강 (audit 결과 = 누락 시)

- `build.yml` 의 `build-tauri-lite` job 안에 `Download rawq sidecar` step 직후 진단 step 추가:
  ```yaml
  - name: Verify rawq sidecar staged for tauri-action
    shell: bash
    run: |
      ls -la src-tauri/binaries
      test -x src-tauri/binaries/rawq-${{ matrix.triple }}${{ matrix.ext }} \
        || { echo "::error::rawq sidecar missing or non-executable"; exit 1; }
  ```
- tauri-action 빌드 후 `.app` 산출물 안 sidecar verify step:
  ```yaml
  - name: Verify rawq sidecar in built bundle
    if: matrix.runner == 'macos-latest'
    run: |
      APP_PATH="src-tauri/target/release/bundle/macos/tunaFlow.app"
      find "$APP_PATH" -name "rawq-*" -ls
      test -n "$(find "$APP_PATH" -name 'rawq-*' -perm -u+x)" \
        || { echo "::error::rawq sidecar missing in built .app"; exit 1; }
  ```

### A3. install.sh / README quarantine 안내 보강

- DMG drag-install 사용자가 install.sh 우회 시 `xattr -cr /Applications/tunaFlow.app` 안 해서 sidecar 실행 차단되는 케이스 보강
- README 의 macOS 설치 섹션에 drag-drop 케이스 명시:
  - "DMG 직접 drag 후 처음 실행 시 `xattr -cr /Applications/tunaFlow.app` 실행 권장 (install.sh 사용 시 자동)"

## Layer B — UX/문서 (B/C 케이스 해결)

### B1. README "rawq" 섹션 신규 또는 보강

추가 내용:
- "tunaFlow 는 시스템 PATH 의 rawq 가 아닌 앱 번들 내부 sidecar 만 사용합니다. `cargo install rawq` 는 영향 없습니다."
- "직접 빌드 시 `./scripts/build.sh` 를 권장합니다 (사이드카 자동 빌드 + Tauri build 처리). `npm run tauri build` 직접 실행 시에는 사전에 `./scripts/build-rawq.sh` 가 필요하며, rawq 소스가 `vendor/rawq/` 또는 `RAWQ_SRC` env 로 지정한 경로에 있어야 합니다 (upstream: https://github.com/auyelbekov/rawq)."
- (선택) Settings → Runtime → rawq 상태 확인 방법

### B2. 사용자 가시 에러 메시지 명료화

- `RawqStatus.unavailable` (또는 `binary_missing`) 표시 메시지에 다음 액션 포함:
  - "앱 번들 내부 rawq sidecar 를 찾을 수 없습니다. macOS 의 경우 `xattr -cr /Applications/tunaFlow.app` 후 재시도, 또는 https://github.com/hang-in/tunaFlow/blob/main/README.md 참조"
- Settings → Runtime → rawq 섹션의 "재빌드" 버튼 (#180 fix 산출물) 옆에 "도움말" 링크 추가

### B3. Build wrapper 부재 시 README pre-flight 메시지

- (선택) `npm run tauri build` 가 직접 실행되면 prebuild npm script 가 sidecar 부재 감지 시 build.sh 권장 메시지 출력 후 fail-fast

## Layer C — 자동 검증 (회귀 방지)

### C1. CI verify step (Layer A2 와 별도)

- `build-tauri-lite` job 끝에 빌드 산출물 .app 안 sidecar 존재/실행권한 검증 step (A2 의 verify step 을 영구화)

### C2. 사용자 보고 회귀 시나리오 INSTALL.md 명시

- "fresh DMG 다운로드 → /Applications drag → 첫 실행 → Settings 에서 rawq 상태 = ready" 시나리오를 INSTALL.md 에 smoke checklist 로 명시

---

# Invariants

- INV-1: release DMG `.app` 안 `rawq-{target}` sidecar 가 존재 + 실행 권한
- INV-2: install.sh 가 quarantine 처리 (`xattr -cr`) 자동 수행
- INV-3: drag-install 사용자도 README/INSTALL.md 의 명시적 안내로 quarantine 처리 가능
- INV-4: 직접 빌드 사용자가 `./scripts/build.sh` 권장 경로로 안내됨
- INV-5: `RawqStatus.unavailable` 시 사용자 가시 메시지가 다음 단계 액션을 제공
- INV-6: CI 가 빌드 산출물 안 sidecar 부재 시 fail (regression guard)

---

# 검증

## 자동
- `npx tsc --noEmit`
- `npx vitest run`
- `cd src-tauri && cargo check`
- `cd src-tauri && cargo test --lib`
- (Layer A2/C1 변경 시) `gh workflow run build.yml -f track=lite` 로 smoke run, verify step 통과

## 수동 smoke
1. fresh release DMG → /Applications drag → install.sh 우회 → 앱 실행 → Settings → rawq 상태 = ready
2. install.sh curl 경로 → 동일
3. `./scripts/build.sh` 로 로컬 빌드 → /Applications 설치 → rawq 상태 = ready
4. (B 케이스) `cargo install rawq` 한 후에도 sidecar-only 동작 + 사용자 가시 에러 메시지 안내성 확인
5. (C 케이스) `npm run tauri build` 직접 실행 시 build.sh 권장 또는 빌드 실패 메시지가 명료한지 확인

---

# Developer 핸드오프 프롬프트

`docs/plans/rawqSidecarReleaseGapPlan_2026-04-26.md` 의 Layer A/B/C 단계별로 작업하세요.

**작업 절차**

1. **Layer A1 (audit) 먼저** — `gh release download v0.1.1-beta -p '*.dmg'` 로 DMG 받아 mount, `.app/Contents/MacOS/` 또는 `Contents/Resources/` 안 sidecar 실재/실행권한/quarantine 검증. 결과를 `docs/reference/rawqSidecarReleaseAudit_2026-04-26.md` 에 기록.

2. **audit 결과에 따라 분기**:
   - sidecar **누락** → Layer A2 build pipeline 수정 + verify step 추가
   - sidecar **존재 + quarantine/권한 문제** → Layer A3 install.sh / README quarantine 안내 보강
   - sidecar **정상** → 코드측 호출 실패 경로 점검 (rawq.rs `Command::new_sidecar` 실패 사유 로깅 강화)

3. **Layer B (audit 결과 무관)**:
   - B1 README rawq 섹션 보강 (sidecar-only 의도 + 빌드 권장 경로)
   - B2 `RawqStatus.unavailable` 사용자 가시 메시지 명료화 (다음 단계 안내 포함)
   - B3 (선택) prebuild script 로 sidecar 부재 시 fail-fast + build.sh 권장

4. **Layer C** — CI verify step 영구화 + INSTALL.md smoke 시나리오 명시

**커밋 분할** (Layer 별 또는 audit/fix 분리):
- `docs(ref): rawq sidecar release audit 2026-04-26` (audit 결과)
- `fix(release): verify rawq sidecar staging in build pipeline` (A2)
- `docs(install): macOS drag-install quarantine 안내 + INSTALL.md smoke` (A3 + C2)
- `docs(readme): rawq sidecar-only design + build.sh 권장 경로` (B1)
- `feat(rawq): improve unavailable status messaging` (B2)
- (선택) `chore(build): prebuild sidecar presence check` (B3)
- `ci(release): verify rawq sidecar in built bundle` (C1)

각 커밋 trailer:
```
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

**검증**:
- `npx tsc --noEmit`
- `npx vitest run`
- `cd src-tauri && cargo check`
- `cd src-tauri && cargo test --lib`
- audit 단계에서 DMG mount 결과 PR body 에 인용

**PR**:
- title: `fix(release): rawq sidecar gap — DMG verify + UX docs (Beta 사용자 보고 follow-up)`
- body: audit 결과 + Layer 별 변경 + INV 충족 + 수동 smoke checklist

**주의사항**:
- audit 결과가 "sidecar 정상 + 코드 호출 실패" 로 나오면 fix 범위가 코드 (`rawq.rs` 호출 로깅) + 사용자 진단 흐름으로 좁혀지니 Layer A2/A3 는 N-A 처리 가능 (audit 문서로 근거 명시)
- Tauri `Command::new_sidecar("rawq")` 가 macOS 에서 실행권한·quarantine 어느 쪽에 영향받는지 실측 우선

---

# 셀프 이슈 본문 초안

> ## bug: rawq sidecar release gap — 사용자 환경 3 케이스 수렴
>
> ### 보고 요약
>
> Beta 첫날 외부 사용자 보고 (2026-04-26):
> - DMG release 설치본 → rawq "없음"
> - `cargo install rawq` 후에도 미인식
> - 직접 빌드 시 `binaries/rawq-aarch64-apple-darwin doesn't exist` 로 중단
>
> ### 진단
>
> 3 케이스 모두 동일 표면 메시지지만 원인이 분리됨:
> - DMG 케이스 — release pipeline 또는 quarantine
> - PATH 케이스 — sidecar-only 디자인 의도, README 명시 부족
> - 직접 빌드 케이스 — `./scripts/build.sh` 권장 경로 안내 부족
>
> 자세한 audit 은 `docs/reference/rawqSidecarReleaseAudit_2026-04-26.md` 에 기록 (Developer 가 진행).
>
> ### Plan
>
> `docs/plans/rawqSidecarReleaseGapPlan_2026-04-26.md` 의 Layer A (release/quarantine) + Layer B (UX/docs) + Layer C (CI verify) 단계로 수정.
>
> ### 회귀 방지
>
> CI 의 build-tauri-lite job 에 빌드 산출물 안 sidecar 부재 시 fail step 추가 → 다음 release 부터 동일 사고 차단.
