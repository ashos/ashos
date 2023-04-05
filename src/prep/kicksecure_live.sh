#!/bin/sh

# If you are installing this on VirtualBox to test it out, set RAM bigger than
# 1024MB as otherwise it errors out (running out of cache/RAM space)

#set echo off

main() {
    if [ -z "$HOME" ]; then HOME=~ ; fi
    RELEASE="bullseye"
    prep_packages="btrfs-progs curl cryptsetup dialog dosfstools efibootmgr git mmdebstrap ntp parted tmux apt-transport-https apt-transport-tor tor"

  # attempt to install and if errors sync time and database

    apt-get -y update
    apt-get -y --fix-broken install --no-install-recommends $prep_packages
    [ $? != 0 ] && sync_time && echo "Please wait for 30 seconds!" && sleep 30 && fixdb && apt-get -y --fix-broken install --no-install-recommends $prep_packages

    configs
    systemctl start tor
    kicksecure_signing
    multimedia_keyring
    #git clone http://github.com/ashos/ashos
    git config --global --add safe.directory $HOME/ashos # prevent fatal error "unsafe repository is owned by someone else"
    #cd ashos
    dialog --stdout --msgbox "CAUTION: If you hit Okay, your HDD will be partitioned. You should confirm you edited script in prep folder!" 0 0
    /bin/bash ./src/prep/parted_gpt_example.sh $2
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

# Kicksecure signing key
kicksecure_signing() {
    if [ ! -f "/usr/share/keyrings/derivative.asc" ]; then
        if [ -x "$(command -v curl)" ]; then
            curl --tlsv1.3 --proto =https --max-time 180 --output "$PWD"/derivative.asc https://www.kicksecure.com/keys/derivative.asc
	    cp "$PWD"/derivative.asc /usr/share/keyrings/derivative.asc
        else
	    echo "F: kicksecure signing key unavailable."
	fi
    fi
}
# Debian multimedia keyring
multimedia_keyring() {
    if [ ! -f "/etc/apt/trusted.gpg.d/deb-multimedia-keyring.gpg" ]; then
	if [ -x "$(command -v curl)" ]; then
	    curl --tlsv1.3 --proto =https --max-time 180 --output "$PWD"/deb-multimedia-keyring.deb https://archive.deb-multimedia.org/pool/main/d/deb-multimedia-keyring/deb-multimedia-keyring_2016.8.1_all.deb
	    apt-get -y install --no-install-recommends "$PWD"/deb-multimedia-keyring.deb
        else
	    echo "F: deb-multimedia signing key unavailable."
	fi
    fi
}

main "$@"; exit

