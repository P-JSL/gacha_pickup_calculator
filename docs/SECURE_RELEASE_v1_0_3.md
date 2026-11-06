# v1.0.3 배포 보안 및 관리자 설정

## 공식 근거

- [GitHub: rulesets 설정](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository)
- [GitHub: immutable releases 설정](https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/establish-provenance-and-integrity/prevent-release-changes)
- [GitHub: artifact attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations)
- [GitHub CLI: 출처 증명 검증](https://cli.github.com/manual/gh_attestation_verify)
- [Microsoft: Smart App Control 코드 서명](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control)
- [CodeQL Rust 지원](https://codeql.github.com/docs/codeql-overview/supported-languages-and-frameworks/)

## 1. 자동 적용된 코드·CI 변경

- `src/data/runtime_updates.rs`: GitHub REST에서 `verification.verified=true` 및 `reason=valid`, GitHub committer, P-JSL author, 40자리 SHA를 확인합니다. 신뢰 확인 실패 시 로컬 캐시를 덮어쓰지 않습니다.
- `.github/workflows/rust.yml`: `main` 배포의 최종 ZIP·EXE·SHA256SUMS에 OIDC/Sigstore 공급망 서명을 생성하고 `gh attestation verify --signer-workflow`로 검증합니다. 결과는 해당 저장소에서 조회할 수 있습니다.
- 릴리즈는 초안 생성→자산 검증→게시 순서이며 GitHub Immutable Releases 정책 활성화에 대응합니다.
- `.github/workflows/security-codeql.yml`: Rust/Python/GitHub Actions CodeQL 분석. `src/lib.rs` 공개 확률 엔진의 unsafe Rust 금지.
- `tests/performance_regression.rs`: FFT/직접 계산과 분위수의 실제 수치·정합성 비교.

## 2. 관리자가 활성화해야 하는 보호 정책

연동 GitHub 앱은 현재 Settings/Rulesets에 대한 쓰기 권한을 제공하지 않습니다. 다음 작업은 저장소 소유자 권한으로 수행해야 하며 **이 문서가 있다고 자동 적용되는 것은 아닙니다.**

1. 저장소 **Settings → Rules → Rulesets**에서 `main` 대상 PR 필수, force-push/삭제 금지, 서명된 커밋 및 필수 CI 검증을 활성화합니다. 기존 검증 상태 이름과 CodeQL 3종을 필수 체크로 지정합니다.
2. 로컬 GitHub CLI `gh auth login`에서 저장소 Administration:write 권한이 있는 계정으로 로그인 후 `python3 scripts/configure_repository_security.py`를 실행해 변경 사항을 검토합니다. 실제 적용은 `python3 scripts/configure_repository_security.py --apply`입니다.
3. **필수 승인 1건**은 다른 실제 리뷰어가 접근 권한을 가진 경우에만 `--approvals 1 --apply`로 적용합니다. 단독 소유자 계정은 자기 PR을 승인할 수 없으므로 기본값 0은 필수 PR+CI를 강제하되 별도 승인자를 강제하지 않습니다.
4. 저장소 **Settings → General → Releases**에서 **Enable release immutability**를 활성화합니다. 기존 릴리즈에는 소급 적용되지 않습니다. 이후 태그와 릴리즈 자산의 수정·삭제가 방지되고 GitHub 추가 릴리즈 증명이 생성됩니다.
5. 저장소에서 **2FA/패스키, 최소 관리자 권한, 토큰 만료 및 보안 경고**를 관리하고 CodeQL 보안 경고를 지속 검토합니다. GitHub에서 승인되지 않은 앱의 저장소 쓰기 권한은 제거합니다.

## 3. Windows 신뢰된 코드 서명(관리자 인증서 필요)

Microsoft Smart App Control이 인정하는 Authenticode 신뢰를 얻으려면 **공인 발급기관 인증서**가 필요합니다. 코드에 자체 서명한 임의 인증서를 배포하지 않습니다.

- 신뢰된 제공자에서 코드 서명 인증서를 발급받고, 관리자가 PFX 형태로 안전하게 준비한 경우 GitHub **Settings → Secrets and variables → Actions**에 `WINDOWS_SIGNING_PFX_BASE64`과 `WINDOWS_SIGNING_PFX_PASSWORD`를 등록합니다.
- `WIN_SIGN_PFX_BASE64`와 비밀번호를 전달받은 Windows `main` 빌드에서만 `scripts/sign_windows_release.ps1`이 `signtool sign` 및 `verify /pa /all`을 실행합니다. 실패 시 릴리즈가 중단됩니다. PR에서는 이 비밀 정보를 사용하지 않습니다.
- 인증서가 없으면 **실행 파일은 미서명** 상태이며, 인증서 미설정 경고를 CI에 남깁니다. Sigstore GitHub 빌드 출처 증명은 별개이고 Windows Smart App Control의 Authenticode 요구사항을 대신하지 않습니다.
- 보안상 가능하면 클라우드 HSM 또는 보호된 발급자 서명 서비스(예: Microsoft Artifact Signing)를 이용하고, 로컬 PFX 방식은 최소 권한과 비밀 보관 정책을 준수해야 합니다.

## 4. 사용자의 릴리즈 검증

GitHub CLI가 설치된 환경에서 공식 릴리즈 파일을 다운로드한 후 다음 명령으로 검증할 수 있습니다.

```sh
gh attestation verify gacha-pickup-calculator-windows-x64-v1.0.3.zip -R P-JSL/gacha_pickup_calculator --signer-workflow P-JSL/gacha_pickup_calculator/.github/workflows/rust.yml
sha256sum -c SHA256SUMS.txt
```

Windows PowerShell은 `Get-FileHash`로 SHA-256 해시를 대조할 수 있습니다. `SHA256SUMS.txt`를 **같은 사이트에서만 받아 독립적 신뢰 없이 비교하는 것**은 게시자 인증이 아닙니다. 출처 증명에서 검증된 주체·서명 워크플로 ID를 확인하십시오.

## 5. 남아 있는 위협과 한계

- GitHub 계정/관리자 권한이 탈취되면 GitHub가 검증한 악성 커밋·릴리즈도 게시될 수 있습니다. 신뢰를 완전히 분리하려면 GitHub 이외의 오프라인 키로 서명한 데이터 매니페스트와 앱의 고정 공개키 검증이 필요합니다. **별도 키가 제공되지 않아 이번 버전은 GitHub 검증 기반입니다.**
- OS 관리자 권한 또는 동일 사용자 프로세스가 침해된 PC는 앱이 완전히 격리할 수 없습니다. Windows Defender, 시스템 업데이트, Smart App Control, 비관리자 실행 및 안전한 다운로드 경로를 병행하십시오.
- 이 공개 저장소의 소스 코드는 `pub(crate)` 적용과 관계없이 누구나 읽을 수 있습니다. 비밀 자산은 Private 저장소 또는 관리 키 스토리지에 보관하십시오.
- 서명된 provenance는 **빌드 출처와 내용 무결성**을 증명하지만 모든 코드의 안전성·오류 부재를 보증하지 않습니다.
