#!/bin/sh

useradd -m -s /bin/bash aur
echo 'aur ALL=(ALL:ALL) NOPASSWD: ALL' >> /etc/sudoers
runuser aur <<'EOF'
tmp_paru=$(mktemp -d -p /tmp paru.XXXXXXXXXXXXXXXX)
curl -o "$tmp_paru/PKGBUILD" -LO "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h=paru-bin"
cd "$tmp_paru" && makepkg --install -f
EOF

