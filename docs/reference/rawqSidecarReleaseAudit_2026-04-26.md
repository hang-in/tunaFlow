---
title: rawq sidecar release audit (Beta v0.1.1-beta)
status: complete
created_at: 2026-04-26
updated_at: 2026-04-26
canonical: true
related:
  - docs/plans/rawqSidecarReleaseGapPlan_2026-04-26.md
  - src-tauri/src/agents/rawq.rs
  - src-tauri/tauri.conf.json
  - .github/workflows/build.yml
  - install.sh
  - INSTALL.md
  - README.md
owners:
  - developer (audit 실행)
---

# 목적

`rawqSidecarReleaseGapPlan_2026-04-26.md` Layer A1 — Beta 첫날 외부 사용자
보고("rawq 가 계속 없다고 나옵니다") 의 원인 분리. 3 케이스 (DMG release /
`cargo install rawq` / 직접 빌드) 모두 동일 표면 메시지 → 실측을 통해
어느 단계에서 깨지는지 확정.

# 입력

- 대상 release: `v0.1.1-beta` (Pre-release Draft)
- 다운로드: `gh release download v0.1.1-beta -R hang-in/tunaFlow -p '*.dmg' -p '*.app.tar.gz'`
- 호스트: macOS 25.4.0 (Darwin), arm64
- 작업 dir: `/tmp/rawq-audit-2026-04-26/`

# 절차 1 — DMG 마운트

```bash
$ hdiutil attach tunaFlow_0.1.1-beta_aarch64.dmg -readonly -nobrowse
/dev/disk4          	GUID_partition_scheme
/dev/disk4s1        	Apple_HFS                      	/Volumes/tunaFlow
```

mount 정상.

# 절차 2 — `.app/Contents/MacOS/` 안 sidecar 실재 + 실행권한

```bash
$ ls -la /Volumes/tunaFlow/tunaFlow.app/Contents/MacOS/
.rwxr-xr-x 1.5M d9ng 25 4월  18:43 eval_retrieval
.rwxr-xr-x  52M d9ng 25 4월  18:43 rawq
.rwxr-xr-x  67M d9ng 25 4월  18:43 tuna-flow
```

```bash
$ file /Volumes/tunaFlow/tunaFlow.app/Contents/MacOS/rawq
/Volumes/tunaFlow/tunaFlow.app/Contents/MacOS/rawq: Mach-O 64-bit executable arm64
```

- sidecar 실재: O (52MB, arm64 Mach-O, 실행권한 `rwxr-xr-x`)
- 위치: `Contents/MacOS/rawq` (Tauri 표준)
- `Contents/Resources/` 에는 `icon.icns` 만 — sidecar 는 `MacOS/` 에 들어감

# 절차 3 — quarantine 부착 검증 (DMG 안 raw 파일)

```bash
$ xattr -l /Volumes/tunaFlow/tunaFlow.app/Contents/MacOS/rawq
(empty)

$ xattr -p com.apple.quarantine /Volumes/tunaFlow/tunaFlow.app/Contents/MacOS/rawq
xattr: /Volumes/.../rawq: No such xattr: com.apple.quarantine
```

DMG 안 sidecar 파일 자체에는 quarantine 부착되지 않음. 정상.

`.app` 번들 자체에도 quarantine 없음 (DMG 마운트 직후 raw 상태).

# 절차 4 — drag-install 시뮬레이션 (사용자 실제 환경 재현)

DMG 사용자 다운로드 → drag /Applications 케이스를 시뮬레이션:

```bash
$ cp -R /Volumes/tunaFlow/tunaFlow.app /tmp/.../sim-tunaFlow.app
$ xattr -w com.apple.quarantine "0083;67890abc;Safari;" /tmp/.../sim-tunaFlow.app
$ xattr -l /tmp/.../sim-tunaFlow.app
com.apple.provenance:
com.apple.quarantine: 0083;67890abc;Safari;
```

**결과** — `.app` 부모에만 quarantine 부착되지만 children 인 sidecar 는
실행 시 `provenance` 상속 + Gatekeeper assess 차단:

```bash
$ /tmp/.../sim-tunaFlow.app/Contents/MacOS/rawq --version
(no output)
$ echo $?
137  # SIGKILL by Gatekeeper

$ spctl --assess --type execute -vvv \
    /tmp/.../sim-tunaFlow.app/Contents/MacOS/rawq
/tmp/.../sim-tunaFlow.app/Contents/MacOS/rawq: rejected
```

**검증** — quarantine 정리 후 정상:

```bash
$ xattr -cr /tmp/.../sim-tunaFlow.app
$ /tmp/.../sim-tunaFlow.app/Contents/MacOS/rawq --version
rawq 0.1.2
$ echo $?
0
```

# 절차 5 — 코드 호출 경로 검증 (`agents/rawq.rs::sidecar_candidates`)

```rust
fn sidecar_file_name() -> String {
    let ext = if cfg!(target_os = "windows") { ".exe" } else { "" };
    format!("rawq-{}{}", host_triple(), ext)   // ← rawq-aarch64-apple-darwin
}

fn sidecar_candidates() -> Vec<PathBuf> {
    let sidecar = sidecar_file_name();   // "rawq-aarch64-apple-darwin"
    candidates.push(exe_dir.join(&sidecar));
    candidates.push(exe_dir.join("binaries").join(&sidecar));
    candidates.push(exe_dir.join("../Resources").join(&sidecar));
    candidates.push(exe_dir.join("../Resources/binaries").join(&sidecar));
    ...
}
```

번들 안 실제 파일 이름 vs 코드가 찾는 이름:

```bash
$ ls /Volumes/tunaFlow/tunaFlow.app/Contents/MacOS/ | grep rawq
rawq                       ← 번들 파일 (Tauri 가 build 시 strip)

# 코드가 검색하는 후보들
rawq-aarch64-apple-darwin  ← 모두 ENOENT
binaries/rawq-aarch64-apple-darwin
../Resources/rawq-aarch64-apple-darwin
../Resources/binaries/rawq-aarch64-apple-darwin
```

`Tauri 2` 표준 동작 — externalBin 으로 등록한 sidecar 는 빌드 시
`{name}-{triple}` 으로 staging → 번들 안에서는 `{name}` 으로 normalize.
하지만 `rawq.rs::sidecar_candidates()` 는 triple-suffix 이름만 검색.

→ **DMG 정상 빌드 + quarantine 정리 후에도 코드측 resolution 자체가
실패**. 사용자 보고와 정확히 일치하는 회귀 경로.

# 종합 진단

| # | 경로 | 실제 원인 | Fix Layer |
|---|---|---|---|
| A | DMG release 미인식 (install.sh 우회) | (1) `Contents/MacOS/rawq` 는 정상 staging 됨, (2) 그러나 코드 `sidecar_candidates()` 가 `rawq-{triple}` 만 검색 → 번들 파일 매칭 실패. (3) 추가로 drag-install quarantine 시 sidecar 실행 자체 차단 (exit 137) | **A3 (코드 fix)** + **A3' (drag-install quarantine 안내)** |
| B | `cargo install rawq` 후 미인식 | sidecar-only 디자인. 시스템 PATH 의 rawq 무시는 의도된 동작이지만 README 명시 부재. PATH fallback 도 quarantine 처리된 .app 안에서는 `Command::new("rawq")` 실패 가능 | **B1 (README 명시)** |
| C | 직접 빌드 시 `binaries/rawq-aarch64-apple-darwin doesn't exist` | `npm run tauri build` 직접 실행 시 사이드카 빌드 누락. `./scripts/build.sh` wrapper 가 자동 처리하지만 안내 부족 | **B1 (README 빌드 가이드)** + **B3 (선택: prebuild presence check)** |

# Fix 범위 결정

plan 의 분기 트리 적용 결과:

- audit 결과 = **"sidecar 정상 staging + 코드 호출 실패 + drag-install quarantine 추가"**
- → **Layer A2 (build pipeline 수정) 는 N-A**. 빌드 산출물은 정확하게 staging 됨.
- → **Layer A3 (drag-install quarantine README 안내)** 진행
- → **코드측 fix (`sidecar_candidates` 보강)** — plan 본문 지시: "audit 결과가
  '정상 + 코드 호출 실패' 로 나오면 fix 범위가 코드 + 사용자 진단 흐름으로 좁혀짐"
- → **Layer B (README/INSTALL.md/RawqStatus 메시지)** 모두 진행
- → **Layer C (CI verify)** — staging 정상이라도 회귀 방지 차원에서 진행 (build pipeline 이 향후 회귀시 즉시 잡기 위함)

# Invariants 충족 추적

| INV | 충족 |
|---|---|
| INV-1 (release `.app` 안 sidecar 존재 + 실행권한) | O — audit 절차 2 |
| INV-2 (install.sh 가 quarantine 처리) | O — `install.sh:103 xattr -cr` |
| INV-3 (drag-install 사용자도 README/INSTALL.md 안내로 quarantine 처리) | △ — INSTALL.md `xattr -cr` 언급 있으나 "rawq 안 보임 → xattr 의심" 매핑 부재. **B1/A3 보강** |
| INV-4 (직접 빌드 사용자가 `./scripts/build.sh` 권장 경로로 안내됨) | X — README 부재. **B1 보강** |
| INV-5 (`RawqStatus.unavailable` 시 사용자 가시 메시지가 다음 단계 액션 제공) | X — 현재 `"rawq not found"` 만. **B2 보강** |
| INV-6 (CI 가 빌드 산출물 안 sidecar 부재 시 fail) | X — verify step 부재. **C1 추가** |

# 후속

- audit 결과 plan 의 § Fix Scope 분기 결정 → 본 audit + 6 개 fix 커밋 후속.
- DMG mount cleanup: `hdiutil detach /Volumes/tunaFlow` 완료.
- 사용자 환경에서 즉시 우회: `xattr -cr /Applications/tunaFlow.app` 후 재실행
  (사이드카 코드 fix 가 배포되기 전까지는 코드측 resolution 실패는 별도 — 이 부분은
  fix release 가 필요).
