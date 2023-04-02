#!/usr/bin/python3

import ast
import os
import re
import subprocess
import sys

# Directories
# All snapshots share one /var
# global boot is always at @boot
# *-deploy and *-deploy-secondary : temporary directories used to boot deployed snapshot
# *-chr                           : temporary directories used to chroot into snapshot or copy snapshots around
# /.snapshots/etc/etc-*           : individual /etc for each snapshot
# /.snapshots/boot/boot-*         : individual /boot for each snapshot
# /.snapshots/rootfs/snapshot-*   : snapshots
# /root/snapshots/*-desc          : descriptions
# /usr/share/ash                  : files that store current snapshot info
# /usr/share/ash/db               : package database
# /var/lib/ash(/fstree)           : ash files, stores fstree, symlink to /.snapshots/ash
# Failed prompts start with "F: "

#   Make a node mutable
def immutability_disable(snapshot):
    if snapshot != "0":
        if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
            print(f"F: Snapshot {snapshot} doesn't exist.")
        else:
            if os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable"):
                print(f"F: Snapshot {snapshot} is already mutable.")
            else:
                os.system(f"btrfs property set -ts /.snapshots/rootfs/snapshot-{snapshot} ro false")
                os.system(f"touch /.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable")
                print(f"Snapshot {snapshot} successfully made mutable.")
                write_desc(snapshot, " MUTABLE ")
    else:
        print(f"F: Snapshot {snapshot} (base) should not be modified.")

#   Make a node immutable
def immutability_enable(snapshot):
    if snapshot != "0":
        if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
            print(f"F: Snapshot {snapshot} doesn't exist.")
        else:
            if not os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable"):
                print(f"F: Snapshot {snapshot} is already immutable.")
            else:
                os.system(f"rm /.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable")
                os.system(f"btrfs property set -ts /.snapshots/rootfs/snapshot-{snapshot} ro true")
                print(f"Snapshot {snapshot} successfully made immutable.")
                os.system(f"sed 's/ MUTABLE //g' /.snapshots/ash/snapshots/{snapshot}-desc")
    else:
        print(f"F: Snapshot {snapshot} (base) should not be modified.")

#   This function returns either empty string or underscore plus name of distro if it was appended to sub-volume names to distinguish
def get_distro_suffix():
    if "ashos" in distro:
        return f'_{distro.replace("_ashos", "")}'
    else:
        return ""

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
                        boot_order = subprocess.check_output("efibootmgr | grep BootOrder | awk '{print $2}'", shell=True).decode('utf-8').strip()
                        temp = boot_order.replace(f'{row["BootOrder"]},', "")
                        new_boot_order = f"{row['BootOrder']},{temp}"
                        os.system(f'efibootmgr --bootorder {new_boot_order}')
            break
        else:
            print("Invalid distro!")
            continue

#   Import filesystem tree file in this function
def import_tree_file(treename):
    treefile = open(treename, "r")
    tree = ast.literal_eval(treefile.readline())
    return(tree)

#   Print out tree with descriptions
def print_tree(tree):
    snapshot = get_current_snapshot()
    for pre, fill, node in anytree.RenderTree(tree):
        if os.path.isfile(f"/.snapshots/ash/snapshots/{node.name}-desc"):
            descfile = open(f"/.snapshots/ash/snapshots/{node.name}-desc", "r")
            desc = descfile.readline()
            descfile.close()
        else:
            desc = ""
        if str(node.name) == "0":
            desc = "base snapshot"
        if snapshot != str(node.name):
            print("%s%s - %s" % (pre, node.name, desc))
        else:
            print("%s%s*- %s" % (pre, node.name, desc))

#   Write new description or append to an existing one
def write_desc(snapshot, desc):
    with open(f"/.snapshots/ash/snapshots/{snapshot}-desc", 'a+') as descfile:
        descfile.write(desc)

#   Add to root tree
def append_base_tree(tree, val):
    add = anytree.Node(val, parent=tree.root)

#   Add child to node
def add_node_to_parent(tree, id, val):
    par = (anytree.find(tree, filter_=lambda node: ("x"+str(node.name)+"x") in ("x"+str(id)+"x")))
    add = anytree.Node(val, parent=par)

#   Clone within node
def add_node_to_level(tree, id, val):
    npar = get_parent(tree, id)
    par = (anytree.find(tree, filter_=lambda node: ("x"+str(node.name)+"x") in ("x"+str(npar)+"x")))
    add = anytree.Node(val, parent=par)

#   Remove node from tree
def remove_node(tree, id):
    par = (anytree.find(tree, filter_=lambda node: ("x"+str(node.name)+"x") in ("x"+str(id)+"x")))
    par.parent = None

#   Save tree to file
def write_tree(tree):
    exporter = DictExporter()
    to_write = exporter.export(tree)
    fsfile = open(fstreepath, "w")
    fsfile.write(str(to_write))

#   Get parent
def get_parent(tree, id):
    par = (anytree.find(tree, filter_=lambda node: ("x"+str(node.name)+"x") in ("x"+str(id)+"x")))
    return(par.parent.name)

#   Return all children for node
def return_children(tree, id):
    children = []
    par = (anytree.find(tree, filter_=lambda node: ("x"+str(node.name)+"x") in ("x"+str(id)+"x")))
    for child in anytree.PreOrderIter(par):
        children.append(child.name)
    if id in children:
        children.remove(id)
    return (children)

#   Return order to recurse tree
def recurse_tree(tree, cid):
    order = []
    for child in (return_children(tree, cid)):
        par = get_parent(tree, child)
        if child != cid:
            order.append(par)
            order.append(child)
    return (order)

#   Get current snapshot
def get_current_snapshot():
    csnapshot = open("/usr/share/ash/snap", "r")
    snapshot = csnapshot.readline()
    csnapshot.close()
    snapshot = snapshot.replace('\n', "")
    return(snapshot)

#   Get drive partition
def get_part():
    cpart = open("/.snapshots/ash/part", "r")
    uuid = cpart.readline().replace('\n', "")
    cpart.close()
    part = str(subprocess.check_output(f"blkid | grep '{uuid}' | awk '{{print $1}}'", shell=True))
    return(part.replace(":", "").replace("b'", "").replace("\\n'", ""))

#   Get tmp partition state
def get_tmp():
    mount = str(subprocess.check_output("mount | grep 'on / type'", shell=True))
    if "tmp0" in mount:
        return("tmp0")
    else:
        return("tmp")

#   Deploy snapshot
def deploy(snapshot):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
        print(f"F: cannot deploy as snapshot {snapshot} doesn't exist.")
    else:
        update_boot(snapshot)
        tmp = get_tmp()
        os.system(f"btrfs sub set-default /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1") # Set default volume
        tmp_delete()
        if "tmp0" in tmp:
            tmp = "tmp"
        else:
            tmp = "tmp0"
        etc = snapshot
        os.system(f"btrfs sub snap /.snapshots/rootfs/snapshot-{snapshot} /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1")
        os.system(f"btrfs sub snap /.snapshots/etc/etc-{snapshot} /.snapshots/etc/etc-{tmp} >/dev/null 2>&1")
        os.system(f"btrfs sub snap /.snapshots/boot/boot-{snapshot} /.snapshots/boot/boot-{tmp} >/dev/null 2>&1")
        os.system(f"mkdir /.snapshots/rootfs/snapshot-{tmp}/etc >/dev/null 2>&1")
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-{tmp}/var >/dev/null 2>&1")
        os.system(f"mkdir /.snapshots/rootfs/snapshot-{tmp}/boot >/dev/null 2>&1")
        os.system(f"cp -r --reflink=auto /.snapshots/etc/etc-{etc}/. /.snapshots/rootfs/snapshot-{tmp}/etc >/dev/null 2>&1")
        # If snapshot is mutable, modify '/' entry (1st line) in fstab to read-write
        if os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable"):
            os.system(f"sed -i '0,/snapshots_tmp/ s/,ro//' /.snapshots/rootfs/snapshot-{tmp}/etc/fstab") # ,rw
        os.system(f"btrfs sub snap /var /.snapshots/rootfs/snapshot-{tmp}/var >/dev/null 2>&1")
        os.system(f"cp -r --reflink=auto /.snapshots/boot/boot-{etc}/. /.snapshots/rootfs/snapshot-{tmp}/boot >/dev/null 2>&1")
        os.system(f"echo '{snapshot}' > /.snapshots/rootfs/snapshot-{tmp}/usr/share/ash/snap")
        switch_tmp()
        os.system(f"rm -rf /var/lib/systemd/* >/dev/null 2>&1")
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-{tmp}/var/lib/systemd/* >/dev/null 2>&1")
        os.system(f"btrfs sub set-default /.snapshots/rootfs/snapshot-{tmp}") # Set default volume
        print(f"Snapshot {snapshot} deployed to /.")

#   Add node to branch
def extend_branch(snapshot, desc=""):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
        print(f"F: cannot branch as snapshot {snapshot} doesn't exist.")
    else:
        if os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable"):
            immutability = ""
        else:
            immutability = "-r"
        i = find_new()
        os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-{snapshot} /.snapshots/rootfs/snapshot-{i} >/dev/null 2>&1")
        #os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{i}/usr/share/ash") ### REVIEW MOST PROBABLY NOT NEEDED
        os.system(f"touch /.snapshots/rootfs/snapshot-{i}/usr/share/ash/mutable")
        os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{snapshot} /.snapshots/etc/etc-{i} >/dev/null 2>&1")
        os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-{snapshot} /.snapshots/boot/boot-{i} >/dev/null 2>&1")
        add_node_to_parent(fstree, snapshot, i)
        write_tree(fstree)
        if desc: write_desc(i, desc)
        print(f"Branch {i} added under snapshot {snapshot}.")

#   Clone branch under same parent
def clone_branch(snapshot):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
        print(f"F: cannot clone as snapshot {snapshot} doesn't exist.")
    else:
        if os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable"):
            immutability = ""
        else:
            immutability = "-r"
        i = find_new()
        os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-{snapshot} /.snapshots/rootfs/snapshot-{i} >/dev/null 2>&1")
        #os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{i}/usr/share/ash") ### REVIEW MOST PROBABLY NOT NEEDED
        os.system(f"touch /.snapshots/rootfs/snapshot-{i}/usr/share/ash/mutable")
        os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{snapshot} /.snapshots/etc/etc-{i} >/dev/null 2>&1")
        os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-{snapshot} /.snapshots/boot/boot-{i} >/dev/null 2>&1")
        add_node_to_level(fstree, snapshot, i)
        write_tree(fstree)
        desc = str(f"clone of {snapshot}")
        write_desc(i, desc)
        print(f"Branch {i} added to parent of {snapshot}.")

#   Clone under specified parent
def clone_under(snapshot, branch):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")) or (not(os.path.exists(f"/.snapshots/rootfs/snapshot-{branch}"))):
        print(f"F: cannot clone as snapshot {snapshot} doesn't exist.")
    else:
        if os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable"):
            immutability = ""
        else:
            immutability = "-r"
        i = find_new()
        os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-{branch} /.snapshots/rootfs/snapshot-{i} >/dev/null 2>&1")
        #os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{i}/usr/share/ash") ### REVIEW MOST PROBABLY NOT NEEDED
        os.system(f"touch /.snapshots/rootfs/snapshot-{i}/usr/share/ash/mutable")
        os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{branch} /.snapshots/etc/etc-{i} >/dev/null 2>&1")
        os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-{branch} /.snapshots/boot/boot-{i} >/dev/null 2>&1")
        add_node_to_parent(fstree, snapshot, i)
        write_tree(fstree)
        desc = str(f"clone of {snapshot}")
        write_desc(i, desc)
        print(f"Branch {i} added under snapshot {snapshot}.")

#   Lock ash
def ast_lock():
    os.system("touch /.snapshots/ash/lock-disable")

#   Unlock
def ast_unlock():
    os.system("rm -rf /.snapshots/ash/lock")

def get_lock():
    if os.path.exists("/.snapshots/ash/lock"):
        return(True)
    else:
        return(False)

#   Recursively remove package in tree
def remove_from_tree(tree, treename, pkg):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{treename}")):
        print(f"F: cannot update as tree {treename} doesn't exist.")
    else:
        remove(treename, pkg)
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
            remove(sarg, pkg)
        print(f"Tree {treename} updated.")

#   Recursively run an update in tree
def update_tree(tree, treename):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{treename}")):
        print(f"F: cannot update as tree {treename} doesn't exist.")
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

#   Recursively run an update in tree
def run_tree(tree, treename, cmd):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{treename}")):
        print(f"F: cannot update as tree {treename} doesn't exist.")
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
                print(f"F: snapshot {snapshot} appears to be in use. If you're certain it's not in use clear lock with 'ash unlock {snapshot}'.")
                print("tree command cancelled.")
                return
            else:
                prepare(sarg)
                os.system(f"chroot /.snapshots/rootfs/snapshot-chr{sarg} {cmd}")
                post_transactions(sarg)
        print(f"Tree {treename} updated.")

#   Sync tree and all it's snapshots
def sync_tree(tree, treename, forceOffline):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{treename}")):
        print(f"F: cannot sync as tree {treename} doesn't exist.")
    else:
        if not forceOffline: # Syncing tree automatically updates it, unless 'force-sync' is used
            update_tree(tree, treename)
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
                print(f"F: snapshot {snapshot} appears to be in use. If you're certain it's not in use clear lock with 'ash unlock {snapshot}'.")
                print("tree sync cancelled.")
                return
            else:
                prepare(sarg)
                os.system(f"cp -n -r --reflink=auto /.snapshots/rootfs/snapshot-{arg}/. /.snapshots/rootfs/snapshot-chr{sarg}/ >/dev/null 2>&1")
                #os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-{arg}/etc/. /.snapshots/rootfs/snapshot-chr{sarg}/etc/ >/dev/null 2>&1") ### Commented out due to causing issues
                post_transactions(sarg)
        print(f"Tree {treename} synced.")

#   Clone tree
def clone_as_tree(snapshot):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
        print(f"F: cannot clone as snapshot {snapshot} doesn't exist.")
    else:
        if os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable"):
            immutability = ""
        else:
            immutability = "-r"
        i = find_new()
        os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-{snapshot} /.snapshots/rootfs/snapshot-{i} >/dev/null 2>&1")
        #os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{i}/usr/share/ash") ### REVIEW MOST PROBABLY NOT NEEDED
        if os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable"):
            os.system(f"touch /.snapshots/rootfs/snapshot-{i}/usr/share/ash/mutable")
        os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{snapshot} /.snapshots/etc/etc-{i} >/dev/null 2>&1")
        os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-{snapshot} /.snapshots/boot/boot-{i} >/dev/null 2>&1")
        append_base_tree(fstree, i)
        write_tree(fstree)
        desc = str(f"clone of {snapshot}")
        write_desc(i, desc)
        print(f"Tree {i} cloned from {snapshot}.")

#   Creates new tree from base file
def new_snapshot(desc=""): # immutability toggle not used as base should always be immutable
    i = find_new()
    os.system(f"btrfs sub snap -r /.snapshots/rootfs/snapshot-0 /.snapshots/rootfs/snapshot-{i} >/dev/null 2>&1")
    os.system(f"btrfs sub snap -r /.snapshots/etc/etc-0 /.snapshots/etc/etc-{i} >/dev/null 2>&1")
    os.system(f"btrfs sub snap -r /.snapshots/boot/boot-0 /.snapshots/boot/boot-{i} >/dev/null 2>&1")
    append_base_tree(fstree, i)
    write_tree(fstree)
    if desc: write_desc(i, desc)
    print(f"New tree {i} created.")

#   Calls print function
def show_fstree():
    print_tree(fstree)

#   Saves changes made to /etc to snapshot
def update_etc():
    tmp = get_tmp()
    snapshot = get_current_snapshot()
    os.system(f"btrfs sub del /.snapshots/etc/etc-{snapshot} >/dev/null 2>&1")
    if os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}/usr/share/ash/mutable"):
        immutability = ""
    else:
        immutability = "-r"
    os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-{tmp} /.snapshots/etc/etc-{snapshot} >/dev/null 2>&1")

#   Update boot
def update_boot(snapshot):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
        print(f"F: cannot update boot as snapshot {snapshot} doesn't exist.")
    else:
        tmp = get_tmp()
        part = get_part()
        prepare(snapshot)
        ### TODO: DELETE grub.cfg.DATE.BAK older than 90 days
        subprocess.check_output("cp /boot/grub/grub.cfg /boot/grub/BAK/grub.cfg.`date '+%Y%m%d-%H%M%S'`", shell=True)
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} grub-mkconfig {part} -o /boot/grub/grub.cfg")
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} sed -i s,snapshot-chr{snapshot},snapshot-{tmp},g /boot/grub/grub.cfg")
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} sed -i '0,/Arch\ Linux/ s##Arch\ Linux\ snapshot\ {snapshot}#' /boot/grub/grub.cfg")
        post_transactions(snapshot)

#   Chroot into snapshot
def chroot(snapshot):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
        print(f"F: cannot chroot as snapshot {snapshot} doesn't exist.")
    elif snapshot == "0":
        print("F: changing base snapshot is not allowed.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"): # Make sure snapshot is not in use by another ash process
        print(f"F: snapshot {snapshot} appears to be in use. If you're certain it's not in use clear lock with 'ash unlock {snapshot}'.")
    else:
        prepare(snapshot)
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot}")
        post_transactions(snapshot)

#   Run command in snapshot
def chr_run(snapshot, cmd):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
        print(f"F: cannot chroot as snapshot {snapshot} doesn't exist.")
    elif snapshot == "0":
        print("F: changing base snapshot is not allowed.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"): # Make sure snapshot is not in use by another ash process
        print(f"F: snapshot {snapshot} appears to be in use. If you're certain it's not in use clear lock with 'ash unlock {snapshot}'.")
    else:
        prepare(snapshot)
        os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} {cmd}")
        post_transactions(snapshot)

#   Clean chroot mount dirs
def chr_delete(snapshot):
    os.system(f"btrfs sub del /.snapshots/etc/etc-chr{snapshot} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/boot/boot-chr{snapshot} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{snapshot}/* >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{snapshot} >/dev/null 2>&1")

#   Clean tmp dirs
def tmp_delete():
    tmp = get_tmp()
    if "tmp0" in tmp:
        tmp = "tmp"
    else:
        tmp = "tmp0"
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{tmp}/* >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/etc/etc-{tmp} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/boot/boot-{tmp} >/dev/null 2>&1")

#   Install live
def install_live(pkg):
    tmp = get_tmp()
    part = get_part()
    os.system(f"mount --bind /.snapshots/rootfs/snapshot-{tmp} /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1")
    os.system(f"mount --bind /home /.snapshots/rootfs/snapshot-{tmp}/home >/dev/null 2>&1")
    os.system(f"mount --bind /var /.snapshots/rootfs/snapshot-{tmp}/var >/dev/null 2>&1")
    os.system(f"mount --bind /etc /.snapshots/rootfs/snapshot-{tmp}/etc >/dev/null 2>&1")
    os.system(f"mount --bind /tmp /.snapshots/rootfs/snapshot-{tmp}/tmp >/dev/null 2>&1")
    os.system(f"chroot /.snapshots/rootfs/snapshot-{tmp} pacman -S --overwrite \\* --noconfirm {pkg}")
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/* >/dev/null 2>&1")
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1")

#   Live unlocked shell
def live_unlock():
    tmp = get_tmp()
    part = get_part()
    os.system(f"mount --bind /.snapshots/rootfs/snapshot-{tmp} /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1")
    os.system(f"mount --bind /home /.snapshots/rootfs/snapshot-{tmp}/home >/dev/null 2>&1")
    os.system(f"mount --bind /var /.snapshots/rootfs/snapshot-{tmp}/var >/dev/null 2>&1")
    os.system(f"mount --bind /etc /.snapshots/rootfs/snapshot-{tmp}/etc >/dev/null 2>&1")
    os.system(f"mount --bind /tmp /.snapshots/rootfs/snapshot-{tmp}/tmp >/dev/null 2>&1")
    os.system(f"chroot /.snapshots/rootfs/snapshot-{tmp}")
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/* >/dev/null 2>&1")
    os.system(f"umount /.snapshots/rootfs/snapshot-{tmp} >/dev/null 2>&1")

#   Install packages
def install(snapshot, pkg):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
        print(f"F: cannot install as snapshot {snapshot} doesn't exist.")
    elif snapshot == "0":
        print("F: changing base snapshot is not allowed.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"): # Make sure snapshot is not in use by another ash process
        print(f"F: snapshot {snapshot} appears to be in use. If you're certain it's not in use clear lock with 'ash unlock {snapshot}'.")
    else:
        prepare(snapshot)
        excode = str(os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} pacman -S {pkg} --overwrite '/var/*'"))
        if int(excode) == 0:
            post_transactions(snapshot)
            print(f"Package {pkg} installed in snapshot {snapshot} successfully.")
        else:
            chr_delete(snapshot)
            print("F: install failed and changes discarded.")

#   Install from a text file
def install_profile(snapshot, profile):
    install(snapshot, subprocess.check_output(f"cat {profile}", shell=True).decode('utf-8').strip())

#   Remove packages
def remove(snapshot, pkg):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
        print(f"F: cannot remove as snapshot {snapshot} doesn't exist.")
    elif snapshot == "0":
        print("F: changing base snapshot is not allowed.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"):
        print(f"F: snapshot {snapshot} appears to be in use. If you're certain it's not in use clear lock with 'ash unlock {snapshot}'.")
    else:
        prepare(snapshot)
        excode = str(os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} pacman --noconfirm -Rns {pkg}"))
        if int(excode) == 0:
            post_transactions(snapshot)
            print(f"Package {pkg} removed from snapshot {snapshot} successfully.")
        else:
            chr_delete(snapshot)
            print("F: remove failed and changes discarded.")

#   Delete tree or branch
def delete(snapshot):
    print(f"Are you sure you want to delete snapshot {snapshot}? (y/N)")
    choice = input("> ")
    run = True
    if choice.casefold() != "y":
        print("Aborted")
        run = False
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
        print(f"F: cannot delete as snapshot {snapshot} doesn't exist.")
    elif snapshot == "0":
        print("F: changing base snapshot is not allowed.")
    elif run == True:
        children = return_children(fstree, snapshot)
        os.system(f"btrfs sub del /.snapshots/boot/boot-{snapshot} >/dev/null 2>&1")
        os.system(f"btrfs sub del /.snapshots/etc/etc-{snapshot} >/dev/null 2>&1")
        os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{snapshot} >/dev/null 2>&1")
        for child in children: # This deletes the node itself along with it's children
            os.system(f"btrfs sub del /.snapshots/boot/boot-{child} >/dev/null 2>&1")
            os.system(f"btrfs sub del /.snapshots/etc/etc-{child} >/dev/null 2>&1")
            os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{child} >/dev/null 2>&1")
        remove_node(fstree, snapshot) # Remove node from tree or root
        write_tree(fstree)
        print(f"Snapshot {snapshot} removed.")

#   Update base
def update_base():
    snapshot = "0"
    if os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"):
        print(f"F: snapshot {snapshot} appears to be in use. If you're certain it's not in use clear lock with 'ash unlock {snapshot}'.")
    else:
        prepare(snapshot)
        excode = str(os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} pacman -Syyu"))
        if int(excode) == 0:
            post_transactions(snapshot)
            print(f"Snapshot {snapshot} upgraded successfully.")
        else:
            chr_delete(snapshot)
            print("F: upgrade failed and changes discarded.")

#   Prepare snapshot to chroot dir to install or chroot into
def prepare(snapshot):
    chr_delete(snapshot)
    part = get_part()
    etc = snapshot
    os.system(f"btrfs sub snap /.snapshots/rootfs/snapshot-{snapshot} /.snapshots/rootfs/snapshot-chr{snapshot} >/dev/null 2>&1")
    os.system(f"btrfs sub snap /.snapshots/etc/etc-{snapshot} /.snapshots/etc/etc-chr{snapshot} >/dev/null 2>&1")
    # pacman gets weird when chroot directory is not a mountpoint, so the following mount is necessary
    os.system(f"mount --bind /.snapshots/rootfs/snapshot-chr{snapshot} /.snapshots/rootfs/snapshot-chr{snapshot} >/dev/null 2>&1")
    os.system(f"mount --bind /var /.snapshots/rootfs/snapshot-chr{snapshot}/var >/dev/null 2>&1")
    os.system(f"mount --rbind /dev /.snapshots/rootfs/snapshot-chr{snapshot}/dev >/dev/null 2>&1")
    os.system(f"mount --rbind /sys /.snapshots/rootfs/snapshot-chr{snapshot}/sys >/dev/null 2>&1")
    os.system(f"mount --rbind /tmp /.snapshots/rootfs/snapshot-chr{snapshot}/tmp >/dev/null 2>&1")
    os.system(f"mount --rbind /proc /.snapshots/rootfs/snapshot-chr{snapshot}/proc >/dev/null 2>&1")
    os.system(f"btrfs sub snap /.snapshots/boot/boot-{snapshot} /.snapshots/boot/boot-chr{snapshot} >/dev/null 2>&1")
    os.system(f"cp -r --reflink=auto /.snapshots/etc/etc-chr{snapshot}/. /.snapshots/rootfs/snapshot-chr{snapshot}/etc >/dev/null 2>&1")
    os.system(f"cp -r --reflink=auto /.snapshots/boot/boot-chr{snapshot}/. /.snapshots/rootfs/snapshot-chr{snapshot}/boot >/dev/null 2>&1")
    os.system(f"rm -rf /.snapshots/rootfs/snapshot-chr{snapshot}/var/lib/systemd/* >/dev/null 2>&1")
    os.system(f"mount --bind /home /.snapshots/rootfs/snapshot-chr{snapshot}/home >/dev/null 2>&1")
    os.system(f"mount --rbind /run /.snapshots/rootfs/snapshot-chr{snapshot}/run >/dev/null 2>&1")
    os.system(f"cp /etc/machine-id /.snapshots/rootfs/snapshot-chr{snapshot}/etc/machine-id")
    os.system(f"mkdir -p /.snapshots/rootfs/snapshot-chr{snapshot}/.snapshots/ash && cp -f /.snapshots/ash/fstree /.snapshots/rootfs/snapshot-chr{snapshot}/.snapshots/ash/")
    os.system(f"mount --bind /etc/resolv.conf /.snapshots/rootfs/snapshot-chr{snapshot}/etc/resolv.conf >/dev/null 2>&1")
    os.system(f"mount --bind /root /.snapshots/rootfs/snapshot-chr{snapshot}/root >/dev/null 2>&1")

#   Post transaction function, copy from chroot dirs back to read only snapshot dir
def post_transactions(snapshot):
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
    os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/etc/. /.snapshots/etc/etc-chr{snapshot} >/dev/null 2>&1")
    # Keep package manager's cache after installing packages. This prevents unnecessary downloads for each snapshot when upgrading multiple snapshots
    os.system(f"cp -n -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/var/cache/pacman/pkg/. /var/cache/pacman/pkg/ >/dev/null 2>&1") ### REVIEW IS THIS NEEDED?
    os.system(f"rm -rf /.snapshots/boot/boot-chr{snapshot}/* >/dev/null 2>&1")
    os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-chr{snapshot}/boot/. /.snapshots/boot/boot-chr{snapshot} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/etc/etc-{etc} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/boot/boot-{etc} >/dev/null 2>&1")
    if os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}/usr/share/ash/mutable"):
        immutability = ""
    else:
        immutability = "-r"
    os.system(f"btrfs sub snap {immutability} /.snapshots/etc/etc-chr{snapshot} /.snapshots/etc/etc-{etc} >/dev/null 2>&1")
    os.system(f"rm -rf /var/lib/systemd/* >/dev/null 2>&1")
    os.system(f"cp -r --reflink=auto /.snapshots/rootfs/snapshot-{tmp}/var/lib/systemd/. /var/lib/systemd >/dev/null 2>&1")
    os.system(f"btrfs sub snap {immutability} /.snapshots/rootfs/snapshot-chr{snapshot} /.snapshots/rootfs/snapshot-{snapshot} >/dev/null 2>&1")
    os.system(f"btrfs sub snap {immutability} /.snapshots/boot/boot-chr{snapshot} /.snapshots/boot/boot-{etc} >/dev/null 2>&1")
    chr_delete(snapshot)

#   Upgrade snapshot
def upgrade(snapshot):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
        print(f"F: cannot upgrade as snapshot {snapshot} doesn't exist.")
    elif snapshot == "0":
        print("F: changing base snapshot is not allowed.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"):
        print(f"F: snapshot {snapshot} appears to be in use. If you're certain it's not in use clear lock with 'ash unlock {snapshot}'.")
    else:
        prepare(snapshot)
        excode = str(os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} pacman -Syyu")) # Default upgrade behaviour is now "safe" update, meaning failed updates get fully discarded
        if int(excode) == 0:
            post_transactions(snapshot)
            print(f"Snapshot {snapshot} upgraded successfully.")
        else:
            chr_delete(snapshot)
            print("F: upgrade failed and changes discarded.")

#   Refresh snapshot
def refresh(snapshot):
    if not (os.path.exists(f"/.snapshots/rootfs/snapshot-{snapshot}")):
        print(f"F: cannot refresh as snapshot {snapshot} doesn't exist.")
    elif snapshot == "0":
        print("F: changing base snapshot is not allowed.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"):
        print(f"F: snapshot {snapshot} appears to be in use. If you're certain it's not in use clear lock with 'ash unlock {snapshot}'.")
    else:
        prepare(snapshot)
        excode = str(os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} pacman -Syy"))
        if int(excode) == 0:
            post_transactions(snapshot)
            print(f"Snapshot {snapshot} refreshed successfully.")
        else:
            chr_delete(snapshot)
            print("F: refresh failed and changes discarded.")

#   Noninteractive update
def auto_upgrade(snapshot):
    prepare(snapshot)
    excode = str(os.system(f"chroot /.snapshots/rootfs/snapshot-chr{snapshot} pacman --noconfirm -Syyu"))
    if int(excode) == 0:
        post_transactions(snapshot)
        os.system("echo 0 > /.snapshots/ash/upstate")
        os.system("echo $(date) >> /.snapshots/ash/upstate")
    else:
        chr_delete(snapshot)
        os.system("echo 1 > /.snapshots/ash/upstate")
        os.system("echo $(date) >> /.snapshots/ash/upstate")

#   Check if last update was successful
def check_update():
    upstate = open("/.snapshots/ash/upstate", "r")
    line = upstate.readline()
    date = upstate.readline()
    if "1" in line:
        print(f"F: Last update on {date} failed.")
    if "0" in line:
        print(f"Last update on {date} completed successfully.")
    upstate.close()

def chroot_check():
    chroot = True # When inside chroot
    with open("/proc/mounts", "r") as mounts:
        for line in mounts:
            if str("/.snapshots btrfs") in str(line):
                chroot = False
    return(chroot)

#   Rollback last booted deployment
def rollback():
    tmp = get_tmp()
    i = find_new()
    clone_as_tree(tmp)
    write_desc(i, "rollback")
    deploy(i)

#   Switch between /tmp deployments
def switch_tmp():
    distro_suffix = get_distro_suffix()
    mount = get_tmp()
    part = get_part()
    os.system(f"mkdir -p /etc/mnt/boot >/dev/null 2>&1")
    os.system(f"mount {part} -o subvol=@boot{distro_suffix} /etc/mnt/boot") # Mount boot partition for writing
    if "tmp0" in mount:
        os.system("cp -r --reflink=auto /.snapshots/rootfs/snapshot-tmp/boot/. /etc/mnt/boot")  ######REZA WHATABOUTTHIS?
        os.system(f"sed -i 's,@.snapshots{distro_suffix}/rootfs/snapshot-tmp0,@.snapshots{distro_suffix}/rootfs/snapshot-tmp,g' /etc/mnt/boot/grub/grub.cfg") # Overwrite grub config boot subvolume
        os.system(f"sed -i 's,@.snapshots{distro_suffix}/rootfs/snapshot-tmp0,@.snapshots{distro_suffix}/rootfs/snapshot-tmp,g' /.snapshots/rootfs/snapshot-tmp/boot/grub/grub.cfg")
        os.system(f"sed -i 's,@.snapshots{distro_suffix}/rootfs/snapshot-tmp0,@.snapshots{distro_suffix}/rootfs/snapshot-tmp,g' /.snapshots/rootfs/snapshot-tmp/etc/fstab") # Write fstab for new deployment
        os.system(f"sed -i 's,@.snapshots{distro_suffix}/etc/etc-tmp0,@.snapshots{distro_suffix}/etc/etc-tmp,g' /.snapshots/rootfs/snapshot-tmp/etc/fstab")
        os.system(f"sed -i 's,@.snapshots{distro_suffix}/boot/boot-tmp0,@.snapshots{distro_suffix}/boot/boot-tmp,g' /.snapshots/rootfs/snapshot-tmp/etc/fstab")
        sfile = open("/.snapshots/rootfs/snapshot-tmp0/usr/share/ash/snap", "r")
        snap = sfile.readline()
        snap = snap.replace(" ", "")
        sfile.close()
    else:
        os.system("cp -r --reflink=auto /.snapshots/rootfs/snapshot-tmp0/boot/. /etc/mnt/boot")
        os.system(f"sed -i 's,@.snapshots{distro_suffix}/rootfs/snapshot-tmp,@.snapshots{distro_suffix}/rootfs/snapshot-tmp0,g' /etc/mnt/boot/grub/grub.cfg")
        os.system(f"sed -i 's,@.snapshots{distro_suffix}/rootfs/snapshot-tmp,@.snapshots{distro_suffix}/rootfs/snapshot-tmp0,g' /.snapshots/rootfs/snapshot-tmp0/boot/grub/grub.cfg")
        os.system(f"sed -i 's,@.snapshots{distro_suffix}/rootfs/snapshot-tmp,@.snapshots{distro_suffix}/rootfs/snapshot-tmp0,g' /.snapshots/rootfs/snapshot-tmp0/etc/fstab")
        os.system(f"sed -i 's,@.snapshots{distro_suffix}/etc/etc-tmp,@.snapshots{distro_suffix}/etc/etc-tmp0,g' /.snapshots/rootfs/snapshot-tmp0/etc/fstab")
        os.system(f"sed -i 's,@.snapshots{distro_suffix}/boot/boot-tmp,@.snapshots{distro_suffix}/boot/boot-tmp0,g' /.snapshots/rootfs/snapshot-tmp0/etc/fstab")
        sfile = open("/.snapshots/rootfs/snapshot-tmp/usr/share/ash/snap", "r")
        snap = sfile.readline()
        snap = snap.replace(" ", "")
        sfile.close()
    #
    snap = snap.replace('\n', "")
    grubconf = open("/etc/mnt/boot/grub/grub.cfg", "r")
    line = grubconf.readline()
    while "BEGIN /etc/grub.d/10_linux" not in line:
        line = grubconf.readline()
    line = grubconf.readline()
    gconf = str("")
    while "}" not in line:
        gconf = str(gconf)+str(line)
        line = grubconf.readline()
    if "snapshot-tmp0" in gconf:
        gconf = gconf.replace("snapshot-tmp0", "snapshot-tmp")
    else:
        gconf = gconf.replace("snapshot-tmp", "snapshot-tmp0")
    if "Arch Linux" in gconf:
        gconf = re.sub('snapshot \d', '', gconf)
        gconf = gconf.replace(f"Arch Linux", f"Arch Linux last booted deployment (snapshot {snap})")
    grubconf.close()
    os.system("sed -i '$ d' /etc/mnt/boot/grub/grub.cfg")
    grubconf = open("/etc/mnt/boot/grub/grub.cfg", "a")
    grubconf.write(gconf)
    grubconf.write("}\n")
    grubconf.write("### END /etc/grub.d/41_custom ###")
    grubconf.close()

    grubconf = open("/.snapshots/rootfs/snapshot-tmp0/boot/grub/grub.cfg", "r")
    line = grubconf.readline()
    while "BEGIN /etc/grub.d/10_linux" not in line:
        line = grubconf.readline()
    line = grubconf.readline()
    gconf = str("")
    while "}" not in line:
        gconf = str(gconf)+str(line)
        line = grubconf.readline()
    if "snapshot-tmp0" in gconf:
        gconf = gconf.replace("snapshot-tmp0", "snapshot-tmp")
    else:
        gconf = gconf.replace("snapshot-tmp", "snapshot-tmp0")
    if "Arch Linux" in gconf:
        gconf = re.sub('snapshot \d', '', gconf)
        gconf = gconf.replace(f"Arch Linux", f"Arch Linux last booted deployment (snapshot {snap})")
    grubconf.close()
    os.system("sed -i '$ d' /.snapshots/rootfs/snapshot-tmp0/boot/grub/grub.cfg")
    grubconf = open("/.snapshots/rootfs/snapshot-tmp0/boot/grub/grub.cfg", "a")
    grubconf.write(gconf)
    grubconf.write("}\n")
    grubconf.write("### END /etc/grub.d/41_custom ###")
    grubconf.close()
    os.system("umount /etc/mnt/boot >/dev/null 2>&1")

#   Show diff of packages between 2 snapshots TODO: make this function not depend on bash
def snapshot_diff(snap1, snap2):
    os.system(f"bash -c \"diff <(ls /.snapshots/rootfs/snapshot-{snap1}/usr/share/ash/db/local) <(ls /.snapshots/rootfs/snapshot-{snap2}/usr/share/ash/db/local) | grep '^>\|^<' | sort\"")

#   Remove temporary chroot for specified snapshot only
#   This unlocks the snapshot for use by other functions
def snapshot_unlock(snap):
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{snap} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/etc/etc-chr{snap} >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/boot/boot-chr{snap} >/dev/null 2>&1")

#   Show some basic ash commands
def ast_help():
    print("all ash commands, aside from 'ash tree' must be used with root permissions!")
    print("\n\ntree manipulation commands:")
    print("\ttree - show the snapshot tree")
    print("\tdiff <snapshot 1> <snapshot 2> - show package diff between snapshots")
    print("\tcurrent - return current snapshot number")
    print("\tdesc <snapshot> <description> - set a description for snapshot by number")
    print("\tdel <tree> - delete a tree and all it's branches recursively")
    print("\tchroot <snapshot> - open a root shell inside specified snapshot")
    print("\tlive-chroot - open a read-write shell inside currently booted snapshot (changes are discarded on new deployment)")
    print("\trun <snapshot> <command> - execute command inside another snapshot")
    print("\ttree-run <tree> <command> - execute command inside another snapshot and all snapshots below it")
    print("\tclone <snapshot> - create a copy of snapshot")
    print("\tbranch <snapshot> - create a new branch from snapshot")
    print("\tcbranch <snapshot> - copy snapshot under same parent branch")
    print("\tubranch <parent> <snapshot> - copy snapshot under specified parent")
    print("\tnew - create a new base snapshot")
    print("\tdeploy <snapshot> - deploy a snapshot for next boot")
    print("\tbase-update - update the base image")
    print("\n\npackage management commands:")
    print("\tinstall <snapshot> <package> - install a package inside specified snapshot")
    print("\tsync <tree> - sync package and configuration changes recursively, requires an internet connection")
    print("\tforce-sync <tree> - same thing as sync but doesn't update snapshots, potentially riskier")
    print("\tremove <snapshot> <package(s)> - remove package(s) from snapshot")
    print("\ttree-rmpkg <tree> <package(s)> - remove package(s) from tree recursively")
    print("\tupgrade <snapshot> - update all packages in snapshot")
    print("\ttree-upgrade <tree> - update all packages in snapshot recursively")
    print("\trollback - rollback the deployment to the last booted snapshot")
    print("\n\nto update ash itself use 'ash upself'")

#   Update ash itself
def ash_update():
    cdir = os.getcwd()
    os.chdir("/tmp")
    excode = str(os.system("curl -O 'https://raw.githubusercontent.com/ashos/ashos/main/ashpk_core.py'"))
    if int(excode) == 0:
        os.system("cp ./ashpk_core.py /.snapshots/ash/ash")
        os.system("chmod +x /.snapshots/ash/ash")
        print("ash updated succesfully.")
    else:
        print("F: failed to download ash")
    os.chdir(cdir)

# Clear all temporary snapshots
def tmp_clear():
    os.system(f"btrfs sub del /.snapshots/etc/etc-chr* >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/boot/boot-chr* >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr*/* >/dev/null 2>&1")
    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr* >/dev/null 2>&1")

def list_subvolumes():
    os.system(f"btrfs sub list / | grep -i {get_distro_suffix()}")

# Find new unused snapshot dir
def find_new():
    i = 0
    snapshots = os.listdir("/.snapshots/rootfs")
    etcs = os.listdir("/.snapshots/etc")
    boots = os.listdir("/.snapshots/boot")
    snapshots.append(etcs)
    snapshots.append(vars)
    snapshots.append(boots)
    while True:
        i += 1
        if str(f"snapshot-{i}") not in snapshots and str(f"etc-{i}") not in snapshots and str(f"var-{i}") not in snapshots and str(f"boot-{i}") not in snapshots:
            return(i)

#   Main function
def main(args):
    distro_suffix = get_distro_suffix()
    snapshot = get_current_snapshot() # Get current snapshot
    etc = snapshot
    importer = DictImporter() # Dict importer
    exporter = DictExporter() # And exporter
    isChroot = chroot_check()
    lock = get_lock() # True = locked
    global fstree # Currently these are global variables, fix sometime
    global fstreepath # ---
    fstreepath = str("/.snapshots/ash/fstree") # Path to fstree file
    fstree = importer.import_(import_tree_file("/.snapshots/ash/fstree")) # Import fstree file
    # Recognize argument and call appropriate function
    if len(args) > 1:
        arg = args[1]
    else:
        print("You need to specify an operation, see 'ash help' for help.")
        sys.exit()
    if isChroot == True and ("--chroot" not in args):
        print("Please don't use ash inside a chroot!")
    elif lock == True:
        print("ash is locked. To manually unlock, run 'rm -rf /var/lib/ash/lock'.")
    elif arg == "new-tree" or arg == "new":
        args_2 = args
        args_2.remove(args_2[0])
        args_2.remove(args_2[0])
        new_snapshot(str(" ").join(args_2))
    elif arg == "boot-update" or arg == "boot":
        update_boot(args[args.index(arg)+1])
    elif arg == "chroot" or arg == "cr" and (lock != True):
        ast_lock()
        chroot(args[args.index(arg)+1])
        ast_unlock()
    elif arg == "live-chroot":
        ast_lock()
        live_unlock()
        ast_unlock()
    elif arg == "install" or (arg == "in") and (lock != True):
        ast_lock()
        args_2 = args
        args_2.remove(args_2[0])
        args_2.remove(args_2[0])
        live = False
        if args_2[0] == "--live":
            args_2.remove(args_2[0])
        if args_2[0] == get_current_snapshot():
            live = True
        csnapshot = args_2[0]
        args_2.remove(args_2[0])
        install(csnapshot, str(" ").join(args_2))
        if live:
            install_live(str(" ").join(args_2))
        ast_unlock()
    elif arg == "run" and (lock != True):
        ast_lock()
        args_2 = args
        args_2.remove(args_2[0])
        args_2.remove(args_2[0])
        csnapshot = args_2[0]
        args_2.remove(args_2[0])
        chr_run(csnapshot, str(" ").join(args_2))
        ast_unlock()
    elif arg == "add-branch" or arg == "branch":
        extend_branch(args[args.index(arg)+1])
    elif arg == "tmpclear" or arg == "tmp":
        tmp_clear()
    elif arg == "clone-branch" or arg == "cbranch":
        clone_branch(args[args.index(arg)+1])
    elif arg == "clone-under" or arg == "ubranch":
        clone_under(args[args.index(arg)+1], args[args.index(arg)+2])
    elif arg == "diff":
        snapshot_diff(args[args.index(arg)+1], args[args.index(arg)+2])
    elif arg == "clone" or arg == "tree-clone":
        clone_as_tree(args[args.index(arg)+1])
    elif arg == "deploy":
        deploy(args[args.index(arg)+1])
    elif arg == "rollback":
        rollback()
    elif arg == "upgrade" or arg == "up" and (lock != True):
        ast_lock()
        upgrade(args[args.index(arg)+1])
        ast_unlock()
    elif arg == "unlock" and (lock != True):
        ast_lock()
        snapshot_unlock(args[args.index(arg)+1])
        ast_unlock()
    elif arg == "refresh" or arg == "ref" and (lock != True):
        ast_lock()
        refresh(args[args.index(arg)+1])
        ast_unlock()
    elif arg == "etc-update" or arg == "etc" and (lock != True):
        ast_lock()
        update_etc()
        ast_unlock()
    elif arg == "current" or arg == "c":
        print(snapshot)
    elif arg == "rm-snapshot" or arg == "del":
        delete(args[args.index(arg)+1])
    elif arg == "remove" and (lock != True):
        ast_lock()
        args_2 = args
        args_2.remove(args_2[0])
        args_2.remove(args_2[0])
        csnapshot = args_2[0]
        args_2.remove(args_2[0])
        remove(csnapshot, str(" ").join(args_2))
        ast_unlock()
    elif arg == "desc" or arg == "description":
        n_lay = args[args.index(arg)+1]
        args_2 = args
        args_2.remove(args_2[0])
        args_2.remove(args_2[0])
        args_2.remove(args_2[0])
        write_desc(n_lay, str(" ").join(args_2))
    elif arg == "base-update" or arg == "bu" and (lock != True):
        ast_lock()
        update_base()
        ast_unlock()
    elif arg == "help":
        ast_help()
    elif arg == "upself" and (lock != True): # Currently this lock is ignored ### REVIEW
        ast_lock()
        ash_update()
        ast_unlock()
    elif arg == "sync" or arg == "tree-sync" and (lock != True):
        ast_lock()
        sync_tree(fstree, args[args.index(arg)+1], False)
        ast_unlock()
    elif arg == "fsync" or arg == "force-sync" and (lock != True):
        ast_lock()
        sync_tree(fstree, args[args.index(arg)+1], True)
        ast_unlock()
    elif arg == "auto-upgrade" and (lock != True):
        ast_lock()
        auto_upgrade(snapshot)
        ast_unlock()
    elif arg == "check":
        check_update()
    elif arg == "tree-upgrade" or arg == "tupgrade" and (lock != True):
        ast_lock()
        upgrade(args[args.index(arg)+1])
        update_tree(fstree, args[args.index(arg)+1])
        ast_unlock()
    elif arg == "tree-run" or arg == "trun" and (lock != True):
        ast_lock()
        args_2 = args
        args_2.remove(args_2[0])
        args_2.remove(args_2[0])
        csnapshot = args_2[0]
        args_2.remove(args_2[0])
        run_tree(fstree, csnapshot, str(" ").join(args_2))
        ast_unlock()
    elif arg == "tree-rmpkg" or arg == "tremove" and (lock != True):
        ast_lock()
        args_2 = args
        args_2.remove(args_2[0])
        args_2.remove(args_2[0])
        csnapshot = args_2[0]
        args_2.remove(args_2[0])
        remove(csnapshot, str(" ").join(args_2))
        remove_from_tree(fstree, csnapshot, str(" ").join(args_2))
        ast_unlock()
    elif arg == "tree":
        show_fstree()
    elif arg == "subs":
        list_subvolumes()
    elif arg == "dist" or arg == "distro" or arg == "distros":
        switch_distro()
    elif arg == "immenable" or arg == "immen":
        ast_lock()
        immutability_enable(args[args.index(arg)+1])
        ast_unlock()
    elif arg == "immdisable" or arg == "immdis":
        ast_lock()
        immutability_disable(args[args.index(arg)+1])
        ast_unlock()
    else:
        print("Operation not found.")

#   Call main
if __name__ == "__main__":
    from anytree.importer import DictImporter
    from anytree.exporter import DictExporter
    import anytree
    args = list(sys.argv)
    distro = subprocess.check_output(['sh', '/usr/bin/detect_os.sh']).decode('utf-8').replace('"', "").strip()
    main(args)

