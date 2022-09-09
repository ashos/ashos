#!/bin/sh

KERNEL=$1

echo "*******************WHAAAAAAAAAAAAAAT 1"
echo $KERNEL

# aurutils needs pacutils, vifm, and unbuffer (in extra/expect)
sudo pacman --noconfirm -Syy expect pacutils vifm

echo "*******************WHAAAAAAAAAAAAAAT 2"
read

# Resize cowspace to have enough space for dependencies for AUR kernels
mount / -o remount,size=2G /run/archiso/cowspace

echo "*******************WHAAAAAAAAAAAAAAT 3"
read

id -u aur &> /dev/null || useradd -m -s /bin/bash aur
echo 'aur ALL=(ALL:ALL) NOPASSWD: ALL' >> /etc/sudoers

echo "*******************WHAAAAAAAAAAAAAAT 4"
read

runuser -u aur << EOF
tmp_aurutils=$(mktemp -d -p /tmp aurutils.XXXXXXXXXXXXXXXX)

echo "*******************WHAAAAAAAAAAAAAAT 5"
read

git clone https://github.com/AladW/aurutils $tmp_aurutils
#sudo make install -C $tmp_aurutils # Same as below
(cd $tmp_aurutils && make install)

echo "*******************WHAAAAAAAAAAAAAAT 6"
read

sudo cp -a ./src/distros/arch/aur/aurutils.conf /etc/pacman.d/aur
echo "Include = /etc/pacman.d/aur" | sudo tee -a /etc/pacman.conf
# Create the repository root in /var/cache/pacman/aur
sudo install -d /var/cache/pacman/aur -o aur

echo "*******************WHAAAAAAAAAAAAAAT 7"
read

# Create the database in /var/cache/pacman/aur/
#repo-add /var/cache/pacman/aur/aur.db.tar.gz
cd $tmp_aurutils && repo-add /var/cache/pacman/aur/aur.db.tar.gz

echo "*******************WHAAAAAAAAAAAAAAT 8"
read

#aur sync linux${KERNEL} --no-view --no-confirm
EOF

echo "WJHAAAAAAAAAAAAAAAAAAT DOEN"


if [[ -n $aur_dir && -d $aur_dir ]]; then
    fd 'pkg.tar' $aur_dir | xargs sudo install -t $PWD
    repo-add -n $(basename $PWD).db.tar *.pkg.tar*
fi
