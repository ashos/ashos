#!/usr/bin/python3

import os
import stat
import subprocess
import sys
from anytree import AsciiStyle, find, Node, PreOrderIter, RenderTree
from anytree.exporter import DictExporter
from anytree.importer import DictImporter
from argparse import ArgumentParser
from ast import literal_eval
from configparser import ConfigParser, NoOptionError, NoSectionError
from filecmp import cmp
from os.path import expanduser
from re import sub
from shutil import copy
from tempfile import TemporaryDirectory
from urllib.request import urlopen
from urllib.error import URLError, HTTPError

# Directories
# All snapshots share one /var
# global boot is always at @boot
# *-deploy and *-deploy-aux        : temporary directories used to boot deployed snapshot
# *-deploy[-aux]-secondary         : temporary directories used to boot secondary deployed snapshot
# *-chr                            : temporary directories used to chroot into snapshots or copy them around
# /.snapshots/ash/ash              : symlinked to /usr/bin/ash
# /.snapshots/etc/etc-*            : individual /etc for each snapshot
# /.snapshots/boot/boot-*          : individual /boot for each snapshot
# /.snapshots/rootfs/snapshot-*    : snapshots
# /.snapshots/ash/snapshots/*-desc : descriptions
# /usr/share/ash                   : files that store current snapshot info
# /usr/share/ash/db                : package database
# /var/lib/ash(/fstree)            : ash files, stores fstree, symlink to /.snapshots/ash
# Failed prompts start with "F: "

distro = subprocess.check_output(['/usr/bin/detect_os.sh', 'id']).decode('utf-8').replace('"', "").strip()
distro_name = subprocess.check_output(['/usr/bin/detect_os.sh', 'name']).decode('utf-8').strip()
GRUB = subprocess.check_output("ls /boot | grep grub", encoding='utf-8', shell=True).strip()
DEBUG = " >/dev/null 2>&1" # options: "", " >/dev/null 2>&1"
URL = "https://raw.githubusercontent.com/ashos/ashos/main/src"
USERNAME = os.getenv("SUDO_USER") or os.getenv("USER")
HOME = os.path.expanduser('~'+ USERNAME) # type: ignore

# ------------------------------ CORE FUNCTIONS ------------------------------ #

#   Clone within node
def add_node_to_level(tree, id, val):
    npar = get_parent(tree, id)
    par = (find(tree, filter_=lambda node: ("x"+str(node.name)+"x") in ("x"+str(npar)+"x")))
    Node(val, parent=par)

#   Add child to node
def add_node_to_parent(tree, id, val):
    par = (find(tree, filter_=lambda node: ("x"+str(node.name)+"x") in ("x"+str(id)+"x")))
    Node(val, parent=par)

#   Add to root tree
def append_base_tree(tree, val):
    Node(val, parent=tree.root)

def ash_chroot_mounts(i, CHR=""):
    os.system(f"mount --bind --make-slave /.snapshots/rootfs/snapshot-{CHR}{i} /.snapshots/rootfs/snapshot-{CHR}{i}{DEBUG}")
    os.system(f"mount --rbind --make-rslave /dev /.snapshots/rootfs/snapshot-{CHR}{i}/dev{DEBUG}")
    os.system(f"mount --bind --make-slave /etc /.snapshots/rootfs/snapshot-{CHR}{i}/etc{DEBUG}")
    os.system(f"mount --bind --make-slave /home /.snapshots/rootfs/snapshot-{CHR}{i}/home{DEBUG}")
    os.system(f"mount --types proc /proc /.snapshots/rootfs/snapshot-{CHR}{i}/proc{DEBUG}")
    #os.system(f"mount --rbind --make-rslave /proc /.snapshots/rootfs/snapshot-{CHR}{i}/proc{DEBUG}") ### REVIEW 2023 USE THIS INSTEAD?
    #os.system(f"mount --bind --make-slave /root /.snapshots/rootfs/snapshot-{CHR}{i}/root{DEBUG}") ### REVIEW 2023 ADD THIS TOO?
    os.system(f"mount --bind --make-slave /run /.snapshots/rootfs/snapshot-{CHR}{i}/run{DEBUG}")
    #os.system(f"mount --rbind --make-rslave /run /.snapshots/rootfs/snapshot-chr{i}/run{DEBUG}") ### REVIEW 2023 USE THIS INSTEAD?
    os.system(f"mount --rbind --make-rslave /sys /.snapshots/rootfs/snapshot-{CHR}{i}/sys{DEBUG}")
    os.system(f"mount --bind --make-slave /tmp /.snapshots/rootfs/snapshot-{CHR}{i}/tmp{DEBUG}")
    #os.system(f"mount --rbind --make-rslave /tmp /.snapshots/rootfs/snapshot-{CHR}{i}/tmp{DEBUG}") ### REVIEW 2023 USE THIS INSTEAD?
    os.system(f"mount --bind --make-slave /var /.snapshots/rootfs/snapshot-{CHR}{i}/var{DEBUG}")
    if is_efi():
        os.system(f"mount --rbind --make-rslave /sys/firmware/efi/efivars /.snapshots/rootfs/snapshot-{CHR}{i}/sys/firmware/efi/efivars{DEBUG}")
    os.system(f"cp --dereference /etc/resolv.conf /.snapshots/rootfs/snapshot-{CHR}{i}/etc/{DEBUG}") ### REVIEW Maybe not needed?

###   Lock ash (no longer needed) REVIEW 2023
###def ash_lock():
###    os.system("touch /.snapshots/ash/lock-disable")

###   Unlock ash (no longer needed) REVIEW 2023
###def ash_unlock():
###    os.system("rm -rf /.snapshots/ash/lock")

#   Update ash itself
def ash_update(dbg):
    dist = distro.split("_")[0] # Remove '_ashos"
    mode = stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH
    with TemporaryDirectory(dir="/.snapshots/tmp", prefix="ashpk.") as tmpdir:
        try:
            #f1 = get(f"{URL}/ashpk_core.py", allow_redirects=True)
            #f2 = get(f"{URL}/distros/{dist}/ashpk.py", allow_redirects=True) ### REVIEW
            #open(f"{tmpdir}/ash", 'w').write(f1.text + f2.text)
            f1 = urlopen(f"{URL}/ashpk_core.py").read().decode('utf-8')
            f2 = urlopen(f"{URL}/distros/{dist}/ashpk.py").read().decode('utf-8')
            open(f"{tmpdir}/ash", 'w').write(f1 + f2)
        except (HTTPError, URLError):
            print(f"F: Failed to download ash.")
        else:
            if dbg: # Just for testing
                copy(f"{tmpdir}/ash", f'{HOME}/ash-latest')
                os.chmod(f'{HOME}/ash-latest', mode)
                print(f"Latest ash downloaded to {HOME}.")
            elif cmp(f"{tmpdir}/ash", __file__, shallow=False):
                print("F: Ash already up to date.")
            else:
                copy(__file__, f"{tmpdir}/ash_old")
                copy(f"{tmpdir}/ash", __file__)
                os.chmod(__file__, mode)
                print(f"Ash updated succesfully. Old Ash moved to {tmpdir}.")

def ash_version():
    os.system('date -r /usr/bin/ash "+%Y%m%d-%H%M%S"')

#   Add node to branch
def branch_create(snap, desc=""): # blank description if nothing is passed
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot branch as snapshot {snap} doesn't exist.")
    else:
        if check_mutability(snap):
            immutability = ""
        else:
            immutability = "-r"
        i = find_new()
        os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-{snap} /.snapshots/boot/boot-{i}{DEBUG}")
        os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{snap} /.snapshots/etc/etc-{i}{DEBUG}")
        os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-{snap} /.snapshots/rootfs/snapshot-{i}{DEBUG}")
        if immutability == "": # Mark newly created snapshot as mutable too
            os.system(f"touch /.snapshots/rootfs/snapshot-{i}/usr/share/ash/mutable")
        add_node_to_parent(fstree, snap, i)
        write_tree(fstree)
        if desc:
            write_desc(" ".join(desc), i)
        print(f"Branch {i} added under snapshot {snap}.")


#   Check if snapshot is mutable
def check_mutability(snap):
    return os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}/usr/share/ash/mutable")

#   Check if last update was successful
def check_update():
    with open("/.snapshots/ash/upstate", "r") as upstate:
        line = upstate.readline()
        date = upstate.readline()
        if "1" in line:
            print(f"F: Last update on {date} failed.")
        if "0" in line:
            print(f"Last update on {date} completed successfully.")

#   Clean chroot mount directories for a snapshot
def chr_delete(snap):
    try:
        if os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snap}"):
            subprocess.check_output(f"btrfs sub del /.snapshots/boot/boot-chr{snap}", shell=True)
            subprocess.check_output(f"btrfs sub del /.snapshots/etc/etc-chr{snap}", shell=True)
            #os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{snap}/*{DEBUG}") # error: Not a Btrfs subvolume
            subprocess.check_output(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{snap}", shell=True)
    except subprocess.CalledProcessError:
        print(f"F: Failed to delete chroot snapshot {snap}.")
#    else:
#        print(f"Snapshot chroot {snap} deleted.") ### just when debugging

#   Chroot into snapshot and optionally run command(s)
def chroot(snap, cmd=""): ### make cmd to cmds (IMPORTANT for install_profile)
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot chroot as snapshot {snap} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snap}"): # Make sure snapshot is not in use by another ash process
        print(f"F: Snapshot {snap} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {snap}'.") ### REMOVE_ALL_THESE_LINES
    elif snap == 0:
        print("F: Changing base snapshot is not allowed.")
    else:
        prepare(snap)
        excode = os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} {' '.join(cmd)}") ### TODO if you chroot, then run 'dua' and exit, it makes excode non-zero!
        if excode:
            chr_delete(snap)
            print("F: Chroot failed and changes discarded!")
            return 1
        else:
            post_transactions(snap)
            return 0

#   Check if inside chroot
def chroot_check():
    chroot = True
    with open("/proc/mounts", "r") as mounts:
        buf = mounts.read() # Read entire file at once into a buffer
        if str("/.snapshots btrfs") in buf:
             chroot = False
#       for line in mounts: if str("/.snapshots btrfs") in str(line): chroot = False ### REVIEW NEWER
    return(chroot)

#   Clone tree
def clone_as_tree(desc, snap):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot clone as snapshot {snap} doesn't exist.")
    else:
        if check_mutability(snap):
            immutability = ""
        else:
            immutability = "-r"
        i = find_new()
        os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-{snap} /.snapshots/boot/boot-{i}{DEBUG}")
        os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{snap} /.snapshots/etc/etc-{i}{DEBUG}")
        os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-{snap} /.snapshots/rootfs/snapshot-{i}{DEBUG}")
        if immutability == "": # Mark newly created snapshot as mutable too
            os.system(f"touch /.snapshots/rootfs/snapshot-{i}/usr/share/ash/mutable")
        append_base_tree(fstree, i)
        write_tree(fstree)
        if not desc:
            description = f"clone of {snap}"
        else:
            description = " ".join(desc)
        write_desc(description, i)
        print(f"Tree {i} cloned from {snap}.")

#   Clone branch under same parent
def clone_branch(snap):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot clone as snapshot {snap} doesn't exist.")
    else:
        if check_mutability(snap):
            immutability = ""
        else:
            immutability = "-r"
        i = find_new()
        os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-{snap} /.snapshots/boot/boot-{i}{DEBUG}")
        os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{snap} /.snapshots/etc/etc-{i}{DEBUG}")
        os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-{snap} /.snapshots/rootfs/snapshot-{i}{DEBUG}")
        if immutability == "": # Mark newly created snapshot as mutable too
            os.system(f"touch /.snapshots/rootfs/snapshot-{i}/usr/share/ash/mutable")
        add_node_to_level(fstree, snap, i)
        write_tree(fstree)
        desc = str(f"clone of {snap}")
        write_desc(desc, i)
        print(f"Branch {i} added to parent of {snap}.")
        return i

#   Recursively clone an entire tree
def clone_recursive(snap):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: cannot clone as tree {snap} doesn't exist.")
    else:
        children = return_children(fstree, snap)
        ch = children.copy()
        children.insert(0, snap)
        ntree = clone_branch(snap)
        new_children = ch.copy()
        new_children.insert(0, ntree)
        for child in ch:
            i = clone_under(new_children[children.index(get_parent(fstree, child))], child)
            new_children[children.index(child)] = i

#   Clone under specified parent
def clone_under(snap, branch):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot clone as snapshot {snap} doesn't exist.")
    elif not os.path.exists(f"/.snapshots/rootfs/snapshot-{branch}"):
        print(f"F: Cannot clone as snapshot {branch} doesn't exist.")
    else:
        if check_mutability(snap):
            immutability = ""
        else:
            immutability = "-r"
        i = find_new()
        os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-{branch} /.snapshots/boot/boot-{i}{DEBUG}")
        os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{branch} /.snapshots/etc/etc-{i}{DEBUG}")
        os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-{branch} /.snapshots/rootfs/snapshot-{i}{DEBUG}")
        if immutability == "": # Mark newly created snapshot as mutable too
            os.system(f"touch /.snapshots/rootfs/snapshot-{i}/usr/share/ash/mutable")
        add_node_to_parent(fstree, snap, i)
        write_tree(fstree)
        desc = str(f"clone of {branch}")
        write_desc(desc, i)
        print(f"Branch {i} added under snapshot {snap}.")
        return i

#   Delete tree or branch
def delete_node(snaps, quiet):
    for snap in snaps:
        if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
            print(f"F: Cannot delete as snapshot {snap} doesn't exist.")
        elif snap == 0:
            print("F: Changing base snapshot is not allowed.")
        elif snap == get_current_snapshot():
            print("F: Cannot delete booted snapshot.")
        elif snap == get_next_snapshot():
            print("F: Cannot delete deployed snapshot.")
        elif not quiet and not yes_no(f"Are you sure you want to delete snapshot {snap}?"):
            sys.exit("Aborted")
        else:
            children = return_children(fstree, snap)
            write_desc("", snap) # Clear description # Why have this? REVIEW 2023
            os.system(f"btrfs sub del /.snapshots/boot/boot-{snap}{DEBUG}")
            os.system(f"btrfs sub del /.snapshots/etc/etc-{snap}{DEBUG}")
            os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{snap}{DEBUG}")
            # Make sure temporary chroot directories are deleted as well
            if (os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snap}")):
                os.system(f"btrfs sub del /.snapshots/boot/boot-chr{snap}{DEBUG}")
                os.system(f"btrfs sub del /.snapshots/etc/etc-chr{snap}{DEBUG}")
                os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{snap}{DEBUG}")
            for child in children: # This deletes the node itself along with its children
                write_desc("", snap) # Why have this? REVIEW 2023
                os.system(f"btrfs sub del /.snapshots/boot/boot-{child}{DEBUG}")
                os.system(f"btrfs sub del /.snapshots/etc/etc-{child}{DEBUG}")
                os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{child}{DEBUG}")
                if (os.path.exists(f"/.snapshots/rootfs/snapshot-chr{child}")):
                    os.system(f"btrfs sub del /.snapshots/boot/boot-chr{child}{DEBUG}")
                    os.system(f"btrfs sub del /.snapshots/etc/etc-chr{child}{DEBUG}")
                    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{child}{DEBUG}")
            remove_node(fstree, snap) # Remove node from tree or root
            write_tree(fstree)
            print(f"Snapshot {snap} removed.")

#   Deploy snapshot
def deploy(snap, secondary=False):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot deploy as snapshot {snap} doesn't exist.")
    else:
        update_boot(snap, secondary)
        tmp = get_tmp()
        os.system(f"btrfs sub set-default /.snapshots/rootfs/snapshot-{tmp}{DEBUG}") # Set default volume
        tmp_delete(secondary)
        tmp = get_aux_tmp(tmp, secondary)
      # Special mutable directories
        options = snapshot_config_get(snap)
        mtbl_dirs = options["mutable_dirs"].split(',').remove('')
        mtbl_dirs_shared = options["mutable_dirs_shared"].split(',').remove('')
      # btrfs snapshot operations
        os.system(f"btrfs sub snap /.snapshots/boot/boot-{snap} /.snapshots/boot/boot-{tmp}{DEBUG}")
        os.system(f"btrfs sub snap /.snapshots/etc/etc-{snap} /.snapshots/etc/etc-{tmp}{DEBUG}")
        os.system(f"btrfs sub snap /.snapshots/rootfs/snapshot-{snap} /.snapshots/rootfs/snapshot-{tmp}{DEBUG}")
###2023        os.system(f"btrfs sub create /.snapshots/var/var-{tmp} >/dev/null 2>&1") # REVIEW pretty sure not needed
        os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{tmp}/boot{DEBUG}")
        os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{tmp}/etc{DEBUG}")
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-{tmp}/var{DEBUG}")
        os.system(f"cp -r --reflink=auto /.snapshots/boot/boot-{snap}/. /.snapshots/rootfs/snapshot-{tmp}/boot{DEBUG}")
        os.system(f"cp -r --reflink=auto /.snapshots/etc/etc-{snap}/. /.snapshots/rootfs/snapshot-{tmp}/etc{DEBUG}")
#       os.system(f"cp --reflink=auto -r /.snapshots/var/var-{etc}/* /.snapshots/var/var-{tmp} >/dev/null 2>&1") # REVIEW 2023 pretty sure not needed
      # If snapshot is mutable, modify '/' entry in fstab to read-write
        if check_mutability(snap):
            os.system(f"sed -i '0,/snapshot-{tmp}/ s|,ro||' /.snapshots/rootfs/snapshot-{tmp}/etc/fstab") ### ,rw
      # Add special user-defined mutable directories as bind-mounts into fstab
        if mtbl_dirs:
            for mnt_path in mtbl_dirs: # type: ignore
                src_path = f"/.snapshots/mutable_dirs/snapshot-{snap}/{mnt_path}"
                os.system(f"mkdir -p /.snapshots/mutable_dirs/snapshot-{snap}/{mnt_path}")
                os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{tmp}/{mnt_path}")
                os.system(f"echo '{src_path} /{mnt_path} none defaults,bind 0 0' >> /.snapshots/rootfs/snapshot-{tmp}/etc/fstab")
      # Same thing but for shared directories
        if mtbl_dirs_shared:
            for mnt_path in mtbl_dirs_shared: # type: ignore
                src_path = f"/.snapshots/mutable_dirs/{mnt_path}"
                os.system(f"mkdir -p /.snapshots/mutable_dirs/{mnt_path}")
                os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{tmp}/{mnt_path}")
                os.system(f"echo '{src_path} /{mnt_path} none defaults,bind 0 0' >> /.snapshots/rootfs/snapshot-{tmp}/etc/fstab")
        os.system(f"btrfs sub snap /var /.snapshots/rootfs/snapshot-{tmp}/var{DEBUG}") ### REVIEW Is this needed? Can it be moved up?
        os.system(f"echo '{snap}' > /.snapshots/rootfs/snapshot-{tmp}/usr/share/ash/snap")
        switch_tmp(secondary)
        init_system_clean(tmp, "deploy")
        os.system(f"btrfs sub set-default /.snapshots/rootfs/snapshot-{tmp}") # Set default volume
        print(f"Snapshot {snap} deployed to /.")

# Find new unused snapshot dir
def find_new():
    i = 0
    boots = os.listdir("/.snapshots/boot")
    etcs = os.listdir("/.snapshots/etc")
    snapshots = os.listdir("/.snapshots/rootfs")
#    snapshots.append(boots)
#    snapshots.append(etcs)
#    snapshots.append(vars) ### Can this be deleted?
    snapshots.append(str(boots))
    snapshots.append(str(etcs))
    snapshots.append(str(vars)) ### Can this be deleted?
    while True:
        i += 1
        if str(f"snapshot-{i}") not in snapshots and str(f"etc-{i}") not in snapshots and str(f"var-{i}") not in snapshots and str(f"boot-{i}") not in snapshots:
            return(i)

#   Get aux tmp
def get_aux_tmp(tmp, secondary=False):
    if secondary:
        if tmp == "secondary":
            tmp = tmp.replace("-secondary", "")
        else:
            tmp = f'{tmp}-secondary'
    else:
        if tmp == "deploy-aux":
            tmp = tmp.replace("deploy-aux", "deploy")
        else:
            tmp = tmp.replace("deploy", "deploy-aux")
    return tmp

#   Get current snapshot
def get_current_snapshot():
    with open("/usr/share/ash/snap", "r") as csnap:
        return int(csnap.read().rstrip())

#   This function returns either empty string or underscore plus name of distro if it was appended to sub-volume names to distinguish
def get_distro_suffix():
    if "ashos" in distro:
        return f'_{distro.replace("_ashos", "")}'
    else:
        sys.exit(1) ### REVIEW before: return ""

#   Get deployed snapshot
def get_next_snapshot(secondary=False):
    d = get_aux_tmp(get_tmp(), secondary)
    if os.path.exists("/.snapshots/rootfs/snapshot-{d}/usr/share/ash/snap"): # Make sure next snapshot exists
        with open(f"/.snapshots/rootfs/snapshot-{d}/usr/share/ash/snap", "r") as csnap:
            return int(csnap.read().rstrip())
    return "" # Return empty string in case no snapshot is deployed ### REVIEW

#   Get parent
def get_parent(tree, id):
    par = (find(tree, filter_=lambda node: ("x"+str(node.name)+"x") in ("x"+str(id)+"x")))
    return(par.parent.name)

#   Get drive partition
def get_part():
    with open("/.snapshots/ash/part", "r") as cpart:
        return subprocess.check_output(f"blkid | grep '{cpart.read().rstrip()}' | awk -F: '{{print $1}}'", shell=True).decode('utf-8').strip()

#   Get tmp partition state
def get_tmp(console=False): # By default just "return" which deployment is running ### removed ", secondary=False" as 2nd arg not needed
    mount = str(subprocess.check_output("cat /proc/mounts | grep ' / btrfs'", shell=True))
    if "deploy-aux-secondary" in mount:
        r = "deploy-aux-secondary"
    elif "deploy-secondary" in mount:
        r = "deploy-secondary"
    elif "deploy-aux" in mount:
        r = "deploy-aux"
    else:
        r = "deploy"
    if console:
        print(r)
    else:
        return r

#   Make a snapshot vulnerable to be modified even further (snapshot should be deployed as mutable)
def hollow(snap):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot make hollow as snapshot {snap} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snap}"): # Make sure snapshot is not in use by another ash process
        print(f"F: Snapshot {snap} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {snap}'.")
    elif snap == 0:
        print("F: Changing base snapshot is not allowed.")
    else:
        ### AUR step might be needed and if so make a distro_specific function with steps similar to install_package(). Call it hollow_helper and change this accordingly().
        prepare(snap)
        os.system(f"mount --rbind --make-rslave / /.snapshots/rootfs/snapshot-chr{snap}")
        if yes_no(f"Snapshot {snap} is now hollow! Whenever done, type yes to deploy and no to discard"):
            post_transactions(snap)
            #os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{s}") OR os.system(f"umount -R /") ### REVIEW NEED to unmount this a second time?! (I BELIEVE NOT NEEDED)
            immutability_enable(snap)
            deploy(snap)
            print(f"Snapshot {snap} hollow operation succeeded. Please reboot!")
        else:
            chr_delete(snap)
            print(f"No changes applied on snapshot {snap}.")

#   Make a node mutable
def immutability_disable(snap):
    if snap != "0":
        if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
            print(f"F: Snapshot {snap} doesn't exist.")
        else:
            if check_mutability(snap):
                print(f"F: Snapshot {snap} is already mutable.")
            else:
                os.system(f"btrfs property set -ts /.snapshots/rootfs/snapshot-{snap} ro false")
                os.system(f"touch /.snapshots/rootfs/snapshot-{snap}/usr/share/ash/mutable")
                print(f"Snapshot {snap} successfully made mutable.")
                write_desc(" MUTABLE ", snap, 'a+')
    else:
        print("F: Snapshot 0 (base) should not be modified.")

#   Make a node immutable
def immutability_enable(snap):
    if snap != "0":
        if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
            print(f"F: Snapshot {snap} doesn't exist.")
        else:
            if not check_mutability(snap):
                print(f"F: Snapshot {snap} is already immutable.")
            else:
                os.system(f"rm /.snapshots/rootfs/snapshot-{snap}/usr/share/ash/mutable")
                os.system(f"btrfs property set -ts /.snapshots/rootfs/snapshot-{snap} ro true")
                print(f"Snapshot {snap} successfully made immutable.")
                os.system(f"sed -i 's| MUTABLE ||g' /.snapshots/ash/snapshots/{snap}-desc")
    else:
        print("F: Snapshot 0 (base) should not be modified.")

#   Import filesystem tree file
def import_tree_file(tname):
    with open(tname, "r") as tfile:
        return literal_eval(tfile.readline())

#   Install packages
def install(pkg, snap):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot install as snapshot {snap} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snap}"): # Make sure snapshot is not in use by another ash process
        print(f"F: Snapshot {snap} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {snap}'.")
    elif snap == 0:
        print("F: Changing base snapshot is not allowed.")
    else:
        excode = install_package(pkg, snap)
        if excode:
            chr_delete(snap)
            print("F: Install failed and changes discarded!")
            return 1

        else:
            post_transactions(snap)
            print(f"Package(s) {pkg} installed in snapshot {snap} successfully.")
            return 0

#   Install live
def install_live(pkg, snap=get_current_snapshot()):
    tmp = get_tmp()
    #options = get_persnap_options(tmp) ### moved this to install_package_live
#    os.system(f"mount --bind /.snapshots/rootfs/snapshot-{tmp} /.snapshots/rootfs/snapshot-{tmp}{DEBUG}") ###REDUNDANT
#    os.system(f"mount --bind /home /.snapshots/rootfs/snapshot-{tmp}/home{DEBUG}") ###REDUNDANT
#    os.system(f"mount --bind /var /.snapshots/rootfs/snapshot-{tmp}/var{DEBUG}") ###REDUNDANT
#    os.system(f"mount --bind /etc /.snapshots/rootfs/snapshot-{tmp}/etc{DEBUG}") ###REDUNDANT
#    os.system(f"mount --bind /tmp /.snapshots/rootfs/snapshot-{tmp}/tmp{DEBUG}") ###REDUNDANT
    ash_chroot_mounts(tmp) ### REVIEW Not having this was the culprit for live install to fail for Arch and derivative. Now, does having this here Ok or does it cause errors in NON-Arch distros? If so move it to ashpk.py
    print("Please wait as installation is finishing.")
    excode = install_package_live(pkg, snap, tmp)
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/*{DEBUG}")
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}{DEBUG}") ### REVIEW not safe
    if not excode:
        print(f"Package(s) {pkg} live installed in snapshot {snap} successfully.")
    else:
        print("F: Live installation failed!")

#   Install a profile from a text file
def install_profile(prof, snap, force=False, secondary=False, section_only=None):
    dist = distro.split("_")[0] # Remove '_ashos"
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot install as snapshot {snap} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snap}"): # Make sure snapshot is not in use by another ash process
        print(f"F: Snapshot {snap} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {snap}'.")
    elif snap == 0:
        print("F: Changing base snapshot is not allowed.")
    else:
        print(f"Updating the system before installing profile {prof}.")
        auto_upgrade(snap) # include these in try except too!
        prepare(snap)
        pkgs = ""
        profconf = ConfigParser(allow_no_value=True, delimiters=("==="), strict=False)
#        profconf.optionxform = lambda option: option # preserve case for letters
        profconf.optionxform = str # type: ignore
        try:
            if os.path.exists(f"/.snapshots/tmp/{prof}.conf") and not force:
                profconf.read(f"/.snapshots/tmp/{prof}.conf")
            else:
                print(f"Downloading profile {prof} from AshOS...")
                resp = urlopen(f"{URL}/profiles/{prof}/{dist}.conf").read().decode('utf-8') ### REVIEW
                with open(f"/.snapshots/tmp/{prof}.conf", 'w') as cfile:
                    cfile.write(resp) # Save for later use
                profconf.read_string(resp)
            if profconf.has_section('presets'):
                presets_helper(profconf, snap) ### BEFORE: profconf['presets'] IMPORTANT BAD PRACTICE
            for p in profconf['packages']:
                pkgs += f"{p} "
            install_package(pkgs.strip(), snap) # remove last space
            for cmd in profconf['commands']:
                os.system(f'chroot /.snapshots/rootfs/snapshot-chr{snap} {cmd}')
        except (NoOptionError, NoSectionError, HTTPError, URLError): ### REVIEW 2023
            chr_delete(snap)
            print("F: Install failed and changes discarded!")
            sys.exit(1) ### REVIEW 2023
        else:
            post_transactions(snap)
            print(f"Profile {prof} installed in snapshot {snap} successfully.")
            print(f"Deploying snapshot {snap}.")
            deploy(snap, secondary)

#   Install profile in live snapshot
def install_profile_live(prof, snap, force):
    dist = distro.split("_")[0] # Remove '_ashos"
    tmp = get_tmp()
    ash_chroot_mounts(tmp)
    print(f"Updating the system before installing profile {prof}.")
    auto_upgrade(tmp)
    pkgs = ""
    profconf = ConfigParser(allow_no_value=True, delimiters=("==="), strict=False)
    try:
        if os.path.exists(f"/.snapshots/tmp/{prof}.conf") and not force:
            profconf.read(f"/.snapshots/tmp/{prof}.conf")
        else:
            print(f"Downloading profile {prof} from AshOS...")
            resp = urlopen(f"{URL}/profiles/{prof}/{dist}.conf").read().decode('utf-8') ### REVIEW
            with open(f"/.snapshots/tmp/{prof}.conf", 'w') as cfile:
                cfile.write(resp) # Save for later use
            profconf.read_string(resp)
        for p in profconf['packages']:
            pkgs += f"{p} "
        install_package_live(pkgs.strip(), snap, tmp) ### REVIEW snapshot argument needed
        for cmd in profconf['commands']:
            os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} {cmd}")
            #excode2 = service_enable(prof, tmp_prof, tmp) ### IMPORTANT: tmp or snap?!
    except (NoOptionError, NoSectionError, HTTPError, URLError): ### REVIEW 2023
        print("F: Install failed!") # Before: Install failed and changes discarded (rephrased as there is no chr_delete here)
        return 1
    else:
        print(f"Profile {prof} installed in current/live snapshot.") ### REVIEW
        return 0
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/*{DEBUG}") ### REVIEW
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}{DEBUG}") ### REVIEW

def is_efi():
    return os.path.exists("/sys/firmware/efi")

#   Return snapshot that has a package
def is_pkg_installed(pkg, snap):
    if pkg in pkg_list(snap, ""):
        return snap
    else:
        return None

#   List sub-volumes for the booted distro only
def list_subvolumes():
    os.system(f"btrfs sub list / | grep -i {get_distro_suffix()} | sort -f -k 9")

#   Live unlocked shell
def live_unlock():
    tmp = get_tmp()
    os.system(f"mount --bind /.snapshots/rootfs/snapshot-{tmp} /.snapshots/rootfs/snapshot-{tmp}{DEBUG}")
    os.system(f"mount --bind /etc /.snapshots/rootfs/snapshot-{tmp}/etc{DEBUG}")
    os.system(f"mount --bind /home /.snapshots/rootfs/snapshot-{tmp}/home{DEBUG}")
    os.system(f"mount --bind /tmp /.snapshots/rootfs/snapshot-{tmp}/tmp{DEBUG}")
    os.system(f"mount --bind /var /.snapshots/rootfs/snapshot-{tmp}/var{DEBUG}")
    os.system(f"chroot /.snapshots/rootfs/snapshot-{tmp}")
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/*{DEBUG}")
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}{DEBUG}")

#   Post transaction function, copy from chroot dirs back to read only snapshot dir
def post_transactions(snap):
    tmp = get_tmp()
  # Some operations were moved below to fix hollow functionality ### IMPORTANT REVIEW 2023
  # File operations in snapshot-chr
#    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{snap}{DEBUG}") ### REVIEW # Moved to a few lines below ### GOOD_TO_DELETE_2023
    os.system(f"rm -rf /.snapshots/boot/boot-chr{snap}/*{DEBUG}")
    os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snap}/boot/. /.snapshots/boot/boot-chr{snap}{DEBUG}")
    os.system(f"rm -rf /.snapshots/etc/etc-chr{snap}/*{DEBUG}")
    os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snap}/etc/. /.snapshots/etc/etc-chr{snap}{DEBUG}")
  # Keep package manager's cache after installing packages. This prevents unnecessary downloads for each snapshot when upgrading multiple snapshots
    cache_copy(snap, "post_transactions")
    os.system(f"btrfs sub del /.snapshots/boot/boot-{snap}{DEBUG}")
    os.system(f"btrfs sub del /.snapshots/etc/etc-{snap}{DEBUG}")
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{snap}{DEBUG}")
    if os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snap}/usr/share/ash/mutable"):
        immutability = ""
    else:
        immutability = "-r"
    os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-chr{snap} /.snapshots/boot/boot-{snap}{DEBUG}")
    os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-chr{snap} /.snapshots/etc/etc-{snap}{DEBUG}")
  # Copy init system files to shared
    init_system_copy(tmp, "post_transactions") # REVIEW 2023 can move this to 2 lines up or 1 line below
    os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-chr{snap} /.snapshots/rootfs/snapshot-{snap}{DEBUG}")
  # ---------------------- fix for hollow functionality ---------------------- #
  # Unmount in reverse order
    os.system(f"umount /.snapshots/rootfs/snapshot-chr{snap}/etc/resolv.conf{DEBUG}")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snap}/dev{DEBUG}")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snap}/home{DEBUG}")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snap}/proc{DEBUG}")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snap}/root{DEBUG}")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snap}/run{DEBUG}")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snap}/sys{DEBUG}")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snap}{DEBUG}")
  # Special mutable directories
    options = snapshot_config_get(snap)
    mtbl_dirs = options["mutable_dirs"].split(',').remove('')
    mtbl_dirs_shared = options["mutable_dirs_shared"].split(',').remove('')
    if mtbl_dirs:
        for mnt_path in mtbl_dirs: # type: ignore
            os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snap}/{mnt_path}{DEBUG}")
    if mtbl_dirs_shared:
        for mnt_path in mtbl_dirs_shared: # type: ignore
            os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snap}/{mnt_path}{DEBUG}")
  # ---------------------- fix for hollow functionality ---------------------- #
    chr_delete(snap)

#   IMPORTANT REVIEW 2023 older to revert if hollow introduces issues
def posttrans(snapshot):
    etc = snapshot
    tmp = get_tmp()
    os.system(f"umount /.snapshots/rootfs/snapshot-chr{snapshot} >/dev/null 2>&1")
    os.system(f"umount /.snapshots/rootfs/snapshot-chr{snapshot}/etc/resolv.conf >/dev/null 2>&1")
    os.system(f"umount /.snapshots/rootfs/snapshot-chr{snapshot}/root >/dev/null 2>&1")
    os.system(f"umount /.snapshots/rootfs/snapshot-chr{snapshot}/home >/dev/null 2>&1")
    os.system(f"umount /.snapshots/rootfs/snapshot-chr{snapshot}/run >/dev/null 2>&1")
    os.system(f"umount /.snapshots/rootfs/snapshot-chr{snapshot}/dev >/dev/null 2>&1")
    os.system(f"umount /.snapshots/rootfs/snapshot-chr{snapshot}/sys >/dev/null 2>&1")
    os.system(f"umount /.snapshots/rootfs/snapshot-chr{snapshot}/proc >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{snapshot} >/dev/null 2>&1")
    os.system(f"rm -rf /.snapshots/etc/etc-chr{snapshot}/* >/dev/null 2>&1")
    os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/etc/* /.snapshots/etc/etc-chr{snapshot} >/dev/null 2>&1")
 #   os.system(f"rm -rf /.snapshots/var/var-chr{snapshot}/* >/dev/null 2>&1")
 #   os.system(f"mkdir -p /.snapshots/var/var-chr{snapshot}/lib/systemd >/dev/null 2>&1")
 #   os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/var/lib/systemd/* /.snapshots/var/var-chr{snapshot}/lib/systemd >/dev/null 2>&1")
    os.system(f"cp -r -n --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/var/cache/pacman/pkg/* /var/cache/pacman/pkg/ >/dev/null 2>&1")
    os.system(f"rm -rf /.snapshots/boot/boot-chr{snapshot}/* >/dev/null 2>&1")
    os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/boot/* /.snapshots/boot/boot-chr{snapshot} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/etc/etc-{etc} >/dev/null 2>&1")
 #   os.system(f"btrfs sub del /.snapshots/var/var-{etc} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/boot/boot-{etc} >/dev/null 2>&1")
    os.system(f"btrfs sub snap -r /.snapshots/etc/etc-chr{snapshot} /.snapshots/etc/etc-{etc} >/dev/null 2>&1")
 #   os.system(f"btrfs sub create /.snapshots/var/var-{etc} >/dev/null 2>&1")
  #  os.system(f"mkdir -p /.snapshots/var/var-{etc}/lib/systemd >/dev/null 2>&1")
  #  os.system(f"cp --reflink=auto -r /.snapshots/var/var-chr{snapshot}/lib/systemd/* /.snapshots/var/var-{etc}/lib/systemd >/dev/null 2>&1")
    os.system(f"rm -rf /var/lib/systemd/* >/dev/null 2>&1")
    os.system(f"cp --reflink=auto -r /.snapshots/rootfs/snapshot-{tmp}/var/lib/systemd/* /var/lib/systemd >/dev/null 2>&1")
    os.system(f"btrfs sub snap -r /.snapshots/rootfs/snapshot-chr{snapshot} /.snapshots/rootfs/snapshot-{snapshot} >/dev/null 2>&1")
    os.system(f"btrfs sub snap -r /.snapshots/boot/boot-chr{snapshot} /.snapshots/boot/boot-{etc} >/dev/null 2>&1")
    chr_delete(snapshot)

#   Prepare snapshot to chroot dir to install or chroot into
def prepare(snap):
    chr_delete(snap)
    os.system(f"btrfs sub snap /.snapshots/rootfs/snapshot-{snap} /.snapshots/rootfs/snapshot-chr{snap}{DEBUG}")
  # Pacman gets weird when chroot directory is not a mountpoint, so the following mount is necessary ### REVIEW
#    ash_chroot_mounts(snap, "chr") ### REVIEW 2023 REPLACE THESE WITH THIS (move etcresolve outside)
    os.system(f"mount --bind --make-slave /.snapshots/rootfs/snapshot-chr{snap} /.snapshots/rootfs/snapshot-chr{snap}{DEBUG}")
    os.system(f"mount --rbind --make-rslave /dev /.snapshots/rootfs/snapshot-chr{snap}/dev{DEBUG}")
    os.system(f"mount --bind --make-slave /home /.snapshots/rootfs/snapshot-chr{snap}/home{DEBUG}")
    os.system(f"mount --rbind --make-rslave /proc /.snapshots/rootfs/snapshot-chr{snap}/proc{DEBUG}")
    os.system(f"mount --bind --make-slave /root /.snapshots/rootfs/snapshot-chr{snap}/root{DEBUG}")
    os.system(f"mount --rbind --make-rslave /run /.snapshots/rootfs/snapshot-chr{snap}/run{DEBUG}")
    os.system(f"mount --rbind --make-rslave /sys /.snapshots/rootfs/snapshot-chr{snap}/sys{DEBUG}")
    os.system(f"mount --rbind --make-rslave /tmp /.snapshots/rootfs/snapshot-chr{snap}/tmp{DEBUG}")
    os.system(f"mount --bind --make-slave /var /.snapshots/rootfs/snapshot-chr{snap}/var{DEBUG}")
  # File operations for snapshot-chr ### IMPORTANT REVIEW 2023 Previously this was placed before the mounts, make difference? (my guess: no)
    os.system(f"btrfs sub snap /.snapshots/boot/boot-{snap} /.snapshots/boot/boot-chr{snap}{DEBUG}")
    os.system(f"btrfs sub snap /.snapshots/etc/etc-{snap} /.snapshots/etc/etc-chr{snap}{DEBUG}")
    os.system(f"cp -r --reflink=auto /.snapshots/boot/boot-chr{snap}/. /.snapshots/rootfs/snapshot-chr{snap}/boot{DEBUG}")
    os.system(f"cp -r --reflink=auto /.snapshots/etc/etc-chr{snap}/. /.snapshots/rootfs/snapshot-chr{snap}/etc{DEBUG}") ### btrfs sub snap etc-{snap} to etc-chr-{snap} not needed before this?
#    os.system(f"cp -r --reflink=auto /.snapshots/var/var-{snap}/lib/systemd/* /.snapshots/rootfs/snapshot-chr{snap}/var/lib/systemd/ >/dev/null 2>&1") # REVIEW 2023 I believe not needed
    init_system_clean(snap, "prepare")
    if os.path.exists("/etc/systemd"): # machine-id is a Systemd thing
        os.system(f"cp /etc/machine-id /.snapshots/rootfs/snapshot-chr{snap}/etc/machine-id")
    os.system(f"mkdir -p /.snapshots/rootfs/snapshot-chr{snap}/.snapshots/ash && cp -f /.snapshots/ash/fstree /.snapshots/rootfs/snapshot-chr{snap}/.snapshots/ash/")
  # Special mutable directories
    options = snapshot_config_get(snap)
    mtbl_dirs = options["mutable_dirs"].split(',').remove('')
    mtbl_dirs_shared = options["mutable_dirs_shared"].split(',').remove('')
    if mtbl_dirs:
        for mnt_path in mtbl_dirs: # type: ignore
            os.system(f"mkdir -p /.snapshots/mutable_dirs/snapshot-{snap}/{mnt_path}")
            os.system(f"mkdir -p /.snapshots/rootfs/snapshot-chr{snap}/{mnt_path}")
            os.system(f"mount --bind /.snapshots/mutable_dirs/snapshot-{snap}/{mnt_path} /.snapshots/rootfs/snapshot-chr{snap}/{mnt_path}")
    if mtbl_dirs_shared:
        for mnt_path in mtbl_dirs_shared: # type: ignore
            os.system(f"mkdir -p /.snapshots/mutable_dirs/{mnt_path}")
            os.system(f"mkdir -p /.snapshots/rootfs/snapshot-chr{snap}/{mnt_path}")
            os.system(f"mount --bind /.snapshots/mutable_dirs/{mnt_path} /.snapshots/rootfs/snapshot-chr{snap}/{mnt_path}")
  # Important: Do not move the following line above (otherwise error) ###UPDATE2023 REVIEW WHEN replaced with ash_chroot_mounts, this would be redundant
    os.system(f"mount --bind --make-slave /etc/resolv.conf /.snapshots/rootfs/snapshot-chr{snap}/etc/resolv.conf{DEBUG}")

#   Return order to recurse tree
def recurse_tree(tree, cid):
    order = []
    for child in (return_children(tree, cid)):
        par = get_parent(tree, child)
        if child != cid:
            order.append(par)
            order.append(child)
    return (order)

#   Refresh snapshot
def refresh(snap):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot refresh as snapshot {snap} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snap}"):
        print(f"F: Snapshot {snap} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {snap}'.")
    elif snap == 0:
        print("F: Changing base snapshot is not allowed.")
    else:
        #sync_time() ### REVIEW At least required in virtualbox, otherwise error in package db update
        prepare(snap)
        excode = refresh_helper(snap)
        if excode:
            chr_delete(snap)
            print("F: Refresh failed and changes discarded!")
        else:
            post_transactions(snap)
            print(f"Snapshot {snap} refreshed successfully.")

#   Recursively remove package in tree
def remove_from_tree(tree, tname, pkg, prof):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{tname}"):
        print(f"F: Cannot update as tree {tname} doesn't exist.")
    else:
        if pkg: ### NEW
            uninstall_package(pkg, tname)
            order = recurse_tree(tree, tname)
            if len(order) > 2:
                order.remove(order[0])
                order.remove(order[0])
            while True:
                if len(order) < 2:
                    break
                arg = order[0]
                sarg = order[1]
                print(arg, sarg)
                order.remove(order[0])
                order.remove(order[0])
                uninstall_package(pkg, sarg)
            print(f"Tree {tname} updated.")
        elif prof:
            print("TODO") ### REVIEW

#   Remove node from tree
def remove_node(tree, id):
    par = (find(tree, filter_=lambda node: ("x"+str(node.name)+"x") in ("x"+str(id)+"x")))
    par.parent = None

#   Return all children for node
def return_children(tree, id):
    children = []
    par = (find(tree, filter_=lambda node: ("x"+str(node.name)+"x") in ("x"+str(id)+"x")))
    for child in PreOrderIter(par):
        children.append(child.name)
    if id in children:
        children.remove(id)
    return (children)

#   Rollback last booted deployment
def rollback():
    tmp = get_tmp()
    i = find_new()
    clone_as_tree("", tmp) ### REVIEW clone_as_tree("rollback", tmp) will do.
    write_desc("rollback", i)
    deploy(i)

#   Enable service(s) (Systemd, OpenRC, etc.) # DELETE
def service_enable(prof, tmp_prof, snap):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot enable services as snapshot {snap} doesn't exist.")
    else: ### No need for other checks as this function is not exposed to user
        try:
            postinst = subprocess.check_output(f"cat {tmp_prof}/packages.conf | grep -E -w '^&' | sed 's|& ||'", shell=True).decode('utf-8').strip().split('\n')
            for cmd in list(filter(None, postinst)): # remove '' from [''] if no postinstalls
                subprocess.check_output(f"chroot /.snapshots/rootfs/snapshot-chr{snap} {cmd}", shell=True)
            services = subprocess.check_output(f"cat {tmp_prof}/packages.conf | grep -E -w '^%' | sed 's|% ||'", shell=True).decode('utf-8').strip().split('\n')
            for cmd in list(filter(None, services)): # remove '' from [''] if no services
                subprocess.check_output(f"chroot /.snapshots/rootfs/snapshot-chr{snap} {cmd}", shell=True)
        except subprocess.CalledProcessError:
            print(f"F: Failed to enable service(s) from {prof}.")
            return 1
        else:
            print(f"Installed service(s) from {prof}.")
            return 0

#   Creates new tree from base file
def snapshot_base_new(desc="clone of base"): # immutability toggle not used as base should always be immutable
    i = find_new()
    os.system(f"btrfs sub snap -r /.snapshots/boot/boot-0 /.snapshots/boot/boot-{i}{DEBUG}")
    os.system(f"btrfs sub snap -r /.snapshots/etc/etc-0 /.snapshots/etc/etc-{i}{DEBUG}")
    os.system(f"btrfs sub snap -r /.snapshots/rootfs/snapshot-0 /.snapshots/rootfs/snapshot-{i}{DEBUG}")
    append_base_tree(fstree, i)
    write_tree(fstree)
    if desc:
        write_desc(desc, i)
    print(f"New tree {i} created.")

#   Edit per-snapshot configuration
def snapshot_config_edit(snap, skip_prep=False, skip_post=False):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot chroot as snapshot {snap} doesn't exist.")
    elif snap == 0:
        print("F: Changing base snapshot is not allowed.")
    else:
        if not skip_prep:
            prepare(snap)
        if "EDITOR" in os.environ:
            os.system(f"$EDITOR /.snapshots/rootfs/snapshot-chr{snap}/etc/ash.conf") # usage: sudo -E ash edit -s <snapshot-number>
        elif not os.system('[ -x "$(command -v nano)" ]'): # nano available:
            os.system(f"nano /.snapshots/rootfs/snapshot-chr{snap}/etc/ash.conf")
        elif not os.system('[ -x "$(command -v vi)" ]'): # vi available
            os.system(f"vi /.snapshots/rootfs/snapshot-chr{snap}/etc/ash.conf")
        else:
            os.system("F: No text editor available!")
        if not skip_post:
            post_transactions(snap)

#   Get per-snapshot configuration options from /etc/ast.conf
def snapshot_config_get(snap):
    options = {"aur":"False","mutable_dirs":"","mutable_dirs_shared":""} # defaults here ### REVIEW This is not distro-generic
    if not os.path.exists(f"/.snapshots/etc/etc-{snap}/ash.conf"):
        return options
    with open(f"/.snapshots/etc/etc-{snap}/ash.conf", "r") as optfile:
        for line in optfile:
            if '#' in line:
                line = line.split('#')[0] # Everything after '#' is a comment
            if '::' in line: # Skip line if there's no option set
                left, right = line.split("::") # Split options with '::'
                options[left] = right[:-1] # Remove newline here
    return options

#   Remove temporary chroot for specified snapshot only
#   This unlocks the snapshot for use by other functions
def snapshot_unlock(snap):
    os.system(f"btrfs sub del /.snapshots/boot/boot-chr{snap}{DEBUG}")
    os.system(f"btrfs sub del /.snapshots/etc/etc-chr{snap}{DEBUG}")
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{snap}{DEBUG}")

#   Switch between distros
def switch_distro():
    while True:
        map_tmp = subprocess.check_output("cat /boot/efi/EFI/map.txt | awk 'BEGIN { FS = "'"'" === "'"'" } ; { print $1 }'", shell=True).decode('utf-8').strip()
        print("Type the name of a distro to switch to: (type 'list' to list them, 'q' to quit)")
        next_distro = input("> ")
        if next_distro == "q":
            break
        elif next_distro == "list":
            print(map_tmp)
        elif next_distro in map_tmp:
            import csv
            with open('/boot/efi/EFI/map.txt', 'r') as file:
                for row in csv.DictReader(file, delimiter=',', quoting=csv.QUOTE_NONE):
                    if row["DISTRO"] == next_distro:
                        try:
                            boot_order = subprocess.check_output("efibootmgr | grep BootOrder | awk '{print $2}'", shell=True).decode('utf-8').strip()
                            temp = boot_order.replace(f'{row["BootOrder"]},', "")
                            new_boot_order = f"{row['BootOrder']},{temp}"
                            subprocess.check_output(f'efibootmgr --bootorder {new_boot_order}{DEBUG}', shell=True)
                        except subprocess.CalledProcessError as e:
                            print(f"F: Failed to switch distros: {e.output}.") ###
                        else:
                            print(f'Done! Please reboot whenever you would like switch to {next_distro}')
                        #break ### REVIEW
            break
        else:
            print("Invalid distro!")
            continue

#   Switch between /tmp deployments
def switch_tmp(secondary=False):
    distro_suffix = get_distro_suffix()
    part = get_part()
    tmp_boot = subprocess.check_output("mktemp -d -p /.snapshots/tmp boot.XXXXXXXXXXXXXXXX", shell=True).decode('utf-8').strip()
    os.system(f"mount {part} -o subvol=@boot{distro_suffix} {tmp_boot}") # Mount boot partition for writing
  # Swap deployment subvolumes: deploy <-> deploy-aux
    source_dep = get_tmp()
    target_dep = get_aux_tmp(get_tmp(), secondary)
    os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-{target_dep}/boot/. {tmp_boot}")
    os.system(f"sed -i 's|@.snapshots{distro_suffix}/rootfs/snapshot-{source_dep}|@.snapshots{distro_suffix}/rootfs/snapshot-{target_dep}|g' {tmp_boot}/{GRUB}/grub.cfg") # Overwrite grub config boot subvolume
    os.system(f"sed -i 's|@.snapshots{distro_suffix}/rootfs/snapshot-{source_dep}|@.snapshots{distro_suffix}/rootfs/snapshot-{target_dep}|g' /.snapshots/rootfs/snapshot-{target_dep}/boot/{GRUB}/grub.cfg")
    os.system(f"sed -i 's|@.snapshots{distro_suffix}/boot/boot-{source_dep}|@.snapshots{distro_suffix}/boot/boot-{target_dep}|g' /.snapshots/rootfs/snapshot-{target_dep}/etc/fstab") # Update fstab for new deployment
    os.system(f"sed -i 's|@.snapshots{distro_suffix}/etc/etc-{source_dep}|@.snapshots{distro_suffix}/etc/etc-{target_dep}|g' /.snapshots/rootfs/snapshot-{target_dep}/etc/fstab")
    os.system(f"sed -i 's|@.snapshots{distro_suffix}/rootfs/snapshot-{source_dep}|@.snapshots{distro_suffix}/rootfs/snapshot-{target_dep}|g' /.snapshots/rootfs/snapshot-{target_dep}/etc/fstab")
    with open(f"/.snapshots/rootfs/snapshot-{source_dep}/usr/share/ash/snap", "r") as sfile:
        snap = sfile.readline().replace(" ", "").replace('\n', "")
  # Update GRUB configurations
    for boot_location in ["/.snapshots/rootfs/snapshot-deploy-aux/boot", tmp_boot]:
        with open(f"{boot_location}/{GRUB}/grub.cfg", "r") as grubconf:
            line = grubconf.readline()
            while "BEGIN /etc/grub.d/10_linux" not in line:
                line = grubconf.readline()
            line = grubconf.readline()
            gconf = str("")
            while "}" not in line:
                gconf = str(gconf)+str(line)
                line = grubconf.readline()
            if "snapshot-deploy-aux" in gconf:
                gconf = gconf.replace("snapshot-deploy-aux", "snapshot-deploy")
            else:
                gconf = gconf.replace("snapshot-deploy", "snapshot-deploy-aux")
            if distro_name in gconf:
                gconf = sub('snapshot \\d', '', gconf) ### IMPORTANT REVIEW 2023
                gconf = gconf.replace(f"{distro_name}", f"{distro_name} last booted deployment (snapshot {snap})")
        os.system(f"sed -i '$ d' {boot_location}/{GRUB}/grub.cfg")
        with open(f"{boot_location}/{GRUB}/grub.cfg", "a") as grubconf:
            grubconf.write(gconf)
            grubconf.write("}\n")
            grubconf.write("### END /etc/grub.d/41_custom ###")
    os.system(f"umount {tmp_boot}{DEBUG}")

def switch_to_windows():
    os.system(f"efibootmgr -c -L 'Windows' -l '\\EFI\\BOOT\\BOOTX64.efi'")

#   Sync time
def sync_time():
    if not os.system('[ -x "$(command -v wget)" ]'): # wget available
        os.system('sudo date -s "$(wget -qSO- --max-redirect=0 google.com 2>&1 | grep Date: | cut -d" " -f5-8)Z"')
    elif not os.system('[ -x "$(command -v curl)" ]'): # curl available
        os.system('sudo date -s "$(curl -I google.com 2>&1 | grep Date: | cut -d" " -f3-6)Z"')

#   Clear all temporary snapshots
def temp_snapshots_clear():
    os.system(f"btrfs sub del /.snapshots/boot/boot-chr*{DEBUG}")
    os.system(f"btrfs sub del /.snapshots/etc/etc-chr*{DEBUG}")
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr*/*{DEBUG}") ### REVIEW
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr*{DEBUG}")

#   Clean tmp dirs
def tmp_delete(secondary=False):
    tmp = get_tmp() ### TODO either 1. combine these two (most likely) and/or 2. sub del tmp1 and tmp2 ?
    tmp = get_aux_tmp(tmp, secondary)
    os.system(f"btrfs sub del /.snapshots/boot/boot-{tmp}{DEBUG}")
    os.system(f"btrfs sub del /.snapshots/etc/etc-{tmp}{DEBUG}")
    #os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{tmp}/*{DEBUG}") # error: Not a Btrfs subvolume
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{tmp}{DEBUG}")

#   Print out tree with descriptions
def tree_print(tree):
    csnap = get_current_snapshot()
    for pre, fill, node in RenderTree(tree, style=AsciiStyle()): # simpler tree style
        if os.path.isfile(f"/.snapshots/ash/snapshots/{node.name}-desc"):
            with open(f"/.snapshots/ash/snapshots/{node.name}-desc", "r") as descfile:
                desc = descfile.readline()
        else:
            desc = ""
        if node.name == 0:
            desc = "base snapshot"
        if csnap != node.name:
            print("%s%s - %s" % (pre, node.name, desc))
        else:
            print("%s%s*- %s" % (pre, node.name, desc))

#   Recursively run an update in tree
def tree_run(tree, tname, cmd):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{tname}"):
        print(f"F: Cannot update as tree {tname} doesn't exist.")
    else:
        prepare(tname)
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{tname} {cmd}")
        post_transactions(tname)
        order = recurse_tree(tree, tname)
        if len(order) > 2:
            order.remove(order[0])
            order.remove(order[0])
        while True:
            if len(order) < 2:
                break
            arg = order[0]
            sarg = order[1]
            print(arg, sarg)
            order.remove(order[0])
            order.remove(order[0])
            if os.path.exists(f"/.snapshots/rootfs/snapshot-chr{sarg}"):
                print(f"F: Snapshot {sarg} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {sarg}'.")
                print("Tree command canceled.")
                return
            else:
                prepare(sarg)
                os.system(f"chroot /.snapshots/rootfs/snapshot-chr{sarg} {cmd}")
                post_transactions(sarg)
        print(f"Tree {tname} updated.")

#   Calls print function
def tree_show():
    tree_print(fstree)

#   Sync tree and all its snapshots
def tree_sync(tree, tname, force_offline, live):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{tname}"):
        print(f"F: Cannot sync as tree {tname} doesn't exist.")
    else:
        if not force_offline: # Syncing tree automatically updates it, unless 'force-sync' is used
            tree_upgrade(tree, tname)
        order = recurse_tree(tree, tname)
        if len(order) > 2:
            order.remove(order[0]) ### TODO: Better way instead of these repetitive removes
            order.remove(order[0])
        while True:
            if len(order) < 2:
                break
            snap_from = order[0]
            snap_to = order[1]
            print(snap_from, snap_to)
            order.remove(order[0])
            order.remove(order[0])
            if os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snap_to}"):
                print(f"F: Snapshot {snap_to} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {snap_to}'.")
                print("Tree sync canceled.")
                return
            else:
                prepare(snap_to)
                tree_sync_helper("chr", snap_from, snap_to) # Pre-sync
                if live and snap_to == get_current_snapshot(): # Live sync
                    tree_sync_helper("", snap_from, get_tmp()) # Post-sync
                post_transactions(snap_to) ### IMPORTANT REVIEW 2023 - Moved here from the line immediately after first tree_sync_helper
        print(f"Tree {tname} synced.")

#   Sync tree helper function ### REVIEW might need to put it in distro-specific ashpk.py
def tree_sync_helper(CHR, s_f, s_t):
    os.system("mkdir -p /.snapshots/tmp-db/local/") ### REVIEW Still resembling Arch pacman folder structure!
    os.system("rm -rf /.snapshots/tmp-db/local/*") ### REVIEW
    pkg_list_from = pkg_list(s_f, "")
    pkg_list_to = pkg_list(s_t, CHR)
  # Get packages to be inherited
    pkg_list_from = [j for j in pkg_list_from if j not in pkg_list_to]
    os.system(f"cp -r /.snapshots/rootfs/snapshot-{CHR}{s_t}/usr/share/ash/db/local/. /.snapshots/tmp-db/local/") ### REVIEW
    os.system(f"cp -n -r --reflink=auto /.snapshots/rootfs/snapshot-{s_f}/. /.snapshots/rootfs/snapshot-{CHR}{s_t}/{DEBUG}")
    os.system(f"rm -rf /.snapshots/rootfs/snapshot-{CHR}{s_t}/usr/share/ash/db/local/*") ### REVIEW
    os.system(f"cp -r /.snapshots/tmp-db/local/. /.snapshots/rootfs/snapshot-{CHR}{s_t}/usr/share/ash/db/local/") ### REVIEW
    for entry in pkg_list_from:
        os.system(f"bash -c 'cp -r /.snapshots/rootfs/snapshot-{s_f}/usr/share/ash/db/local/{entry}-[0-9]* /.snapshots/rootfs/snapshot-{CHR}{s_t}/usr/share/ash/db/local/'") ### REVIEW
    os.system("rm -rf /.snapshots/tmp-db/local/*")

#   Recursively run an update in tree
def tree_upgrade(tree, tname):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{tname}"):
        print(f"F: Cannot update as tree {tname} doesn't exist.")
    else:
        upgrade(tname)
        order = recurse_tree(tree, tname)
        if len(order) > 2:
            order.remove(order[0])
            order.remove(order[0])
        while True:
            if len(order) < 2:
                break
            arg = order[0]
            sarg = order[1]
            print(arg, sarg)
            order.remove(order[0])
            order.remove(order[0])
            auto_upgrade(sarg)
        print(f"Tree {tname} updated.")

#   Uninstall package(s)
def uninstall_package(pkg, snap):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot remove as snapshot {snap} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snap}"):
        print(f"F: Snapshot {snap} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {snap}'.")
    elif snap == 0:
        print("F: Changing base snapshot is not allowed.")
    else:
        prepare(snap)
        excode = uninstall_package_helper(pkg, snap)
        if excode:
            chr_delete(snap)
            print("F: Remove failed and changes discarded!")
        else:
            post_transactions(snap)
            print(f"Package {pkg} removed from snapshot {snap} successfully.")

#   Update boot
def update_boot(snap, secondary = False): ### TODO Other bootloaders
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot update boot as snapshot {snap} doesn't exist.")
    else:
        tmp = get_tmp()
        if secondary:
            if tmp == "secondary":
                tmp = tmp.replace("-secondary", "")
            else:
                tmp = f'{tmp}-secondary'
        part = get_part()
        prepare(snap)
        if os.path.exists(f"/boot/{GRUB}/BAK/"): # REVIEW
            os.system(f"find /boot/{GRUB}/BAK/. -mtime +30 -exec rm -rf" + " {} \\;") # Delete 30-day-old grub.cfg.DATE files
        os.system(f"cp /boot/{GRUB}/grub.cfg /boot/{GRUB}/BAK/grub.cfg.`date '+%Y%m%d-%H%M%S'`")
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} {GRUB}-mkconfig {part} -o /boot/{GRUB}/grub.cfg")
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} sed -i 's|snapshot-chr{snap}|snapshot-{tmp}|g' /boot/{GRUB}/grub.cfg")
####    os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} sed -i '0,\|{distro_name}| s||{distro_name} snapshot {snap}|' /boot/{GRUB}/grub.cfg")
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snap} sed -i '0,\\|{distro_name}| s||{distro_name} snapshot {snap}|' /boot/{GRUB}/grub.cfg")
        post_transactions(snap)

#   Saves changes made to /etc to snapshot
def update_etc():
    tmp = get_tmp() ### TODO for secondary too?
    csnap = get_current_snapshot()
    os.system(f"btrfs sub del /.snapshots/etc/etc-{csnap}{DEBUG}")
    if check_mutability(csnap):
        immutability = ""
    else:
        immutability = "-r"
    os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{tmp} /.snapshots/etc/etc-{csnap}{DEBUG}")

#   Upgrade snapshot
def upgrade(snap, baseup=False):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snap}"):
        print(f"F: Cannot upgrade as snapshot {snap} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snap}"):
        print(f"F: Snapshot {snap} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {snap}'.")
    elif snap == 0 and not baseup:
        print("F: Changing base snapshot is not allowed.")
    else:
#        prepare(snap) ### REVIEW Moved to a distro-specific function as it needs to go after setup_aur_if_enabled()
      # Default upgrade behaviour is now "safe" update, meaning failed updates get fully discarded
        excode = upgrade_helper(snap)
        if excode:
            chr_delete(snap)
            print("F: Upgrade failed and changes discarded!")
            return 1 # REVIEW 2023 not needed
        else:
            post_transactions(snap)
            print(f"Snapshot {snap} upgraded successfully.")
            return 0 # REVIEW 2023 not needed

#   Print snaps that has a package
def which_snapshot_has(pkg):
    snaps = ("1", "2", "3", "4", "5", "6", "7")
    for s in snaps:
        if is_pkg_installed(pkg, str(s)): ### REVIEW
            print(s)

#   Write new description (default) or append to an existing one (i.e. toggle immutability)
def write_desc(desc, snap, mode='w'):
    with open(f"/.snapshots/ash/snapshots/{snap}-desc", mode) as descfile:
        descfile.write(desc)

#   Save tree to file
def write_tree(tree):
    exporter = DictExporter()
    to_write = exporter.export(tree)
    with open(fstreepath, "w") as fsfile:
        fsfile.write(str(to_write))

#   Generic yes no prompt
def yes_no(msg):
    while True:
        print(f"{msg} (y/n)")
        reply = input("> ")
        if reply.casefold() in ('yes', 'y'):
            e = True
            break
        elif reply.casefold() in ('no', 'n'):
            e = False
            break
        else:
            print("F: Invalid choice!")
            continue
    return e

#   Main function
def main():
    if os.geteuid() != 0: # TODO 2023 exception: Make 'ash tree' run without root permissions
        exit("sudo/doas is required to run ash!")
    else:
        importer = DictImporter() # Dict importer
#        isChroot = chroot_check() ### TODO
        global fstree # Currently these are global variables, fix sometime
        global fstreepath # ---
        fstreepath = str("/.snapshots/ash/fstree") # Path to fstree file
        fstree = importer.import_(import_tree_file("/.snapshots/ash/fstree")) # Import fstree file
        #if isChroot == True and ("--chroot" not in args): ### TODO
        #    print("Please don't use ash inside a chroot!") ### TODO
      # Recognize argument and call appropriate function
        parser = ArgumentParser(prog='ash', description='Any Snapshot Hierarchical OS')
        subparsers = parser.add_subparsers(dest='command', required=True, help='Different commands for ash')
      # Ash update
        upme_par = subparsers.add_parser("upme", aliases=['ash-update'], allow_abbrev=True, help="Update ash itself")
        upme_par.add_argument('--dbg', '--debug', '--test', '-d', '-t', action='store_true', required=False, help='Enable live install for snapshot')
        upme_par.set_defaults(func=ash_update)
      # Ash version
        ashv_par = subparsers.add_parser("version", help="Print ash version")
        ashv_par.set_defaults(func=ash_version)
      # Auto upgrade
        autou_par = subparsers.add_parser("auto-upgrade", aliases=['autoup', 'au'], allow_abbrev=True, help='Update a snapshot quietly')
###     autou_par.add_argument("snapshot", type=int, help="snapshot number") ### IMPORTANT REVIEW 2023 any given snapshot or get_current_snapshot() ?
        autou_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number") ### REVIEW maybe for subparsers that only take snapshot as argument, leave it as old version (above)
        autou_par.set_defaults(func=auto_upgrade)
      # Branch create
        branch_par = subparsers.add_parser("branch", aliases=['add-branch'], allow_abbrev=True, help='Create a new branch from snapshot')
        branch_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        branch_par.add_argument("--desc", "--description", "-d", nargs='+', required=False, help="description for the snapshot")
        branch_par.set_defaults(func=branch_create)
      # Check update
        cu_par = subparsers.add_parser("check", help="Check update")
        cu_par.set_defaults(func=check_update)
      # Chroot
        chroot_par = subparsers.add_parser("chroot", aliases=['chr', 'ch'], allow_abbrev=True, help='Open a root shell inside a snapshot')
        chroot_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        chroot_par.set_defaults(func=chroot)
      # Chroot run
        run_par = subparsers.add_parser("run", help='Run command(s) inside another snapshot (chrooted)')
        run_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        run_par.add_argument("cmd", nargs='+', help="command")
        run_par.set_defaults(func=chroot)
      # Clone
        clone_par = subparsers.add_parser("clone", aliases=['cl'], allow_abbrev=True, help='Create a copy of a snapshot (as top-level tree node)')
        clone_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        clone_par.add_argument("--desc", "--description", "-d", nargs='+', required=False, help="description for the snapshot")
        clone_par.set_defaults(func=clone_as_tree)
      # Clone a branch
        clonebr_par = subparsers.add_parser("clone-branch", aliases=['cb', 'cbranch'], allow_abbrev=True, help='Copy snapshot under same parent branch (clone as a branch)')
        clonebr_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        clonebr_par.set_defaults(func=clone_branch)
      # Clone recursively
        clonerec_par = subparsers.add_parser("clone-tree", aliases=['ct'], allow_abbrev=True, help='Clone a whole tree recursively')
        clonerec_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        clonerec_par.set_defaults(func=clone_recursive)
      # Clone under a branch
        cloneunder_par = subparsers.add_parser("clone-under", aliases=['cu', 'ubranch'], allow_abbrev=True, help='Copy snapshot under specified parent (clone under a branch)') # REVIEW 2023 confusing naming convention inside function: ubranch <parent> <snapshot>
        cloneunder_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        cloneunder_par.add_argument("branch", type=int, help="branch number")
        cloneunder_par.set_defaults(func=clone_under)
      # Del
        del_par = subparsers.add_parser("del", aliases=["delete", "rm", "rem", "remove", "rm-snapshot"], allow_abbrev=True, help="Remove snapshot(s)/tree(s) and any branches recursively")
        del_par.add_argument("--snaps", "--snapshots", "-s", nargs='+', type=int, required=True, help="snapshot number")
        del_par.add_argument('--quiet', '-q', action='store_true', required=False, help='Force delete snapshot(s)')
        del_par.set_defaults(func=delete_node)
      # Deploy
        dep_par = subparsers.add_parser("deploy", aliases=['dep', 'd'], allow_abbrev=True, help='Deploy a snapshot for next boot')
        dep_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        dep_par.add_argument('--secondary', '--sec', '-2', action='store_true', required=False, help='Deploy into secondary snapshot slot')
        dep_par.set_defaults(func=deploy)
      # Description
        desc_par = subparsers.add_parser("desc", help='Set a description for a snapshot by number')
        desc_par.add_argument("desc", nargs='+', help="description to be added") ### TODO Can have bad combinations if both required=False
        desc_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        desc_par.set_defaults(func=lambda desc, snap: write_desc(" ".join(desc), snap))
      # Fix db command ### MAYBE ash_unlock was needed?
        fixdb_par = subparsers.add_parser("fixdb", aliases=['fix'], allow_abbrev=True, help='Fix package database of a snapshot')
        fixdb_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        fixdb_par.set_defaults(func=fix_package_db)
      # Get current snapshot
        cs_par = subparsers.add_parser("current", aliases=['c'], allow_abbrev=True, help='Return current snapshot number')
        cs_par.set_defaults(func=lambda: print(get_current_snapshot()))
      # Hollow a node
        hol_par = subparsers.add_parser("hollow", help='Make a snapshot hollow (vulnerable)')
        hol_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        hol_par.set_defaults(func=hollow)
      # Immutability disable
        immdis_par = subparsers.add_parser("immdis", aliases=["disimm", "immdisable", "disableimm"], allow_abbrev=True, help='Disable immutability of a snapshot')
        immdis_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        immdis_par.set_defaults(func=immutability_disable)
      # Immutability enable
        immen_par = subparsers.add_parser("immen", aliases=["enimm", "immenable", "enableimm"], allow_abbrev=True, help='Enable immutability of a snapshot')
        immen_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        immen_par.set_defaults(func=immutability_enable)
      # Install
        inst_par = subparsers.add_parser("install", aliases=['in'], allow_abbrev=True, help='Install package(s) inside a snapshot')
        inst_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        g1i = inst_par.add_mutually_exclusive_group(required=True)
        g1i.add_argument('--pkg', '--package', '-p', nargs='+', required=False, help='package(s)')
        g1i.add_argument('--prof', '--profile', '-P', type=str, required=False, help='profile')
        g2i = inst_par.add_mutually_exclusive_group(required=False)
        g2i.add_argument('--live', '-l', action='store_true', required=False, help='Enable live install for snapshot')
        g2i.add_argument('--not-live', '-nl', action='store_true', required=False, help='Disable live install for snapshot')
        g2i.add_argument('--force', '-f', '-u', action='store_true', required=False, help='Force download profile (Ignore cache)')
        inst_par.set_defaults(func=install_triage)
      # List subvolumes
        sub_par = subparsers.add_parser("sub", aliases=["subs", "subvol", "subvols", "subvolumes"], allow_abbrev=True, help="List subvolumes of active snapshot (currently booted)")
        sub_par.set_defaults(func=list_subvolumes)
      # Live chroot
        lc_par = subparsers.add_parser("live-chroot", aliases=['lchroot', 'lc'], allow_abbrev=True, help='Open a shell inside currently booted snapshot with read-write access. Changes are discarded on new deployment.')
        lc_par.set_defaults(func=live_unlock)
      # Package list
        pl_par = subparsers.add_parser("list", aliases=["ls"], allow_abbrev=True, help='Get list of installed packages in a snapshot')
        pl_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        pl_par.set_defaults(func=pkg_list)
      # Refresh
        ref_par = subparsers.add_parser("refresh", aliases=["ref"], allow_abbrev=True, help='Refresh package manager db of a snapshot')
        ref_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        ref_par.set_defaults(func=refresh)
      # Remove package(s) from tree
        trem_par = subparsers.add_parser("tremove", aliases=["tree-rmpkg"], allow_abbrev=True, help='Uninstall package(s) or profile(s) from a tree recursively')
        trem_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        g1tr = trem_par.add_mutually_exclusive_group(required=True)
        g1tr.add_argument('--pkg', '--package', '-p', nargs='+', required=False, help='package(s)')
        g1tr.add_argument('--prof', '--profile', '-P', type=str, required=False, help='profile') ### TODO nargs='+' for multiple profiles
        trem_par.set_defaults(func=lambda snap, pkg, prof: remove_from_tree(fstree, snap, pkg, prof)) # REVIEW 2023 is it needed to run uninstall_package(pkg, tname) and then again remove_from_tree ? I believe not as it is redundant!
      # Rollback
        roll_par = subparsers.add_parser("rollback", help="Rollback the deployment to the last booted snapshot")
        roll_par.set_defaults(func=rollback)
      # Snapshot base new
        new_par = subparsers.add_parser("new", aliases=['new-tree'], allow_abbrev=True, help='Create a new base snapshot')
        new_par.add_argument("--desc", "--description", "-d", nargs='+', required=False, help="description for the snapshot")
        new_par.set_defaults(func=snapshot_base_new)
      # Snapshot configuration edit
        editconf_par = subparsers.add_parser("edit", aliases=['edit-conf'], allow_abbrev=True, help='Edit configuration for a snapshot')
        editconf_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        editconf_par.set_defaults(func=snapshot_config_edit)
      # Snapshot diff
        diff_par = subparsers.add_parser("diff", aliases=['dif'], allow_abbrev=True, help='Show package diff between snapshots')
        diff_par.add_argument("snap1", type=int, help="source snapshot")
        diff_par.add_argument("snap2", type=int, help="target snapshot")
        diff_par.set_defaults(func=snapshot_diff)
      # Snapshot unlock
        unl_par = subparsers.add_parser("unlock", aliases=["ul"], allow_abbrev=True, help='Unlock a snapshot')
        unl_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        unl_par.set_defaults(func=snapshot_unlock)
      # Switch distros
        switch_par = subparsers.add_parser("dist", aliases=["distro", "distros"], allow_abbrev=True, help="Switch to another distro")
        switch_par.set_defaults(func=switch_distro)
      # Switch to Windows (semi plausible deniability)
        hide_par = subparsers.add_parser("hide", allow_abbrev=False, help="Hide AshOS and switch to Windows")
        hide_par.set_defaults(func=switch_to_windows)
      # Temp snapshots clear
        tmpclear_par = subparsers.add_parser("tmp", aliases=["tmpclear"], allow_abbrev=True, help="Clear all temporary snapshots")
        tmpclear_par.set_defaults(func=temp_snapshots_clear)
      # Tree
        tree_par = subparsers.add_parser("tree", aliases=["t"], allow_abbrev=True, help="Show ash tree")
        tree_par.set_defaults(func=tree_show)
      # Tree run
        trun_par = subparsers.add_parser("trun", aliases=["tree-run"], allow_abbrev=True, help='Run command(s) inside another snapshot and all snapshots below it')
        trun_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        trun_par.add_argument('--cmd', '--command', '-c', nargs='+', required=False, help='command(s) to run')
        trun_par.set_defaults(func=lambda snap, cmd: tree_run(fstree, snap, ' '.join(cmd)))
      # Tree sync
        tsync_par = subparsers.add_parser("sync", aliases=["tree-sync", "tsync"], allow_abbrev=True, help='Sync packages and configuration changes recursively (requires an internet connection)')
        tsync_par.add_argument("tname", type=int, help="snapshot number")
        tsync_par.add_argument('-f', '--force', '--force-offline', action='store_true', required=False, help='Snapshots would not get updated (potentially riskier)')
        tsync_par.add_argument('--not-live', '-nl', action='store_true', required=False, help='Disable live sync') ### REVIEW store_false
        tsync_par.set_defaults(func=lambda tname, force_offline, not_live: tree_sync(fstree, tname, force_offline, not not_live))
      # Tree upgrade
        tupg_par = subparsers.add_parser("tupgrade", aliases=["tree-upgrade", "tup"], allow_abbrev=True, help='Update all packages in a snapshot recursively')
        tupg_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        tupg_par.set_defaults(func=lambda snap: tree_upgrade(fstree, snap))
      # Uninstall package(s) from a snapshot
        uninst_par = subparsers.add_parser("uninstall", aliases=["unin", "uninst", "unins", "un"], allow_abbrev=True, help='Uninstall package(s) from a snapshot')
        uninst_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        g1u = uninst_par.add_mutually_exclusive_group(required=True)
        g1u.add_argument('--pkg', '--package', '-p', nargs='+', required=False, help='package(s)')
        g1u.add_argument('--prof', '--profile', '-P', type=str, required=False, help='profile')
        g2u = uninst_par.add_mutually_exclusive_group(required=False)
        g2u.add_argument('--live', '-l', action='store_true', required=False, help='make snapshot install live')
        g2u.add_argument('--not-live', '-nl', action='store_false', required=False, help='make snapshot install not live')
        uninst_par.set_defaults(func=uninstall_triage)
      # Update boot
        boot_par = subparsers.add_parser("boot", aliases=['boot-update', 'update-boot'], allow_abbrev=True, help='Update boot of a snapshot')
        boot_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        boot_par.set_defaults(func=update_boot)
      # Update etc
        etc_par = subparsers.add_parser("etc-update", aliases=['etc', 'etc-up', 'up-etc'], allow_abbrev=True, help='Update /etc')
        etc_par.set_defaults(func=update_etc)
      # Upgrade a snapshot
        upg_par = subparsers.add_parser("upgrade", aliases=["up"], allow_abbrev=True, help='Update all packages in a snapshot')
        upg_par.add_argument("--snap", "--snapshot", "-s", type=int, required=False, default=get_current_snapshot(), help="snapshot number")
        upg_par.set_defaults(func=upgrade)
      # Upgrade base snapshot
        bu_par = subparsers.add_parser("base-update", aliases=['bu', 'ub'], allow_abbrev=True, help='Update the base snapshot')
        bu_par.set_defaults(func=lambda: upgrade("0", True))
      # Which deployment is active
        whichtmp_par = subparsers.add_parser("whichtmp", aliases=["whichdep", "which"], allow_abbrev=True, help="Show which deployment snapshot is in use")
        whichtmp_par.set_defaults(func=lambda: get_tmp(console=True)) # print to console REVIEW 2023 NEWER: test print(get_tmp)
      # Which snapshot(s) contain a package
        which_snap_par = subparsers.add_parser("whichsnap", aliases=["ws"], allow_abbrev=True, help='Get list of installed packages in a snapshot')
        which_snap_par.add_argument('pkg', nargs='+', help='a package')
        which_snap_par.set_defaults(func=which_snapshot_has)
      # Call relevant functions
        #args_1 = parser.parse_args()
        args_1 = parser.parse_args(args=None if sys.argv[1:] else ['--help']) # Show help if no command used
        args_2 = vars(args_1).copy()
        args_2.pop('command', None)
        args_2.pop('func', None)
        args_1.func(**args_2)

#-------------------- Triage functions for argparse method --------------------#

def install_triage(live, not_live, pkg, prof, snap, force):
    excode = 1 ### REVIEW Use try except
    if prof:
        excode = install_profile(prof, snap, force)
    elif pkg:
        excode = install(" ".join(pkg), snap)
  # Do live install only if: live flag is used OR target snapshot is current and
  # not_live flag is not used. And anyway only run if install above succeeded!
    if not excode and (live or (snap == get_current_snapshot() and not not_live)):
        if prof:
            install_profile_live(prof, snap, force)
        elif pkg:
            install_live(" ".join(pkg), snap)

def uninstall_triage(live, not_live, pkg, prof, snap): ### TODO add live, not_live
    if prof:
        #excode = uninstall_profile(prof, snap)
        print("TODO: uninstall profile")
    elif pkg:
        uninstall_package(" ".join(pkg), snap)

