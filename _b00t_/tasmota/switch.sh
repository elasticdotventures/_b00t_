#!/bin/bash

# Function to resolve IP from MAC
resolve_ip() {
    local MAC="$1"
    ARP_RESULT=$(arp -an | grep -i "$MAC" | awk '{print $2}' | tr -d '()')
    if [[ -z "$ARP_RESULT" ]]; then
        echo "ERROR: Could not find IP for MAC: $MAC" >&2
        exit 1
    fi
    echo "$ARP_RESULT"
}

# Function to query switch status
query_switch() {
    curl -s "http://$TASMOTA_IP/cm?cmnd=Power"
}

# Function to turn switch ON
switch_on() {
    curl -s "http://$TASMOTA_IP/cm?cmnd=Power%20On"
}

# Function to turn switch OFF
switch_off() {
    curl -s "http://$TASMOTA_IP/cm?cmnd=Power%20Off"
}

# Check arguments
if [[ $# -ne 2 ]]; then
    echo "Usage: $0 <mac_address> {status|on|off}"
    exit 1
fi

MAC_ADDRESS=$1
TASMOTA_IP=$(resolve_ip "$MAC_ADDRESS")

case "$2" in
    status) query_switch ;;
    on) switch_on ;;
    off) switch_off ;;
    *) echo "Usage: $0 <mac_address> {status|on|off}" ;;
esac

