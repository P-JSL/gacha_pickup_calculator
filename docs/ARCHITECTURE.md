# Rust 모듈 책임 및 공개 API 경계 (v1.0.3 기준)

## 외부 계약

- `src/lib.rs`의 `RateMode`, `RateSchedule`, `SoftPity`, `Banner`, `BannerRule`, `Spark`, `Distribution`, `Allocation`, `optimal_allocation`을 1.x 공개 API로 재수출합니다.
- `games`, `feedback`, `tracker`, `observations`도 기존 공개 모듈 경로를 유지합니다.
- `src/main.rs`는 기존 GUI 진입점, 애플리케이션 단위 테스트를 유지하며 앱/화면/계산 파일은 `#[path = "..."] mod`로 독립 컴파일됩니다.
- private 필드·메서드는 불필요한 외부 노출 없이 `pub(crate)`로 공유합니다. 임의의 `include!` 소스 병합을 하지 않습니다.

## 실제 책임

| 경로 | 소유 책임 |
|---|---|
| `src/probability/rates.rs` | 기본 확률, 비공식 소프트 천장 |
| `src/probability/banners.rs` | 배너 전이, 천장, 입력 검증 |
| `src/probability/distributions.rs` | PMF/CDF, 분위수, 적응 배분 |
| `src/probability/fft.rs` | 밀집 확률 분포의 O(N log N) 합성 |
| `src/domain/games.rs` | 게임별 규칙과 재화 |
| `src/domain/feedback.rs` | 환급 Monte Carlo 추정 |
| `src/analysis/*` | 관측 자료·트래커 |
| `src/data/*` | 카탈로그·보안·라이브 데이터 |
| `src/app/state.rs` | 게임별 세션과 Calculator 상태 |
| `src/app/compute.rs` | 작업 요청/응답과 결과 집계 |
| `src/app/funding.rs` | 목표 예산/전용권 계산, 순수 데이터 모델 |
| `src/ui/funding_view.rs` | 목표 예산 위젯 및 전용권 입력 |
| `src/app/controller_*.rs` | 게임 전환, 편집, 환급, 분석, 관측 |
| `src/app/lifecycle.rs` | eframe 수명주기·렌더 루프·폰트 |
| `src/ui/navigation.rs`, `src/ui/charts.rs`, `src/ui/dashboard.rs` | 내비게이션, 차트, 대시보드 |

## 설계상의 현재 경계

- 파일 수준뿐 아니라 Rust `mod` 경계는 적용했습니다. 그러나 일부 컨트롤러와 UI는 여전히 `Calculator`의 crate-private 상태에 접근합니다. 이 결합은 내부 상태를 무리하게 복제하거나 이벤트 순서를 바꾸지 않기 위해 보존한 부분이며, 완전한 도메인 독립성이 달성되었다고 주장하지 않습니다.
- `GoalFunding`은 egui를 사용하지 않으며, 렌더링은 `funding_view`가 담당합니다.
- 계산 워커의 generation 검사, 게임별 세션, 캐릭터 전용 장비 자동 연동, 기존 공개 API를 변경하지 않습니다.
- 확률 계산의 복잡도 및 정확도 조건은 [성능 설계](PERFORMANCE_V1_0_1.md)를 참고하십시오.

## v1.0.2 외부/내부 공개 범위

- `src/lib.rs`에서 재수출하는 `Banner`, `Distribution`, `optimal_allocation`과 공개된 `games`, `feedback`, `tracker`, `observations`는 기존 외부 사용자에게 제공하는 **계약**이므로 호환성을 유지합니다.
- `src/app/`, `src/data/`, `src/ui/`는 실행 파일 전용 구현으로 Rust의 외부 import가 불가능해야 합니다. 내부 형식은 `pub(crate)` 이하를 사용하며 CI가 공개 범위를 검사합니다.
- Rust `pub(crate)`는 호출 경계일 뿐 소스 코드 기밀성이나 악성 로컬 프로세스로부터의 보호를 의미하지 않습니다. 자세한 위협 모델은 [보안 경계](SECURITY_BOUNDARY_V1_0_2.md)를 참조합니다.

## CI의 책임 분리와 경량 검증

- `.github/workflows/rust.yml`은 문서·릴리즈 계약, **OS별 테스트·Clippy와 OS별 Release 빌드를 서로 독립 Job**으로 분리하고, RustSec·성능·PNG 무결성·ClamAV를 별도로 실행합니다. 실제 GUI 캡처는 Linux Release 실행 파일만 기다리며, 새 릴리즈는 모든 Job의 성공을 확인한 후 진행합니다.
- Markdown/라이선스·과거 릴리즈 노트만 변경되면 문서 무결성 검사를 수행하고, Rust 빌드·PNG 바이러스 검사·실제 GUI 캡처·릴리즈 재발행은 생략합니다.
- Rust 확률·관측 분석 변경은 Windows/Linux 컴파일·테스트·Clippy를 실행합니다. 이미지/카탈로그 또는 UI 변경 시 관련 검증을 추가하며, 새 버전 릴리즈가 필요할 때에는 모든 필수 게이트를 수행합니다.
- 브랜치 보호의 필수 체크를 Pending으로 만들지 않도록 workflow-level `paths-ignore`를 피하고, Windows/Ubuntu 및 CodeQL **매트릭스 Job 이름을 유지한 채 Job 내부의 무거운 단계만 조건부 실행**합니다. 판별이 불가능한 변경은 전체 검사로 처리합니다.
- 최초 v1.0.1의 책임별 분리는 런타임 성능 배수를 증명하지 않습니다. [v1.0.3 실측 결과](PERFORMANCE_V1_0_3.md)에 기록된 FFT/분위수 개선은 알고리즘별 비교입니다.
