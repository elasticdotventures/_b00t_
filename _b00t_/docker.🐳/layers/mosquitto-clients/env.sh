#!/bin/bash
# mosquitto-clients layer environment

export MOSQUITTO_LAYER_VERSION="2.0"
export MQTT_BROKER="${MQTT_BROKER:-mqtt://localhost:1883}"

# Docker wrapper for mosquitto_pub
_mosquitto_pub_docker() {
    docker run --rm -it \
        --network host \
        b00t-layer/mosquitto-clients:2.0 \
        mosquitto_pub "$@"
}

# Docker wrapper for mosquitto_sub
_mosquitto_sub_docker() {
    docker run --rm -it \
        --network host \
        b00t-layer/mosquitto-clients:2.0 \
        mosquitto_sub "$@"
}

alias mosquitto_pub='_mosquitto_pub_docker'
alias mosquitto_sub='_mosquitto_sub_docker'
