# v1.0.0 Rust 모듈 책임 분리

## 목적과 불변 계약

- `gacha_pickup_calculator::games`, `::feedback`, `::tracker`, `::observations`, `Banner`, `RateMode`, `Distribution`, `optimal_allocation` 공개 경로는 기존 사용자와 테스트를 위해 유지합니다.
- `#[path]`로 기존 공개 모듈 이름을 유지하면서 소스 파일 위치를 변경했습니다. CLI/GUI 엔트리포인트는 `src/main.rs`에 남깁니다.
- `main.rs`의 egui 콜백, 계산기 수명주기 및 UI 전역 보조 함수는 현재 crate-root 가시성에 의존합니다. 이에 `include!`를 사용해 물리적 코드 소유권만 먼저 나눴으며, 임의 `pub` 확대나 이벤트 순서 변경은 피했습니다.
- 새로운 외부 dependency는 추가하지 않고 `Cargo.lock` 고정을 유지합니다.

## 파일 구성

| 경로 | 책임 | 호출/데이터 방향 |
|---|---|---|
| `src/probability/rates.rs` | 기본/추정 확률, 소프트 천장과 이환 커뮤니티 가정 | 계산 핵심 |
| `src/probability/banners.rs` | 배너 규칙·천장·확률상태 전이·유효성 검사 | `Distribution` 생성 |
| `src/probability/distributions.rs` | CDF/PMF, 분위수, 합성 및 두 배너 적응 자금 배분 | UI가 읽기 전용 소비 |
| `src/domain/games.rs` | 4개 게임 규칙·목표·재화·전용권 변환 | 확률/환급의 입력 |
| `src/domain/feedback.rs` | 환급/중복 보유 가정, Monte Carlo 상태, 지급·재투입 | DP와 별도로 표본 산출 |
| `src/analysis/observations.rs` | 우측 검열 CSV/Kaplan–Meier 분석 | UI에서 호출 |
| `src/analysis/tracker.rs` | 제3자 참조 통계·출처 메타데이터 | 공식 데이터와 구분 |
| `src/data/catalog.rs` | 캐릭터/장비 ID·이미지 매핑, 원자적 live 상태 | UI만 접근 |
| `src/data/runtime_updates.rs` | HTTPS 소스 조회·캐시 검증·버전 갱신 | 바이너리 무인 갱신 금지 |
| `src/data/security.rs` | CSV/PNG 크기·해시·파일 검증 | 업데이트/분석 |
| `src/ui/navigation.rs` | 색상·벡터 네비게이션 | egui |
| `src/ui/dashboard.rs` | 위젯 대시보드/설정 편집/결과 | `Calculator` 사용 |
| `src/ui/i18n.rs` | 로캘 및 번역 | 모든 화면 |
| `src/ui/charts.rs` | CDF 및 시각화 | 배너/환급 결과 읽기 |
| `src/app/funding.rs` | 목표 부족분, 전용권·재화 투입 | 계산 결과 처리 |
| `src/app/compute.rs` | 비동기 계산 요청·응답·집계 | GUI 스레드 밖 계산 |
| `src/app/state.rs` | 화면/언어/게임별 세션 상태 | 앱 수명주기 |
| `src/app/controller_session.rs` | 게임 전환·선택·검색·계산 워커 동기화 | UI 이벤트 |
| `src/app/controller_editor.rs` | 편집·천장·재화 입력 | 입력 값 변경 |
| `src/app/controller_refunds.rs` | 환급 파라미터 편집 | 입력 값 변경 |
| `src/app/controller_analytics.rs` | 분석 결과 패널 | 계산 결과 읽기 |
| `src/app/controller_evidence.rs` | 트래커 및 CSV 증거 화면 | 관측 통계 읽기 |
| `src/app/lifecycle.rs` | `eframe::App` 실행 및 앱 환경 | UI 루프, 계산 debounce |

## 안전한 추가 리팩터링 규칙

외부 공개 경로, 게임별 보유 상태, `selected_character_id` 자동 매핑, `Generation` 기반 오래된 계산 결과 폐기, 창 수명주기 및 데이터 처리량 한도는 변경하지 않습니다. 개별 코드의 독립성·가독성을 개선하려면 먼저 독립된 테스트와 CI 성능 기준을 준비한 후 `include!` 기반 경계를 정식 Rust 모듈과 명시적 인터페이스로 전환해야 합니다. 현재의 물리적 분리를 논리적 의존성 완전 분리라고 주장하지 않습니다.
