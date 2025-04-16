FROM ubuntu:latest

WORKDIR /app

RUN apt update && \
    apt install -y ca-certificates && \
    useradd -m eatlink && \
    chown -R eatlink /app
USER eatlink
COPY --chown=eatlink target/release/eatlink_bot /app

ARG RUST_LOG=INFO
ARG TELOXIDE_TOKEN=

ENV RUST_LOG=${RUST_LOG}
ENV DOWNLOAD_DIR=/downloads
ENV TELOXIDE_TOKEN=${TELOXIDE_TOKEN}

CMD ["./eatlink_bot"]
