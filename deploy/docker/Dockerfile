FROM rust:alpine as builder

WORKDIR /backer

COPY . .

RUN echo -e http://mirrors.ustc.edu.cn/alpine/latest-stable/main/ > /etc/apk/repositories && apk update && apk add --no-cache -U musl-dev

RUN ln -sf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime
RUN echo 'Asia/Shanghai' >/etc/timezone

RUN cp deploy/docker/rust-proxy-config.toml /usr/local/cargo/config.toml \
    && RUSTFLAGS="-D warnings" \
    && rustup override set nightly \
    && cargo clean \
    && cargo build --release --target=x86_64-unknown-linux-musl

FROM alpine:latest as backer

LABEL maintainer=duyunzhi<duyzmail@163.com>\
description="backer"

RUN echo -e http://mirrors.ustc.edu.cn/alpine/latest-stable/main/ > /etc/apk/repositories && apk update && apk add tzdata
RUN ln -sf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime
RUN echo 'Asia/Shanghai' >/etc/timezone

COPY --from=builder /backer/config/backer.yaml /etc/backer/backer.yaml
COPY --from=builder /backer/target/x86_64-unknown-linux-musl/release/backer /usr/local/bin/backer

ENTRYPOINT backer -c /etc/backer/backer.yaml

FROM alpine:latest as backer-server

LABEL maintainer=duyunzhi<duyzmail@163.com>\
description="backer"

RUN echo -e http://mirrors.ustc.edu.cn/alpine/latest-stable/main/ > /etc/apk/repositories && apk update && apk add tzdata
RUN ln -sf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime
RUN echo 'Asia/Shanghai' >/etc/timezone

COPY --from=builder /backer/target/x86_64-unknown-linux-musl/release/backer-server /usr/local/bin/backer-server

ENTRYPOINT backer-server -p 9618 --backup-dir /opt/backer_dir