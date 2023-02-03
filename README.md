# Backer

> Back up your files to the designated server or Qiniu oss, Alibaba oss and Tencent oss.


## Supported target list

- [x] backer-server
- [x] qiniu
- [ ] aliyun-oss
- [ ] tencent-oss

## Quick Start
download the package corresponding to your operating system:
### run backer
```bash
wget https://github.com/duyunis/backer/releases/download/|latest version|/backer_x86_64-linux.tar.gz
tar -zxvf backer_x86_64-linux.tar.gz
cd backer
./backer -c backer.yaml
```
### run backer-server
```bash
wget https://github.com/duyunis/backer/releases/download/|latest version|/backer-server_x86_64-linux.tar.gz
tar -zxvf backer-server_x86_64-linux.tar.gz
cd backer-server
./backer-server -p 9618 --backup-dir /opt/backer_dir
```

## Build

```bash
git clone https://github.com/duyunis/backer.git
cd backer
cargo build --release
```

## Build docker image

build backer
```bash
docker build -f deploy/docker/Dockerfile -t backer:v1 --target backer .
```

build backer server
```bash
docker build -f deploy/docker/Dockerfile -t backer-server:v1 --target backer-server .
```

## Run

### run backer in docker
```bash
docker run -it -d --restart=always -v /etc/backer/backer.yaml:/etc/backer/backer.yaml -v "{dir of files to be backed up}:{dir of files configured in the backer.yaml file}" duyunis/backer:latest
```

### run backer-server in docker
```bash
docker run -it -d --restart=always -p 9618:9618 -v /opt/backer_dir:/opt/backer_dir  duyunis/backer-server:latest
```