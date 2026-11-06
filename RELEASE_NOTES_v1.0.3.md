# v1.0.3 — 배포 서명·검증과 공급망 보안 및 실측 성능 검사

2026-11-07

## 보안·무결성
- 원격 업데이트는 GitHub가 검증한 커밋 서명/예상 게시자/정확한 SHA가 확인된 경우에만 게임 카탈로그를 적용합니다. 검증 실패 시 기존 데이터가 유지됩니다. 이 확인은 GitHub 계정·저장소 침해에 대한 독립적인 신뢰 루트가 아닙니다.
- 공개 라이브러리 내부 안전성 강화를 위해 `#![forbid(unsafe_code)]`를 추가했습니다.
- `CodeQL (rust/python/actions)` 워크플로로 보안 정적 분석을 실행하며 Cargo.lock RustSec, ClamAV, CSV·이미지 검사도 유지합니다.
- GitHub Actions의 OIDC/Sigstore SLSA 빌드 provenance를 ZIP/실행 파일/체크섬 등 5개 최종 릴리즈 자산에 발급한 뒤, CLI 검증에 성공해야 공개합니다. 기존 공개 GitHub 계정 권한 경계는 유지됩니다.
- 릴리즈를 임시 초안으로 등록하고 모든 자산을 검증한 뒤 공개하여 GitHub의 immutable releases 정책에 대응합니다(저장소 관리자가 정책을 활성화해야 실제 불변성이 적용됩니다).

## Windows 보안
- 신뢰된 Authenticode PFX와 비밀번호가 GitHub Secrets에 설정된 `main` 릴리즈에서만 `signtool sign /fd SHA256` + `verify /pa /all`을 수행합니다.
- 인증서가 없으면 Windows 실행 파일은 기존처럼 미서명입니다. **Sigstore provenance가 Windows Smart App Control에서 인정하는 Authenticode 서명을 대체하지 않습니다.**
- 사용자의 개인키·토큰·PFX는 저장소에 등록하지 않습니다. 인증서 발급 및 GitHub Secret 등록은 별도의 관리자 작업입니다.

## 성능·회귀
- 밀집 PMF 직접 합성과 FFT의 수치 동등성 테스트 유지, release 빌드에서 실제 소요 시간/속도 향상 계수 출력.
- O(n) 선형 분위수 탐색과 O(log n) CDF 하한 이진 검색을 같은 데이터로 비교합니다.
- 전체 프로그램의 복잡도는 DP 상태 공간, Monte Carlo 시뮬레이션 및 UI 렌더링에 의존합니다. benchmark를 실측하기 전 개선 배수를 주장하지 않습니다.
- 외부 1.x 확률 API, 캐릭터–장비 매핑, 천장·환급 계산 및 게임별 세션 로직을 변경하지 않았습니다.

## 운영상 별도 권한 필요
- 현재 GitHub 계정·저장소 설정은 실행 중인 애플리케이션만으로 보호할 수 없습니다. 관리자 2FA/패스키, 최소 권한, main Ruleset, Immutable Releases 활성화 및 Windows 신뢰된 인증서 발급·등록은 직접 수행해야 합니다.
- [보안 배포/관리자 설정 절차](docs/SECURE_RELEASE_v1_0_3.md)와 [성능 측정 보고](docs/PERFORMANCE_V1_0_3.md)를 참조하십시오.
