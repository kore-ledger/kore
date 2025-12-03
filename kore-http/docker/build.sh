#!/bin/bash

DOCKER_USERNAME="koreadmin"
DOCKER_REPO="kore-node"
TAG_ARRAY=("0.8.2-sqlite-prometheus" "0.8.2-rocksdb-prometheus")
DOCKERFILE_ARRAY=("./kore/kore-http/docker/Dockerfile.sqlite" "./kore/kore-http/docker/Dockerfile.rocksdb")
FEATURES_ARRAY=("ext-sqlite sqlite prometheus" "ext-sqlite rocksdb prometheus")

for i in "${!FEATURES_ARRAY[@]}"; do
    FEATURES="${FEATURES_ARRAY[i]}"
    TAG="${TAG_ARRAY[i]}"
    DOCKERFILE="${DOCKERFILE_ARRAY[i]}"

    echo "######################################################################"
    echo "########################## $TAG #########################"
    echo "######################################################################"

    # Construir la imagen para ARM64
    echo ""
    echo "Construyendo la imagen para ARM64 con características: $FEATURES.."
    docker build --no-cache --platform linux/arm64 --build-arg FEATURES="$FEATURES" -t ${DOCKER_USERNAME}/${DOCKER_REPO}:arm64-${TAG} --target arm64 -f $DOCKERFILE .

    # Construir la imagen para AMD64
    echo ""
    echo "Construyendo la imagen para AMD64 con características: $FEATURES.."
    docker build --no-cache --platform linux/amd64 --build-arg FEATURES="$FEATURES" -t ${DOCKER_USERNAME}/${DOCKER_REPO}:amd64-${TAG} --target amd64 -f $DOCKERFILE .

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

    echo "Proceso completado para características: $FEATURES. Las imágenes han sido subidas a Docker Hub."
done


echo "######################################################################"
echo "############################## LIMPIANDO #############################"
echo "######################################################################"

for i in "${!TAG_ARRAY[@]}"; do
    TAG="${TAG_ARRAY[i]}"
    docker rmi ${DOCKER_USERNAME}/${DOCKER_REPO}:arm64-${TAG} ${DOCKER_USERNAME}/${DOCKER_REPO}:amd64-${TAG}
done