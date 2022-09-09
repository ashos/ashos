#!/usr/bin/python3

import os
import subprocess
import sys
from anytree import AsciiStyle, find, Node, PreOrderIter, RenderTree
from anytree.exporter import DictExporter
from anytree.importer import DictImporter
from argparse import ArgumentParser
from ast import literal_eval
from re import sub

# Directories
# All snapshots share one /var
# global boot is always at @boot
# *-deploy and *-deploy-aux         : temporary directories used to boot deployed snapshot
# *-chr                             : temporary directories used to chroot into snapshot or copy snapshots around
# /.snapshots/ast/ast               : symlinked to /usr/bin/ash
# /.snapshots/etc/etc-*             : individual /etc for each snapshot
# /.snapshots/boot/boot-*           : individual /boot for each snapshot
# /.snapshots/rootfs/snapshot-*     : snapshots
# /.snapshots/ast/snapshots/*-desc  : descriptions
# /usr/share/ash                    : files that store current snapshot info
# /usr/share/ash/db                 : package database
# /var/lib/ash(/fstree)             : ash files, stores fstree, symlink to /.snapshots/ash
# Failed prompts start with "F: "

distro = subprocess.check_output(['/usr/bin/detect_os.sh', 'id']).decode('utf-8').replace('"', "").strip()
distro_name = subprocess.check_output(['/usr/bin/detect_os.sh', 'name']).decode('utf-8').strip()
GRUB = subprocess.check_output("ls /boot | grep grub", encoding='utf-8', shell=True).strip()

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
    os.system(f"mount --bind --make-slave /.snapshots/rootfs/snapshot-{CHR}{i} /.snapshots/rootfs/snapshot-{CHR}{i} >/dev/null 2>&1")
    os.system(f"mount --rbind --make-rslave /dev /.snapshots/rootfs/snapshot-{CHR}{i}/dev >/dev/null 2>&1")
    os.system(f"mount --bind --make-slave /etc /.snapshots/rootfs/snapshot-{CHR}{i}/etc >/dev/null 2>&1")
    os.system(f"mount --bind --make-slave /home /.snapshots/rootfs/snapshot-{CHR}{i}/home >/dev/null 2>&1")
    os.system(f"mount --types proc /proc /.snapshots/rootfs/snapshot-{CHR}{i}/proc >/dev/null 2>&1")
    os.system(f"mount --bind --make-slave /run /.snapshots/rootfs/snapshot-{CHR}{i}/run >/dev/null 2>&1")
    os.system(f"mount --rbind --make-rslave /sys /.snapshots/rootfs/snapshot-{CHR}{i}/sys >/dev/null 2>&1")
    os.system(f"mount --bind --make-slave /tmp /.snapshots/rootfs/snapshot-{CHR}{i}/tmp >/dev/null 2>&1")
    os.system(f"mount --bind --make-slave /var /.snapshots/rootfs/snapshot-{CHR}{i}/var >/dev/null 2>&1")
    if is_efi():
        os.system(f"mount --rbind --make-rslave /sys/firmware/efi/efivars /.snapshots/rootfs/snapshot-{CHR}{i}/sys/firmware/efi/efivars >/dev/null 2>&1")
    os.system(f"cp --dereference /etc/resolv.conf /.snapshots/rootfs/snapshot-{CHR}{i}/etc/ >/dev/null 2>&1") ### REVIEW_LATER Maybe not needed?

#   Update ash itself
def ash_update():
    try:
        d = distro.split("_")[0] # Remove '_ashos"
        tmp_ash = subprocess.check_output("mktemp -d -p /.snapshots/tmp ashpk.XXXXXXXXXXXXXXXX", shell=True, encoding='utf-8').strip()
        subprocess.check_output(f"curl --fail -H 'pragma:no-cache' -H 'cache-control:no-cache,no-store' -s -o {tmp_ash}/ashpk_core.py -O \
                                'https://raw.githubusercontent.com/ashos/ashos/main/src/ashpk_core.py'", shell=True) # GitHub still caches
        subprocess.check_output(f"curl --fail -H 'pragma:no-cache' -H 'cache-control:no-cache,no-store' -s -o {tmp_ash}/ashpk.py -O \
                                'https://raw.githubusercontent.com/ashos/ashos/main/src/distros/{d}/ashpk.py'", shell=True) ### temporary URL
        os.system(f"cat {tmp_ash}/ashpk_core.py {tmp_ash}/ashpk.py > {tmp_ash}/ash")
        os.system(f"chmod +x {tmp_ash}/ash")
    except subprocess.CalledProcessError as e:
        print(f"F: Failed to download ash: {e.output}.")
    else:
        if os.system(f"diff {tmp_ash}/ash /.snapshots/ash/ash"):
            os.system(f"cp -a {tmp_ash}/ash /.snapshots/ash/ash")
        else:
            print("F: Ash already up to dated.")

def ash_version():
    os.system('date -r /usr/bin/ash "+%Y%m%d-%H%M%S"')

#   Check if snapshot is mutable
def check_mutability(snapshot):
    return os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable")

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
def chr_delete(snapshot):
    try:
        if os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"):
            subprocess.check_output(f"btrfs sub del /.snapshots/boot/boot-chr{snapshot}", shell=True)
            subprocess.check_output(f"btrfs sub del /.snapshots/etc/etc-chr{snapshot}", shell=True)
            subprocess.check_output(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{snapshot}", shell=True)
    except subprocess.CalledProcessError as e:
        print(f"F: Failed to delete chroot snapshot {snapshot}: {e.output}.")
    else:
        print(f"Snapshot chroot {snapshot} deleted.")

#   Run command in snapshot
def chr_run(snapshot, cmd): ### make cmd to cmds (VERY IMPORTANT FOR install_profile()
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot chroot as snapshot {snapshot} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"): # Make sure snapshot is not in use by another ash process
        print(f"F: Snapshot {snapshot} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {snapshot}'.") ### REMOVE_ALL_THESE_LINES
    elif snapshot == "0":
        print("F: Changing base snapshot is not allowed.")
    else:
        prepare(snapshot)
        #os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} {cmd}") ### Before argparse
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} {' '.join(cmd)}")
        post_transactions(snapshot)

#   Chroot into snapshot
def chroot(snapshot):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot chroot as snapshot {snapshot} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"): # Make sure snapshot is not in use by another ash process
        print(f"F: Snapshot {snapshot} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {snapshot}'.")
    elif snapshot == "0":
        print("F: Changing base snapshot is not allowed.")
    else:
        prepare(snapshot)
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot}")
        post_transactions(snapshot)

#   Check if inside chroot
def chroot_check():
    chroot = True
    with open("/proc/mounts", "r") as mounts:
        buf = mounts.read() # Read entire file at once into a buffer
        if str("/.snapshots btrfs") in buf:
             chroot = False
    return(chroot)

#   Clone tree
def clone_as_tree(snapshot, desc):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot clone as snapshot {snapshot} doesn't exist.")
    else:
        if check_mutability(snapshot):
            immutability = ""
        else:
            immutability = "-r"
        i = find_new()
        os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-{snapshot} /.snapshots/boot/boot-{i} >/dev/null 2>&1")
        os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{snapshot} /.snapshots/etc/etc-{i} >/dev/null 2>&1")
        os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-{snapshot} /.snapshots/rootfs/snapshot-{i} >/dev/null 2>&1")
        if immutability == "": # Mark newly created snapshot as mutable too
            os.system(f"touch /.snapshots/rootfs/snapshot-{i}/usr/share/ash/mutable")
        append_base_tree(fstree, i)
        write_tree(fstree)
        #desc = str(f"clone of {snapshot}") ###
        if not desc:
            description = f"clone of {snapshot}"
        else:
            description = " ".join(desc)
        write_desc(i, description)
        print(f"Tree {i} cloned from {snapshot}.")

#   Clone branch under same parent
def clone_branch(snapshot):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot clone as snapshot {snapshot} doesn't exist.")
    else:
        if check_mutability(snapshot):
            immutability = ""
        else:
            immutability = "-r"
        i = find_new()
        os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-{snapshot} /.snapshots/boot/boot-{i} >/dev/null 2>&1")
        os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{snapshot} /.snapshots/etc/etc-{i} >/dev/null 2>&1")
        os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-{snapshot} /.snapshots/rootfs/snapshot-{i} >/dev/null 2>&1")
        if immutability == "": # Mark newly created snapshot as mutable too
            os.system(f"touch /.snapshots/rootfs/snapshot-{i}/usr/share/ash/mutable")
        add_node_to_level(fstree, snapshot, i)
        write_tree(fstree)
        desc = str(f"clone of {snapshot}")
        write_desc(i, desc)
        print(f"Branch {i} added to parent of {snapshot}.")
        return i

#   Recursively clone an entire tree
def clone_recursive(snapshot):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: cannot clone as tree {snapshot} doesn't exist.")
    else:
        children = return_children(fstree, snapshot)
        ch = children.copy()
        children.insert(0, snapshot)
        ntree = clone_branch(snapshot)
        new_children = ch.copy()
        new_children.insert(0, ntree)
        for child in ch:
            i = clone_under(new_children[children.index(get_parent(fstree, child))], child)
            new_children[children.index(child)] = i

#   Clone under specified parent
def clone_under(snapshot, branch):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot clone as snapshot {snapshot} doesn't exist.")
    elif not os.path.exists(f"/.snapshots/rootfs/snapshot-{branch}"):
        print(f"F: Cannot clone as snapshot {branch} doesn't exist.")
    else:
        if check_mutability(snapshot):
            immutability = ""
        else:
            immutability = "-r"
        i = find_new()
        os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-{branch} /.snapshots/boot/boot-{i} >/dev/null 2>&1")
        os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{branch} /.snapshots/etc/etc-{i} >/dev/null 2>&1")
        os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-{branch} /.snapshots/rootfs/snapshot-{i} >/dev/null 2>&1")
        if immutability == "": # Mark newly created snapshot as mutable too
            os.system(f"touch /.snapshots/rootfs/snapshot-{i}/usr/share/ash/mutable")
        add_node_to_parent(fstree, snapshot, i)
        write_tree(fstree)
        desc = str(f"clone of {branch}")
        write_desc(i, desc)
        print(f"Branch {i} added under snapshot {snapshot}.")
        return i

#   Delete tree or branch
def delete_node(snapshots, quiet):
    for snapshot in snapshots:
        if not quiet: ### NEWLY ADDED
            print(f"Are you sure you want to delete snapshot {snapshot}? (y/N)")
            choice = input("> ")
            run = True
            if choice.casefold() != "y":
                print("Aborted")
                run = False
        else: ### NEWLY ADDED
            run = True ### NEWLY ADDED
        if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
            print(f"F: Cannot delete as snapshot {snapshot} doesn't exist.")
        elif snapshot == "0":
            print("F: Changing base snapshot is not allowed.")
        elif snapshot == get_current_snapshot():
            print("F: Cannot delete booted snapshot.")
        elif snapshot == get_next_snapshot():
            print("F: Cannot delete deployed snapshot.")
        elif run == True:
            children = return_children(fstree, snapshot)
            os.system(f"btrfs sub del /.snapshots/boot/boot-{snapshot} >/dev/null 2>&1")
            os.system(f"btrfs sub del /.snapshots/etc/etc-{snapshot} >/dev/null 2>&1")
            os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{snapshot} >/dev/null 2>&1")
            # Make sure temporary chroot directories are deleted as well
            if (os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}")):
                os.system(f"btrfs sub del /.snapshots/boot/boot-chr{snapshot} >/dev/null 2>&1")
                os.system(f"btrfs sub del /.snapshots/etc/etc-chr{snapshot} >/dev/null 2>&1")
                os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{snapshot} >/dev/null 2>&1")
            for child in children: # This deletes the node itself along with its children
                os.system(f"btrfs sub del /.snapshots/boot/boot-{child} >/dev/null 2>&1")
                os.system(f"btrfs sub del /.snapshots/etc/etc-{child} >/dev/null 2>&1")
                os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{child} >/dev/null 2>&1")
                if (os.path.exists(f"/.snapshots/rootfs/snapshot-chr{child}")):
                    os.system(f"btrfs sub del /.snapshots/boot/boot-chr{child} >/dev/null 2>&1")
                    os.system(f"btrfs sub del /.snapshots/etc/etc-chr{child} >/dev/null 2>&1")
                    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{child} >/dev/null 2>&1")
            remove_node(fstree, snapshot) # Remove node from tree or root
            write_tree(fstree)
            print(f"Snapshot {snapshot} removed.")

#   Deploy snapshot
def deploy(snapshot):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot deploy as snapshot {snapshot} doesn't exist.")
    else:
        update_boot(snapshot)
        tmp = get_tmp()
        os.system(f"btrfs sub set-default /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1") # Set default volume
        tmp_delete()
        if "deploy-aux" in tmp:
            tmp = "deploy"
        else:
            tmp = "deploy-aux"
        etc = snapshot
        options = per_snap_options(snapshot)
        mutable_dirs = options["mutable_dirs"].split(',').remove('')
        mutable_dirs_shared = options["mutable_dirs_shared"].split(',').remove('')
        os.system(f"btrfs sub snap /.snapshots/boot/boot-{snapshot} /.snapshots/boot/boot-{tmp} >/dev/null 2>&1")
        os.system(f"btrfs sub snap /.snapshots/etc/etc-{snapshot} /.snapshots/etc/etc-{tmp} >/dev/null 2>&1")
        os.system(f"btrfs sub snap /.snapshots/rootfs/snapshot-{snapshot} /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1")
        os.system(f"mkdir /.snapshots/rootfs/snapshot-{tmp}/boot >/dev/null 2>&1")
        os.system(f"mkdir /.snapshots/rootfs/snapshot-{tmp}/etc >/dev/null 2>&1")
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-{tmp}/var >/dev/null 2>&1")
        os.system(f"cp --reflink=auto -r /.snapshots/boot/boot-{etc}/* /.snapshots/rootfs/snapshot-{tmp}/boot >/dev/null 2>&1")
        os.system(f"cp --reflink=auto -r /.snapshots/etc/etc-{etc}/* /.snapshots/rootfs/snapshot-{tmp}/etc >/dev/null 2>&1")
      # If snapshot is mutable, modify '/' entry in fstab to read-write
        if check_mutability(snapshot):
            os.system(f"sed -i '0,/snapshot-{tmp}/ s|,ro||' /.snapshots/rootfs/snapshot-{tmp}/etc/fstab") ### ,rw
      # Add special user-defined mutable directories as bind-mounts into fstab
        for mount_path in mutable_dirs:
            source_path = f"/.snapshots/mutable_dirs/snapshot-{snapshot}/{mount_path}"
            os.system(f"mkdir -p /.snapshots/mutable_dirs/snapshot-{snapshot}/{mount_path}")
            os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{tmp}/{mount_path}")
            os.system(f"echo '{source_path} {mount_path} none defaults,bind 0 0' >> /.snapshots/rootfs/snapshot-{tmp}/etc/fstab")
      # Same thing but for shared directories
        for mount_path in mutable_dirs_shared:
            source_path = f"/.snapshots/mutable_dirs/{mount_path}"
            os.system(f"mkdir -p /.snapshots/mutable_dirs/{mount_path}")
            os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{tmp}/{mount_path}")
            os.system(f"echo '{source_path} {mount_path} none defaults,bind 0 0' >> /.snapshots/rootfs/snapshot-{tmp}/etc/fstab")
        os.system(f"btrfs sub snap /var /.snapshots/rootfs/snapshot-{tmp}/var >/dev/null 2>&1") ### Is this needed?
        os.system(f"echo '{snapshot}' > /.snapshots/rootfs/snapshot-{tmp}/usr/share/ash/snap")
        switch_tmp()
        init_system_clean(tmp, "deploy")
        os.system(f"btrfs sub set-default /.snapshots/rootfs/snapshot-{tmp}") # Set default volume
        print(f"Snapshot {snapshot} deployed to /.")

#   Add node to branch
def extend_branch(snapshot, desc=""): # blank description if nothing is passed
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot branch as snapshot {snapshot} doesn't exist.")
    else:
        if check_mutability(snapshot):
            immutability = ""
        else:
            immutability = "-r"
        i = find_new()
        os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-{snapshot} /.snapshots/boot/boot-{i} >/dev/null 2>&1")
        os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{snapshot} /.snapshots/etc/etc-{i} >/dev/null 2>&1")
        os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-{snapshot} /.snapshots/rootfs/snapshot-{i} >/dev/null 2>&1")
        if immutability == "": # Mark newly created snapshot as mutable too
            os.system(f"touch /.snapshots/rootfs/snapshot-{i}/usr/share/ash/mutable")
        add_node_to_parent(fstree, snapshot, i)
        write_tree(fstree)
        if desc:
            write_desc(i, " ".join(desc))
        print(f"Branch {i} added under snapshot {snapshot}.")

# Find new unused snapshot dir
def find_new():
    i = 0
    boots = os.listdir("/.snapshots/boot")
    etcs = os.listdir("/.snapshots/etc")
    snapshots = os.listdir("/.snapshots/rootfs")
    snapshots.append(etcs)
    snapshots.append(vars) ### Can this be deleted?
    snapshots.append(boots)
    while True:
        i += 1
        if str(f"snapshot-{i}") not in snapshots and str(f"etc-{i}") not in snapshots and str(f"var-{i}") not in snapshots and str(f"boot-{i}") not in snapshots:
            return(i)

#   This function returns either empty string or underscore plus name of distro if it was appended to sub-volume names to distinguish
def get_distro_suffix():
    if "ashos" in distro:
        return f'_{distro.replace("_ashos", "")}'
    else:
        return ""

#   Get parent
def get_parent(tree, id):
    par = (find(tree, filter_=lambda node: ("x"+str(node.name)+"x") in ("x"+str(id)+"x")))
    return(par.parent.name)

#   Get drive partition
def get_part():
    with open("/.snapshots/ash/part", "r") as cpart:
        return subprocess.check_output(f"blkid | grep '{cpart.read().rstrip()}' | awk -F: '{{print $1}}'", shell=True).decode('utf-8').strip()

#   Get current snapshot
def get_current_snapshot():
    with open("/usr/share/ash/snap", "r") as csnapshot:
        return csnapshot.read().rstrip()

#   Get deployed snapshot
def get_next_snapshot():
    if "deploy-aux" in get_tmp():
        d = "deploy"
    else:
        d = "deploy-aux"
    with open(f"/.snapshots/rootfs/snapshot-{d}/usr/share/ash/snap", "r") as csnapshot:
        return csnapshot.read().rstrip()

#   Get tmp partition state
def get_tmp(console=False): # By default just return which deployment is running
    mount = str(subprocess.check_output("cat /proc/mounts | grep ' / btrfs'", shell=True))
    if "deploy-aux" in mount:
        r = "deploy-aux"
    else:
        r = "deploy"
    if console:
        print(r)
    else:
        return r

#   Make a node mutable
def immutability_disable(snapshot):
    if snapshot != "0":
        if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
            print(f"F: Snapshot {snapshot} doesn't exist.")
        else:
            if check_mutability(snapshot):
                print(f"F: Snapshot {snapshot} is already mutable.")
            else:
                os.system(f"btrfs property set -ts /.snapshots/rootfs/snapshot-{snapshot} ro false")
                os.system(f"touch /.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable")
                print(f"Snapshot {snapshot} successfully made mutable.")
                write_desc(snapshot, " MUTABLE ", 'a+')
    else:
        print("F: Snapshot 0 (base) should not be modified.")

#   Make a node immutable
def immutability_enable(snapshot):
    if snapshot != "0":
        if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
            print(f"F: Snapshot {snapshot} doesn't exist.")
        else:
            if not check_mutability(snapshot):
                print(f"F: Snapshot {snapshot} is already immutable.")
            else:
                os.system(f"rm /.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable")
                os.system(f"btrfs property set -ts /.snapshots/rootfs/snapshot-{snapshot} ro true")
                print(f"Snapshot {snapshot} successfully made immutable.")
                os.system(f"sed -i 's| MUTABLE ||g' /.snapshots/ash/snapshots/{snapshot}-desc")
    else:
        print("F: Snapshot 0 (base) should not be modified.")

#   Import filesystem tree file
def import_tree_file(treename):
    with open(treename, "r") as treefile:
        return literal_eval(treefile.readline())

#   Install packages
def install(snapshot, pkg):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot install as snapshot {snapshot} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"): # Make sure snapshot is not in use by another ash process
        print(f"F: Snapshot {snapshot} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {snapshot}'.")
    elif snapshot == "0":
        print("F: Changing base snapshot is not allowed.")
    else:
        prepare(snapshot)
        try:
            install_package(snapshot, pkg)
        except Exception:
            chr_delete(snapshot)
            print("F: Install failed and changes discarded.")
        else:
            post_transactions(snapshot)
            print(f"Package(s) {pkg} installed in snapshot {snapshot} successfully.")

#   Install live
def install_live(pkg): ### add snapshot arg so live install can be done on none-active
    tmp = get_tmp()
#    os.system(f"mount --bind /.snapshots/rootfs/snapshot-{tmp} /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1")
#    os.system(f"mount --bind /home /.snapshots/rootfs/snapshot-{tmp}/home >/dev/null 2>&1")
#    os.system(f"mount --bind /var /.snapshots/rootfs/snapshot-{tmp}/var >/dev/null 2>&1")
#    os.system(f"mount --bind /etc /.snapshots/rootfs/snapshot-{tmp}/etc >/dev/null 2>&1")
#    os.system(f"mount --bind /tmp /.snapshots/rootfs/snapshot-{tmp}/tmp >/dev/null 2>&1")
    ash_chroot_mounts(tmp)
    print("Please wait as installation is finishing.")
    install_package_live(tmp, pkg)
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/* >/dev/null 2>&1")
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1") ### REVIEW_LATER not safe

#   Install profile in live snapshot
def install_profile_live(profile):
    tmp = get_tmp()
    ash_chroot_mounts(tmp)
    print(f"Updating the system before installing profile {profile}.")
    auto_upgrade(tmp)
    tmp_prof = subprocess.check_output("mktemp -d -p /tmp ashpk_profile.XXXXXXXXXXXXXXXX", shell=True, encoding='utf-8').strip()
    subprocess.check_output(f"curl --fail -o {tmp_prof}/packages.txt -LO https://raw.githubusercontent.com/ashos/ashos/main/src/profiles/{profile}/packages{get_distro_suffix()}.txt", shell=True)
  # Ignore empty lines or ones starting with # [ % &
    pkg = subprocess.check_output(f"cat {tmp_prof}/packages.txt | grep -E -v '^#|^\[|^%|^$'", shell=True).decode('utf-8').strip().replace('\n', ' ')
    excode1 = install_package_live(tmp, pkg)
    excode2 = service_enable(tmp, profile, tmp_prof)
    if excode1 == 0 and excode2 == 0:
        print(f"Profile {profile} installed in current/live snapshot.") ###
    else:
        print("F: Install failed and changes discarded.")
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/* >/dev/null 2>&1")
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1")

#   Install a profile from a text file
def install_profile(snapshot, profile):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot install as snapshot {snapshot} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"): # Make sure snapshot is not in use by another ash process
        print(f"F: Snapshot {snapshot} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {snapshot}'.")
    elif snapshot == "0":
        print("F: Changing base snapshot is not allowed.")
    else:
        print(f"Updating the system before installing profile {profile}.")
        auto_upgrade(snapshot)
        tmp_prof = subprocess.check_output("mktemp -d -p /tmp ashpk_profile.XXXXXXXXXXXXXXXX", shell=True, encoding='utf-8').strip()
        subprocess.check_output(f"curl --fail -o {tmp_prof}/packages.txt -LO https://raw.githubusercontent.com/ashos/ashos/main/src/profiles/{profile}/packages{get_distro_suffix()}.txt", shell=True)
        prepare(snapshot)
        try: # Ignore empty lines or ones starting with # [ % &
            pkg = subprocess.check_output(f"cat {tmp_prof}/packages.txt | grep -E -v '^#|^\[|^%|^&|^$'", shell=True).decode('utf-8').strip().replace('\n', ' ')
            install_package(snapshot, pkg)
            service_enable(snapshot, profile, tmp_prof)
        except subprocess.CalledProcessError as e:
            chr_delete(snapshot)
            print("F: Install failed and changes discarded.")
            sys.exit(1)
        else:
            post_transactions(snapshot)
            print(f"Profile {profile} installed in snapshot {snapshot} successfully.")
            print(f"Deploying snapshot {snapshot}.")
            deploy(snapshot)

def is_efi():
    return os.path.exists("/sys/firmware/efi")

#   List sub-volumes for the booted distro only
def list_subvolumes():
    os.system(f"btrfs sub list / | grep -i {get_distro_suffix()} | sort -f -k 9")

#   Live unlocked shell
def live_unlock():
    tmp = get_tmp()
    os.system(f"mount --bind /.snapshots/rootfs/snapshot-{tmp} /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1")
    os.system(f"mount --bind /etc /.snapshots/rootfs/snapshot-{tmp}/etc >/dev/null 2>&1")
    os.system(f"mount --bind /home /.snapshots/rootfs/snapshot-{tmp}/home >/dev/null 2>&1")
    os.system(f"mount --bind /tmp /.snapshots/rootfs/snapshot-{tmp}/tmp >/dev/null 2>&1")
    os.system(f"mount --bind /var /.snapshots/rootfs/snapshot-{tmp}/var >/dev/null 2>&1")
    os.system(f"chroot /.snapshots/rootfs/snapshot-{tmp}")
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/* >/dev/null 2>&1")
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1")

#   Creates new tree from base file
def new_snapshot(desc="clone of base"): # immutability toggle not used as base should always be immutable
    i = find_new()
    os.system(f"btrfs sub snap -r /.snapshots/boot/boot-0 /.snapshots/boot/boot-{i} >/dev/null 2>&1")
    os.system(f"btrfs sub snap -r /.snapshots/etc/etc-0 /.snapshots/etc/etc-{i} >/dev/null 2>&1")
    os.system(f"btrfs sub snap -r /.snapshots/rootfs/snapshot-0 /.snapshots/rootfs/snapshot-{i} >/dev/null 2>&1")
    append_base_tree(fstree, i)
    write_tree(fstree)
    if desc:
        write_desc(i, desc)
    print(f"New tree {i} created.")

#   Get per-snapshot configuration
def per_snap_options(snap):
    options = {"aur":"False","mutable_dirs":"","mutable_dirs_shared":""} # defaults here
    if not os.path.exists(f"/.snapshots/etc/etc-{snap}/ast.conf"):
        return options
    with open(f"/.snapshots/etc/etc-{snap}/ast.conf", "r") as optfile:
        for line in optfile:
            if '#' in line:
                line = line.split('#')[0] # Everything after '#' is a comment
            if '::' in line: # Skip line if there's no option set
                left, right = line.split("::") # Split options with '::'
                options[left] = right[:-1] # Remove newline here
    return options

#   Edit per-snapshot configuration
def per_snap_conf(snapshot):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot chroot as snapshot {snapshot} doesn't exist.")
    elif snapshot == "0":
        print("F: Changing base snapshot is not allowed.")
    else:
        prepare(snapshot)
        os.system(f"$EDITOR /.snapshots/rootfs/snapshot-chr{snapshot}/etc/ast.conf")
        posttrans(snapshot)

#   Post transaction function, copy from chroot dirs back to read only snapshot dir
def post_transactions(snapshot):
    etc = snapshot
    tmp = get_tmp()
  # Unmount in reverse order
    os.system(f"umount /.snapshots/rootfs/snapshot-chr{snapshot}/etc/resolv.conf >/dev/null 2>&1")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/dev >/dev/null 2>&1")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/home >/dev/null 2>&1")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/proc >/dev/null 2>&1")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/root >/dev/null 2>&1")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/run >/dev/null 2>&1")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/sys >/dev/null 2>&1")
    os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot} >/dev/null 2>&1")
  # For special mutable dirs
    options = per_snap_options(snapshot)
    mutable_dirs = options["mutable_dirs"].split(',').remove('')
    mutable_dirs_shared = options["mutable_dirs_shared"].split(',').remove('')
    for mount_path in mutable_dirs:
        os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/{mount_path} >/dev/null 2>&1")
    for mount_path in mutable_dirs_shared:
        os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{snapshot}/{mount_path} >/dev/null 2>&1")
  # File operations in snapshot-chr
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{snapshot} >/dev/null 2>&1")
    os.system(f"rm -rf /.snapshots/boot/boot-chr{snapshot}/* >/dev/null 2>&1")
    os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/boot/* /.snapshots/boot/boot-chr{snapshot} >/dev/null 2>&1")
    os.system(f"rm -rf /.snapshots/etc/etc-chr{snapshot}/* >/dev/null 2>&1")
    os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/etc/* /.snapshots/etc/etc-chr{snapshot} >/dev/null 2>&1")
  # Keep package manager's cache after installing packages. This prevents unnecessary downloads for each snapshot when upgrading multiple snapshots
    cache_copy(snapshot, "post_transactions")
    os.system(f"btrfs sub del /.snapshots/boot/boot-{etc} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/etc/etc-{etc} >/dev/null 2>&1")
    if os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}/usr/share/ash/mutable"):
        immutability = ""
    else:
        immutability = "-r"
    os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-chr{snapshot} /.snapshots/boot/boot-{etc} >/dev/null 2>&1")
    os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-chr{snapshot} /.snapshots/etc/etc-{etc} >/dev/null 2>&1")
  # Copy init system files to shared
    init_system_copy(tmp, "post_transactions")
    os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-chr{snapshot} /.snapshots/rootfs/snapshot-{snapshot} >/dev/null 2>&1")
    chr_delete(snapshot)

#   Prepare snapshot to chroot dir to install or chroot into
def prepare(snapshot):
    chr_delete(snapshot)
    os.system(f"btrfs sub snap /.snapshots/rootfs/snapshot-{snapshot} /.snapshots/rootfs/snapshot-chr{snapshot} >/dev/null 2>&1")
    os.system(f"btrfs sub snap /.snapshots/etc/etc-{snapshot} /.snapshots/etc/etc-chr{snapshot} >/dev/null 2>&1")
  # Pacman gets weird when chroot directory is not a mountpoint, so the following mount is necessary ### REVIEW
    os.system(f"mount --bind --make-slave /.snapshots/rootfs/snapshot-chr{snapshot} /.snapshots/rootfs/snapshot-chr{snapshot} >/dev/null 2>&1")
    os.system(f"mount --rbind --make-rslave /dev /.snapshots/rootfs/snapshot-chr{snapshot}/dev >/dev/null 2>&1")
    os.system(f"mount --bind --make-slave /home /.snapshots/rootfs/snapshot-chr{snapshot}/home >/dev/null 2>&1")
    os.system(f"mount --rbind --make-rslave /proc /.snapshots/rootfs/snapshot-chr{snapshot}/proc >/dev/null 2>&1")
    os.system(f"mount --bind --make-slave /root /.snapshots/rootfs/snapshot-chr{snapshot}/root >/dev/null 2>&1")
    os.system(f"mount --rbind --make-rslave /run /.snapshots/rootfs/snapshot-chr{snapshot}/run >/dev/null 2>&1")
    os.system(f"mount --rbind --make-rslave /sys /.snapshots/rootfs/snapshot-chr{snapshot}/sys >/dev/null 2>&1")
    os.system(f"mount --rbind --make-rslave /tmp /.snapshots/rootfs/snapshot-chr{snapshot}/tmp >/dev/null 2>&1")
    os.system(f"mount --bind --make-slave /var /.snapshots/rootfs/snapshot-chr{snapshot}/var >/dev/null 2>&1")
  # File operations for snapshot-chr
    os.system(f"btrfs sub snap /.snapshots/boot/boot-{snapshot} /.snapshots/boot/boot-chr{snapshot} >/dev/null 2>&1")
    os.system(f"cp -r --reflink=auto /.snapshots/boot/boot-chr{snapshot}/* /.snapshots/rootfs/snapshot-chr{snapshot}/boot >/dev/null 2>&1")
    os.system(f"cp -r --reflink=auto /.snapshots/etc/etc-chr{snapshot}/* /.snapshots/rootfs/snapshot-chr{snapshot}/etc >/dev/null 2>&1") ### btrfs sub snap etc-{snapshot} to etc-chr-{snapshot} not needed before this?
    init_system_clean(snapshot, "prepare")
    os.system(f"cp /etc/machine-id /.snapshots/rootfs/snapshot-chr{snapshot}/etc/machine-id")
    os.system(f"mkdir -p /.snapshots/rootfs/snapshot-chr{snapshot}/.snapshots/ash && cp -f /.snapshots/ash/fstree /.snapshots/rootfs/snapshot-chr{snapshot}/.snapshots/ash/")
  # For special mutable dirs
    options = per_snap_options(snapshot)
    mutable_dirs = options["mutable_dirs"].split(',').remove('')
    mutable_dirs_shared = options["mutable_dirs_shared"].split(',').remove('')
    for mount_path in mutable_dirs:
        os.system(f"mkdir -p /.snapshots/mutable_dirs/snapshot-{snapshot}/{mount_path}")
        os.system(f"mkdir -p /.snapshots/rootfs/snapshot-chr{snapshot}/{mount_path}")
        os.system(f"mount --bind /.snapshots/mutable_dirs/snapshot-{snapshot}/{mount_path} /.snapshots/rootfs/snapshot-chr{snapshot}/{mount_path}")
    for mount_path in mutable_dirs_shared:
        os.system(f"mkdir -p /.snapshots/mutable_dirs/{mount_path}")
        os.system(f"mkdir -p /.snapshots/rootfs/snapshot-chr{snapshot}/{mount_path}")
        os.system(f"mount --bind /.snapshots/mutable_dirs/{mount_path} /.snapshots/rootfs/snapshot-chr{snapshot}/{mount_path}")
  # Important: Do not move the following line above (otherwise error)
    os.system(f"mount --bind --make-slave /etc/resolv.conf /.snapshots/rootfs/snapshot-chr{snapshot}/etc/resolv.conf >/dev/null 2>&1")

#   Print out tree with descriptions
def print_tree(tree):
    snapshot = get_current_snapshot()
    for pre, fill, node in RenderTree(tree, style=AsciiStyle()):
        if os.path.isfile(f"/.snapshots/ash/snapshots/{node.name}-desc"):
            with open(f"/.snapshots/ash/snapshots/{node.name}-desc", "r") as descfile:
                desc = descfile.readline()
        else:
            desc = ""
        if str(node.name) == "0":
            desc = "base snapshot"
        if snapshot != str(node.name):
            print("%s%s - %s" % (pre, node.name, desc))
        else:
            print("%s%s*- %s" % (pre, node.name, desc))

#   Return order to recurse tree
def recurse_tree(tree, cid):
    order = []
    for child in (return_children(tree, cid)):
        par = get_parent(tree, child)
        if child != cid:
            order.append(par)
            order.append(child)
    return (order)

#   Recursively remove package in tree
def remove_from_tree(tree, treename, pkg, profile):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{treename}"):
        print(f"F: Cannot update as tree {treename} doesn't exist.")
    else:
        if pkg: ### NEW
            uninstall_package(treename, pkg)
            order = recurse_tree(tree, treename)
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
                uninstall_package(sarg, pkg)
            print(f"Tree {treename} updated.")
        elif profile:
            print("TODO") ### REVIEW_LATER

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
    clone_as_tree(tmp)
    write_desc(i, "rollback")
    deploy(i)

#   Recursively run an update in tree
def run_tree(tree, treename, cmd):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{treename}"):
        print(f"F: Cannot update as tree {treename} doesn't exist.")
    else:
        prepare(treename)
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{treename} {cmd}")
        post_transactions(treename)
        order = recurse_tree(tree, treename)
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
                print(f"F: Snapshot {sarg} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {sarg}'.")
                print("Tree command canceled.")
                return
            else:
                prepare(sarg)
                os.system(f"chroot /.snapshots/rootfs/snapshot-chr{sarg} {cmd}")
                post_transactions(sarg)
        print(f"Tree {treename} updated.")

#   Enable service(s) (Systemd, OpenRC, etc.)
def service_enable(snapshot, profile, tmp_prof):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot enable services as snapshot {snapshot} doesn't exist.")
    else: ### No need for other checks as this function is not exposed to user
        try:
            postinst = subprocess.check_output(f"cat {tmp_prof}/packages.txt | grep -E -w '^&' | sed 's|& ||'", shell=True).decode('utf-8').strip().split('\n')
            for cmd in list(filter(None, postinst)): # remove '' from [''] if no postinstalls
                subprocess.check_output(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} {cmd}", shell=True)
            services = subprocess.check_output(f"cat {tmp_prof}/packages.txt | grep -E -w '^%' | sed 's|% ||'", shell=True).decode('utf-8').strip().split('\n')
            for cmd in list(filter(None, services)): # remove '' from [''] if no services
                subprocess.check_output(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} {cmd}", shell=True)
        except subprocess.CalledProcessError as e:
            print(f"F: Failed to enable service(s) from {profile}: {e.output}.")
            return 1
        else:
            print(f"Installed service(s) from {profile}.")
            return 0

#   Calls print function
def show_fstree():
    print_tree(fstree)

#   Remove temporary chroot for specified snapshot only
#   This unlocks the snapshot for use by other functions
def snapshot_unlock(snap):
    os.system(f"btrfs sub del /.snapshots/boot/boot-chr{snap} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/etc/etc-chr{snap} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{snap} >/dev/null 2>&1")

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
            with open('/boot/efi/EFI/map.txt', 'r') as f:
                input_file = csv.DictReader(f, delimiter=',', quoting=csv.QUOTE_NONE)
                for row in input_file:
                    if row["DISTRO"] == next_distro:
                        try:
                            boot_order = subprocess.check_output("efibootmgr | grep BootOrder | awk '{print $2}'", shell=True).decode('utf-8').strip()
                            temp = boot_order.replace(f'{row["BootOrder"]},', "")
                            new_boot_order = f"{row['BootOrder']},{temp}"
                            subprocess.check_output(f'efibootmgr --bootorder {new_boot_order} >/dev/null 2>&1', shell=True)
                        except subprocess.CalledProcessError as e:
                            print(f"F: Failed to switch distros: {e.output}.") ###
                        else:
                            print(f'Done! Please reboot whenever you would like switch to {next_distro}')
                        #break ### REVIEW_LATER
            break
        else:
            print("Invalid distro!")
            continue

#   Switch between /tmp deployments
def switch_tmp():
    distro_suffix = get_distro_suffix()
    part = get_part()
    tmp_boot = subprocess.check_output("mktemp -d -p /.snapshots/tmp boot.XXXXXXXXXXXXXXXX", shell=True).decode('utf-8').strip()
    os.system(f"mount {part} -o subvol=@boot{distro_suffix} {tmp_boot}") # Mount boot partition for writing
  # Swap deployment subvolumes: deploy <-> deploy-aux
    if "deploy-aux" in get_tmp():
        source_dep = "deploy-aux"
        target_dep = "deploy"
    else:
        source_dep = "deploy"
        target_dep = "deploy-aux"
    os.system(f"cp --reflink=auto -r /.snapshots/rootfs/snapshot-{target_dep}/boot/* {tmp_boot}")
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
                gconf = sub('snapshot \d', '', gconf)
                gconf = gconf.replace(f"{distro_name}", f"{distro_name} last booted deployment (snapshot {snap})")
        os.system(f"sed -i '$ d' {boot_location}/{GRUB}/grub.cfg")
        with open(f"{boot_location}/{GRUB}/grub.cfg", "a") as grubconf:
            grubconf.write(gconf)
            grubconf.write("}\n")
            grubconf.write("### END /etc/grub.d/41_custom ###")
    os.system(f"umount {tmp_boot} >/dev/null 2>&1")

# Sync time
def sync_time():
    if not os.system('[ -x "$(command -v wget)" ]'): # wget available
        os.system('sudo date -s "$(wget -qSO- --max-redirect=0 google.com 2>&1 | grep Date: | cut -d" " -f5-8)Z"')
    elif not os.system('[ -x "$(command -v curl)" ]'): # curl available
        os.system('sudo date -s "$(curl -I google.com 2>&1 | grep Date: | cut -d" " -f3-6)Z"')

#   Sync tree and all its snapshots
def sync_tree(tree, treename, force_offline):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{treename}"):
        print(f"F: Cannot sync as tree {treename} doesn't exist.")
    else:
        if not force_offline: # Syncing tree automatically updates it, unless 'force-sync' is used
            update_tree(tree, treename)
        order = recurse_tree(tree, treename)
        if len(order) > 2:
            order.remove(order[0]) ### I do not like these repetetitve removes
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
                print(f"F: Snapshot {sarg} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {sarg}'.")
                print("Tree sync canceled.")
                return
            else:
                prepare(sarg)
                os.system(f"cp --reflink=auto -n -r /.snapshots/rootfs/snapshot-{arg}/* /.snapshots/rootfs/snapshot-chr{sarg}/ >/dev/null 2>&1")
                #os.system(f"cp --reflink=auto -r /.snapshots/rootfs/snapshot-{arg}/etc/* /.snapshots/rootfs/snapshot-chr{sarg}/etc/ >/dev/null 2>&1") ### Commented out due to causing issues
                post_transactions(sarg)
        print(f"Tree {treename} synced.")

#   Clear all temporary snapshots
def tmp_clear():
    os.system("btrfs sub del /.snapshots/boot/boot-chr* >/dev/null 2>&1")
    os.system("btrfs sub del /.snapshots/etc/etc-chr* >/dev/null 2>&1")
    os.system("btrfs sub del /.snapshots/rootfs/snapshot-chr*/* >/dev/null 2>&1")
    os.system("btrfs sub del /.snapshots/rootfs/snapshot-chr* >/dev/null 2>&1")

#   Clean tmp dirs
def tmp_delete():
    tmp = get_tmp()
    if "deploy-aux" in tmp:
        tmp = "deploy"
    else:
        tmp = "deploy-aux"
    os.system(f"btrfs sub del /.snapshots/boot/boot-{tmp} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/etc/etc-{tmp} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{tmp}/* >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1")

#   Update boot
def update_boot(snapshot):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}"):
        print(f"F: Cannot update boot as snapshot {snapshot} doesn't exist.")
    else:
        tmp = get_tmp()
        part = get_part()
        prepare(snapshot)
        if os.path.exists(f"/boot/{GRUB}/BAK/"):
            os.system(f"find /boot/{GRUB}/BAK/. -mtime +30 -exec rm -rf" + " {} \;") # Delete 30-day-old grub.cfg.DATE files
        os.system(f"cp /boot/{GRUB}/grub.cfg /boot/{GRUB}/BAK/grub.cfg.`date '+%Y%m%d-%H%M%S'`")
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} {GRUB}-mkconfig {part} -o /boot/{GRUB}/grub.cfg")
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} sed -i 's|snapshot-chr{snapshot}|snapshot-{tmp}|g' /boot/{GRUB}/grub.cfg")
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} sed -i '0,\|{distro_name}| s||{distro_name} snapshot {snapshot}|' /boot/{GRUB}/grub.cfg")
        post_transactions(snapshot)

#   Saves changes made to /etc to snapshot
def update_etc():
    tmp = get_tmp()
    snapshot = get_current_snapshot()
    os.system(f"btrfs sub del /.snapshots/etc/etc-{snapshot} >/dev/null 2>&1")
    if check_mutability(snapshot):
        immutability = ""
    else:
        immutability = "-r"
    os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{tmp} /.snapshots/etc/etc-{snapshot} >/dev/null 2>&1")

#   Recursively run an update in tree
def update_tree(tree, treename):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{treename}"):
        print(f"F: Cannot update as tree {treename} doesn't exist.")
    else:
        upgrade(treename)
        order = recurse_tree(tree, treename)
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
        print(f"Tree {treename} updated.")

#   Write new description (default) or append to an existing one (i.e. toggle immutability)
def write_desc(snapshot, desc, mode='w'):
    with open(f"/.snapshots/ash/snapshots/{snapshot}-desc", mode) as descfile:
        descfile.write(desc)

#   Save tree to file
def write_tree(tree):
    exporter = DictExporter()
    to_write = exporter.export(tree)
    with open(fstreepath, "w") as fsfile:
        fsfile.write(str(to_write))

#   Main function
def main():
    if os.geteuid() != 0:
        exit("sudo/doas is required to run ash!")
    else:
        importer = DictImporter() # Dict importer
#        isChroot = chroot_check() ### NEEDS TO BE IMPLEMENTED
        global fstree # Currently these are global variables, fix sometime
        global fstreepath # ---
        fstreepath = str("/.snapshots/ash/fstree") # Path to fstree file
        fstree = importer.import_(import_tree_file("/.snapshots/ash/fstree")) # Import fstree file
        #if isChroot == True and ("--chroot" not in args): ### LATER
        #    print("Please don't use ash inside a chroot!") ### LATER
      # Recognize argument and call appropriate function
        parser = ArgumentParser(prog='ash', description='Any Snapshot Hierarchical OS')
        subparsers = parser.add_subparsers(dest='command', required=True, help='Different commands for ash')
      # Ash version
        ashv_par = subparsers.add_parser("version", help="Print ash version")
        ashv_par.set_defaults(func=ash_version)
      # Auto upgrade
        autou_par = subparsers.add_parser("auto-upgrade", aliases=['autoup', 'au'], allow_abbrev=True, help='Update a snapshot quietly')
        autou_par.add_argument("snapshot", type=int, help="snapshot number") ### REVIEW_LATER any given snapshot or get_current_snapshot() ?
        autou_par.set_defaults(func=auto_upgrade)
      # boot update command
        boot_par = subparsers.add_parser("boot", aliases=['boot-update'], allow_abbrev=True, help='update boot of a snapshot')
        boot_par.add_argument("snapshot", type=int, help="snapshot number")
        boot_par.set_defaults(func=update_boot)
      # check update
        cu_par = subparsers.add_parser("check", help="Check update")
        cu_par.set_defaults(func=check_update)
      # base update
        bu_par = subparsers.add_parser("base-update", aliases=['bu'], allow_abbrev=True, help='Update the base snapshot')
        bu_par.set_defaults(func=lambda: upgrade("0", True))
      # chroot
        chroot_par = subparsers.add_parser("chroot", aliases=['chr', 'ch'], allow_abbrev=True, help='Open a root shell inside a snapshot')
        chroot_par.add_argument("snapshot", type=int, help="snapshot number")
        chroot_par.set_defaults(func=chroot)
      # clone
        clone_par = subparsers.add_parser("clone", aliases=['cl'], allow_abbrev=True, help='Create a copy of a snapshot (as top-level tree node)')
        clone_par.add_argument("snapshot", type=int, help="snapshot number")
        clone_par.add_argument("--desc", "--description", "-d", nargs='+', required=False, help="description for the snapshot") ### required=False
        clone_par.set_defaults(func=clone_as_tree)
      # clone a branch
        clonebr_par = subparsers.add_parser("clone-branch", aliases=['cb'], allow_abbrev=True, help='Copy snapshot under same parent branch (clone as a branch)')
        clonebr_par.add_argument("snapshot", type=int, help="snapshot number")
        clonebr_par.set_defaults(func=clone_branch)
      # clone recursively
        clonerec_par = subparsers.add_parser("clone-tree", aliases=['ct'], allow_abbrev=True, help='clone a whole tree recursively')
        clonerec_par.add_argument("snapshot", type=int, help="snapshot number")
        clonerec_par.set_defaults(func=clone_recursive)
      # clone under a branch
        cloneunder_par = subparsers.add_parser("clone-under", aliases=['cu', 'ubranch'], allow_abbrev=True, help='Copy snapshot under specified parent (clone under a branch)')
        cloneunder_par.add_argument("snapshot", type=int, help="snapshot number")
        cloneunder_par.add_argument("branch", type=int, help="branch number")
        cloneunder_par.set_defaults(func=clone_under)
      # current snapshot
        cs_par = subparsers.add_parser("current", aliases=['c'], allow_abbrev=True, help='Show current snapshot number')
        cs_par.set_defaults(func=get_current_snapshot)
      # branch
        branch_par = subparsers.add_parser("branch", aliases=['add-branch'], allow_abbrev=True, help='Create a new branch from snapshot')
        branch_par.add_argument("snapshot", type=int, help="snapshot number")
        branch_par.add_argument("--desc", "--description", "-d", nargs='+', required=False, help="description for the snapshot") ### required=False
        branch_par.set_defaults(func=extend_branch)
      # deploy
        dep_par = subparsers.add_parser("deploy", aliases=['dep', 'd'], allow_abbrev=True, help='Deploy a snapshot for next boot')
        dep_par.add_argument("snapshot", type=int, help="snapshot number")
        dep_par.set_defaults(func=deploy)
      # diff two snapshots
        diff_par = subparsers.add_parser("diff", aliases=['dif'], allow_abbrev=True, help='Show package diff between snapshots')
        diff_par.add_argument("snap1", type=int, help="Source snapshot")
        diff_par.add_argument("snap2", type=int, help="Target snapshot")
        diff_par.set_defaults(func=snapshot_diff)
      # etc update
        etc_par = subparsers.add_parser("etc-update", aliases=['etc'], allow_abbrev=True, help='update /etc')
        etc_par.set_defaults(func=update_etc)
      # fix db command ### MAYBE ash_unlock was needed?
        fixdb_par = subparsers.add_parser("fixdb", aliases=['fix'], allow_abbrev=True, help='fix package database of a snapshot')
        fixdb_par.add_argument("snapshot", type=int, help="snapshot number")
        fixdb_par.set_defaults(func=fix_package_db)
      # immutability disable
        immdis_par = subparsers.add_parser("immdis", aliases=["disimm", "immdisable", "disableimm"], allow_abbrev=True, help='Disable immutability of a snapshot')
        immdis_par.add_argument("snapshot", type=int, help="snapshot number")
        immdis_par.set_defaults(func=immutability_disable)
      # immutability enable
        immen_par = subparsers.add_parser("immen", aliases=["enimm", "immenable", "enableimm"], allow_abbrev=True, help='Enable immutability of a snapshot')
        immen_par.add_argument("snapshot", type=int, help="snapshot number")
        immen_par.set_defaults(func=immutability_enable)
      # install command
        inst_par = subparsers.add_parser("install", aliases=['in'], allow_abbrev=True, help='install package(s) inside a snapshot')
        inst_par.add_argument("snapshot", type=int, help="snapshot number")
        g1i = inst_par.add_mutually_exclusive_group(required=True)
        g1i.add_argument('--pkg', '--package', '-p', nargs='+', required=False, help='install package') ### switch pkg and package
        g1i.add_argument('--profile', '-P', type=str, required=False, help='install profile')
        g2i = inst_par.add_mutually_exclusive_group(required=False)
        g2i.add_argument('--live', '-l', action='store_true', required=False, help='make snapshot install live')
        g2i.add_argument('--not-live', '-nl', action='store_false', required=False, help='make snapshot install not live')
        inst_par.set_defaults(func=triage_install)
      # live chroot
        lc_par = subparsers.add_parser("live-chroot", aliases=['lchroot', 'lc'], allow_abbrev=True, help='Open a shell inside currently booted snapshot with read-write access. Changes are discarded on new deployment.')
        lc_par.set_defaults(func=live_unlock)
      # new
        new_par = subparsers.add_parser("new", aliases=['new-tree'], allow_abbrev=True, help='Create a new base snapshot')
        new_par.add_argument("--desc", "--description", "-d", nargs='+', required=False, help="description for the snapshot") ### required=False
        new_par.set_defaults(func=new_snapshot)
      # upself
        upself_par = subparsers.add_parser("upself", aliases=['ash-update'], allow_abbrev=True, help="Update ash itself")
        upself_par.set_defaults(func=ash_update)
      # del
        del_par = subparsers.add_parser("del", aliases=["delete", "rem", "remove"], allow_abbrev=True, help="Remove snapshot(s)/tree(s) and any branches recursively")
        del_par.add_argument("snapshots", nargs='+', help="snapshot number")
        del_par.add_argument('--quiet', '-q', action='store_true', required=False, help='Force delete snapshot(s)')
        del_par.set_defaults(func=delete_node)
      # description
        desc_par = subparsers.add_parser("desc", help='set a description for a snapshot')
        desc_par.add_argument("snapshot", type=int, help="snapshot number")
        desc_par.add_argument("desc", nargs='+', help="description to be added")
        desc_par.set_defaults(func=lambda snapshot, desc: write_desc(snapshot, " ".join(desc)))
      # "refresh", "ref"
        ref_par = subparsers.add_parser("refresh", aliases=["ref"], allow_abbrev=True, help='Refresh package manager db of a snapshot')
        ref_par.add_argument("snapshot", type=int, help="snapshot number")
        ref_par.set_defaults(func=refresh)
      # run a command
        run_par = subparsers.add_parser("run", help='Run command(s) inside another snapshot (chrooted)')
        run_par.add_argument("snapshot", type=int, help="snapshot number")
        run_par.add_argument("cmd", nargs='+', help="command")
        run_par.set_defaults(func=chr_run)
      # rollback
        roll_par = subparsers.add_parser("rollback", help="Revert the deployment to the last booted snapshot")
        roll_par.set_defaults(func=rollback)
      # subvolumes list
        sub_par = subparsers.add_parser("subs", aliases=["sub", "subvol", "subvols", "subvolumes"], allow_abbrev=True, help="List subvolumes of active snapshot (currently booted)")
        sub_par.set_defaults(func=list_subvolumes)
      # Switch distros
        switch_par = subparsers.add_parser("dist", aliases=["distro", "distros"], allow_abbrev=True, help="Switch to another distro")
        switch_par.set_defaults(func=switch_distro)
      # per-snapshot conf edit
        edit_par = subparsers.add_parser("edit", aliases=["conf-edit"], allow_abbrev=True, help="Edit snapshot configuration")
        edit_par.add_argument("snapshot", type=int, help="snapshot number")
        edit_par.set_defaults(func=lambda snapshot: per_snap_conf(snapshot))
      # tree
        tree_par = subparsers.add_parser("tree", aliases=["t"], allow_abbrev=True, help="Show ash tree")
        tree_par.set_defaults(func=show_fstree)
      # tree-remove
        trem_par = subparsers.add_parser("tremove", aliases=["tree-rmpkg"], allow_abbrev=True, help='Uninstall package(s) or profile(s) from a tree recursively')
        trem_par.add_argument("snapshot", type=int, help="snapshot number")
        g1tr = trem_par.add_mutually_exclusive_group(required=True)
        g1tr.add_argument('--pkg', '--package', '-p', nargs='+', required=False, help='package(s) to be uninstalled')
        g1tr.add_argument('--profile', '-P', type=str, required=False, help='profile(s) to be uninstalled') ###LATER nargs='+' for multiple profiles
        trem_par.set_defaults(func=lambda snapshot, pkg, profile: remove_from_tree(fstree, snapshot, pkg, profile))
      # tree-run
        trun_par = subparsers.add_parser("trun", aliases=["tree-run"], allow_abbrev=True, help='Execute command(s) inside another snapshot and all snapshots below it')
        trun_par.add_argument("snapshot", type=int, help="snapshot number")
        trun_par.add_argument('--cmd', '--command', '-c', nargs='+', required=False, help='command(s) to run')
        trun_par.set_defaults(func=lambda snapshot, cmd: run_tree(fstree, snapshot, ' '.join(cmd)))
      # tree-sync
        tsync_par = subparsers.add_parser("sync", aliases=["tree-sync", "tsync"], allow_abbrev=True, help='Sync packages and configuration changes recursively (requires an internet connection)')
        tsync_par.add_argument("treename", type=int, help="snapshot number")
        tsync_par.add_argument('-f', '--force-offline', action='store_true', required=False, help='Snapshots would not updated (potentially riskier)')
        tsync_par.set_defaults(func=lambda treename, force_offline: sync_tree(fstree, treename, force_offline))
      # tree-upgrade
        tupg_par = subparsers.add_parser("tupgrade", aliases=["tree-upgrade", "tup"], allow_abbrev=True, help='Update all packages in a snapshot recursively')
        tupg_par.add_argument("snapshot", type=int, help="snapshot number")
        tupg_par.set_defaults(func=lambda snapshot: update_tree(fstree, snapshot))
      # clear tmp
        tmpclear_par = subparsers.add_parser("tmp", aliases=["tmpclear"], allow_abbrev=True, help="Show ash tree")
        tmpclear_par.set_defaults(func=tmp_clear)
      # Uninstall/remove package(s) from a snapshot
        uninst_par = subparsers.add_parser("uninstall", aliases=["unin", "uninst", "unins", "un"], allow_abbrev=True, help='Uninstall package(s) from a snapshot')
        uninst_par.add_argument("snapshot", type=int, help="snapshot number")
        g1u = uninst_par.add_mutually_exclusive_group(required=True)
        g1u.add_argument('--pkg', '--package', '-p', nargs='+', required=False, help='package(s) to be uninstalled') ### switch pkg and package
        g1u.add_argument('--profile', '-P', type=str, required=False, help='profile(s) to be uninstalled')
        g2u = uninst_par.add_mutually_exclusive_group(required=False)
        g2u.add_argument('--live', '-l', action='store_true', required=False, help='make snapshot install live')
        g2u.add_argument('--not-live', '-nl', action='store_false', required=False, help='make snapshot install not live')
        uninst_par.set_defaults(func=triage_uninstall)
      # Unlock a snapshot
        unl_par = subparsers.add_parser("unlock", aliases=["ul"], allow_abbrev=True, help='Unlock a snapshot')
        unl_par.add_argument("snap", type=int, help="snapshot number")
        unl_par.set_defaults(func=snapshot_unlock)
      # Upgrade a snapshot
        upg_par = subparsers.add_parser("upgrade", aliases=["up"], allow_abbrev=True, help='Update all packages in a snapshot')
        upg_par.add_argument("snapshot", type=int, help="snapshot number")
        upg_par.set_defaults(func=upgrade)
      # which deployment is active
        whichtmp_par = subparsers.add_parser("whichtmp", aliases=["whichdep", "which"], allow_abbrev=True, help="Show which deployment snapshot is in use")
        whichtmp_par.set_defaults(func=lambda: get_tmp(console=True)) # print to console
      # Call relevant functions
        #args_1 = parser.parse_args()
        args_1 = parser.parse_args(args=None if sys.argv[1:] else ['--help']) # Show help if no command used
        args_2 = vars(args_1).copy()
        args_2.pop('command', None)
        args_2.pop('func', None)
        args_1.func(**args_2)

#-------------------- Triage functions for argparse method --------------------#

def triage_install(snapshot, live, profile, pkg, not_live):
    if profile:
        install_profile(snapshot, profile)
        #install_profile(snapshot, " ".join(profile))
    elif pkg:
        install(snapshot, " ".join(pkg))
  # If installing into current snapshot and no not_live flag, use live install
    if (snapshot == get_current_snapshot() and not_live) or live:
        if profile:
            #install_profile_live(" ".join(profile))
            install_profile_live(profile)
        elif pkg:
            install_live(" ".join(pkg))

def triage_uninstall(snapshot, profile, pkg, live, not_live): ### LATER add live, not_live
    if profile:
        #excode = install_profile(snapshot, profile)
        print("TODO")
    elif pkg:
        uninstall_package(snapshot, " ".join(pkg))

