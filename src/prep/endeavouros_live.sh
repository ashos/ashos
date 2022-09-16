#!/bin/sh

main() {
    if [ -z "$HOME" ]; then HOME=~ ; fi
    prep_packages="tmux"

    # Most probably not needed
    #su

    #sync_time()

    # Ignore signature checking in pacman.conf (quick but bad idea - use fix below instead but takes long)
    #sed -e '/^SigLevel/ s|^#*|SigLevel = Never\n#|' -i /etc/pacman.conf
    #sed -e '/^LocalFileSigLevel/ s|^#*|#|' -i /etc/pacman.conf

    #fixdb()
    pacman -S --noconfirm $prep_packages
    configs
    #git clone http://github.com/ashos/ashos
    #git config --global --add safe.directory ./ashos # prevent fatal error "unsafe repository is owned by someone else"
    #cd ashos
    #/bin/sh ./src/prep/parted_gpt_example.sh $2
    #python3 init.py $1 $2 $3
}

# Configurations
configs() {
    setfont lat9-16 # /usr/share/kbd/consolefonts/ter-132n.psf.gz
    echo "export LC_ALL=C LC_CTYPE=C LANGUAGE=C" | tee -a $HOME/.zshrc
    #echo "alias p='curl -F "'"sprunge=<-"'" sprunge.us'" | tee -a $HOME/.zshrc
    echo "alias p='curl -F "'"f:1=<-"'" ix.io'" | tee -a $HOME/.zshrc
    echo "alias d='df -h | grep -v sda'" | tee -a $HOME/.zshrc
    echo "setw -g mode-keys vi" | tee -a $HOME/.tmux.conf
    echo "set -g history-limit 999999" | tee -a $HOME/.tmux.conf
}

# Fix signature invalid error
fixdb() {
    rm -rf /etc/pacman.d/gnupg ~/.gnupg
    rm -r /var/lib/pacman/db.lck
    pacman -Syy
    gpg --refresh-keys
    killall gpg-agent
    pacman-key --init
    pacman-key --populate archlinux
    pacman-key --populate endeavouros
    pacman -S --noconfirm archlinux-keyring endeavouros-keyring
}

sync_time() {
    if [ -x "$(command -v wget)" ]; then
        date -s "$(wget -qSO- --max-redirect=0 google.com 2>&1 | grep Date: | cut -d' ' -f5-8)Z"
    elif [ -x "$(command -v wget)" ]; then
        date -s "$(curl -I google.com 2>&1 | grep Date: | cut -d' ' -f3-9)Z"
    else
        echo "F: Syncing time failed! Neither wget nor curl available."
    fi
}

main "$@"; exit
