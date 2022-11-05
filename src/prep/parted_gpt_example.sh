#!/bin/sh

if [ $(id -u) -ne 0 ]; then echo "Please run as root!"; exit 1; fi

if [ $# -eq 0 ]; then
    echo "No hard drive specified!"
    exit 1
fi

parted --align minimal --script $1 mklabel gpt unit MiB \
        mkpart ESP fat32 0% 256 set 1 boot on \
        mkpart primary ext4 256 80% \
        mkpart primary ext4 80% 100%
mkfs.vfat -F32 -n EFI ${1}1

# External boot partition
#parted --align minimal --script $2 mklabel gpt unit MiB \
#        mkpart primary ext4 0% 100%
#mkfs.btrfs -L BOOT ${2}1

