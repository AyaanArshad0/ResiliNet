#!/bin/bash
# Simulate unstable mobile network (Linux)
# Requires 'tc' (iproute2)

IFACE=$(ip route | grep default | awk '{print $5}')
echo "🌪️  Adding Network Chaos to $IFACE..."

# Add 10% packet loss and 200ms delay
# 'netem' = Network Emulator
tc qdisc add dev $IFACE root netem loss 10% delay 200ms

echo "Network is now UNSTABLE (10% Drop, 200ms Delay)."
echo "Run './reset_network.sh' to fix."
