#!/bin/sh

if [ $(id -u) -ne 0 ]; then echo "Please run as root!"; exit 1; fi

if [ $# -eq 0 ]; then
    echo "No hard drive specified!"
    exit 1
fi

pvcreate ${1}2
vgcreate vg_DISTRO ${1}2
