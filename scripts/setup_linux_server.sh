#!/bin/bash
# Setup script for GhostTunnel Server (Linux)
# Needs Root privileges (sudo)

echo "🔧 Configuring Linux Networking for GhostTunnel..."

# 1. Enable IP Forwarding
echo "1. Enabling IP Forwarding..."
sysctl -w net.ipv4.ip_forward=1

# 2. Identify Interface (Heuristic: Route to default)
IFACE=$(ip route | grep default | awk '{print $5}')
echo "2. Detected Internet Interface: $IFACE"

# 3. Enable NAT (Masquerading)
echo "3. Enabling NAT (Masquerading) on $IFACE..."
iptables -t nat -A POSTROUTING -o $IFACE -j MASQUERADE

# 4. Allow forwarding between TUN and Eth
echo "4. Allowing traffic forwarding..."
iptables -A FORWARD -i tun0 -o $IFACE -j ACCEPT
iptables -A FORWARD -i $IFACE -o tun0 -m state --state RELATED,ESTABLISHED -j ACCEPT

echo " Network Configuration Complete."
echo "You can now run GhostTunnel server."
