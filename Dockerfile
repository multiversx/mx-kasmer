# Development image for mx-kasmer: K Framework plus what is needed to build the
# semantics and run the (still Python-based) tooling: OpenSSL headers for the
# SHA-256 hook, wabt for .wat test fixtures, Python and Poetry.
#
# Deliberately contains no Rust toolchain and no sc-meta: contracts are compiled
# elsewhere and supplied as .wasm files.
#
#   docker build --platform linux/amd64 --build-arg K_VERSION=7.1.140 -t mx-kasmer-dev:k7.1.140 .
#
# The K base image is published for amd64 only; on Apple Silicon it runs emulated.

ARG K_VERSION
FROM runtimeverificationinc/kframework-k:ubuntu-jammy-${K_VERSION}

RUN    apt-get update            \
    && apt-get install --yes     \
         curl                    \
         git                     \
         libssl-dev              \
         python3                 \
         python3-pip             \
         python3-venv            \
         wabt                    \
    && rm -rf /var/lib/apt/lists/*

RUN curl -sSL https://install.python-poetry.org | POETRY_HOME=/usr python3 - \
    && poetry --version

ARG USER=user
ARG USER_ID=1000
RUN useradd -m -u ${USER_ID} -s /bin/bash ${USER}

USER ${USER}
WORKDIR /home/${USER}

# Build products (compiled K definitions, Python virtual environments) go here.
# Created up front so that a named volume mounted on it is owned by the user.
RUN mkdir -p /home/${USER}/.cache
