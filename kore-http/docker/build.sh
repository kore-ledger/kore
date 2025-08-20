#!/bin/bash

DOCKER_USERNAME="koreledgerhub"
DOCKER_REPO="kore-http"
TAG_ARRAY=("0.7.2-rockdb-prometheus")
DOCKERFILE_ARRAY=("./kore/kore-http/docker/Dockerfile.rockdb")


    TAG="${TAG_ARRAY[0]}"
    DOCKERFILE="${DOCKERFILE_ARRAY[0]}"

    echo "######################################################################"
    echo "########################## $TAG #########################"
    echo "######################################################################"

    # Construir la imagen para ARM64
    echo ""
    echo "Construyendo la imagen para ARM64"
    docker build --no-cache --platform linux/arm64 -t ${DOCKER_USERNAME}/${DOCKER_REPO}:arm64-${TAG} --target arm64 -f $DOCKERFILE .

    # Construir la imagen para AMD64
    echo ""
    echo "Construyendo la imagen para AMD64"
    docker build --no-cache --platform linux/amd64 -t ${DOCKER_USERNAME}/${DOCKER_REPO}:amd64-${TAG} --target amd64 -f $DOCKERFILE .

    echo ""
    echo "Subiendo las imágenes a Docker Hub..."
    docker push ${DOCKER_USERNAME}/${DOCKER_REPO}:arm64-${TAG}
    docker push ${DOCKER_USERNAME}/${DOCKER_REPO}:amd64-${TAG}

    # Crear una imagen multi-arquitectura usando manifest
    echo ""
    echo "Creando imagen multi-arquitectura..."
    docker manifest rm ${DOCKER_USERNAME}/${DOCKER_REPO}:${TAG}
    docker manifest create ${DOCKER_USERNAME}/${DOCKER_REPO}:${TAG} \
        ${DOCKER_USERNAME}/${DOCKER_REPO}:amd64-${TAG} \
        ${DOCKER_USERNAME}/${DOCKER_REPO}:arm64-${TAG}

    # Marcar la plataforma para cada arquitectura
    docker manifest annotate ${DOCKER_USERNAME}/${DOCKER_REPO}:${TAG} ${DOCKER_USERNAME}/${DOCKER_REPO}:amd64-${TAG} --os linux --arch amd64
    docker manifest annotate ${DOCKER_USERNAME}/${DOCKER_REPO}:${TAG} ${DOCKER_USERNAME}/${DOCKER_REPO}:arm64-${TAG} --os linux --arch arm64

    # Subir el manifiesto a Docker Hub
    echo ""
    echo "Subiendo el manifiesto a Docker Hub..."
    docker manifest push ${DOCKER_USERNAME}/${DOCKER_REPO}:${TAG}

    echo "Las imágenes han sido subidas a Docker Hub."

echo "######################################################################"
echo "############################## LIMPIANDO #############################"
echo "######################################################################"

docker rmi ${DOCKER_USERNAME}/${DOCKER_REPO}:arm64-${TAG} ${DOCKER_USERNAME}/${DOCKER_REPO}:amd64-${TAG}