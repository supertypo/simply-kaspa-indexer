#!/bin/sh
ARCHES="linux/amd64 linux/arm64"

REPO_URL="https://github.com/supertypo/simply-kaspa-indexer"
DOCKER_REPO1="supertypo/simply-kaspa-indexer"
DOCKER_REPO2="supertypo/kaspa-db-filler-ng"
BUILD_DIR="$(dirname $0)"
PUSH=$1
VERSIONS=$2
TAG=${3:-main}
REPO_DIR="$BUILD_DIR/.work"

if [ -z "$PUSH" ] || [ -z "$VERSIONS" ]; then
  echo "Usage: $0 push|nopush \"multiple versions\" [tag] [git-repo]"
  exit 1
fi

set -e

if [ ! -d "$REPO_DIR" ]; then
  git clone "$REPO_URL" "$REPO_DIR"
  echo $(cd "$REPO_DIR" && git reset --hard HEAD~1)
fi

echo "============================================================="
echo " Pulling $REPO_URL"
echo "============================================================="
(cd "$REPO_DIR" && git fetch && git checkout $TAG > /dev/null && (git pull 2>/dev/null | true))

tag=$(cd "$REPO_DIR" && git log -n1 --format="%cs.%h")
gitDescribe=$(cd "$REPO_DIR" && git describe --tags --dirty --always)

docker=docker
id -nG $USER | grep -qw docker || docker="sudo $docker"

plain_build() {
  echo
  echo "============================================================="
  echo " Running current arch build"
  echo "============================================================="

  $docker build --pull \
    --build-arg REPO_DIR="$REPO_DIR" \
    --build-arg VERSION="$gitDescribe" \
    --tag ${DOCKER_REPO1}:$tag "$BUILD_DIR"

  $docker tag ${DOCKER_REPO1}:$tag ${DOCKER_REPO2}:$tag
  echo Tagged $DOCKER_REPO2:$tag

  for version in $VERSIONS; do
    $docker tag $DOCKER_REPO1:$tag $DOCKER_REPO1:$version
    echo Tagged $DOCKER_REPO1:$version
    $docker tag $DOCKER_REPO2:$tag $DOCKER_REPO2:$version
    echo Tagged $DOCKER_REPO2:$version
  done

  if [ "$PUSH" = "push" ]; then
    $docker push $DOCKER_REPO1:$tag
    $docker push $DOCKER_REPO2:$tag
    for version in $VERSIONS; do
      $docker push $DOCKER_REPO1:$version
      $docker push $DOCKER_REPO2:$version
    done
  fi
  echo "============================================================="
  echo " Completed current arch build"
  echo "============================================================="
}

multi_arch_build() {
  echo
  echo "============================================================="
  echo " Running multi arch build"
  echo "============================================================="
  dockerRepoArgs=

  if [ "$PUSH" = "push" ]; then
    dockerRepoArgs="$dockerRepoArgs --push"
  fi

  for version in $VERSIONS; do
    dockerRepoArgs="$dockerRepoArgs --tag $DOCKER_REPO1:$version"
    dockerRepoArgs="$dockerRepoArgs --tag $DOCKER_REPO2:$version"
  done

  $docker buildx build --pull --platform=$(echo $ARCHES | sed 's/ /,/g') $dockerRepoArgs \
    --build-arg REPO_DIR="$REPO_DIR" \
    --build-arg VERSION="$gitDescribe" \
    --tag $DOCKER_REPO1:$tag \
    --tag $DOCKER_REPO2:$tag "$BUILD_DIR"
  echo "============================================================="
  echo " Completed multi arch build"
  echo "============================================================="
}

if [ "$PUSH" = "push" ]; then
  echo
  echo "============================================================="
  echo " Setup multi arch build ($ARCHES)"
  echo "============================================================="
  if $docker buildx create --name=mybuilder --append --node=mybuilder0 --platform=$(echo $ARCHES | sed 's/ /,/g') --bootstrap --use 1>/dev/null 2>&1; then
    echo "SUCCESS - doing multi arch build"
    multi_arch_build
  else
    echo "FAILED - building on current arch"
    plain_build
  fi
else
  plain_build
fi
