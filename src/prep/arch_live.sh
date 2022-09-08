#!/bin/sh

if [ -z "$HOME" ]; then HOME=~ ; fi

prep_packages="git make fakeroot"

# Sync time
#sudo date -s "$(wget -qSO- --max-redirect=0 google.com 2>&1 | grep Date: | cut -d' ' -f5-8)Z"
#sudo date -s "$(curl -I google.com 2>&1 | grep Date: | cut -d' ' -f3-9)Z"

# Ignore signature checking in pacman.conf (bad idea - use fix below)
#sed -e '/^SigLevel/ s|^#*|SigLevel = Never\n#|' -i /etc/pacman.conf
#sed -e '/^LocalFileSigLevel/ s|^#*|#|' -i /etc/pacman.conf

# Fix signature invalid error
sudo rm -rf /etc/pacman.d/gnupg ~/.gnupg
sudo rm -r /var/lib/pacman/db.lck
sudo pacman -Syy
sudo gpg --refresh-keys
sudo killall gpg-agent
sudo pacman-key --init
sudo pacman-key --populate archlinux
sudo pacman -S --noconfirm archlinux-keyring

sudo pacman -S --noconfirm $prep_packages

# Configurations
setfont ter-132n # /usr/share/kbd/consolefonts/ter-120n.psf.gz
echo "export LC_ALL=C LC_CTYPE=C LANGUAGE=C" | tee -a $HOME/.zshrc
#echo "alias p='curl -F "'"sprunge=<-"'" sprunge.us'" | tee -a $HOME/.zshrc
echo "alias p='curl -F "'"f:1=<-"'" ix.io'" | tee -a $HOME/.zshrc
echo "alias d='df -h | grep -v sda'" | tee -a $HOME/.zshrc
echo "setw -g mode-keys vi" | tee -a $HOME/.tmux.conf
echo "set -g history-limit 999999" | tee -a $HOME/.tmux.conf

#git clone http://github.com/i2/ashos-dev
#git config --global --add safe.directory ./ashos-dev # prevent fatal error "unsafe repository is owned by someone else"
#cd ashos-dev
#git checkout debian
#/bin/sh ./src/prep/parted_gpt_example.sh $2
#sudo python3 init.py $1 $2 $3

