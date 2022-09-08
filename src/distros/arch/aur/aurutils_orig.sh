#!/bin/sh

KERNEL=$1

AUR_ASROOT=1
tmp_aurutils=$(mktemp -d -p /tmp aurutils.XXXXXXXXXXXXXXXX)
git clone https://github.com/AladW/aurutils $tmp_aurutils
sudo make install -C $tmp_aurutils # Same as below
#(cd $tmp_aurutils && AUR_ASROOT=1 && sudo make install)
cp -a ./src/distros/arch/aur/aurutils.conf /etc/pacman.d/aur

echo "Include = /etc/pacman.d/aur" | sudo tee -a /etc/pacman.conf
# Create the repository root in /var/cache/pacman/aur
sudo install -d /var/cache/pacman/aur -o $USER
# Create the database in /var/cache/pacman/aur/
repo-add /var/cache/pacman/aur/aur.db.tar.gz

# aurutils needs pacutils, vifm, and unbuffer (in extra/expect)
sudo pacman -Syy expect pacutils vifm

#AUR_ASROOT=1 bash -c 'aur sync linux${KERNEL}'
AUR_ASROOT=1 bash -c 'aur sync linux-mainline'

