#!/bin/sh

if [ -z "$HOME" ]; then HOME=~ ; fi
prep_packages="git make fakeroot"

su

# Prevent error of running out of space in /
mount / -o remount,size=4G /run/archiso/cowspace

# Sync time
#date -s "$(wget -qSO- --max-redirect=0 google.com 2>&1 | grep Date: | cut -d' ' -f5-8)Z"
#date -s "$(curl -I google.com 2>&1 | grep Date: | cut -d' ' -f3-9)Z"

# Ignore signature checking in pacman.conf (bad idea - use fix below)
#sed -e '/^SigLevel/ s|^#*|SigLevel = Never\n#|' -i /etc/pacman.conf
#sed -e '/^LocalFileSigLevel/ s|^#*|#|' -i /etc/pacman.conf

# Fix signature invalid error
rm -rf /etc/pacman.d/gnupg ~/.gnupg
rm -r /var/lib/pacman/db.lck
pacman -Syy
gpg --refresh-keys
killall gpg-agent
pacman-key --init
pacman-key --populate archlinux
pacman -S --noconfirm archlinux-keyring

pacman -S --noconfirm $prep_packages

# Configurations
setfont ter-132n # /usr/share/kbd/consolefonts/ter-132n.psf.gz
echo "export LC_ALL=C LC_CTYPE=C LANGUAGE=C" | tee -a $HOME/.zshrc
#echo "alias p='curl -F "'"sprunge=<-"'" sprunge.us'" | tee -a $HOME/.zshrc
echo "alias p='curl -F "'"f:1=<-"'" ix.io'" | tee -a $HOME/.zshrc
echo "alias d='df -h | grep -v sda'" | tee -a $HOME/.zshrc
echo "setw -g mode-keys vi" | tee -a $HOME/.tmux.conf
echo "set -g history-limit 999999" | tee -a $HOME/.tmux.conf

#git clone http://github.com/ashos/ashos
#git config --global --add safe.directory ./ashos # prevent fatal error "unsafe repository is owned by someone else"
#cd ashos
#/bin/sh ./src/prep/parted_gpt_example.sh $2
#python3 init.py $1 $2 $3

