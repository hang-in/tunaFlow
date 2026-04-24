# rawq 설치 / sidecar 준비 가이드

tunaFlow는 코드 검색(ContextPack Full 모드)에 rawq를 사용한다.
현재 rawq는 선택 기능이 아니라, tunaFlow가 함께 관리해야 하는 **필수 런타임 의존성**으로 본다.

즉 권장 운영 방식은:

- 배포: rawq sidecar 번들
- 개발: rawq를 명시적으로 build해서 `src-tauri/binaries/`에 배치
- 런타임: 준비된 rawq를 자동 탐색

rawq가 없으면 조용히 degraded mode로 가지 않는 것이 원칙이다.

## 바이너리 탐색 순서

`src-tauri/src/agents/rawq.rs`의 `resolve_rawq_bin()` 참조:

1. `RAWQ_BIN` 환경변수 (명시 지정)
2. `src-tauri/binaries/rawq-<target-triple>` sidecar
3. 개발용 로컬 빌드 경로
4. PATH의 `rawq` 명령어 (개발 보조 경로)

현재 호스트 target triple은 다음 명령으로 확인할 수 있다.

```bash
rustc --print host-tuple
```

## 권장 준비 방법

### 방법 1: bootstrap script 사용 (권장)

macOS/Linux:

```bash
./scripts/build-rawq.sh
```

Windows PowerShell:

```powershell
./scripts/build-rawq.ps1
```

이 스크립트는 rawq 소스 위치를 탐색한 뒤 release build를 수행하고,
산출물을 `src-tauri/binaries/rawq-<target-triple>`로 복사한다.

우선 탐색하는 경로:

1. `RAWQ_SRC`
2. `./vendor/rawq`
3. `../tunaDish/vendor/rawq`
4. `../_research/_util/rawq`

### 방법 2: 직접 빌드 후 sidecar 위치로 복사

```bash
cd ~/privateProject/_research/_util/rawq
cargo build --release
cp target/release/rawq \
  /Users/d9ng/privateProject/tunaFlow/src-tauri/binaries/rawq-$(rustc --print host-tuple)
```

### 방법 3: `RAWQ_BIN` 환경변수 지정

빌드 산출물을 sidecar 위치에 복사하지 않고 직접 경로를 지정할 수도 있다.

```bash
export RAWQ_BIN=~/privateProject/_research/_util/rawq/target/release/rawq
```

## 확인 방법

### 1. 바이너리 존재 확인

```bash
ls src-tauri/binaries/rawq-$(rustc --print host-tuple)
```

### 2. rawq 자체 확인

```bash
src-tauri/binaries/rawq-$(rustc --print host-tuple) --version
```

### 3. 앱 상태 확인

프로젝트를 열면 tunaFlow가:

- rawq binary availability
- index status
- 필요 시 index build

를 확인한다.

## 비권장 방향

- 앱 시작 시 매번 rawq를 즉석 빌드하는 것
- rawq 실패 시 예전 최소 검색 fallback으로 조용히 내려가는 것

이 둘은 운영 관점에서 원인 추적과 품질 보장을 어렵게 만든다.

## 제외 패턴 (Exclude patterns) — Issue #180 hotfix

rawq 의 `.gitignore` 존중이 실측상 신뢰할 수 없어 (빌드 산출물이 인덱싱되어 시스템 프리즈 유발), tunaFlow 는 `src-tauri/src/agents/rawq.rs` 의 `ensure_index` 에 **하드코딩된 `-x` 패턴**으로 방어한다.

현재 제외 목록 (코드 SSOT — 변경 시 이 목록도 업데이트):

| 패턴 | 대상 |
|---|---|
| `target/**` | Rust 빌드 출력 |
| `node_modules/**` | Node 의존성 |
| `dist/**` | 프런트엔드 빌드 출력 |
| `build/**` | 일반 빌드 디렉터리 |
| `.venv/**`, `venv/**` | Python 가상환경 |
| `__pycache__/**` | Python 캐시 |
| `.next/**`, `.nuxt/**` | Next.js / Nuxt 빌드 |
| `.cache/**` | 일반 캐시 |
| `coverage/**` | 테스트 커버리지 |
| `*.min.js`, `*.min.css` | 미니파이 산출물 |
| `*.lock` | lockfile |
| `*.log` | 로그 |

### 변경 정책 (INV-3)

**추가만 가능, 삭제 금지.** 한 번 제외한 디렉터리를 다시 인덱싱 대상으로 바꾸면 기존 사용자의 인덱스 DB 에 대량 재인덱싱이 일시에 유발된다. 삭제가 필요하면 별도 plan 으로 분리.

### 패턴 추가 요청

- 새 빌드 도구 / 프레임워크 산출물이 빈번하게 인덱싱된다면 이슈 또는 PR 로 제보
- 패턴 추가는 단일 라인 변경이라 PR 승인 빠름

### 기존 사용자 — 레거시 인덱스 정리

hotfix 이전에 한 번이라도 프로젝트를 열었다면 DB 에 `target/` 등이 이미 인덱싱되어 있을 수 있다. **Settings → Runtime → rawq 섹션** 의 `인덱스 재빌드` 버튼으로 기존 인덱스를 제거하고 현재 exclude 패턴으로 재빌드한다. 프로젝트 크기에 따라 수 분 소요.
