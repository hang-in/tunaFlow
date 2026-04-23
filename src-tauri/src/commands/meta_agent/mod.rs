//! metaAgent — 프로세스 관리자 + 정체성 분석 trigger + background insight worker.
//!
//! 본 모듈은 metaAgentPlan 의 Phase 3/4 를 구현한다. 현재 포함:
//! - `background_jobs` — `agent_jobs` 확장 컬럼 기반 enqueue/cancel/count
//! - `background_worker` — low-priority job 폴링 + 디스패처
//! - Settings 토글 `BACKGROUND_INSIGHT_ENABLED` (INV-3)
//!
//! Phase 3 의 identity_analysis 실행 경로는 subtask-03 이후 별도 모듈 (`identity_analysis`)
//! 로 추가된다.

pub mod background_jobs;
pub mod background_worker;
pub mod identity_trigger;
