#!/bin/sh

#KERNEL=$1

# aurutils needs pacutils, vifm, and unbuffer (in extra/expect)
sudo pacman --noconfirm -Syy --needed expect pacutils vifm patch flex
sudo pacman --noconfirm -S --needed gcc # install big packags one by one (weird issue)
sudo pacman -Scc
sudo pacman --noconfirm -S glibc
sudo pacman -Scc

# Resize cowspace to have enough space for dependencies for AUR kernels
mount / -o remount,size=4G /run/archiso/cowspace
mount /run/user/0 -o remount,size=2G /run/user/0

id -u aur &> /dev/null || useradd -m -s /bin/bash aur
echo 'aur ALL=(ALL:ALL) NOPASSWD: ALL' >> /etc/sudoers

#runuser aur << EOF # <--------- WORKS

runuser aur -c 'git clone https://github.com/AladW/aurutils /tmp/aurutils_temp'
runuser aur -c '(cd /tmp/aurutils_temp && sudo make install)' # Do not change pwd
runuser aur -c 'sudo cp -a ./src/distros/arch/aur/aurutils.conf /etc/pacman.d/aur'
runuser aur -c 'sudo install -d /var/cache/pacman/aur -o aur'
runuser aur -c 'echo "Include = /etc/pacman.d/aur" | sudo tee -a /etc/pacman.conf'
runuser aur -c 'sudo install -d /var/cache/pacman/aur -o aur'
runuser aur -c 'cd /tmp/aurutils_temp && repo-add /var/cache/pacman/aur/aur.db.tar.gz'

# echo "keyserver hkp://keys.gnupg.net" >> ~/.gnupg/gpg.conf
sudo pacman -Sy

runuser aur -c 'aur sync linux-xanmod --no-view --no-confirm --makepkg-args=--skipinteg'
#aur sync linux${KERNEL} --no-view --no-confirm
#EOF

