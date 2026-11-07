# 변경 범위별 GitHub Actions 실행 정책 (v1.0.3 코드 유지)

이 문서는 `main`의 `.github/workflows/rust.yml` 및 `security-codeql.yml` 설정과 실제 운영상 주의사항을 설명합니다. **기존 v1.0.3 프로그램·게임 데이터와 릴리즈 파일은 변경하지 않습니다.**

## 변경 유형별 검증

| 변경 대상 | Rust 빌드·테스트 | ClamAV·이미지 | 실제 GUI 캡처 | RustSec·성능 벤치마크 | CodeQL |
|---|---|---|---|---|---|
| `README.md`, `docs/*.md`, `THIRD_PARTY_ASSETS.md`, `LICENSE`, 과거 릴리즈 노트만 수정 | 생략 | 생략 | 생략 | 생략 | 언어 분석 생략 |
| `src/probability/`, `src/domain/`, `src/analysis/` | 실행 | 생략 | 생략 | 실행 | Rust |
| `src/app/`, `src/ui/`, `src/main.rs` | 실행 | 필요시만¹ | 실행 | 다른 관련 파일 없으면 생략 | Rust |
| `Image/`, 카탈로그·매핑 TSV, 자산 감사 스크립트 | 실행 | 실행 | 실행 | 의존성 변경이 없으면 생략 | 관련 언어가 변경된 경우 |
| `*.py` / `requirements-image.txt` | 변경 경로에 따라 실행 | 자산 관련 경로면 실행 | 변경 경로에 따라 실행 | 필요시 실행 | Python |
| `.github/workflows/*`, `Cargo.toml`, `Cargo.lock`, 알 수 없는 입력 | 전체 검증 | 전체 검증 | 실행 | 실행 | 변경 언어 / 불명확하면 전체 |
| 게시되지 않은 새 버전이 `main`에 있는 경우 | **전체 검증** | **전체 검증** | **실제 캡처** | **실행** | 소스 변경 언어 및 정기 스캔 |

¹ `src/app/`이나 `src/ui/`만 변경되면 이미 저장소에 포함된 이미지를 GUI 캡처에 사용합니다. 이미지 자산이 변경되지 않은 경우 반복적인 ClamAV 스캔 및 아티팩트 업로드를 생략합니다.

**항상 수행:** `Validate documentation and release contracts`는 파일·문서 링크·버전 계약을 확인합니다. 파일 비교에 실패하거나 분류할 수 없는 경로가 있으면 최소 검사 대신 전체 검증을 선택합니다.

## GitHub 브랜치 보호에 필요한 점

- 워크플로 최상단의 `paths` / `paths-ignore` 조건으로 CI 자체를 생략하지 않습니다. 이렇게 하면 **Required status checks가 Pending**으로 남을 수 있습니다.
- `Verify (windows-latest)`, `Verify (ubuntu-24.04)` 및 `CodeQL (rust/python/actions)`의 **매트릭스 Job 이름을 항상 생성**합니다. 문서 전용 변경에서는 Job 내부의 비용이 큰 단계만 건너뜁니다.
- 이미지 정합성(`Verify bundled image integrity`), ClamAV(`Audit and scan checked-in offline images`), RustSec(`RustSec dependency audit`), 성능(`Measure probability performance`), Linux/Windows Release 빌드 및 GUI 캡처처럼 매트릭스가 아닌 Job은 관련 없는 변경에서 Skipped일 수 있습니다. 새 버전 배포 시에는 **모두 성공해야** 릴리즈 작업이 실행됩니다.
- Ruleset을 활성화할 경우 `Classify CI scope`, `Validate documentation and release contracts`, `Classify CodeQL inputs`, 두 Verify 매트릭스, **두 Release Build Job** 및 필요한 RustSec·성능·PNG 정합성 검사의 이름을 확인해야 합니다. 이 목록은 관리자 설정이 자동 적용되는 것이 아닙니다. 판별 단계가 실패할 때 뒤따르는 검사만 Skipped 되는 것을 성공으로 오해하지 않도록 합니다.
- 기존 `scripts/configure_repository_security.py --apply`는 여러 검사와 정책을 **즉시 Active**로 설정하므로, 각 체크 이름과 PR 병합 경로를 검증하기 전 실행하지 않는 편이 안전합니다.

## 장시간 단계의 병렬 분리

[실제 2026-11-07 이전 CI 실행 #42307552082](https://github.com/P-JSL/gacha_pickup_calculator/actions/runs/42307552082)에서 다음 병목을 확인했습니다.

| 기존 직렬/독립 단계 | 이전 실행 로그의 소요시간 | 지금 실행되는 Job |
|---|---:|---|
| Linux `Verify` 내부 RustSec 설치·검사 | 약 2분 56초 | 독립 `RustSec dependency audit` |
| Linux `Verify` 내부 Release 성능 테스트 | 약 1분 39초 | 독립 `Measure probability performance` |
| 이미지 Job 내부 ClamAV 패키지 설치·검사 | 약 7분 10초 (러너 미러 영향) | 독립 `Audit and scan checked-in offline images` |
| PNG 정합성 및 이미지 업로드 | 짧은 작업 | 독립 `Verify bundled image integrity` |
| 실제 GUI 캡처 | 약 1분 33초 | 기존 `Capture latest-character GUI for README` 계속 독립 실행 |

- 4개 무거운 작업은 `Classify CI scope`가 해당 작업을 필요로 판단하면 **동시에 시작할 수 있습니다.** OS 빌드의 결과를 기다렸다가 RustSec·성능·ClamAV를 실행하지 않습니다.
- `Verify (ubuntu-24.04)` 및 `Verify (windows-latest)`는 기존대로 단위 테스트·Clippy와 문서 API 확인을 수행합니다. 실제 Release 실행 파일 빌드는 각각 독립된 `Build Linux GUI executable`과 `Build Windows release executable`에서 **테스트와 동시에** 시작합니다.
- 유효성 검사와 Release 빌드가 분리되더라도 **최종 게시 전에는 Verify 두 Job과 Build 두 Job 모두 성공**해야 합니다. Windows Authenticode는 이전처럼 신규 버전 `main` 릴리즈 빌드에서만 실행합니다.
- 화면 캡처는 완성된 **Linux 실행 파일**만 필요하므로, 이제 `Build Linux GUI executable`이 완료되자마자 시작합니다. **Windows 테스트·Windows 빌드·RustSec·성능·ClamAV의 종료를 기다리지 않습니다.**
- 새 버전 릴리즈는 `Verify` 2개, **Release Build 2개**, RustSec, 성능, PNG 정합성, ClamAV, GUI 및 문서 계약 **전부 성공**이 확인되어야만 준비·Sigstore 서명·게재를 시작합니다.
- ClamAV는 **Ubuntu 공식 패키지 저장소를 통한 동일한 서명 검증**을 사용하며, GitHub 호스티드 러너의 지연이 긴 Azure 미러 대신 공식 `archive.ubuntu.com`을 우선 사용합니다.
- 이 변경은 **Job 간 병렬화**이며 GitHub 호스티드 러너 가용성과 네트워크 대역폭에 따라 전체 실행 시간 개선 폭이 달라집니다. 별도 러너가 늘면 무료 Actions 사용량도 상황에 따라 증가할 수 있습니다.

## 신규 릴리즈와 수동 워크플로

- `main`에 해당 버전 릴리즈가 **이미 존재하면** 기존 버전을 다시 게시하지 않습니다. 신규 버전의 태그가 없으면 전체 빌드·이미지 검사·실제 GUI 캡처·Sigstore 서명 검증을 수행한 뒤 공개합니다.
- 릴리즈 CI는 진행 중인 `main` 실행을 새 푸시 때문에 취소하지 않습니다. PR에서는 오래된 중복 검증을 취소할 수 있습니다.
- `dependency-security.yml`의 주간 RustSec·Pillow 확인은 코드 변경과 관계없이 계속 실행합니다. `refresh-artwork.yml`은 관리자가 명시적으로 요청했을 때만 후보 이미지를 수집하며 자동 수락하지 않습니다.
- CodeQL은 예약 및 수동 실행에서 **Rust·Python·Actions 모두 분석**합니다. 변경 전용 PR에서는 수정된 언어만 분석하고, 다른 언어는 경량 체크로 완료합니다.

## 확인과 증거

1. PR 페이지에서 `Classify CI scope` 출력의 `build / images / ui / perf / audit / release` 값을 확인합니다.
2. 문서 전용 PR이라면 Windows·Ubuntu·CodeQL 매트릭스 검사 이름이 **존재하는지**, 내부 빌드/분석은 생략됐는지 확인합니다.
3. 신규 버전 PR 및 `main`은 실제 산출물·GUI 캡처·이미지 검사·서명 검증이 모두 통과해야 합니다.
4. 현행 보안 운영 상태는 [보안 배포 안내](SECURE_RELEASE_v1_0_3.md), 프로그램 안내는 [README](../README.md)를 참조하십시오.

[GitHub Actions 필수 체크·Skipped Job 규칙](https://docs.github.com/en/pull-requests/how-tos/merge-and-close-pull-requests/troubleshooting-required-status-checks) · [GitHub Job 조건](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-jobs-with-conditions)

## 추가 최적화: 테스트와 Release 빌드 병렬화 (2026-11-07)

[기존 `main` 실행 #42312700762](https://github.com/P-JSL/gacha_pickup_calculator/actions/runs/42312700762)은 Windows 단위 테스트 약 88초→Clippy 24초→Release 빌드 140초를 **동일 Job에서 순차 실행**했습니다. Linux도 단위 테스트 약 47초→Clippy 20초→Release 빌드 89초가 순차였습니다.

이번 변경은 다음과 같이 책임을 분리합니다.

| Job | 수행 | 완료 후 연결 |
|---|---|---|
| `Verify (windows-latest)` | Windows 테스트·Clippy | 릴리즈에 필수 |
| `Verify (ubuntu-24.04)` | Linux 테스트·Clippy·라이브러리 문서 검사 | 릴리즈에 필수 |
| `Build Windows release executable` | Release 컴파일, 선택적 서명, 해당 시 Windows UI 진단, 릴리즈 실행 파일 업로드 | 릴리즈에 필수 |
| `Build Linux GUI executable` | Release 컴파일, Linux GUI 실제 시작 검사, 스크린샷용 실행 파일 업로드 | GUI 캡처 시작 및 릴리즈에 필수 |
| `Capture latest-character GUI for README` | **Linux 빌드 직후** 실제 GUI 캡처 | 릴리즈에 필수 |

모든 Job의 코드·의존성·테스트 명령은 기존 값 그대로 보존합니다. **새 의존성·액션 캐시를 사용하지 않습니다.** 특히 GitHub Actions 빌드 캐시는 변조된 캐시가 바이너리에 영향을 줄 수 있으므로, 신뢰 경계를 별도 설계하지 않은 상태에서는 출처 증명 릴리즈에 적용하지 않습니다. Job 수가 늘어 총 러너 실행 분량은 증가할 수 있지만, 서로 기다리는 구간을 줄여 **사용자가 보는 완료시간(벽시계 시간)**을 줄이도록 설계했습니다. 실제 개선 폭은 변경 전후 GitHub Actions 실행 기록으로 확인해야 합니다.

## 입력 완료·명시적 시뮬레이션 버튼·실제 작업 완료 기반 캡처

- **자동 계산 기능 제거:** 앱 시작, 입력 타이핑, 게임 전환, 캐릭터·무기 선택에서는 백그라운드 계산을 시작하지 않습니다. 앱 환경 설정의 `auto_recalculate` 및 디바운스 입력은 더 이상 제공하지 않습니다.
- **명시적 계산:** 사용자가 입력을 모두 마치고 Dashboard 또는 Simulator의 `Run Simulation`을 클릭한 경우에만 새 계산을 요청합니다. Reset은 입력만 초기화합니다.
- **세대별 결과 무효화:** 이미 실행 중인 계산과 관계없는 입력이 변경되면 이전 작업 세대를 무효화합니다. 이전 결과가 늦게 도착해도 현재 게임·입력에 덮어쓰지 않습니다.
- **실시간 CI 감지:** 스크린샷 CI는 `GACHA_CI_SIMULATION_EVENTS=1`로 **이벤트 관찰만 활성화**합니다. 실제 버튼을 클릭한 뒤 같은 게임·동일 세대의 `STARTED`와 `COMPLETED`가 확인되어야 캡처합니다. 자동 계산을 켜던 `GACHA_CI_AUTO_REFRESH` 사용은 제거했습니다.
- **시각적 검증:** 기존 캐릭터/게임 탭 이미지 비교 및 9장 실제 스크린샷 검증은 유지합니다. 버튼 좌표는 **egui의 실제 UI 위젯 좌표·클리핑 여부를 실시간으로 확인**하여 결정하고, 표시되지 않은 버튼은 클릭하지 않습니다. CI는 기록된 해당 좌표를 마우스로 클릭하며 예상 버튼이 없으면 실패합니다.
- **대기 방식:** 기존 게임 변경 후 고정 `sleep 11`을 제거하고 최대 90초 한도로 완료 이벤트를 100ms 주기로 확인합니다. 계산 결과 렌더링 이후 화면 촬영하도록 한 프레임의 완료 경계를 둡니다. 실제 소요시간은 게임·계산 모델에 따라 달라집니다.

[관련 스크립트](../scripts/capture_readme_screenshots.sh) · [이벤트 대기 검증](../scripts/test_ui_capture_controls.py)
