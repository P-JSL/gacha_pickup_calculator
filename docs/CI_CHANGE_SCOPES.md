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
- 이미지 검사·GUI 캡처처럼 매트릭스가 아닌 Job은 관련 없는 변경에서 Job 자체가 Skipped일 수 있습니다.
- Ruleset을 활성화할 경우 `Classify CI scope`, `Validate documentation and release contracts`, `Classify CodeQL inputs`과 필요한 매트릭스별 검사 이름을 함께 등록하는 것이 안전합니다. 판별 단계가 실패할 때 뒤따르는 검사만 Skipped 되는 것을 성공으로 오해하지 않도록 합니다.
- 기존 `scripts/configure_repository_security.py --apply`는 여러 검사와 정책을 **즉시 Active**로 설정하므로, 각 체크 이름과 PR 병합 경로를 검증하기 전 실행하지 않는 편이 안전합니다.

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
