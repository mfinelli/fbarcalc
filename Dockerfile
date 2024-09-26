FROM rust:slim as source
WORKDIR /fbarcalc
COPY . /fbarcalc
RUN cargo vendor --locked

FROM source as build
RUN apt-get update && apt-get install -y libssl-dev pkg-config
RUN cargo build --frozen --release --verbose

FROM build as test
RUN cargo test

FROM debian:stable-slim

LABEL org.opencontainers.image.title=fbarcalc
LABEL org.opencontainers.image.version=v0.1.0
LABEL org.opencontainers.image.description="find maximum account value"
LABEL org.opencontainers.image.url=https://github.com/mfinelli/fbarcalc
LABEL org.opencontainers.image.source=https://github.com/mfinelli/fbarcalc
LABEL org.opencontainers.image.licenses=GPL-3.0-or-later

RUN useradd -r -U -m fbarcalc \
  && mkdir /home/fbarcalc/.config \
  && chown fbarcalc:fbarcalc /home/fbarcalc/.config
RUN apt-get update && apt-get install -y libssl3 \
  && apt-get clean && rm -rf /var/lib/apt/lists/*
COPY --from=source /fbarcalc /usr/src/fbarcalc
COPY --from=build /fbarcalc/target/release/fbarcalc /usr/bin/fbarcalc
USER fbarcalc
CMD ["fbarcalc"]
