# v1.0.6 — Rust 코드 책임별 모듈화

2026-11-07

## 코드 구조 개선
- 공개 `src/lib.rs`와 GUI `src/main.rs`의 회귀 테스트를 각각 `src/probability/tests.rs`, `src/app/tests.rs`로 분리합니다. 실행 진입점과 기존 라이브러리 API는 유지합니다.
- 환급 Monte Carlo 구현을 `src/domain/feedback/{config,results,rolls,runner,tests}.rs`로 나눕니다. 설정 검증, 공개 시나리오/통계 자료형, 캐릭터·무기 뽑기 규칙, 제한된 병렬 표본 처리/취소, 회귀 검증의 소유권을 분리했습니다.
- 실측 CSV 분석을 `src/analysis/observations/{parser,tests}.rs`로 분리하고 기존 `parse_observation_csv`, `ObservationDataset` 공개 경로를 재수출합니다.
- 게임 카탈로그는 `src/data/catalog.rs`의 데이터/색인 조회와 `src/data/catalog/ui.rs`의 이미지 캐시·픽커를 분리합니다.
- 원격 업데이트는 `src/data/runtime_updates/{network,cache,bundle,tests}.rs`로 구분합니다. 검증된 HTTPS 수신, 원자적 캐시 쓰기, 카탈로그/PNG 서명·해시 확인, 사용자 진행 상태가 서로 다른 모듈에서 관리됩니다.
- Dashboard는 `src/ui/dashboard/{overview,settings,results}.rs`로, 편집 UI는 `src/app/controller_editor/{preferences,targets,resources}.rs`로 나눕니다. 앱 세션에서도 `src/app/controller_session/{calculation,navigation}.rs`로 워커 수명주기와 내비게이션을 분리합니다.
- 일회성 경량 래퍼 `include!`을 사용하지 않고 Rust `#[path]` 모듈 및 기존 접근 수준에 따른 내부 재수출을 사용합니다.

## 유지하는 동작 및 계약
- 전체 4게임의 천장/확률/환급 재투자, 동일 Monte Carlo 시드·병렬 순서·산출물, 캐릭터–전용 장비 매핑, UI 선택·이벤트, 네트워크 신뢰 경계/캐시 트랜잭션을 변경하지 않습니다.
- 사용자가 모든 입력을 마친 후 `Run Simulation`을 클릭해야 계산하는 규칙, 구세대 취소, 완료 렌더링, `Arc` 결과 공유를 유지합니다.
- `feedback::{estimate, estimate_with_options, FeedbackConfig, FeedbackEstimate, RefundScenario, SuccessfulBudgetMeans}` 및 crate root 재수출과 외부 CSV 파서 시그니처를 그대로 유지합니다.

## 검증 조건
- Windows/Linux 단위·통합 테스트와 Clippy -D warnings, 공개 라이브러리 문서 생성, 기존 Release 성능 검증, RustSec, 이미지 무결성, 실제 GUI 스모크 및 캡처를 필수로 유지합니다.
- 이 버전은 **모듈의 책임 분리**이며 시뮬레이션 연산 자체를 더 빠르게 했다고 주장하지 않습니다.
- `Calculator`의 egui 상태와 UI 컨트롤러 사이에는 의도된 내부 결합이 남습니다. 외부 API 호환성과 이벤트 순서 유지 때문에 이를 독립 마이크로서비스나 불필요한 trait 추상화로 변경하지 않았습니다.
