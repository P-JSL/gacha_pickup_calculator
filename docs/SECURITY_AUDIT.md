# 보안·패키징 검증

**대상: v1.0.12** · [릴리즈](https://github.com/P-JSL/gacha_pickup_calculator/releases/tag/v1.0.12)

## 검증되는 범위

| 영역 | 현재 코드/워크플로의 방어 |
|---|---|
| 공개 API | `src/lib.rs`는 `unsafe` Rust 금지, 내부 모듈은 `pub(crate)` 가시성 유지 |
| 원격 카탈로그 업데이트 | 승인된 GitHub 계정·검증된 커밋·SHA 확인과 입력 크기 제한, 실패 시 기존 데이터 유지 |
| 로컬 파일 | 업데이트 캐시 크기/포맷 제한, 일반 파일 검증, 임시 파일 안전한 생성·대체 |
| 이미지·카탈로그 | PNG 형식·크기·해시, 중복/전용 장비 매핑, 출처 허용 목록 점검 |
| 빌드·배포 | Windows/Linux 테스트·Clippy, RustSec·CodeQL, SHA256SUMS, Sigstore 출처 증명과 자산 일치 검증 |
| GUI | 실제 실행 후 수동 시뮬레이션 클릭 및 완료 이벤트 감지 기반 화면 캡처 |

실행해볼 수 있는 기본 명령:

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
python3 scripts/verify_embedded_assets.py
python3 scripts/verify_release_contract.py
```

`cargo audit`는 별도 도구 설치가 필요하며, 빌드/배포 워크플로에서 RustSec 감사를 수행합니다. `scripts/build_offline_images.py`는 수동 이미지 수집용이며 일반 릴리즈 빌드가 외부 원본을 새로 내려받지 않습니다.

## 검증 결과의 해석

- [v1.0.12 배포 Actions](https://github.com/P-JSL/gacha_pickup_calculator/actions/runs/42464003186)는 정상 종료되었으나 **테스트 성공만으로 취약점 0건을 보장하지 않습니다.**
- Sigstore는 GitHub 빌드 출처와 해시 확인을 위한 것이며 Windows Authenticode 서명을 대신하지 않습니다.
- `main` 브랜치 보호, Immutable Releases, 관리자 계정 2FA와 개별 릴리즈 파일의 Windows 서명은 별도 확인 대상입니다.
- 타사 캐릭터·장비 이미지의 라이선스와 원본 매핑의 외형 정확도는 이미지 해시로 검증할 수 없습니다.
- 공개 저장소에 있는 Rust 소스는 `pub(crate)` 처리해도 외부에 보입니다. GitHub 관리자 계정 침해에 대한 독립 오프라인 키 검증 기능은 없습니다.

[관리자 설정 및 배포 보안](SECURE_RELEASE.md) · [카탈로그 점검](CATALOG_AUDIT.md).
