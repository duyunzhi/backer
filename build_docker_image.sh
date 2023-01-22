#!/bin/bash

REPO_URL=duyunzhi1

VERSION=0.1.0
LATEST_VERSION=latest

BACKER_IMAGE_NAME=backer
BACKER_SERVER_IMAGE_NAME=backer-server

# build image
docker build -f deploy/docker/Dockerfile-backer -t ${REPO_URL}/${BACKER_IMAGE_NAME}:"${VERSION}" .
docker build -f deploy/docker/Dockerfile-backer-server -t ${REPO_URL}/${BACKER_SERVER_IMAGE_NAME}:"${VERSION}" .

# tag latest image
docket tag ${REPO_URL}/${BACKER_IMAGE_NAME}:"${VERSION}" ${REPO_URL}/${BACKER_IMAGE_NAME}:"${LATEST_VERSION}"
docket tag ${REPO_URL}/${BACKER_SERVER_IMAGE_NAME}:"${VERSION}" ${REPO_URL}/${BACKER_SERVER_IMAGE_NAME}:"${LATEST_VERSION}"

# push to repo
docker push ${REPO_URL}/${BACKER_IMAGE_NAME}:"${VERSION}"
docker push ${REPO_URL}/${BACKER_IMAGE_NAME}:"${LATEST_VERSION}"
docker push ${REPO_URL}/${BACKER_SERVER_IMAGE_NAME}:"${VERSION}"
docker push ${REPO_URL}/${BACKER_SERVER_IMAGE_NAME}:"${LATEST_VERSION}"

# remove builder intermediate image
sleep 5s
docker image prune --force
