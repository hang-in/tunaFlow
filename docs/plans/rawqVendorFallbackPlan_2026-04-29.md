---
title: rawq vendor 자동 clone fallback — 외부 contributor 빌드 진입 장벽 차단
status: ready
phase: planning
priority: P0 (release blocker, 외부 사용자 보고)
created_at: 2026-04-29
canonical: true
related:
  - scripts/build-rawq.sh
  - scripts/build-rawq.ps1
  - INSTALL.md
  - README.md
issue_source: batmania52 보고 (#3, 2026-04-29)
---

# rawq vendor fallback

## Context

`scripts/build-rawq.sh` 가 로컬 3 경로 (`vendor/rawq`, `../tunaDish/vendor/rawq`, `../_research/_util/rawq`) 만 검색. 외부 contributor 가 `git clone tunaFlow` 후 `./scripts/build-rawq.sh` 하면 무조건 fail. submodule 도 없음. 외부 사용자가 임시 우회 (배포 binary 복사) 로 dev 까지 도달했지만 공식 경로 아님.

## Goals

- (G1) `RAWQ_SRC` 미지정 + 로컬 3 경로 모두 부재 시 → `vendor/rawq` 에 자동 `git clone --depth 1 https://github.com/hang-in/rawq` fallback.
- (G2) `RAWQ_REPO_URL` 환경변수로 fork URL override 가능.
- (G3) `.gitignore` 에 `vendor/rawq` 추가 (자동 clone 산출물이라 추적 X).
- (G4) `INSTALL.md` 에 자동 clone 설명 + 오프라인/private fork 케이스 안내.
- (G5) Windows 등가 (`scripts/build-rawq.ps1`) 동일 fallback.

## Non-goals

- ❌ rawq 를 git submodule 로 등록 (사용자가 `git clone --recursive` 안 쓰면 또 fail, fallback 우월).
- ❌ rawq binary GitHub Release 직접 다운로드 (build-from-source SSOT 유지).
- ❌ rawq 소스를 tunaFlow 모노레포로 합치기 (관리 부담).

## Subtasks

### Task 01 — `scripts/build-rawq.sh` 자동 clone fallback

**Changed files**: `scripts/build-rawq.sh`, `scripts/build-rawq.ps1`, `.gitignore`

**Change description (sh)**:
```bash
RAWQ_REPO_URL="${RAWQ_REPO_URL:-https://github.com/hang-in/rawq}"

# (기존 search loop 후) RAWQ_SRC_DIR 가 비어있으면 자동 clone fallback
if [[ -z "$RAWQ_SRC_DIR" ]]; then
  AUTO_CLONE_DIR="$ROOT_DIR/vendor/rawq"
  if [[ ! -d "$AUTO_CLONE_DIR/.git" ]]; then
    echo "[rawq] source not found locally — auto cloning $RAWQ_REPO_URL → $AUTO_CLONE_DIR"
    mkdir -p "$ROOT_DIR/vendor"
    git clone --depth 1 "$RAWQ_REPO_URL" "$AUTO_CLONE_DIR"
  else
    echo "[rawq] using existing auto-cloned vendor at $AUTO_CLONE_DIR"
  fi
  RAWQ_SRC_DIR="$AUTO_CLONE_DIR"
fi
```

**Change description (ps1)**: 같은 로직 PowerShell 등가. `git clone --depth 1` 동일.

**`.gitignore` 추가**:
```
# Auto-cloned rawq sidecar source (build-rawq.sh fallback)
vendor/rawq/
```

**Verification**:
- 로컬 `vendor/rawq` 가 없는 상태에서 `./scripts/build-rawq.sh` 실행 → 자동 clone + 빌드 성공
- `vendor/rawq` 가 이미 있는 상태에서 재실행 → 기존 폴더 재사용 (clone skip), 빌드 성공
- `RAWQ_SRC=/path/to/local ./scripts/build-rawq.sh` → 환경변수 우선 동작 (fallback 안 탐)
- `RAWQ_REPO_URL=https://github.com/<fork>/rawq ./scripts/build-rawq.sh` → fork URL 사용
- Windows: `.\scripts\build-rawq.ps1` 도 동일 시나리오 통과
- `git status` 결과에 `vendor/rawq/` 항목이 안 나와야 함 (gitignore 효과)

**회귀 위험 가드**:
- 기존 `RAWQ_SRC` 환경변수 path / 3개 로컬 fallback path 의 우선순위는 변경 금지 (auto clone 은 *마지막* fallback).
- `git clone` 실패 시 (네트워크 / 권한) 명확한 에러 메시지 + exit 1.
- Windows `Invoke-WebRequest` 가 아닌 `git clone` 사용 (rawq 가 일반 git repo 라 단순).

### Task 02 — INSTALL.md / README 보강

**Changed files**: `INSTALL.md`, `README.md` (Known Constraints / Build 섹션)

**Change description**:
- INSTALL.md "Build from source" 섹션에 다음 추가:
  - 기본: `./scripts/build-rawq.sh` 가 자동으로 `vendor/rawq` 에 clone
  - 오프라인 또는 private fork: `RAWQ_REPO_URL` 환경변수 또는 `RAWQ_SRC` 로 로컬 path 지정
  - 자주 묻는 질문: rawq 가 무엇인지 1줄 설명 + GitHub repo 링크
- README "Known Constraints" 에 한 줄: rawq 는 build-time 자동 clone (`vendor/rawq`)

**Verification**:
- `rg "vendor/rawq|RAWQ_REPO_URL" INSTALL.md README.md` 로 신규 텍스트 확인

**회귀 위험 가드**:
- 기존 INSTALL.md 의 다른 섹션 (drag-install, xattr, rawq sidecar 안내) 변경 금지.

## Cross-cutting risks

| 위험 | 대응 |
|---|---|
| 자동 clone 이 corporate firewall 등에서 실패 | `RAWQ_SRC` 환경변수로 로컬 path 지정 가능. 명확한 에러 메시지로 안내. |
| fork 가 private repo 라 인증 필요 | `RAWQ_REPO_URL` 에 SSH URL 또는 token URL 가능 (사용자 책임). |
| 빌드 산출물이 vendor/rawq 안에 생성되어 후속 clone 시 dirty | clone 은 한 번만, target 은 `src-tauri/target/rawq-sidecar/` (이미 분리). 영향 없음. |

## Rollback

`scripts/build-rawq.sh`, `scripts/build-rawq.ps1`, `.gitignore`, `INSTALL.md`, `README.md` 각각 git revert 가능. 단일 commit 단위 revert 권장.
