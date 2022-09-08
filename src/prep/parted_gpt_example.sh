#!/bin/sh

if [ $# -eq 0 ]; then
    echo "No hard drive specified!"
    exit 1
fi

sudo parted --align minimal --script $1 mklabel gpt unit MiB mkpart ESP fat32 0% 256 \
            set 1 boot on mkpart primary ext4 256 20% mkpart primary ext4 20% 40%
sudo mkfs.vfat -F32 -n EFI ${1}1

