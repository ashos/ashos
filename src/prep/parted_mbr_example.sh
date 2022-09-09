#!/bin/sh

if [ $# -eq 0 ]; then
    echo "No hard drive specified!"
    exit 1
fi

sudo parted --align minimal --script $1 mklabel msdos unit MiB mkpart primary \
            ext4 1MiB 80% set 1 boot on mkpart primary ext4 80% 100%
#sudo mkfs.ext4 -L MBR ${1}1

