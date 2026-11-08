# 배포 보안 및 관리자 설정

**현재 기준: v1.0.12**

## 현행 릴리즈의 검증 범위

- `main` 배포는 Windows/Linux 빌드·테스트, RustSec, CodeQL(별도 워크플로), PNG 무결성·악성코드 검사, 성능 회귀 및 실제 GUI 캡처 검증을 거칩니다.
- 릴리즈 ZIP·EXE·체크섬에는 GitHub OIDC/Sigstore 출처 증명을 발급하고, 지정한 `.github/workflows/rust.yml`의 서명 주체를 확인합니다.
- **검증 증거:** [v1.0.12 릴리즈](https://github.com/P-JSL/gacha_pickup_calculator/releases/tag/v1.0.12) · [GitHub Actions](https://github.com/P-JSL/gacha_pickup_calculator/actions/runs/42464003186).
- `src/data/runtime_updates.rs`는 승인된 GitHub 계정·커밋·서명 검증을 요구하고 캐시 파일 입력을 제한합니다. 원격 자료를 검증하지 못하면 기존 캐시/내장 리소스로 동작합니다.
- 배포된 실행 파일이 Windows **Authenticode로 서명됐는지**는 사용자 PC에서 별도 검증해야 합니다. Sigstore 증명은 OS의 코드 서명·SmartScreen 신뢰를 대체하지 않습니다.

## 다운로드 파일 검증

```sh
gh attestation verify gacha-pickup-calculator-windows-x64-v1.0.12.zip -R P-JSL/gacha_pickup_calculator --signer-workflow P-JSL/gacha_pickup_calculator/.github/workflows/rust.yml
sha256sum --check SHA256SUMS.txt
```

Windows PowerShell의 `Get-FileHash`로도 SHA-256을 확인할 수 있습니다. 같은 출처에서 받은 체크섬 비교만으로 독립된 게시자 신뢰가 입증되는 것은 아니므로, 출처 증명의 리포지토리/워크플로 주체도 함께 확인하십시오.

## 저장소 관리자가 별도 확인해야 할 사항

1. **Settings → Rules → Rulesets:** `main` 보호, PR 필수, 강제 푸시 제한, 실제 실행되는 CI 체크 이름을 검증한 다음 활성화합니다.
2. **Settings → General → Releases:** Immutable Releases 설정을 확인합니다. 활성화 후에는 게시된 릴리즈 수정이 제한됩니다.
3. **Settings → Secrets and variables → Actions:** 실제 공인 코드 서명 인증서와 비밀 키 관리 체계를 사용할 경우에만 Windows 서명용 시크릿을 설정합니다. 인증서가 없으면 서명 완료라고 표시하지 않습니다.
4. 계정 2FA/패스키, 앱 권한, 토큰 수명, 보안 경고 및 공급망 의존성을 주기적으로 점검합니다.

`scripts/configure_repository_security.py`는 규칙 변경의 미리보기를 지원하나, `--apply`를 설정하면 실제 보호 정책이 바뀝니다. 단독 소유자 계정에서는 자신이 만든 PR에 대한 승인 강제 조항을 신중하게 적용해야 합니다.

## 보안 경계와 남은 위험

- `src/lib.rs`의 API 공개와 `src/app`·`src/ui`의 내부 가시성은 코드 계약이지, **공개 저장소의 소스 기밀성을 보장하지 않습니다.**
- GitHub 관리자 계정이 침해되면 GitHub가 검증한 악성 커밋도 가능하므로 독립 오프라인 키 신뢰가 없는 한 완전한 침해 방어를 주장할 수 없습니다.
- 로컬 관리자 권한/동일 사용자 프로세스 침해, 제3자 이미지 재배포 권한, 미공개 취약점은 CI 검사만으로 보장하지 않습니다.
- CodeQL·RustSec의 실행 성공은 모든 취약점이 없다는 증명이 아닙니다.

공식 안내: [GitHub rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository) · [GitHub artifact attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations) · [GitHub CLI 검증](https://cli.github.com/manual/gh_attestation_verify).
