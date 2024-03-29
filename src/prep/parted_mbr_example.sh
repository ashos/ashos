#!/bin/sh

if [ $(id -u) -ne 0 ]; then echo "Please run as root!"; exit 1; fi

if [ $# -eq 0 ]; then
    echo "No hard drive specified!"
    exit 1
fi

parted --align minimal --script $1 mklabel msdos unit MiB \
        mkpart primary ext4 1MiB 80% set 1 boot on \
        mkpart primary ext4 80% 100%

# if you would like to have ash as bundle (first partition)
#parted --align minimal --script $1 mklabel msdos unit MiB \
#        mkpart primary fat32 1MiB 33MiB \
#        mkpart primary ext4 33MiB 80% set 2 boot on \
#        mkpart primary ext4 80% 100%
#mkfs.vfat -F32 -n ASH ${1}1

# External boot partition
#parted --align minimal --script $2 mklabel msdos unit MiB \
#        mkpart primary ext4 0% 100% set 1 boot on \
#mkfs.btrfs -L BOOT ${2}1

# Bundled ash on external partition
#parted --align minimal --script $3 mklabel msdos unit MiB \
#        mkpart primary fat32 0% 100%
#mkfs.vfat -F32 -n EFI ${3}1

