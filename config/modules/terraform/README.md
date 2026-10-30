# Terraform 모듈

[tenv](https://github.com/tofuutils/tenv)를 설치하고 프로젝트에 맞는 Terraform 버전을 실행한다. Devsisters 환경에서 활성화하며, Terraform 바이너리는 Nix로 빌드하지 않고 처음 실행할 때 tenv가 다운로드한다.

## 설정

```nix
modules.terraform = {
  enable = true;
  runEnv = {
    AWS_PROFILE = "saml";
  };
};
```

`enable`의 기본값은 `false`이고, `runEnv`의 기본값은 `{}`이다. `runEnv`는 `tf` alias에만 적용한다. 위 설정에서 `tf plan`은 `AWS_PROFILE=saml terraform plan`으로 실행된다.

## 버전 선택

`terraform`은 tenv proxy를 통해 실행된다. 프로젝트의 `.tf` 또는 `.tf.json` 파일에 `required_version`을 선언하면 `TFENV_TERRAFORM_DEFAULT_VERSION=latest-allowed`에 따라 현재 디렉터리의 제약 조건을 만족하는 버전을 선택한다.

```hcl
terraform {
  required_version = ">= 1.10.0, < 1.11.0, != 1.10.1"
}
```

tenv는 먼저 설치된 버전 중 조건을 만족하는 가장 높은 버전을 사용한다. 설치된 버전으로 조건을 만족하지 못하면 원격 목록에서 버전을 선택하며, `TENV_AUTO_INSTALL=true`에 따라 필요한 바이너리를 자동으로 설치한다. 처음 다운로드할 때는 네트워크가 필요하다. 프로젝트에 버전 파일과 제약 조건이 모두 없으면 같은 방식으로 최신 안정 버전을 선택한다.

정확한 버전을 고정하려면 `required_version`에 `= 1.10.5`처럼 명시한다. 선택적으로 `.terraform-version` 같은 프로젝트 버전 파일을 사용할 수도 있으며, 이 파일은 `latest-allowed` 기본 전략보다 우선한다.

```text
1.10.5
```

`latest-allowed` 기본 전략은 `tenv tf use`로 저장한 전역 기본값보다 우선하며, 명시적인 `TFENV_TERRAFORM_VERSION` 환경 변수는 프로젝트 버전 파일보다 우선한다. 명시한 버전과 `required_version` 조건이 충돌하면 Terraform 자체의 제약 조건 검사가 실패하므로 두 설정을 함께 관리한다. Terraform 버전과 tenv 상태는 기본적으로 `~/.tenv`에 저장된다. `TENV_ROOT`를 설정하면 저장 위치를 바꿀 수 있다.

## 기존 direnv 설정 전환

버전 선택은 각 `terraform` 실행 시 tenv가 처리하므로 Terraform 전용 direnv hook이나 `.envrc`가 필요하지 않다. `use_terraform` 함수는 제공하지 않으므로 기존 `.envrc`의 `use_terraform` 호출을 삭제한다. 호출을 남겨 두면 direnv가 `.envrc`를 불러올 때 실패한다. `home-manager switch` 후 기존 셸을 다시 시작하고, 수정한 `.envrc`는 `direnv allow`로 허용한다.

`command -v terraform`이 이전 모듈의 `.direnv/bin/terraform`을 가리키면 해당 wrapper를 정리하고 tenv proxy가 선택되는지 확인한다.

이전 `modules.terraform.defaultVersion` 옵션은 제거했다. 해당 설정이 있으면 삭제하고, 프로젝트 버전은 `required_version`으로 지정한다. 버전 목록을 갱신하기 위한 별도 Terraform flake input도 필요하지 않다.
