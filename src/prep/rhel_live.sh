#!/bin/sh

main() {
    if [ $(id -u) -ne 0 ]; then echo "Please run as root!"; exit 1; fi
    if [ -z "$HOME" ]; then HOME=~ ; fi
    prep_packages="dialog git stratis-cli stratisd sudo"

  # Prevent error of running out of space in /
#    mount / -o remount,size=4G /run/archiso/cowspace ### NOT NEEDED AT ALL

    subscription-manager register --auto-attach

    subscription-manager register
    subscription-manager attach --auto

  # attempt to install and if errors sync time and database
    dnf install -y $prep_packages
    #[ $? ] && sync_time && fixdb && pacman -S --noconfirm $prep_packages

    configs
    #git clone http://github.com/ashos/ashos
    #git config --global --add safe.directory ./ashos # prevent fatal error "unsafe repository is owned by someone else"
    #cd ashos
    dialog --stdout --msgbox "CAUTION: If you hit Okay, your HDD will be partitioned. You should confirm you edited script in prep folder!" 0 0
    /bin/sh ./src/prep/parted_gpt_example.sh $2
    #python3 setup.py $1 $2 $3
}

# Configurations
configs() {
    setfont ter-132n # /usr/share/kbd/consolefonts/
    echo "export LC_ALL=C LC_CTYPE=C LANGUAGE=C" | tee -a $HOME/.zshrc
    #echo "alias p='curl -F "'"sprunge=<-"'" sprunge.us'" | tee -a $HOME/.zshrc
    echo "alias p='curl -F "'"f:1=<-"'" ix.io'" | tee -a $HOME/.zshrc
    echo "alias d='df -h | grep -v sda'" | tee -a $HOME/.zshrc
    echo "setw -g mode-keys vi" | tee -a $HOME/.tmux.conf
    echo "set -g history-limit 999999" | tee -a $HOME/.tmux.conf
}

# Fix signature invalid error
fixdb() {
  # Ignore signature checking in pacman.conf (insecure) - use fix below (slow)
    #sed -e '/^SigLevel/ s|^#*|SigLevel = Never\n#|' -i /etc/pacman.conf
    #sed -e '/^LocalFileSigLevel/ s|^#*|#|' -i /etc/pacman.conf
    pacman -U /var/cache/pacman/pkg/archlinux-keyring*.pkg.tar.xz
    rm -rf /etc/pacman.d/gnupg ~/.gnupg
    rm -r /var/lib/pacman/db.lck
    systemctl start haveged # otherwise --init step takes long
    pacman-mirrors -f # --geoip
    pacman -Syy --noconfirm gnupg
    gpg --refresh-keys
    killall gpg-agent
    pacman-key --init
    pacman-key --populate archlinux
    pacman-key --refresh-keys
    pacman -Syvv --noconfirm archlinux-keyring
}

# Sync time
sync_time() {
    if [ -x "$(command -v wget)" ]; then
        date -s "$(wget -qSO- --max-redirect=0 google.com 2>&1 | grep Date: | cut -d' ' -f5-8)Z"
    elif [ -x "$(command -v wget)" ]; then
        date -s "$(curl -I google.com 2>&1 | grep Date: | cut -d' ' -f3-6)Z" ############## REVIEW
    else
        echo "F: Syncing time failed! Neither wget nor curl available."
    fi
}

main "$@"; exit

