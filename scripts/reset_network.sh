#!/bin/bash
# Reset network conditions

IFACE=$(ip route | grep default | awk '{print $5}')
echo "🧹 Resetting Network Chaos on $IFACE..."

tc qdisc del dev $IFACE root

echo " Network restored to normal."
