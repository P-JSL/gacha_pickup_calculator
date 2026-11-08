# Rust 모듈 책임 및 공개 API 경계 (v1.0.12 기준)

## 외부 계약

- `src/lib.rs`의 `RateMode`, `RateSchedule`, `SoftPity`, `Banner`, `BannerRule`, `Spark`, `Distribution`, `Allocation`, `optimal_allocation`을 1.x 공개 API로 재수출합니다.
- `games`, `feedback`, `tracker`, `observations`도 기존 공개 모듈 경로를 유지합니다.
- `src/main.rs`는 기존 GUI 진입점만 유지하고 애플리케이션 회귀 테스트는 `src/app/tests.rs`로 분리합니다. 앱/화면/계산 파일은 `#[path = "..."] mod`로 독립 컴파일됩니다.
- private 필드·메서드는 불필요한 외부 노출 없이 `pub(crate)`로 공유합니다. 임의의 `include!` 소스 병합을 하지 않습니다.

## 실제 책임

| 경로 | 소유 책임 |
|---|---|
| `src/probability/rates.rs` | 기본 확률, 비공식 소프트 천장 |
| `src/probability/banners.rs` | 배너 전이, 천장, 입력 검증 |
| `src/probability/distributions.rs` | PMF/CDF, 분위수, 적응 배분 |
| `src/probability/fft.rs` | 밀집 확률 분포의 O(N log N) 합성 |
| `src/domain/games.rs` | 게임별 규칙과 재화 |
| `src/domain/feedback.rs` | 환급 도메인 모듈 루트·난수/Wallet 공유 경계 및 기존 외부 API 재수출 |
| `src/domain/feedback/config.rs` | 게임별 교환/환급 설정 및 입력 검증 |
| `src/domain/feedback/results.rs` | 공개 결과 자료형·분위수/조건부 통계 |
| `src/domain/feedback/rolls.rs` | 배너별 뽑기 경로와 환급 트랜잭션 규칙 |
| `src/domain/feedback/runner.rs` | 취소 가능한 시뮬레이션 청크·CPU 예산·정수 합산·정렬 |
| `src/domain/feedback/tests.rs` | 환급 재현성·수치 회귀·극단 값 검사 |
| `src/analysis/observations.rs` | 관측 결과 자료형·Greenwood 오차 및 모델 비교 |
| `src/analysis/observations/parser.rs` | CSV 입력/검열 데이터 검증 및 위험집합 구성 |
| `src/analysis/observations/tests.rs`, `src/analysis/tracker.rs` | 실측 회귀 사례 및 트래커 출처/기대값 |
| `src/data/catalog.rs`, `src/data/catalog/ui.rs` | 정적/실시간 카탈로그 색인과 이미지 디코딩/픽커 UI |
| `src/data/runtime_updates.rs` | 비동기 업데이트 상태·완료 이벤트 |
| `src/data/runtime_updates/network.rs` | 검증된 HTTPS·SHA 인증·릴리즈 메타데이터 |
| `src/data/runtime_updates/cache.rs` | 제한된 로컬 캐시 읽기·원자적 쓰기·검증된 스테이징 |
| `src/data/runtime_updates/bundle.rs` | 카탈로그/PNG 포맷·해시·매니페스트 검증 |
| `src/data/security.rs` | 로컬 업데이트 및 이미지 신뢰 경계 |
| `src/app/state.rs` | 게임별 세션과 Calculator 상태 |
| `src/app/compute.rs` | 작업 요청/응답과 결과 집계 |
| `src/app/funding.rs` | 목표 예산/전용권 계산, 순수 데이터 모델 |
| `src/ui/funding_view.rs` | 목표 예산 위젯 및 전용권 입력 |
| `src/app/controller_session.rs` + `controller_session/*` | 게임별 세션·데이터 선택 / 작업 취소·결과 수신 / 내비게이션 |
| `src/app/controller_editor.rs` + `controller_editor/*` | 환경 설정 / 캐릭터·무기·천장 편집 / 재화 입력 |
| `src/app/controller_refunds.rs`, `controller_analytics.rs`, `controller_evidence.rs` | 환급 옵션, 분석 UI, 실측 근거 입력 |
| `src/app/lifecycle.rs` | eframe 수명주기·렌더 루프·폰트 |
| `src/ui/navigation.rs`, `src/ui/charts.rs` | 내비게이션·공통 차트·프레임 스타일 |
| `src/ui/dashboard.rs` + `src/ui/dashboard/*` | 공통 렌더링 프리미티브 / 개요 / 입력 설정 / 결과 패널 |
| `src/lib.rs`, `src/main.rs` + `src/probability/tests.rs`, `src/app/tests.rs` | 외부 라이브러리·GUI 진입점과 독립 회귀 테스트 |

## 설계상의 현재 경계

- 각 구현 책임을 Rust `mod` 하위 경계로 추가 분리했습니다. 그러나 일부 컨트롤러와 UI는 여전히 `Calculator`의 crate-private 상태에 접근합니다. 이 결합은 내부 상태를 무리하게 복제하거나 이벤트 순서를 바꾸지 않기 위해 보존한 부분이며, 완전한 도메인 독립성이 달성되었다고 주장하지 않습니다.
- `GoalFunding`은 egui를 사용하지 않으며, 렌더링은 `funding_view`가 담당합니다.
- 계산 워커의 generation 검사, 게임별 세션, 캐릭터 전용 장비 자동 연동, 기존 공개 API를 변경하지 않습니다.
- 확률 계산의 복잡도 및 정확도 조건은 [성능 설계](PERFORMANCE.md)를 참고하십시오.

## 외부/내부 공개 범위

- `src/lib.rs`에서 재수출하는 `Banner`, `Distribution`, `optimal_allocation`과 공개된 `games`, `feedback`, `tracker`, `observations`는 기존 외부 사용자에게 제공하는 **계약**이므로 호환성을 유지합니다.
- `src/app/`, `src/data/`, `src/ui/`는 실행 파일 전용 구현으로 Rust의 외부 import가 불가능해야 합니다. 내부 형식은 `pub(crate)` 이하를 사용하며 CI가 공개 범위를 검사합니다.
- Rust `pub(crate)`는 호출 경계일 뿐 소스 코드 기밀성이나 악성 로컬 프로세스로부터의 보호를 의미하지 않습니다. 자세한 위협 모델은 [보안 경계](SECURE_RELEASE.md)를 참조합니다.

## CI의 책임 분리와 경량 검증

- `.github/workflows/rust.yml`은 문서·릴리즈 계약, **OS별 테스트·Clippy와 OS별 Release 빌드를 서로 독립 Job**으로 분리하고, RustSec·성능·PNG 무결성·ClamAV를 별도로 실행합니다. 실제 GUI 캡처는 Linux Release 실행 파일만 기다리며, 새 릴리즈는 모든 Job의 성공을 확인한 후 진행합니다.
- Markdown/라이선스 문서만 변경되면 문서 무결성 검사를 수행하고, Rust 빌드·PNG 바이러스 검사·실제 GUI 캡처·릴리즈 재발행은 생략합니다.
- Rust 확률·관측 분석 변경은 Windows/Linux 컴파일·테스트·Clippy를 실행합니다. 이미지/카탈로그 또는 UI 변경 시 관련 검증을 추가하며, 새 버전 릴리즈가 필요할 때에는 모든 필수 게이트를 수행합니다.
- 브랜치 보호의 필수 체크를 Pending으로 만들지 않도록 workflow-level `paths-ignore`를 피하고, Windows/Ubuntu 및 CodeQL **매트릭스 Job 이름을 유지한 채 Job 내부의 무거운 단계만 조건부 실행**합니다. 판별이 불가능한 변경은 전체 검사로 처리합니다.
- 책임별 모듈 분리는 런타임 성능 배수를 증명하지 않습니다. [성능 측정과 해석](PERFORMANCE.md)에 기록된 FFT/분위수 개선은 알고리즘별 비교입니다.

## 명시적 계산과 CI 관찰 이벤트

- 입력 편집은 모델 값만 변경하고 결과 세대를 무효화합니다. 자동 작업 스케줄이나 디바운스는 생성하지 않습니다.
- Dashboard/Simulator에서 `Run Simulation`을 클릭하면 해당 입력 스냅샷과 고유 세대를 Worker 채널로 전송하고, 유효 세대의 결과만 받아 결과·이력을 갱신합니다.
- CI 한정 환경변수 `GACHA_CI_SIMULATION_EVENTS=1`은 로그의 `STARTED/COMPLETED/FAILED` 상태만 노출합니다. 이 기능은 **계산을 자동으로 시작하지 않습니다.** 완료 로그는 작업 결과를 반영한 다음 GUI 프레임에 그린 후 내보냅니다.
- GitHub Actions 스크린샷은 **egui가 보고한 화면에 표시되는 실제 Run Simulation 위젯 좌표 확인 → 실제 마우스 클릭 → 동일 세대의 계산 완료 이벤트 → 기존 그래픽·게임 탭 검증** 순서를 따릅니다. 유효한 클릭이나 완료 이벤트가 없으면 90초 이내 실패하며, 이전 결과를 캡처 완료로 인정하지 않습니다.
