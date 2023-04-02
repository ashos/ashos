<p align="center">
  <img src="res/logos/logo.png" alt="AshOS">
  <br>
  [<a href="res/docs/README_CZ.md">Čeština</a>] | [<a href="res/docs/README_zh-CN.md">中文</a>] | [<a href="res/docs/README_FA.md">پارسی</a>]
  <br>
  <b>We need your help to translate this README. <a href="https://github.com/i2/ashos-dev/tree/main/res/docs">Look here!</a></b>
</p>

# AshOS (Any Snapshot Hierarchical OS)
### An immutable tree-shaped meta-distribution using snapshots
###### (for mere mortals!)

---

**Please note that, for the purpose of this project, conforming to 'pythonic' way was not a goal as in future, implementation might change to Rust, C, C++, etc. We would like to be as close to POSIX-compliant sans-bashism shell as possible.**

---

## Table of contents
* [What is AshOS?](https://github.com/ashos/ashos#what-is-ashos)
* [AshOS compared to other similar distributions](https://github.com/ashos/ashos#ashos-compared-to-other-similar-distributions)
* [ash and AshOS documentation](https://github.com/ashos/ashos#ashos-documentation)
  * [Installation](https://github.com/ashos/ashos#installation)
  * [Post installation](https://github.com/ashos/ashos#post-installation-setup)
  * [Snapshot management and deployments](https://github.com/ashos/ashos#snapshot-management)
  * [Package management](https://github.com/ashos/ashos#package-management)
  * [Snapshot configuration](https://github.com/ashos/ashos#snapshot-configuration)
* [Additional documentation](https://github.com/ashos/ashos#additional-documentation)
  * [Updating the pacman keys](https://github.com/ashos/ashos#fixing-pacman-corrupt-packages--key-issues)
  * [Saving configuration changes made in /etc persistently](https://github.com/ashos/ashos#saving-configuration-changes-made-in-etc-persistently)
  * [Configuring dual boot](https://github.com/ashos/ashos#dual-boot)
  * [Updating ash itself](https://github.com/ashos/ashos#updating-ash-itself)
  * [Miscellaneous](https://github.com/ashos/ashos#miscellaneous)
* [Advanced features](https://github.com/ashos/ashos#advanced-features)
  * [Multi-boot](https://github.com/ashos/ashos#multi-boot)
  * [LUKS](https://github.com/ashos/ashos#luks)
  * [Mutability toggle](https://github.com/ashos/ashos#mutability-toggle)
  * [Debugging ash](https://github.com/ashos/ashos#debugging-ash)
  * [Debuggin installer](https://github.com/ashos/ashos#debugging-installer)
* [Known bugs](https://github.com/ashos/ashos#known-bugs)
* [Contributing](https://github.com/ashos/ashos#contributing)
* [Community](https://github.com/ashos/ashos#community)
* [ToDos](https://github.com/ashos/ashos#todos)
* [Distro-Specific Notes](https://github.com/ashos/ashos#distro-notes)
  * [Arch](https://github.com/ashos/ashos#arch-linux)
    * [AUR](https://github.com/ashos/ashos#aur)
    * [Fixing pacman corrupt packages or key issues](https://github.com/ashos/ashos#fixing-pacman-corrupt-packages-or-key-issues)
  * [CachyOS](https://github.com/ashos/ashos#cachyos)
  * [Debian](https://github.com/ashos/ashos#debian)
  * [EndeavourOS](https://github.com/ashos/ashos#endeavouros)
  * [Ubuntu](https://github.com/ashos/ashos#ubuntu)
  * [Proxmox](https://github.com/ashos/ashos#proxmox)

---

## What is AshOS?

You always wanted to try Fedora Rawhide but after a few days, its fragility got on your nerves. Then, maybe you tried Fedora Silverblue Rawhide but then its complicated and slow git-like ostree operations killed your mood! Well, no more! Now you can try this bleeding edge distro (and many more distros like Debian sid) with more peace of mind. AshOS provides ultimate flexibility for installing operating systems, for instance you can easily setup an immutable Proxmox VE!

AshOS is a unique meta-distribution that:
* aims to bring immutability even to distros that do not have this very useful feature i.e. Arch Linux, Gentoo, etc.
* wraps around *any* Linux distribution that can be bootstrapped (pretty much any major distribution)
* targets to become a universal installer for different distros and different Desktop Environments/Window Managers
* can install, deploy and multi-boot any number of distros

Initially inspired by Arch Linux, AshOS uses an immutable (read-only) root filesystem to set itself apart from any other distro out there.
Software is installed and configured into individual snapshot trees, which can then be deployed and booted into.
It does not invent yet another package format or package manager, but instead relies on the native package manager for instance [pacman](https://wiki.archlinux.org/title/pacman) from Arch.

Ashes are one of the oldest trees in the world and they inspired naming AshOS.

In AshOS, there are several keywords:
* Vanilla: we try to be as close to the "vanilla" version of target distribution that is being installed.
* Minimalism: we adhere to a lego build system. Start small and build as complex a system as you would like. The main focus of development is on having a solid minimal installed snapshot, based on which user can have infinite immutable permutations!
* Generality: We strive to cater for the most common denominator between distros and architectures (x64, aarch64, sparc, etc). As such, when there is a choice between convenience and comprehensiveness/generality, we go with the latter. To clarify with an example, it might be easier to use grub-btrfs instead of implementing our own GRUB update mechanism, but because that particular package might not be readily available in all distros, we develop an AshOS specific solution. This way, we can potentially cater to any distro in future!

**This has several advantages:**

* Security
  * Even if running an application with eleveted permissions, it cannot replace system libraries with malicious versions
* Stability and reliability
  * Due to the system being mounted as read only, it's not possible to accidentally overwrite system files
  * If the system runs into issues, you can easily rollback the last working snapshot within minutes
  * Atomic updates - Updating your system all at once is more reliable
  * Thanks to the snapshot feature, AshOS can ship cutting edge software without becoming unstable
  * AshOS needs little maintenance, as it has a built in fully automatic update tool that creates snapshots before updates and automatically checks if the system upgraded properly before deploying the new snapshot
* Configurability
  * With the snapshots organised into a tree, you can easily have multiple different configurations of your software available, with varying packages, without any interference
  * For example: you can have a single Gnome desktop installed and then have 2 snapshots on top - one with your video games, with the newest kernel and drivers, and the other for work, with the LTS kernel and more stable software, you can then easily switch between these depending on what you're trying to do
  * You can also easily try out software without having to worry about breaking your system or polluting it with unnecessary files, for example you can try out a new desktop environment in a snapshot and then delete the snapshot after, without modifying your main system at all
  * This can also be used for multi-user systems, where each user has a completely separate system with different software, and yet they can share certain packages such as kernels and drivers
  * AshOS allows you to install software by chrooting into snapshots, therefore (for example in Arch flavor) you can use software such as the AUR to install additional packages
  * AshOS is, very customizable, you can choose exactly which software you want to use (just like Arch Linux)

* Thanks to its reliabilty and automatic upgrades, AshOS is well suitable for single use or embedded devices
* It also makes for a good workstation or general use distribution utilizing development containers and flatpak for desktop applications

**IMPORTANT NOTE:** First try AshOS in a virtual machine and get comfortable with it before installing it on bare metal. AshOS installer is ***ruthless** and it assumes user knows what they are doing. Running installer as is wipes the disk!

As AshOS strives to be minimal solid and follow a LEGO like structure (start small, customize as you go), we primarily focus development on the base, meaning by default no Desktop Environment (not even Window Manager) is installed. This is by design as otherwise team has to support many DEs on many distros. What is provided is `profiles`. As DEs/WMs are just packages, with power of snapshotting, one can use ash to install the desired DE/WM.
For instance to install GNOME in snapshot 1:
```
`ash clone 0` : create snapshot #1
`ash install 1 -P gnome` : install gnome in snapshot #1
`ash deploy 1`
`reboot`
```

---
## AshOS compared to other similar distributions
* **NixOS / GNU Guix** - compared to nixOS, AshOS is a more traditional system with how it's setup and maintained. While nixOS is entirely configured using the Nix programming language, AshOS uses the native package manager of target distribution, for instance pacman for Arch, apt-get for Debian, etc. AshOS consumes less storage, and configuring your system is faster and easier (less reproducible however), it also gives you more customization options. AshOS is FHS compliant, ensuring proper software compatability. **Unlike NixOS, you don't need to have a Ph.D. in computer science to just run an immutable os**
  * AshOS allows declarative configuration using Ansible, for somewhat similar functionality to NixOS
* **Fedora Silverblue/Kinoite** - AshOS is more customizable, but does require more manual setup. AshOS supports dual boot, unlike Silverblue.
* **OpenSUSE MicroOS** - AshOS is a more customizable system, but once again requires a bit more manual setup. MicroOS works similarly in the way it utilizes btrfs snapshots. AshOS has an official KDE install, but also supports other desktop environments, while MicroOS only properly supports Gnome. AshOS supports dual boot, as well as live-patching the system and installing packages without reboot

---
## Installation
* (Note: All scripts during install should be run with super user privileges)
* AshOS is installed from the official live iso for target distribution. For example [Arch Linux](https://archlinux.org/download/), [Debian](https://www.debian.org/CD/http-ftp/)/[Debian netinstaller](https://www.debian.org/distrib/netinst) etc.
* Arch iso can be generally used to bootstrap other distros except: Use Debian live iso-hybrid to bootstrap Debian, Ubuntu iso to bootstrap Ubuntu
* Depending on the live iso, it is **very important** that scripts in `./src/prep/` be executed (preparing live environment as well as partition/formatting) otherwise there would be error because time is not in sync etc. By default the installer will call these scripts, but if you want to do them manually, just comment the respective lines
* The commands to fix package db issues in live iso (i.e. arch_live.iso) take a long time to run. One can comment these to have installer run significantly faster. They are included mostly for virtual machine installation where time syncing issues are abundant.
* If you run into issues installing packages during installation, make sure you're using the newest iso, and update the package manager's keyring if needed
* If running from an old arch iso, run the commands in section `# Fix signature invalid error` in `./src/prep/arch-live.sh`
* You need an internet connection to install AshOS
* AshOS used to ship with 3 installation profiles, one for minimal installs and two for desktop (Gnome, KDE Plasma). To make it more modular, we redesigned it and by default it only installs a bare minimum base snapshot. Once that is done, you can install any desktop environment you would like. For instance for GNOME, once booted in base snapshot, run:
```
ash branch 0 # This produces node #N
ash install N --profile gnome
ash deploy N
```
* Support for more DE's will be added but it will not be part of the base install.
* The installation script is easily configurable and adjusted for your needs (but it works just fine without any modifications)

Install git first - this will allow us to download the install script

```
pacman -Sy git
```
Clone repository

```
git clone "https://github.com/ashos/ashos"
cd ashos
```
Partition and format drive

* If installing on a BIOS system, use a dos (MBR) partition table
* On EFI you can use GPT
* The EFI partition has to be formatted to FAT32 before running the installer (```mkfs.fat -F32 /dev/<part>```)
* There are prep scripts under `./src/prep/`

```
lsblk  # Find your drive name
cfdisk /dev/*** # Format drive, make sure to add an EFI partition, if using BIOS leave 2M free space before first partition
mkfs.btrfs /dev/*** # Create a btrfs filesystem, don't skip this step!
```
Run installer

```
python3 setup.py /dev/<root_partition> /dev/<drive> [/dev/<efi part>] [distro_id] ["distro_name"]# Skip the EFI partition if installing in BIOS mode

Here are 3 example scenarios:

example 1 (BIOS): python3 setup.py /dev/vda1 /dev/vda
This is a simple case when iso from the `same distro` is used

example 2 (UEFI): python3 setup.py /dev/nvm0p2 /dev/nvm0 /dev/nvm0p1 fedora "Fedora Linux"
When installing a distro using another distro's iso, the last two arguments are necessary

example 3 (UEFI): python3 setup.py /dev/sda2 /dev/sda /dev/sda1 cachyos "CachyOS Linux"
If for any reason, there is a mismatch between what distro actually is and its /etc/os-release file, it is [usually] mandatory to pass two additional arguments. Here even though we are using CachyOS iso file (which is based on Arch Linux), by investigating in /etc/os-release file, you would see ID and NAME are same as Arch Linux. In a single boot install, it is okay to not pass the last two arguments, but if you want a multiboot system (say dual boot with Arch Linux), they are required.

```
The arguments inside square brackets are optional. Regarding the fourth argument: say if you want to install Alpine Linux using Arch Linux iso, run `python3 setup.py /dev/vda2 /dev/vda /dev/vda1 alpine`.

## Post installation setup
* Post installation setup is not necessary if you install one of the desktop editions (Gnome or KDE)
* A lot of information for how to handle post-install setup is available on the [ArchWiki page](https://wiki.archlinux.org/title/general_recommendations)
* Here is a small example setup procedure:
  * Start by creating a new snapshot from `base` using ```ash clone 0```
  * Chroot inside this new snapshot (```ash chroot <snapshot>```) and begin setup
    * Start by adding a new user account: ```useradd username```
    * Set the user password ```passwd username```
    * Now set a new password for root user ```passwd root```
    * Now you can install additional packages (desktop environments, container technologies, flatpak) using pacman
    * Once done, exit the chroot with ```exit```
    * Then you can deploy it with ```ash deploy <snapshot>```

## Additional documentation
* For further information that is not covered in this project, it is advised to refer to the the target distro i.e. [Arch wiki](https://wiki.archlinux.org/)
* Report issues/bugs on the [Github issues page](https://github.com/ashos/ashos/issues)
* **HINT: you can use `ash --help` to get a quick cheatsheet of all available commands**
* **Ideally, we would like to keep Ash as a single file executable**
* ash script is divided into 2 files: common code (ashpk_core.py) and distro specific code (i.e gentoo.py). Note that neither of these files can be run standalone (import one script into the other is not intended). The division is just to ease using files as templates in developing Ash for other distributions. At the time of installing a distro, the two files are simply concatenated.
* To not need additional fonts, we use ASCII style when printing ash tree. For a nicer output, feel free to replace AsciiStyle() with ContStyle(), ContRoundStyle(), or DoubleStyle()
* The other nice thing about AshOS is your related-distros are very similarly built. For instance, installer for Arch and Manjaro are very similar, difference just being the package repos!

#### Base snapshot
* The snapshot ```0``` is reserved for the base system snapshot, it cannot be changed and can only be updated using ```ash base-update```

## Snapshot Management

#### Show filesystem tree

```
ash tree
```

* The output can look for example like this:

```
root - root
├── 0 - base snapshot
└── 1 - multi-user system
    └── 4 - applications
        ├── 6 - MATE full desktop
        └── 2*- Plasma full desktop
```
* The asterisk shows which snapshot is currently selected as default

* You can also get only the number of the currently booted snapshot with

```
ash current
```
#### Add descritption to snapshot
* Snapshots allow you to add a description to them for easier identification

```
ash desc <snapshot> <description>
```
#### Delete a tree
* This removes the tree and all it's branches

```
ash del <tree>
```
#### Custom boot configuration
* If you need to use a custom grub configuration, chroot into a snapshot and edit ```/etc/default/grub```, then deploy the snapshot and reboot

#### chroot into snapshot
* Once inside the chroot the OS behaves like regular Arch, so you can install and remove packages using pacman or similar
* Do not run ash from inside a chroot, it could cause damage to the system, there is a failsafe in place, which can be bypassed with ```--chroot``` if you really need to (not recommended)
* The chroot has to be exited properly with ```exit```, otherwise the changes made will not be saved
* If you don't exit chroot the "clean" way with ```exit```, it's recommended to run ```ash tmp``` to clear temporary files left behind


```
ash chroot <snapshot>
```

* You can enter an unlocked shell inside the current booted snapshot with

```
ash live-chroot
```

* The changes made to live session are not saved on new deployments

#### Other chroot options

* Runs a specified command inside snapshot

```
ash run <snapshot> <command>
```

* Runs a specified command inside snapshot and all it's branches

```
ash tree-run <tree> <command>
```

#### Clone snapshot
* This clones the snapshot as a new tree

```
ash clone <snapshot>
```

#### Clone a tree recursively  
* This clones an entire tree recursively

```
ash clone-tree <snapshot>
```

#### Create new tree branch

* Adds a new branch to specified snapshot

```
ash branch <snapshot-to-branch-from>
```
#### Clone snapshot under same parent

```
ash cbranch <snapshot>
```
#### Clone snapshot under specified parent

* Make sure to sync the tree afterwards

```
ash ubranch <parent> <snapshot>
```
#### Create new base tree

```
ash new
```
#### Deploy snapshot

* Reboot to  boot into the new snapshot after deploying

```
ash deploy <snapshot>
```

#### Update base which new snapshots are built from

```
ash base-update
```
* Note: the base itself is located at ```/.snapshots/rootfs/snapshot-0``` with it's specific ```/var``` files and ```/etc``` being located at ```/.snapshots/var/var-0``` and ```/.snapshots/etc/etc-0``` respectively, therefore if you really need to make a configuration change, you can mount snapshot these as read-write and then snapshot back as read only

## Package management

#### Software installation
* Run ```ash deploy <snapshot>``` and reboot after installing new software for changes to apply (unless using live install, more info below) This is no longer needed by default.
* Software can also be installed using pacman in a chroot
* AUR can be used under the chroot
* Flatpak can be used for persistent package installation
* Using containers for additional software installation is also an option. An easy way of doing this is with [distrobox](https://github.com/89luca89/distrobox)

```
ash install <snapshot> <package>
```

* After installing you can sync the newly installed packages to all the branches of the tree with
* Syncing the tree also automatically updates all the snapshots

```
ash sync <tree>
```

* If you wish to sync without updating (could cause package duplication in database) then use

```
ash sync <tree> -f
```

* ash also supports installing packages without rebooting. This is no longer needed. ### REVIEW_LATER
```
ash install <snapshot> -p <package> --live
```

#### Removing software

* For a single snapshot

```
ash remove <snapshot> <package or packages>
```

* Recursively

```
ash tree-rmpkg <tree> <pacakge or packages>
```



#### Updating
* It is advised to clone a snapshot before updating it, so you can roll back in case of failure
* This update only updates the system packages, in order to update ash itself see [this section](https://github.com/ashos/ashos#updating-ash-itself)


* To update a single snapshot

```
ash upgrade <snapshot>
```
* To recursively update an entire tree

```
ash tree-upgrade <tree>
```

* This can be configured in a script (ie. a crontab script) for easy and safe automatic updates

* If the system becomes unbootable after an update, you can boot last working deployment (select in grub menu) and then perform a rollback

```
ash rollback
```

* Then you can reboot back to a working system

## Snapshot configuration
* AshOS has a per-snapshot configuration system
* Using this system we can toggle some functionalities. For instance for AUR (Arch User Repository), look under Arch notes below.
* You need to have a text editor. For nano or vi, skip step0 altogether:

step0:
```
set $EDITOR environment variable (i.e. add 'EDITOR=nvim' line in ~/.bashrc)
reload it (i.e. source ~/.bashrc)
Add 'Defaults env_keep = "EDITOR PROMPT"' line to /etc/sudoers
```

```
ash edit <snapshot>
```

* Save changes and quit

Note that you can skip the line for /etc/sudoers, but each time you should instead type 'sudo -E ash edit <snapshot>' to pass environment variable with sudo!

## Extras

#### Saving configuration changes made in ``/etc`` persistently
* Normally configuration should be done with ``ash chroot``, but sometimes you may want to apply changes you've made to the booted system persistently
* To do this use the following command

```
ash etc-update
```

* This allows you to configure your system by modifying ``/etc`` as usual, and then saving these changes.

#### Dual boot
* AshOS supports dual boot using the GRUB bootloader
* When installing the system, use the existing EFI partition
* to configure dual boot, we must begin by installing the ```os-prober``` package:

```
ash install <snapshot> os-prober
```

* Now we have to configure grub

```
ash chroot <snapshot>
echo 'GRUB_DISABLE_OS_PROBER=false' >> /etc/default/grub
exit
```

* Now just deploy the snapshot to reconfigure the bootloader

```
ash deploy <snapshot>
```

If Windows is detected, ash should return output along the lines of `Found Windows Boot Manager on...`. You may need to install `ntfs-3g` first and re-deploy if you don't see a Windows entry.  ###REVIEW_LATER

#### Updating ash itself
* ash doesn't get updated alongside the system when `ash upgrade` is used
* sometimes it may be necessary to update ash itself
* ash can be updated with a single command

```
ash upself
```

#### Miscellaneous

Read-write access to various parts of filesystem:
/.snapshots/rootfs/snapshot-*   : ro
/.snapshots/etc/etc-*           : ro
/var                            : rw
/                               : mounted as ro, but the snapshot itself is rw
/usr                            : ro
/etc                            : rw

For Gnome and KDE profiles, we are assuming user just want things to work as default and as such we install default login manager. For any other profile, we focus on minimalism, and just install tbsm. One can obviously easily modify this if they choose to.

When using an older iso file, make sure to uncomment fixdb line in the prep script for the corresponding distro. Otherwise there are circumstances that, even if installation appear to be successful, you may get sync package manager db errors when booted in your distro. This fixdb can make the initial part of installation process very slow, but might be worth it.

## Advanced features

These are some advanced feature and we suggest you use them only if you are ready for breakage, doing data backups and occasional fixes. They may not be prime-time ready.

#### Multi-boot

To multi-boot different distros, generally follow this procedure:
* Install first distro-A with option number 2 when prompted at the beginnning of installer
* Install consequent distros with option number 3 (Important: other options will wipe either the root or both root and efi partitions)

#### LUKS

Full-disk encryption using LUKS2 is implemented. This means also encrypting /boot which is an experimental feature of GRUB since v2.06. Right now in mainstream, it only supports pbkdf2 and not the default argon2. This will significantly slow down booting as for example cryptomount decryption is about 30 seconds on 8kb keyfile. If you plan to multi-boot with other OS, do not use this feature *yet*!
We monnitor development of GRUB closely and will update as soon as possible.

#### Mutability toggle

The beauty of customizability of AshOS is that we can have a mix of immutable and non-immutable nodes!
Within the forest/tree of AshOS, one can make any snapshot (other than base `0`) mutable. For instance, to make node 9 mutable run `sudo ash immen 9`. This makes a node and any children (that are created afterwards) mutable.

#### Debugging ash

* sometimes it may be necessary to debug ash
* the following command is useful as it shows outputs of commands when running ashpk.py:

```
sed -e 's| >/dev/null 2>&1||g' /usr/bin/ash > ashpk.py
```

#### Debugging installer

* While working on any OS development project, a lot of issues might come up. It's just the nature of debugging in this project and needs patience!
* When facing issues in installer or after initial setup is done, go to GRUB and delete `quiet` from arguments
* Boot using Ctrl+X and report what you observe
* If possible, go to TTY2 by Ctrl+Alt+F2 and run `journalctl -b` and report back

## Known bugs

* At the end of installer if LUKS is used, there would be warning `remove ioctl device or resource busy`. They can be ignore. Most likely cause: systemd-journald
* Swap partition doesn't work, it's recommended to use a swapfile or zram instead
* Docker has issues with permissions, to fix run
```
sudo chmod 666 /var/run/docker.sock
```

* If you run into any issues, report them on [the issues page](https://github.com/ashos/ashos/issues)

# Contributing
* Star this repo!
* Please take a look under `./src/profiles/` and add a desktop environment or windows manager if missing. Please try to be as minimal and vanilla as possible. If a package has different names in different distros (like networkmanager in Arch and network-manager in Debian, create a file with the distro suffix for the profile i.e. under gnome: packages-arch.txt vs. packages-debian.txt
* If AshOS does not already support your distro, you can do it with 'relative' ease! A good way would be to use Arch as a template (./src/distros/arch/) For installer.py, usually, only SOME of the numbered sections (indicated as `#   1.` to `#   6.`) need to be adapted to new OS. For ashpk.py, convert the commands to the new package manager.
* When adding new functions to ashpk_core.py or ashpk_distro.py, add them in alphabetical order (except main() which is the last function in ashpk_core.py for easier access)
* We would like to keep ashpk_distro.py as small as possible, so it is easier to translate it to other distros. Keep this in mind if adding new features/function... as much as possible, make features distro-agnostic. (i.e. add functions in the shared ashpk.py when possible instead)
* If you are adding a new profile (windows manager, desktop environment), absolutely include the most minimally required packages. Take a look at gnome, jwm, or i3 for example. To come up with minimal viable packages, if you already have an AshOS installation of target distro (i.e. Debian, Arch, Alpine, etc.), create a test snapshot and try to install as few packages and see if it works. Alternatively, you can use a clean vanilla environment/chroot of destination distro or in a virtual machine or docker image. Some time systemd service commands might be required. Make sure to include them in the profile conf file as well.
* Code and documentation contributions are welcome!
* Bug reports are a good way of contributing to the project too
* Before submitting a pull request test your code and make sure to comment it properly
* If a part of code needs further todo, indicate it with `### YOUR_COMMENT`
* When adding contributing code to the project, always follow fork-and-clone approach: Fork the main organizational repo (ashos/ashos) under your personal git, make the changes, push your changes to your git, and finally create a pull request back to main repo.

# Community
* Please feel free to join us on [Matrix](https://matrix.to/#/#ashos:matrix.org) or [Discord](https://discord.gg/YVHEC6XNZw) for further discussion and support!
* Happy worry-free snapshotting!

# ToDos
* A clean way to completely unistall ash
* Implement AUR package maintenance between snapshots
* Make AshOS more accessible to non-advanced users

# Distro-Specific Notes
* For packages-distro.conf, the leanest display manager (either Xorg or Wayland) that is included in the official repo for the given distro would be included. For instance for Arch Linux, this would be 'slim', even though there are slimmer display managers like 'ly', 'tbsm', 'cdm' etc. but unfortunately there are in AUR at the time of writing this document.

## Arch Linux
### AUR
* AshOS supports the AUR natively
* AshOS uses the [paru AUR helper](https://github.com/morganamilo/paru) to provide this functionality
* If you already have paru installed, please make sure it is removed from the snapshot first: ``ash remove <snapshot> paru``, then proceed to the other steps
* To enable AUR support, first open the snapshot configuration:

```
EDITOR=nano ash edit-conf <snapshot> # set the EDITOR variable
```

* Now we can enable AUR by editing the file like so:

```
aur::True
```

* Save changes and quit
* Now AUR Support is enabled, you can use ``ash install`` and ``ash upgrade`` as usual with AUR packages

### Fixing pacman corrupt packages or key issues
* Arch's pacman package manager sometimes requires a refresh of the PGP keys
* To fix this issue we can simply reinstall they arch keyring

```
ash install <snapshots> archlinux-keyring
```

If this didn't solve the issue, run:

```
ash refresh <snapshots>
```

and as a last resort, run: (CAUTION: This might have undesired effects)

```
ash fixdb <snapshots>
```

## CachyOS
* You can use either Arch iso or CachyOS iso for installation. Pass extra arguments if using former (look at examples above)
* [CLI installer](https://sourceforge.net/projects/cachyos-arch/files/cli-installer/cli/) suffices, no need to download GUI installer. 

## Debian
* When issuing `sudo python3 setup.py /dev/sdXY /dev/sdX /dev/sdXZ`, it might seem installer has frozen but it is actually doing its thing! Please be patient and you will get a prompt to initiate install in about 30 seconds! For some reason, it was not showing what is going on in a nice way, so I put a `set echo off` command.
* Make sure not to miss sudo in the command above, otherwise there would be permission error when writing to /mnt/.snapshots/...

## EndeavourOS
* Start the boot process from iso file
* When GRUB is prompted, press keys 'e' and then END to add 'single' to the end of kernel parameters. When prompted about rescue mode, press Enter.
* This will drop you to single user mode instead of default Desktop Environment
* if internet access is not be available, run `sudo dhclient`
* Otherwise if installing from within desktop environment, comment out "su" in endevouros_live.sh as it will ask for a password. Instead run the script as sudo.

## Ubuntu
* Download links: [Link 1](https://releases.ubuntu.com), [Link 2](https://launchpad.net/ubuntu/+cdmirrors)
* In GRUB, you can pass `single` as an argument to linux kernel to boot in single mode and press Enter for maintenance (As no GUI is required for this installer, it is much faster to boot)
* As of July 2022, Debian iso can't be used to bootstrap 'jammy' (zstd bug: https://bugs.debian.org/892664)
---

## Proxmox
* Use Debian live iso-hybrid
* Find latest version of pve-kernel_*-pve from http://download.proxmox.com/debian/dists/{RELEASE}/pve-no-subscription/binary-amd64/ and update KERNEL_VERSION.
* `sudo python3 setup.py /dev/sdXY /dev/sdX /dev/sdXZ proxmox "Proxmox VE"`
* Note for the package `postfix`: Configure packages which require user input on installation according to your needs. If you have a mail server in your network, you should configure postfix as a satellite system. Your existing mail server will then be the relay host which will route the emails sent by Proxmox VE to their final recipient. If you don't know what to enter here, choose local only and leave the system name as is.

**This project is licensed under the AGPLv3.**

