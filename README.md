# astOS (Arch Snapshot Tree OS)
### An immutable Arch based distribution utilizing btrfs snapshots  

![astos-logo](logo.jpg)

---

## Table of contents
* [What is astOS?](https://github.com/CuBeRJAN/astOS#what-is-astos)
* [astOS compared to other similar distributions](https://github.com/CuBeRJAN/astOS#astos-compared-to-other-similar-distributions)
* [ast and astOS documentation](https://github.com/CuBeRJAN/astOS#additional-documentation)
  * [Installation](https://github.com/CuBeRJAN/astOS#installation)
  * [Post installation](https://github.com/CuBeRJAN/astOS#post-installation-setup)
  * [Snapshot management and deployments](https://github.com/CuBeRJAN/astOS#snapshot-management)
  * [Package management](https://github.com/CuBeRJAN/astOS#package-management)
* [Additional documentation](https://github.com/CuBeRJAN/astOS#additional-documentation)
  * [Updating the pacman keys](https://github.com/CuBeRJAN/astOS#fixing-pacman-corrupt-packages--key-issues)
  * [Saving configuration changes in /etc persistently](https://github.com/CuBeRJAN/astOS#saving-configuration-changes-made-in-etc)
  * [Configuring dual boot](https://github.com/CuBeRJAN/astOS#dual-boot)
  * [Updating ast itself](https://github.com/CuBeRJAN/astOS#updating-ast-itself)
  * [Debugging ast](https://github.com/CuBeRJAN/astOS#debugging-ast)
* [Known bugs](https://github.com/CuBeRJAN/astOS#known-bugs)
* [Contributing](https://github.com/CuBeRJAN/astOS#contributing)
* [Community](https://github.com/CuBeRJAN/astOS#community)

---

## What is astOS?  

astOS is a modern distribution based on [Arch Linux](https://archlinux.org).  
Unlike Arch it uses an immutable (read-only) root filesystem.  
Software is installed and configured into individual snapshot trees, which can then be deployed and booted into.  
It doesn't use it's own package format or package manager, instead relying on [pacman](https://wiki.archlinux.org/title/pacman) from Arch.


**This has several advantages:**

* Security
  * Even if running an application with eleveted permissions, it cannot replace system libraries with malicious versions
* Stability and reliability
  * Due to the system being mounted as read only, it's not possible to accidentally overwrite system files
  * If the system runs into issues, you can easily rollback the last working snapshot within minutes
  * Atomic updates - Updating your system all at once is more reliable
  * Thanks to the snapshot feature, astOS can ship cutting edge software without becoming unstable
  * astOS needs little maintenance, as it has a built in fully automatic update tool that creates snapshots before updates and automatically checks if the system upgraded properly before deploying the new snapshot
* Configurability
  * With the snapshots organised into a tree, you can easily have multiple different configurations of your software available, with varying packages, without any interference
  * For example: you can have a single Gnome desktop installed and then have 2 snapshots on top - one with your video games, with the newest kernel and drivers, and the other for work, with the LTS kernel and more stable software, you can then easily switch between these depending on what you're trying to do
  * You can also easily try out software without having to worry about breaking your system or polluting it with unnecessary files, for example you can try out a new desktop environment in a snapshot and then delete the snapshot after, without modifying your main system at all
  * This can also be used for multi-user systems, where each user has a completely separate system with different software, and yet they can share certain packages such as kernels and drivers
  * astOS allows you to install software by chrooting into snapshots, therefore you can use software such as the AUR to install additional packages
  * astOS is, just like Arch, very customizable, you can choose exactly which software you want to use

* Thanks to it's reliabilty and automatic upgrades, astOS is well suitable for single use or embedded devices
* It also makes for a good workstation or general use distribution utilizing development containers and flatpak for desktop applications 

---
## astOS compared to other similar distributions
* **NixOS** - compared to nixOS, astOS is a more traditional system with how it's setup and maintained. While nixOS is entirely configured using the Nix programming language, astOS uses Arch's pacman package manager. astOS consumes less storage, and configuring your system is faster and easier (less reproducible however), it also gives you more customization options. astOS is FHS compliant, ensuring proper software compatability.
  * astOS allows declarative configuration using Ansible, for somewhat similar functionality to NixOS
* **Fedora Silverblue/Kinoite** - astOS is more customizable, but does require more manual setup. astOS supports dual boot, unlike Silverblue.
* **OpenSUSE MicroOS** - astOS is a more customizable system, but once again requires a bit more manual setup. MicroOS works similarly in the way it utilizes btrfs snapshots. astOS has an official KDE install, but also supports other desktop environments, while MicroOS only properly supports Gnome. astOS supports dual boot, as well as live-patching the system and installing packages without reboot.

---
## Installation
* astOS is installed from the official Arch Linux live iso available on [https://archlinux.org/](https://archlinux.org)
* If you run into issues installing packages during installation, make sure you're using the newest arch iso, and if needed update the pacman keyring
* You need an internet connection to install astOS
* Currently astOS ships 4 installation profiles, one for minimal installs and two for desktop, one with the Gnome desktop environment, one with KDE Plasma, and one with MATE, but support for more DE's will be added
* The installation script is easily configurable and adjusted for your needs (but it works just fine without any modifications)

Install git first - this will allow us to download the install script

```
pacman -Sy git
```
Clone repository

```
git clone "https://github.com/CuBeRJAN/astOS"  
cd astOS  
```
Partition and format drive

* If installing on a BIOS system, use a dos (MBR) partition table
* On EFI you can use GPT
* The EFI partition has to be formatted to FAT32 before running the installer (```mkfs.fat -F32 /dev/<part>```)

```
lsblk  # Find your drive name
cfdisk /dev/*** # Format drive, make sure to add an EFI partition, if using BIOS leave 2M free space before first partition  
mkfs.btrfs /dev/*** # Create a btrfs filesystem, don't skip this step!
```
Run installer

```
python3 main.py /dev/<partition> /dev/<drive> /dev/<efi part> # Skip the EFI partition if installing in BIOS mode
```

## Post installation setup
* Post installation setup is not necessary if you install one of the desktop editions (Gnome or KDE)
* A lot of information for how to handle post-install setup is available on the [ArchWiki page](https://wiki.archlinux.org/title/general_recommendations) 
* Here is a small example setup procedure:
  * Start by creating a new snapshot from `base` using ```ast clone 0```
  * Chroot inside this new snapshot (```ast chroot <snapshot>```) and begin setup
    * Start by adding a new user account: ```useradd username```
    * Set the user password ```passwd username```
    * Now set a new password for root user ```passwd root```
    * Now you can install additional packages (desktop environments, container technologies, flatpak) using pacman
    * Once done, exit the chroot with ```exit 0```
    * Then you can deploy it with ```ast deploy <snapshot>```

## Additional documentation
* It is advised to refer to the [Arch wiki](https://wiki.archlinux.org/) for documentation not part of this project
* Report issues/bugs on the [Github issues page](https://github.com/CuBeRJAN/astOS/issues)
* **HINT: you can use `ast help` to get a quick cheatsheet of all available commands**

#### Base snapshot
* The snapshot ```0``` is reserved for the base system snapshot, it cannot be changed and can only be updated using ```ast base-update```

## Snapshot Management

#### Show filesystem tree

```
ast tree
```

* The output can look for example like this:

```
root - root
├── 0 - base snapshot
└── 1 - multiuser system
    └── 4 - applications
        ├── 6 - MATE full desktop
        └── 2*- Plasma full desktop
```
* The asterisk shows which snapshot is currently selected as default

* You can also get only the number of the currently booted snapshot with

```
ast current
```
#### Add descritption to snapshot
* Snapshots allow you to add a description to them for easier identification

```
ast desc <snapshot> <description>
```
#### Delete a tree
* This removes the tree and all it's branches

```
ast del <tree>
```
#### Custom boot configuration
* If you need to use a custom grub configuration, chroot into a snapshot and edit ```/etc/default/grub```, then deploy the snapshot and reboot

#### chroot into snapshot 
* Once inside the chroot the OS behaves like regular Arch, so you can install and remove packages using pacman or similar
* Do not run ast from inside a chroot, it could cause damage to the system, there is a failsafe in place, which can be bypassed with ```--chroot``` if you really need to (not recommended)  
* The chroot has to be exited properly with ```exit 0```, otherwise the changes made will not be saved
* To discard the changes made, use ```exit 1``` instead
* If you don't exit chroot the "clean" way with ```exit 0```, it's recommended to run ```ast tmp``` to clear temporary files left behind


```
ast chroot <snapshot>
```

* You can enter an unlocked shell inside the current booted snapshot with

```
ast live-chroot
```

* The changes made to live session are not saved on new deployments 

#### Other chroot options

* Runs a specified command inside snapshot

```
ast run <snapshot> <command>
```

* Runs a specified command inside snapshot and all it's branches

```
ast tree-run <tree> <command>
```

#### Clone snapshot
* This clones the snapshot as a new tree

```
ast clone <snapshot>
```

#### Clone a tree recursively  
* This clones an entire tree recursively

```
ast clone-tree <snapshot>
```

#### Create new tree branch

* Adds a new branch to specified snapshot

```
ast branch <snapshot to branch from>
```
#### Clone snapshot under same parent

```
ast cbranch <snapshot>
```
#### Clone snapshot under specified parent

* Make sure to sync the tree after

```
ast ubranch <parent> <snapshot>
```
#### Create new base tree

```
ast new
```
#### Deploy snapshot  

* Reboot to  boot into the new snapshot after deploying

```
ast deploy <snapshot>  
```

#### Update base which new snapshots are built from

```
ast base-update
```
* Note: the base itself is located at ```/.snapshots/rootfs/snapshot-0``` with it's specific ```/var``` files and ```/etc``` being located at ```/.snapshots/var/var-0``` and ```/.snapshots/etc/etc-0``` respectively, therefore if you really need to make a configuration change, you can mount snapshot these as read-write and then snapshot back as read only

## Package management

#### Software installation
* Software can also be installed using pacman in a chroot
* AUR can be used under the chroot
* Flatpak can be used for persistent package installation
* Using containers for additional software installation is also an option. An easy way of doing this is with [distrobox](https://github.com/89luca89/distrobox)

```
ast install <snapshot> <package>
```

* After installing you can sync the newly installed packages to all the branches of the tree with
* Syncing the tree also automatically updates all the snapshots

```
ast sync <tree>
```

* If you wish to sync without updating (could cause package duplication in database) then use

```
ast force-sync <tree>
```

* astOS also supports the AUR natively
* Before we can enable AUR support we first have to make sure ``paru`` is not installed:

```
ast remove <snapshot> paru
```

* To use this feature we first need to enable AUR support in the snapshot configuration:

```
EDITOR=nano ast edit-conf <snapshot> # set the EDITOR variable
```

* Now we need to add the following line into the file:

```
aur::True
```

* Save and quit
* AUR support is now enabled - ``ast install`` and other operations can now install AUR packages as usual

#### Removing software

* For a single snapshot

```
ast remove <snapshot> <package or packages>
```

* Recursively

```
ast tree-rmpkg <tree> <pacakge or packages>
```



#### Updating
* It is advised to clone a snapshot before updating it, so you can roll back in case of failure
* This update only updates the system packages, in order to update ast itself see [this section](https://github.com/CuBeRJAN/astOS#updating-ast-itself)
 

* To update a single snapshot

```
ast upgrade <snapshot>
```
* To recursively update an entire tree

```
ast tree-upgrade <tree>
```

* This can be configured in a script (ie. a crontab script) for easy and safe automatic updates

* If the system becomes unbootable after an update, you can boot last working deployment (select in grub menu) and then perform a rollback

```
ast rollback
```

* Then you can reboot back to a working system

## Extras

#### Fixing pacman corrupt packages / key issues
* Arch's pacman package manager sometimes requires a refresh of the PGP keys
* To fix this issue we can simply reinstall they arch keyring

```
ast install <snapshots> archlinux-keyring
```

#### Saving configuration changes made in ``/etc``
* Normally configuration should be done with ``ast chroot``, but sometimes you may want to apply changes you've made to the booted system persistently
* To do this use the following command

```
ast etc-update
```

* This allows you to configure your system by modifying ``/etc`` as usual, and then saving these changes

#### Dual boot
* astOS supports dual boot using the GRUB bootloader
* When installing the system, use the existing EFI partition
* to configure dual boot, we must begin by installing the ```os-prober``` package:

```
ast install <snapshot> os-prober
```

* Now we have to configure grub

```
ast chroot <snapshot>
echo 'GRUB_DISABLE_OS_PROBER=false' >> /etc/default/grub
exit 0
```

* Now just deploy the snapshot to reconfigure the bootloader

```
ast deploy <snapshot>
```

If Windows is detected, ast should return output along the lines of `Found Windows Boot Manager on...`

You may need to install `ntfs-3g` first and re-deploy if you don't see a Windows entry.

#### Updating ast itself
* ast doesn't get updated alongside the system when `ast upgrade` is used
* sometimes it may be necessary to update ast itself
* ast can be updated with a single command

```
ast ast-sync
```

#### Debugging ast

- sometimes it may be necessary to debug ast
- copy `ast` to any location:

```
cp /usr/local/sbin/ast astpk.py
```

- the following command is useful as it shows outputs of commands when running astpk.py:

```
sed -i -e s,\ 2\>\&1\>\ \/dev\/null,,g astpk.py
```

If you have modified the original ast file (possible but not recommended), please make sure to revert it back when done!

## Known bugs

* When running ast without arguments - IndexError: list index out of range
* Running ast without root permissions shows permission denied errors instead of an error message
* Swap partition doesn't work, it's recommended to use a swapfile or zram instead
* Docker has issues with permissions, to fix run
```
sudo chmod 666 /var/run/docker.sock
```

* If you run into any issues, report them on [the issues page](https://github.com/CuBeRJAN/astOS/issues)

# Contributing
* Code and documentation contributions are welcome
* Bug reports are a good way of contributing to the project too
* Before submitting a pull request test your code and make sure to comment it properly

# Community
* Please feel free to join us on [Discord](https://discord.gg/YVHEC6XNZw) for further discussion and support!
* Happy worry-free snapshotting!

---

**Project is licensed under the AGPLv3 license**

