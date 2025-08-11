#!/bin/bash
REMOTE_IP=`ip a | grep enp4s0 | tail -n 1 | awk '{print $2}' | sed -e "s/\/.*//"` cargo build
