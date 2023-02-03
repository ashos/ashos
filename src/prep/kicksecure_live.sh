#!/bin/sh

# If you are installing this on VirtualBox to test it out, set RAM bigger than
# 1024MB as otherwise it errors out (running out of cache/RAM space)

#set echo off

main() {
    if [ -z "$HOME" ]; then HOME=~ ; fi
    RELEASE="bullseye"
    prep_packages="btrfs-progs curl cryptsetup mmdebstrap dosfstools efibootmgr git ntp parted tmux apt-transport-https apt-transport-tor tor"

  # attempt to install and if errors sync time and database

    systemctl start tor
    apt-get -y --fix-broken install --no-install-recommends $prep_packages
    [ $? != 0 ] && sync_time && echo "Please wait for 30 seconds!" && sleep 30 && fixdb && apt-get -y --fix-broken install --no-install-recommends $prep_packages

    configs
    sign_kicksecure
    #git clone http://github.com/ashos/ashos
    git config --global --add safe.directory $HOME/ashos # prevent fatal error "unsafe repository is owned by someone else"
    #cd ashos
    #/bin/bash ./src/prep/parted_gpt_example.sh $2
    #python3 setup.py $1 $2 $3
}

# Configurations
configs() {
    setfont Lat38-TerminusBold24x12 # /usr/share/consolefonts/
    echo "export LC_ALL=C LC_CTYPE=C LANGUAGE=C" | tee -a $HOME/.bashrc
    #echo "alias p='curl -F "'"sprunge=<-"'" sprunge.us'" | tee -a $HOME/.bashrc
    echo "alias p='curl -F "'"f:1=<-"'" ix.io'" | tee -a $HOME/.bashrc
    echo "alias d='df -h | grep -v sda'" | tee -a $HOME/.bashrc
    echo "setw -g mode-keys vi" | tee -a $HOME/.tmux.conf
    echo "set -g history-limit 999999" | tee -a $HOME/.tmux.conf
}

# Remove man pages (fixes slow man-db trigger) and update packages db
fixdb() {
    sed -i "s/[^ ]*[^ ]/$RELEASE/3" /etc/apt/sources.list
    apt-get -y autoremove
    apt-get -y autoclean
    apt-get -y clean # Needed?
    apt-get -y remove --purge man-db # Fix slow man-db trigger
    apt-get -y update
    apt-get -y check # Needed?
}

# Sync time
sync_time() {
    if [ -x "$(command -v wget)" ]; then
        date -s "$(wget -qSO- --max-redirect=0 google.com 2>&1 | grep Date: | cut -d' ' -f5-8)Z"
    elif [ -x "$(command -v curl)" ]; then
        date -s "$(curl -I google.com 2>&1 | grep Date: | cut -d' ' -f3-6)Z"
    else
        echo "F: Syncing time failed! Neither wget nor curl available."
    fi
}

# kicksecure signing key
sign_kicksecure() {
    if [ ! -f "/usr/share/keyrings/derivative.asc" ]; then
        if [ -x "$(command -v curl)" ]; then
            curl --tlsv1.3 --proto =https --max-time 180 --output "$PWD"/derivative.asc https://www.kicksecure.com/keys/derivative.asc
	    cp "$PWD"/derivative.asc /usr/share/keyrings/derivative.asc
        else
	    echo "F: kicksecure signing key unavailable."
	fi
    fi
}

main "$@"; exit

