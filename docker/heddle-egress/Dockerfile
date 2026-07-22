FROM rust:1.92.0-bookworm

# Mirrors script/linux/install_build_deps.
#
# strace is the observation instrument; curl backs the positive control that
# proves the instrument works before any PASS is trusted.
RUN apt-get update && apt-get install -y --no-install-recommends \
        strace curl git ca-certificates unzip \
        build-essential cmake pkg-config \
        libssl-dev libfreetype-dev libexpat1-dev libgit2-dev \
        libfontconfig1-dev libasound2-dev libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# protoc must come from the official release, NOT apt. Debian's
# protobuf-compiler ships the binary without the well-known type definitions,
# so builds fail with:
#   protoc failed: google/protobuf/descriptor.proto: File not found.
# Both crates/remote_server/build.rs and the warp_multi_agent_api git
# dependency compile protos, and the latter imports descriptor.proto.
# Upstream extracts 'include/*' for exactly this reason
# (script/linux/install_build_deps:60).
ARG PROTOC_VERSION=25.1
RUN set -eux; \
    arch="$(uname -m)"; \
    case "$arch" in \
      x86_64)  zip="protoc-${PROTOC_VERSION}-linux-x86_64.zip" ;; \
      aarch64) zip="protoc-${PROTOC_VERSION}-linux-aarch_64.zip" ;; \
      *) echo "unsupported arch: $arch" >&2; exit 1 ;; \
    esac; \
    curl -fsSL -o /tmp/protoc.zip \
      "https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/${zip}"; \
    unzip -o /tmp/protoc.zip -d /usr/local bin/protoc 'include/*'; \
    rm /tmp/protoc.zip; \
    protoc --version

WORKDIR /src
ENTRYPOINT ["/bin/bash", "-c"]
