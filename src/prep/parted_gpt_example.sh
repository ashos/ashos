#!/bin/sh

if [ $(id -u) -ne 0 ]; then echo "Please run as root!"; exit 1; fi

if [ $# -eq 0 ]; then
    echo "No hard drive specified!"
    exit 1
fi

parted --align minimal --script $1 mklabel gpt unit MiB mkpart ESP fat32 0% 256 \
            set 1 boot on mkpart primary ext4 256 80% mkpart primary ext4 80% 90%
mkfs.vfat -F32 -n EFI ${1}1

