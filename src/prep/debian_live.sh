#!/bin/sh

# If you are installing this on VirtualBox to test it out, set RAM bigger than
# 1024MB as otherwise it errors out (running out of cache/RAM space)

#set echo off

if [ -z "$HOME" ]; then HOME=~ ; fi
RELEASE="sid"
prep_packages="git tmux parted btrfs-progs dosfstools cryptsetup debootstrap ntp efibootmgr"

# Sync time
if [ -x "$(command -v wget)" ]; then
    sudo date -s "$(wget -qSO- --max-redirect=0 google.com 2>&1 | grep Date: | cut -d' ' -f5-8)Z"
elif [ -x "$(command -v curl)" ]; then
    sudo date -s "$(curl -I google.com 2>&1 | grep Date: | cut -d' ' -f3-6)Z"
fi

echo "Please wait for 30 seconds!"
sleep 30 # Wait before updating repo and downloading packages

sudo apt-get -y autoremove
sudo apt-get -y autoclean
sudo apt-get -y remove --purge man-db # Fix slow man-db trigger
sudo apt-get -y update
sudo apt-get -y --fix-broken install $prep_packages
echo "export LC_ALL=C LC_CTYPE=C LANGUAGE=C" | tee -a $HOME/.bashrc
#echo "alias p='curl -F "'"sprunge=<-"'" sprunge.us'" | tee -a $HOME/.bashrc
echo "alias p='curl -F "'"f:1=<-"'" ix.io'" | tee -a $HOME/.bashrc
echo "alias d='df -h | grep -v sda'" | tee -a $HOME/.bashrc
echo "setw -g mode-keys vi" | tee -a $HOME/.tmux.conf
echo "set -g history-limit 999999" | tee -a $HOME/.tmux.conf
#git clone http://github.com/i2/ashos-dev
git config --global --add safe.directory $HOME/ashos-dev # prevent fatal error "unsafe repository is owned by someone else"
#cd ashos-dev
#git checkout debian
#/bin/bash ./src/prep/parted_gpt_example.sh $2
#sudo python3 init.py $1 $2 $3

sudo sed -i "s/[^ ]*[^ ]/$RELEASE/3" /etc/apt/sources.list
sudo apt-get clean && sudo apt-get -y update && sudo apt-get -y check

