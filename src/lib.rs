mod detect_distro;
mod distros;
mod tree;

use crate::detect_distro as detect;

use configparser::ini::Ini;
use curl::easy::{Easy, HttpVersion, List, SslVersion};
use libbtrfsutil::{create_snapshot, CreateSnapshotFlags, create_subvolume, CreateSubvolumeFlags,
                   delete_subvolume, DeleteSubvolumeFlags, set_subvolume_read_only};
use nix::mount::{mount, MntFlags, MsFlags, umount2};
use partition_identity::{PartitionID, PartitionSource};
use proc_mounts::MountIter;
use std::collections::HashMap;
use std::fs::{copy, DirBuilder, File, OpenOptions, read_dir, read_to_string};
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, stdin, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use tempfile::TempDir;
use tree::*;
use walkdir::{DirEntry, WalkDir};

cfg_if::cfg_if! {
    if #[cfg(feature = "arch")] {
        use distros::arch::ashpk::*;
    }
}

// Ash chroot mounts
pub fn ash_mounts(i: &str, chr: &str) -> nix::Result<()> {
    let snapshot_path = format!("/.snapshots/rootfs/snapshot-{}{}", chr, i);

    // Mount snapshot to itself as a bind mount
    mount(Some(snapshot_path.as_str()), snapshot_path.as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>)?;
    // Mount /dev
    mount(Some("/dev"), format!("{}/dev", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>)?;
    // Mount /etc
    mount(Some("/etc"), format!("{}/etc", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>)?;
    // Mount /home
    mount(Some("/home"), format!("{}/home", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>)?;
    // Mount /proc
    mount(Some("/proc"), format!("{}/proc", snapshot_path).as_str(),
          Some("proc"), MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV, None::<&str>)?;
    // Mount /root
    mount(Some("/root"), format!("{}/root", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>)?;
    // Mount /run
    mount(Some("/run"), format!("{}/run", snapshot_path).as_str(),
          Some("tmpfs"), MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE, None::<&str>)?;
    // Mount /sys
    mount(Some("/sys"), format!("{}/sys", snapshot_path).as_str(),
          Some("sysfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>)?;
    // Mount /tmp
    mount(Some("/tmp"), format!("{}/tmp", snapshot_path).as_str(),
          Some("tmpfs"), MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE, None::<&str>)?;
    // Mount /var
    mount(Some("/var"), format!("{}/var", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>)?;

    // Check EFI
    if is_efi() {
        // Mount /sys/firmware/efi/efivars
        mount(Some("/sys/firmware/efi/efivars"), format!("{}/sys/firmware/efi/efivars", snapshot_path).as_str(),
              Some("efivarfs"), MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE, None::<&str>)?;
    }

    // Mount /etc/resolv.conf
    mount(Some("/etc/resolv.conf"), format!("{}/etc/resolv.conf", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>)?;

    Ok(())
}

// Ash chroot umounts
pub fn ash_umounts(i: &str, chr: &str) -> nix::Result<()> {
    // Unmount in reverse order
    let snapshot_path = format!("/.snapshots/rootfs/snapshot-{}{}", chr, i);

    // Unmount /etc/resolv.conf
    umount2(Path::new(&format!("{}/etc/resolv.conf", snapshot_path)),
            MntFlags::empty())?;

    // Check EFI
    if is_efi() {
        // Umount /sys/firmware/efi/efivars
        umount2(Path::new(&format!("{}/sys/firmware/efi/efivars", snapshot_path)),
                MntFlags::empty())?;
    }

    // Unmount /var
    umount2(Path::new(&format!("{}/var", snapshot_path)),
            MntFlags::empty())?;
    // Unmount chroot /tmp
    umount2(Path::new(&format!("{}/tmp", snapshot_path)),
            MntFlags::MNT_DETACH)?;
    // Unmount chroot /sys
    umount2(Path::new(&format!("{}/sys", snapshot_path)),
            MntFlags::MNT_DETACH)?;
    // Unmount chroot /run
    umount2(Path::new(&format!("{}/run", snapshot_path)),
            MntFlags::MNT_DETACH)?;
    // Unmount chroot /root
    umount2(Path::new(&format!("{}/root", snapshot_path)),
            MntFlags::MNT_DETACH)?;
    // Unmount chroot /proc
    umount2(Path::new(&format!("{}/proc", snapshot_path)),
            MntFlags::MNT_DETACH)?;
    // Unmount chroot /home
    umount2(Path::new(&format!("{}/home", snapshot_path)),
            MntFlags::MNT_DETACH)?;
    // Unmount chroot /etc
    umount2(Path::new(&format!("{}/etc", snapshot_path)),
            MntFlags::MNT_DETACH)?;
    // Unmount chroot /dev
    umount2(Path::new(&format!("{}/dev", snapshot_path)),
            MntFlags::MNT_DETACH)?;
    // Unmount chroot directory
    umount2(Path::new(&format!("{}", snapshot_path)),
            MntFlags::MNT_DETACH)?;

    Ok(())
}

//Ash version
pub fn ash_version() -> Result<String, Error> {
    let pkg = "ash";
    let version = pkg_query(pkg)?.to_string();
    Ok(version)
}

// Add node to branch
pub fn branch_create(snapshot: &str, desc: &str) -> Result<i32, Error> {
    // Find the next available snapshot number
    let i = find_new();

    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot branch as snapshot {} doesn't exist.", snapshot)));

    } else {
        // Check mutability
        let immutability: CreateSnapshotFlags = if check_mutability(snapshot) {
            CreateSnapshotFlags::empty()
        } else {
            CreateSnapshotFlags::READ_ONLY
        };

        // Create snapshot
        create_snapshot(format!("/.snapshots/boot/boot-{}", snapshot),
                        format!("/.snapshots/boot/boot-{}", i),
                        immutability, None).unwrap();
        create_snapshot(format!("/.snapshots/etc/etc-{}", snapshot),
                        format!("/.snapshots/etc/etc-{}", i),
                        immutability, None).unwrap();
        create_snapshot(format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                        format!("/.snapshots/rootfs/snapshot-{}", i),
                        immutability, None).unwrap();

        // Mark newly created snapshot as mutable
        if immutability ==  CreateSnapshotFlags::empty() {
            File::create(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", i))?;
        }

        // Import tree file
        let tree = fstree().unwrap();
        // Add child to node
        add_node_to_parent(&tree, snapshot, i).unwrap();
        // Save tree to fstree
        write_tree(&tree)?;
        // Write description for snapshot
        if !desc.is_empty() {
            write_desc(&i.to_string(), &desc, true)?;
        }
    }
    Ok(i)
}

// Check if snapshot is mutable
pub fn check_mutability(snapshot: &str) -> bool {
    Path::new(&format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", snapshot))
        .try_exists().unwrap()
}

// Check if last update was successful
pub fn check_update() -> Result<(), Error> {
    // Open and read upstate file
    let upstate = File::open("/.snapshots/ash/upstate")?;
    let buf_read = BufReader::new(upstate);
    let mut read = buf_read.lines();

    // Read state line
    let line = read.next().unwrap()?;
    // Read data line
    let data = read.next().unwrap()?;

    // Check state line
    if line.contains("1") {
        eprintln!("Last update on {} failed.", data);
    }
    if line.contains("0") {
        println!("Last update on {} completed successfully.", data);
    }
    Ok(())
}

// Clean chroot mount directories for a snapshot
pub fn chr_delete(snapshot: &str) -> Result<(), Error> {
    // Path to boot mount directory
    let boot_path = format!("/.snapshots/boot/boot-chr{}", snapshot);
    // Path to etc mount directory
    let etc_path = format!("/.snapshots/etc/etc-chr{}", snapshot);
    // Path to snapshot mount directory
    let snapshot_path = format!("/.snapshots/rootfs/snapshot-chr{}", snapshot);

    // Delete boot,etc and snapshot subvolumes
    if Path::new(&snapshot_path).try_exists()? {
        delete_subvolume(&boot_path, DeleteSubvolumeFlags::empty()).unwrap();
        delete_subvolume(&etc_path, DeleteSubvolumeFlags::empty()).unwrap();
        delete_subvolume(&snapshot_path, DeleteSubvolumeFlags::empty()).unwrap();
    }
    Ok(())
}

// Run command in snapshot
pub fn chroot(snapshot: &str, cmds: Vec<String>) -> Result<(), Error> {
    let path = format!("/.snapshots/rootfs/snapshot-chr{}", snapshot);

    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists()? {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot clone as snapshot {} doesn't exist.", snapshot)));

    } else if Path::new(&path).try_exists()? {
        // Make sure snapshot is not in use by another ash process
        return Err(Error::new
                   (ErrorKind::Unsupported,
                    format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.",
                            snapshot,snapshot)));

    } else if snapshot == "0" {
        // Make sure snapshot is not base snapshot
        return Err(Error::new(ErrorKind::Unsupported, format!("Changing base snapshot is not allowed.")));

    } else {
        // Prepare snapshot for chroot and run command if existed
        if !cmds.is_empty() {
            // Chroot to snapshot path
            for  cmd in cmds {
                if prepare(snapshot).is_ok() {
                    // Run command in chroot
                    if chroot_exec(&path, &cmd)?.success() {
                        // Make sure post_transactions exit properly
                        match post_transactions(snapshot) {
                            Ok(()) => {
                            }
                            Err(error) => {
                                eprintln!("post_transactions error: {}", error);
                                // Clean chroot mount directories if command failed
                                chr_delete(snapshot)?;
                            }
                        }
                    } else {
                        // Exit chroot and unlock snapshot
                        chr_delete(snapshot)?;
                    }
                } else {
                    // Unlock snapshot
                    chr_delete(snapshot)?;
                }
            }
        } else if prepare(snapshot).is_ok() {
            // Chroot
            if chroot_in(&path)?.code().is_some() {
                // Make sure post_transactions exit properly
                match post_transactions(snapshot) {
                    Ok(()) => {
                    }
                    Err(error) => {
                        eprintln!("post_transactions error: {}", error);
                        // Clean chroot mount directories if command failed
                        chr_delete(snapshot)?;
                    }
                }
            } else {
                // Unlock snapshot
                chr_delete(snapshot)?;
            }
        } else {
            // Unlock snapshot
            chr_delete(snapshot)?;
        }
    }
    Ok(())
}

// Check if inside chroot
pub fn chroot_check() -> bool {
    let read = read_to_string("/proc/mounts").unwrap();
    if read.contains("/.snapshots btrfs") {
        return false;
    } else {
        return true;
    }
}

// Run command in chroot
pub fn chroot_exec(path: &str,cmd: &str) -> Result<ExitStatus, Error> {
    let exocde = Command::new("sh").arg("-c").arg(format!("chroot {} {}", path,cmd)).status();
    exocde
}

// Enter chroot
pub fn chroot_in(path: &str) -> Result<ExitStatus, Error> {
    let excode = Command::new("chroot").arg(path).status();
    excode
}

// Clone tree
pub fn clone_as_tree(snapshot: &str, desc: &str) -> Result<i32, Error> {
    // Find the next available snapshot number
    let i = find_new();

    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot clone as snapshot {} doesn't exist.", snapshot)));

    } else {
        // Make snapshot mutable or immutable
        let immutability: CreateSnapshotFlags = if check_mutability(snapshot) {
            CreateSnapshotFlags::empty()
        } else {
            CreateSnapshotFlags::READ_ONLY
        };

        // Create snapshot
        create_snapshot(format!("/.snapshots/boot/boot-{}", snapshot),
                        format!("/.snapshots/boot/boot-{}", i),
                        immutability, None).unwrap();
        create_snapshot(format!("/.snapshots/etc/etc-{}", snapshot),
                        format!("/.snapshots/etc/etc-{}", i),
                        immutability, None).unwrap();
        create_snapshot(format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                        format!("/.snapshots/rootfs/snapshot-{}", i),
                        immutability, None).unwrap();

        // Mark newly created snapshot as mutable
        if immutability ==  CreateSnapshotFlags::empty() {
            File::create(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", i))?;
        }

        // Import tree file
        let tree = fstree().unwrap();
        // Add to root tree
        append_base_tree(&tree, i).unwrap();
        // Save tree to fstree
        write_tree(&tree)?;
        // Write description for snapshot
        if desc.is_empty() {
            let description = format!("clone of {}.", snapshot);
            write_desc(&i.to_string(), &description, true)?;
        } else {
            write_desc(&i.to_string(), &desc, true)?;
        }
    }
    Ok(i)
}

// Clone branch under same parent
pub fn clone_branch(snapshot: &str) -> Result<i32, Error> {
    // Find the next available snapshot number
    let i = find_new();

    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot clone as snapshot {} doesn't exist.", snapshot)));

    } else {
        // Make snapshot mutable or immutable
        let immutability: CreateSnapshotFlags = if check_mutability(snapshot) {
            CreateSnapshotFlags::empty()
        } else {
            CreateSnapshotFlags::READ_ONLY
        };

        // Create snapshot
        create_snapshot(format!("/.snapshots/boot/boot-{}", snapshot),
                        format!("/.snapshots/boot/boot-{}", i),
                        immutability, None).unwrap();
        create_snapshot(format!("/.snapshots/etc/etc-{}", snapshot),
                        format!("/.snapshots/etc/etc-{}", i),
                        immutability, None).unwrap();
        create_snapshot(format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                        format!("/.snapshots/rootfs/snapshot-{}", i),
                        immutability, None).unwrap();

        // Mark newly created snapshot as mutable
        if immutability ==  CreateSnapshotFlags::empty() {
            File::create(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", i))?;
        }

        // Import tree file
        let tree = fstree().unwrap();
        // Clone within node
        add_node_to_level(&tree, snapshot, i).unwrap();
        // Save tree to fstree
        write_tree(&tree)?;
        // Write description for snapshot
        let desc = format!("clone of {}.", snapshot);
        write_desc(&i.to_string(), &desc, true)?;
    }
    Ok(i)
}

// Recursively clone an entire tree
pub fn clone_recursive(snapshot: &str) -> Result<(), Error> {
    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot clone as snapshot {} doesn't exist.", snapshot)));

    } else {
        // Import tree file
        let tree = fstree().unwrap();
        // Clone its branch and replace the original child with the clone
        let mut children = return_children(&tree, snapshot);
        let ch = children.clone();
        children.insert(0, snapshot.to_string());
        let ntree = clone_branch(snapshot)?;
        let mut new_children = ch.clone();
        new_children.insert(0, ntree.to_string());

        // Clone each child's branch under the corresponding parent in the new children list
        for child in ch {
            let parent = get_parent(&tree, &child).unwrap().to_string();
            let index = children.iter().position(|x| x == &parent).unwrap();
            let i = clone_under(&new_children[index], &child)?;
            new_children[index] = i.to_string();
        }
    }
    Ok(())
}

// Clone under specified parent
pub fn clone_under(snapshot: &str, branch: &str) -> Result<i32, Error> {
    // Find the next available snapshot number
    let i = find_new();

    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot clone as snapshot {} doesn't exist.", snapshot)));

        // Make sure branch does exist
        } else if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", branch)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot clone as snapshot {} doesn't exist.", branch)));

    } else {

        // Check mutability
        let immutability: CreateSnapshotFlags = if check_mutability(snapshot) {
            CreateSnapshotFlags::empty()
        } else {
            CreateSnapshotFlags::READ_ONLY
        };

        // Create snapshot
        create_snapshot(format!("/.snapshots/boot/boot-{}", snapshot),
                        format!("/.snapshots/boot/boot-{}", i),
                        immutability, None).unwrap();
        create_snapshot(format!("/.snapshots/etc/etc-{}", snapshot),
                        format!("/.snapshots/etc/etc-{}", i),
                        immutability, None).unwrap();
        create_snapshot(format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                        format!("/.snapshots/rootfs/snapshot-{}", i),
                        immutability, None).unwrap();

        // Mark newly created snapshot as mutable
        if immutability ==  CreateSnapshotFlags::empty() {
            File::create(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", i))?;
        }

        // Import tree file
        let tree = fstree().unwrap();
        // Add child to node
        add_node_to_parent(&tree, snapshot, i).unwrap();
        // Save tree to fstree
        write_tree(&tree)?;
        // Write description for snapshot
        let desc = format!("clone of {}.", branch);
        write_desc(&i.to_string(), &desc, true)?;
    }
    Ok(i)
}

// Everything after '#' is a comment
fn comment_after_hash(line: &mut String) -> &str {
    if line.contains("#") {
        let line = line.split("#").next().unwrap();
        return line;
    } else {
        return line;
    }
}

// Delete deploys subvolumes //TODO
pub fn delete_deploys() -> Result<(), Error> {
    for snap in ["deploy", "deploy-aux"] {
        if Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snap)).try_exists().unwrap() {
            delete_subvolume(format!("/.snapshots/boot/boot-{}", snap), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(format!("/.snapshots/etc/etc-{}", snap), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(format!("/.snapshots/rootfs/snapshot-{}", snap), DeleteSubvolumeFlags::empty()).unwrap();
        }

        // Make sure temporary chroot directories are deleted as well
        if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snap)).try_exists().unwrap() {
            delete_subvolume(format!("/.snapshots/boot/boot-chr{}", snap), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(format!("/.snapshots/etc/etc-chr{}", snap), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(format!("/.snapshots/rootfs/snapshot-chr{}", snap), DeleteSubvolumeFlags::empty()).unwrap();
        }
    }
    Ok(())
}

// Delete tree or branch
pub fn delete_node(snapshots: &Vec<String>, quiet: bool, nuke: bool) -> Result<(), Error> {
    // Get some values
    let current_snapshot = get_current_snapshot();
    let next_snapshot = get_next_snapshot(false);
    let mut run = false;

    // Iterating over snapshots
    for snapshot in snapshots {
        // Make sure snapshot is not base snapshot
        if snapshot.as_str() == "0" {
            return Err(Error::new(ErrorKind::Unsupported, format!("Changing base snapshot is not allowed.")));
        }

        if !nuke {
            // Make sure snapshot does exist
            if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
                return Err(Error::new(ErrorKind::NotFound, format!(
                    "Cannot delete as snapshot {} doesn't exist.", snapshot)));

                // Make sure snapshot is not current working snapshot
                } else if snapshot == &current_snapshot {
                return Err(Error::new(ErrorKind::Unsupported, format!(
                    "Cannot delete booted snapshot.")));

                // Make sure snapshot is not deploy snapshot
                } else if snapshot == &next_snapshot {
                return Err(Error::new(ErrorKind::Unsupported, format!(
                    "Cannot delete deployed snapshot.")));

                // Abort if not quiet and confirmation message is false
                } else if !quiet && !yes_no(&format!("Are you sure you want to delete snapshot {}?", snapshot)) {
                return Err(Error::new(ErrorKind::Interrupted, format!(
                    "Aborted.")));
            } else {
                run = true;
            }
        }

        if nuke | run {
            // Delete snapshot
            let tree = fstree().unwrap();
            let children = return_children(&tree, &snapshot);
            let desc_path = format!("/.snapshots/ash/snapshots/{}-desc", snapshot);
            std::fs::remove_file(desc_path)?;
            delete_subvolume(format!("/.snapshots/boot/boot-{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(format!("/.snapshots/etc/etc-{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(format!("/.snapshots/rootfs/snapshot-{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();

            // Make sure temporary chroot directories are deleted as well
            if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
                delete_subvolume(format!("/.snapshots/boot/boot-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
                delete_subvolume(format!("/.snapshots/etc/etc-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
                delete_subvolume(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
            }

            for child in children {
                // This deletes the node itself along with its children
                let desc_path = format!("/.snapshots/ash/snapshots/{}-desc", child);
                std::fs::remove_file(desc_path)?;
                delete_subvolume(&format!("/.snapshots/boot/boot-{}", child), DeleteSubvolumeFlags::empty()).unwrap();
                delete_subvolume(format!("/.snapshots/etc/etc-{}", child), DeleteSubvolumeFlags::empty()).unwrap();
                delete_subvolume(format!("/.snapshots/rootfs/snapshot-{}", child), DeleteSubvolumeFlags::empty()).unwrap();
                if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", child)).try_exists().unwrap() {
                    delete_subvolume(format!("/.snapshots/boot/boot-chr{}", child), DeleteSubvolumeFlags::empty()).unwrap();
                    delete_subvolume(format!("/.snapshots/etc/etc-chr{}", child), DeleteSubvolumeFlags::empty()).unwrap();
                    delete_subvolume(format!("/.snapshots/rootfs/snapshot-chr{}", child), DeleteSubvolumeFlags::empty()).unwrap();
                }
            }

            // Remove node from tree or root
            remove_node(&tree, snapshot).unwrap();
            write_tree(&tree)?;
        }
    }
    Ok(())
}

// Delete old grub.cfg
pub fn delete_old_grub_files(grub: &str) -> Result<(), Error> {
    let bak_path = Path::new(grub).join("BAK");
    let cutoff_time = std::time::SystemTime::now() - std::time::Duration::from_secs(30 * 24 * 60 * 60);
    for entry in WalkDir::new(bak_path) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.to_str().unwrap().contains("grub.cfg") && path.metadata()?.modified()? < cutoff_time {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

// Deploy snapshot
pub fn deploy(snapshot: &str, secondary: bool, reset: bool) -> Result<(), Error> {
    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot deploy as snapshot {} doesn't exist.", snapshot)));

    } else {
        update_boot(snapshot, secondary)?;

        // Set default volume
        let tmp = get_tmp();
        Command::new("btrfs").args(["sub", "set-default"])
                             .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                             .output()?;
        tmp_delete(secondary)?;
        let tmp = get_aux_tmp(tmp, secondary);

        // Special mutable directories
        let options = snapshot_config_get(snapshot);
        let mutable_dirs: Vec<&str> = options.get("mutable_dirs")
                                             .map(|dirs| {dirs.split(',').flat_map(|dir| {
                                                 if let Some(index) = dir.find("::") {
                                                     vec![&dir[..index], &dir[index + 2..]]
                                                 } else {
                                                     vec![dir]
                                                 }
                                             }).filter(|dir| !dir.trim().is_empty()).collect()})
                                             .unwrap_or_else(|| Vec::new());
        let mutable_dirs_shared: Vec<&str> = options.get("mutable_dirs_shared")
                                                    .map(|dirs| {dirs.split(',').flat_map(|dir| {
                                                        if let Some(index) = dir.find("::") {
                                                            vec![&dir[..index], &dir[index + 2..]]
                                                        } else {
                                                            vec![dir]
                                                        }
                                                    }).filter(|dir| !dir.trim().is_empty()).collect()})
                                                    .unwrap_or_else(|| Vec::new());

        // btrfs snapshot operations
        create_snapshot(format!("/.snapshots/boot/boot-{}", snapshot),
                        format!("/.snapshots/boot/boot-{}", tmp),
                        CreateSnapshotFlags::empty(), None).unwrap();
        create_snapshot(format!("/.snapshots/etc/etc-{}", snapshot),
                        format!("/.snapshots/etc/etc-{}", tmp),
                        CreateSnapshotFlags::empty(), None).unwrap();
        create_snapshot(format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                        format!("/.snapshots/rootfs/snapshot-{}", tmp),
                        CreateSnapshotFlags::empty(), None).unwrap();
        DirBuilder::new().recursive(true)
                         .create(format!("/.snapshots/rootfs/snapshot-{}/boot", tmp))?;
        DirBuilder::new().recursive(true)
                         .create(format!("/.snapshots/rootfs/snapshot-{}/etc", tmp))?;
        std::fs::remove_dir_all(&format!("/.snapshots/rootfs/snapshot-{}/var", tmp))?;
        Command::new("cp").args(["-r", "--reflink=auto"])
                          .arg(format!("/.snapshots/boot/boot-{}/.", snapshot))
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/boot", tmp))
                          .output()?;
        Command::new("cp").args(["-r", "--reflink=auto"])
                          .arg(format!("/.snapshots/etc/etc-{}/.", snapshot))
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/etc", tmp))
                          .output()?;

        // If snapshot is mutable, modify '/' entry in fstab to read-write
        if check_mutability(snapshot) {
            let mut fstab_file = File::open(format!("/.snapshots/rootfs/snapshot-{}/etc/fstab", tmp))?;
            let mut contents = String::new();
            fstab_file.read_to_string(&mut contents)?;

            let pattern = format!("snapshot-{}", tmp);
            if let Some(index) = contents.find(&pattern) {
                if let Some(end) = contents[index..].find(",ro") {
                    let replace_index = index + end;
                    let mut new_contents = String::with_capacity(contents.len());
                    new_contents.push_str(&contents[..replace_index]);
                    new_contents.push_str(&contents[replace_index + 3..]);
                    std::fs::write(format!("/.snapshots/rootfs/snapshot-{}/etc/fstab", tmp), new_contents)?;
                }
            }
        }

        // Add special user-defined mutable directories as bind-mounts into fstab
        if !mutable_dirs.is_empty() {
            for mount_path in mutable_dirs {
                let source_path = format!("/.snapshots/mutable_dirs/snapshot-{}/{}", snapshot,mount_path);
                DirBuilder::new().recursive(true)
                                 .create(format!("/.snapshots/mutable_dirs/snapshot-{}/{}", snapshot,mount_path))?;
                DirBuilder::new().recursive(true)
                                 .create(format!("/.snapshots/rootfs/snapshot-{}/{}", tmp,mount_path))?;
                let fstab = format!("{} /{} none defaults,bind 0 0", source_path,mount_path);
                let mut fstab_file = OpenOptions::new().append(true)
                                                       .create(true)
                                                       .read(true)
                                                       .open(format!("/.snapshots/rootfs/snapshot-{}/etc/fstab", tmp))?;
                fstab_file.write_all(format!("{}\n", fstab).as_bytes())?;
            }
        }

        // Same thing but for shared directories
        if !mutable_dirs_shared.is_empty() {
            for mount_path in mutable_dirs_shared {
                let source_path = format!("/.snapshots/mutable_dirs/{}", mount_path);
                DirBuilder::new().recursive(true)
                                 .create(format!("/.snapshots/mutable_dirs/{}", mount_path))?;
                DirBuilder::new().recursive(true)
                                 .create(format!("/.snapshots/rootfs/snapshot-{}/{}", tmp,mount_path))?;
                let fstab = format!("{} /{} none defaults,bind 0 0", source_path,mount_path);
                let mut fstab_file = OpenOptions::new().append(true)
                                                       .create(true)
                                                       .read(true)
                                                       .open(format!("/.snapshots/rootfs/snapshot-{}/etc/fstab", tmp))?;
               fstab_file.write_all(format!("{}\n", fstab).as_bytes())?;
            }
        }
        create_snapshot("/var",
                        format!("/.snapshots/rootfs/snapshot-{}/var", tmp),
                        CreateSnapshotFlags::empty(), None).unwrap();
        let snap_num = format!("{}", snapshot);
        let mut snap_file = OpenOptions::new().truncate(true)
                                              .create(true)
                                              .read(true)
                                              .write(true)
                                              .open(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", tmp))?;
        snap_file.write_all(snap_num.as_bytes())?;
        switch_tmp(secondary, reset)?;
        init_system_clean(&tmp, "deploy")?;

        // Set default volume
        Command::new("btrfs").args(["sub", "set-default"])
                             .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                             .output()?;
    }
    Ok(())
}

// Show diff of packages
pub fn diff(snapshot1: &str, snapshot2: &str) {
    let diff = snapshot_diff(snapshot1, snapshot2);
    match diff {
        Ok(diff) => diff,
        Err(e) => eprintln!("{}", e),
    }
}

// Find new unused snapshot dir
pub fn find_new() -> i32 {
    let mut i = 0;
    let boots = read_dir("/.snapshots/boot")
        .unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
    let etcs = read_dir("/.snapshots/etc")
        .unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
    //let vars = read_dir("/.snapshots/var")
        //.unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>(); // Can this be deleted?
    let mut snapshots = read_dir("/.snapshots/rootfs")
        .unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
    snapshots.append(&mut etcs.clone());
    //snapshots.append(&mut vars.clone());
    snapshots.append(&mut boots.clone());

    loop {
        i += 1;
        if !snapshots.contains
            (&PathBuf::from(format!("/.snapshots/rootfs/snapshot-{}", i))) && !snapshots
            .contains
            (&PathBuf::from(format!("/.snapshots/etc/etc-{}", i))) && !snapshots
            /*.contains
            (&PathBuf::from(format!("var-{}", i))) && !snapshots*/.contains
            (&PathBuf::from(format!("/.snapshots/boot/boot-{}", i))) {
                break i;
        }
    }
}

// FixDB
pub fn fixdb(snapshot: &str) -> Result<(), Error> {
    fix_package_db(snapshot)
}

// Get aux tmp
pub fn get_aux_tmp(tmp: String, secondary: bool) -> String {
    let tmp = if secondary {
        if tmp.contains("secondary") {
            tmp.replace("-secondary", "")
        } else {
            format!("{}-secondary", tmp)
        }
    } else {
        if tmp.contains("deploy-aux") {
            tmp.replace("deploy-aux", "deploy")
        } else {
            tmp.replace("deploy", "deploy-aux")
        }
    };
    tmp
}

// Get current snapshot
pub fn get_current_snapshot() -> String {
    let csnapshot = read_to_string("/usr/share/ash/snap").unwrap();
    csnapshot.trim_end().to_string()
}

// This function returns either empty string or underscore plus name of distro if it was appended to sub-volume names to distinguish
pub fn get_distro_suffix(distro: &str) -> String {
    if distro.contains("ashos") {
        return format!("_{}", distro.replace("_ashos", ""));
    } else {
        std::process::exit(1);
    }
}

// Get Grub path
fn get_grub() -> Option<String> {
    let boot_dir = "/boot";
    let grub_dirs: Vec<String> = WalkDir::new(boot_dir)
        .into_iter()
        .filter_map(|entry| {
            let entry: DirEntry = entry.unwrap();
            if entry.file_type().is_dir() {
                if let Some(dir_path) = entry.path().file_name() {
                    if dir_path == "grub" {
                        return Some(entry.path().strip_prefix("/boot/").unwrap().to_string_lossy().into_owned());
                    }
                }
            }
            None
        })
        .collect();
    grub_dirs.get(0).cloned()
}

// Get deployed snapshot
pub fn get_next_snapshot(secondary: bool) -> String {
    let tmp = get_tmp();
    let d = get_aux_tmp(tmp, secondary);

    // Make sure next snapshot exists
    if Path::new(&format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", d)).try_exists().unwrap() {
        let mut file = File::open(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", d)).unwrap();
        let mut contents = String::new();
        let csnapshot = file.read_to_string(&mut contents).unwrap();
        return csnapshot.to_string().trim().to_string();
    } else {
        // Return empty string in case no snapshot is deploye
        return "".to_string()
    }
}

// Get drive partition
pub fn get_part() -> String {
    // Read part UUID
    let mut file = File::open("/.snapshots/ash/part").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    // Get partition path from UUID
    let cpart = PartitionID::new(PartitionSource::UUID, contents.trim_end().to_string());
    cpart.get_device_path().unwrap().to_string_lossy().into_owned()
}

// Get tmp partition state
pub fn get_tmp() -> String {
    // By default just return which deployment is running
    let file = File::open("/proc/mounts").unwrap();
    let reader = BufReader::new(file);
    let mount: Vec<String> = reader.lines().filter_map(|line| {
        let line = line.unwrap();
        if line.contains(" / btrfs") {
            Some(line)
        } else {
            None
        }
    })
    .collect();
    if mount.iter().any(|element| element.contains("deploy-aux-secondary")) {
        let r = String::from("deploy-aux-secondary");
        return r;
    } else if mount.iter().any(|element| element.contains("deploy-secondary")) {
        let r = String::from("deploy-secondary");
        return r;
    } else if mount.iter().any(|element| element.contains("deploy-aux")) {
        let r = String::from("deploy-aux");
        return r;
    } else {
        let r = String::from("deploy");
        return r;
    }
}

// Make a snapshot vulnerable to be modified even further (snapshot should be deployed as mutable)
pub fn hollow(snapshot: &str) -> Result<(), Error> {
    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot make hollow as snapshot {} doesn't exist.", snapshot)));

        // Make sure snapshot is not in use by another ash process
        } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        return Err(
            Error::new(
                ErrorKind::Unsupported,
                format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.",
                        snapshot,snapshot)));

        // Make sure snapshot is not  base snapshot
        } else if snapshot == "0" {
        return Err(Error::new(ErrorKind::Unsupported, format!("Changing base snapshot is not allowed.")));

    } else {
        // AUR step might be needed and if so make a distro_specific function with steps similar to install_package
        // Call it hollow_helper and change this accordingly
        prepare(snapshot)?;
        // Mount root
        mount(Some("/"), format!("/.snapshots/rootfs/snapshot-chr{}", snapshot).as_str(),
              Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE, None::<&str>)?;
        // Deploy or not
        if yes_no(&format!("Snapshot {} is now hollow! Whenever done, type yes to deploy and no to discard", snapshot)) {
            post_transactions(snapshot)?;
            immutability_enable(snapshot)?;
            deploy(snapshot, false, false)?;
        } else {
            chr_delete(snapshot)?;
            return Err(Error::new(ErrorKind::Interrupted,
                                  format!("No changes applied on snapshot {}.", snapshot)));
        }
    }
    Ok(())
}

// Make a node mutable
pub fn immutability_disable(snapshot: &str) -> Result<(), Error> {
    // If not base snapshot
    if snapshot != "0" {
        // Make sure snapshot exists
        if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
            return Err(Error::new(ErrorKind::NotFound, format!("Snapshot {} doesn't exist.", snapshot)));

        } else {
            // Make sure snapshot is not already mutable
            if check_mutability(snapshot) {
                return Err(Error::new(ErrorKind::AlreadyExists,
                                      format!("Snapshot {} is already mutable.", snapshot)));

            } else {
                // Disable immutability
                set_subvolume_read_only(format!("/.snapshots/rootfs/snapshot-{}", snapshot), false).unwrap();
                File::create(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", snapshot))?;
                write_desc(snapshot, " MUTABLE ", false)?;
            }
        }

    } else {
        // Base snapshot unsupported
        return Err(Error::new(ErrorKind::Unsupported, format!("Snapshot 0 (base) should not be modified.")));
    }
    Ok(())
}

//Make a node immutable
pub fn immutability_enable(snapshot: &str) -> Result<(), Error> {
    // If not base snapshot
    if snapshot != "0" {
        // Make sure snapshot exists
        if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
            return Err(Error::new(ErrorKind::NotFound, format!("Snapshot {} doesn't exist.", snapshot)));

        } else {
            // Make sure snapshot is not already immutable
            if !check_mutability(snapshot) {
                return Err(Error::new(ErrorKind::AlreadyExists,
                                      format!("Snapshot {} is already immutable.", snapshot)));
            } else {
                // Enable immutability
                std::fs::remove_file(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", snapshot))?;
                set_subvolume_read_only(format!("/.snapshots/rootfs/snapshot-{}", snapshot), true).unwrap();
                // Read the desc file into a string
                let mut contents = std::fs::read_to_string(format!("/.snapshots/ash/snapshots/{}-desc", snapshot))?;
                // Replace MUTABLE word with an empty string
                contents = contents.replace(" MUTABLE ", "");
                // Write the modified contents back to the file
                std::fs::write(format!("/.snapshots/ash/snapshots/{}-desc", snapshot), contents)?;
            }
        }

    } else {
        // Base snapshot unsupported
        return Err(Error::new(ErrorKind::Unsupported, format!("Snapshot 0 (base) should not be modified.")));
    }
    Ok(())
}

// Install packages
pub fn install(snapshot: &str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot install as snapshot {} doesn't exist.", snapshot)));

        // Make sure snapshot is not in use by another ash process
        } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        return Err(
            Error::new(ErrorKind::Unsupported,
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.",
                               snapshot,snapshot)));

        // Make sure snapshot is not base snapshot
        } else if snapshot == "0" {
        return Err(Error::new(ErrorKind::Unsupported,
                              format!("Changing base snapshot is not allowed.")));

        // Install package
        } else {
        if install_package_helper(snapshot, &pkgs, noconfirm).is_ok() {
            post_transactions(snapshot)?;
            } else {
            chr_delete(snapshot)?;
            return Err(Error::new(ErrorKind::Interrupted,
                                  format!("Install failed and changes discarded.")));
        }
    }
    Ok(())
}

// Install live
pub fn install_live(pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    let snapshot = &get_current_snapshot();
    let tmp = get_tmp();
    ash_mounts(&tmp, "").unwrap();
    install_package_helper_live(snapshot, &tmp, &pkgs, noconfirm)?;
    //println!("Please wait as installation is finishing.");
    ash_umounts(&tmp, "").unwrap();
    Ok(())
}

// Install a profile from a text file
fn install_profile(snapshot: &str, profile: &str, force: bool, secondary: bool, /*section_only: Option<String>,*/
                   user_profile: &str, noconfirm: bool) -> Result<bool, Error> {
    // Get some values
    let dist = detect::distro_id();
    let cfile = format!("/usr/share/ash/profiles/{}/{}.conf", profile,dist);

    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot install as snapshot {} doesn't exist.", snapshot)));

        // Make sure snapshot is not in use by another ash process
        } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        return Err(
            Error::new(ErrorKind::Unsupported,
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.",
                               snapshot,snapshot)));

        // Make sure snapshot is not base snapshot
        } else if snapshot == "0" {
        return Err(Error::new(ErrorKind::Unsupported,
                              format!("Changing base snapshot is not allowed.")));
    } else {
        // Install profile
        println!("Updating the system before installing profile {}.", profile);
        // Prepare
        auto_upgrade(snapshot)?;
        prepare(snapshot)?;

        // profile configurations
        let mut profconf = Ini::new();
        profconf.set_comment_symbols(&['#']);
        profconf.set_multiline(true);
        // Load profile if exist
        if !Path::new(&cfile).try_exists().unwrap() && !force && user_profile.is_empty() {
            profconf.load(&cfile).unwrap();
        } else if force {
            println!("Installing AshOS profiles.");
            install_package_helper(snapshot, &vec!["ash-profiles".to_string()], true)?;
            profconf.load(&cfile).unwrap();
        } else if !user_profile.is_empty() {
            profconf.load(user_profile).unwrap();
        }

        // Read presets section in configuration file
        if profconf.sections().contains(&"presets".to_string()) {
            if !aur_check(snapshot) {
                return Err(Error::new(ErrorKind::Unsupported,
                                      format!("Please enable AUR.")));
            }
        }

        // Read packages section in configuration file
        if profconf.sections().contains(&"packages".to_string()) {
            let mut pkgs: Vec<String> = Vec::new();
            for pkg in profconf.get_map().unwrap().get("packages").unwrap().keys() {
                pkgs.push(pkg.to_string());
            }
            // Install package(s)
            install_package_helper(snapshot, &pkgs, noconfirm)?;
        }

        // Read commands section in configuration file
        if profconf.sections().contains(&"install-commands".to_string()) {
            for cmd in profconf.get_map().unwrap().get("install-commands").unwrap().keys() {
                chroot_exec(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot), cmd)?;
            }
        }
    }
    Ok(secondary)
}

// Install profile in live snapshot
fn install_profile_live(snapshot: &str,profile: &str, force: bool, user_profile: &str, noconfirm: bool) -> Result<(), Error> {
    // Get some values
    let dist = detect::distro_id();
    let cfile = format!("/usr/share/ash/profiles/{}/{}.conf", profile,dist);
    let tmp = get_tmp();

    // Prepare
    if user_profile.is_empty() {
        println!("Updating the system before installing profile {}.", profile);
    } else {
        println!("Updating the system before installing profile {}.", user_profile);
    }
    ash_mounts(&tmp, "")?;
    if upgrade_helper_live(&tmp).success() {

        // profile configurations
        let mut profconf = Ini::new();
        profconf.set_comment_symbols(&['#']);
        profconf.set_multiline(true);

        // Load profile if exist
        if !Path::new(&cfile).try_exists().unwrap() && !force && user_profile.is_empty() {
            profconf.load(&cfile).unwrap();
        } else if force {
            println!("Installing AshOS profiles.");
            install_package_helper_live(snapshot, &tmp, &vec!["ash-profiles".to_string()], true)?;
            profconf.load(&cfile).unwrap();
        } else if !user_profile.is_empty() {
            profconf.load(user_profile).unwrap();
        }

        // Read presets section in configuration file
        if profconf.sections().contains(&"presets".to_string()) {
            if !aur_check(snapshot) {
                return Err(Error::new(ErrorKind::Unsupported,
                                      format!("Please enable AUR.")));
            }
        }

        // Read packages section in configuration file
        if profconf.sections().contains(&"packages".to_string()) {
            let mut pkgs: Vec<String> = Vec::new();
            for pkg in profconf.get_map().unwrap().get("packages").unwrap().keys() {
                pkgs.push(pkg.to_string());
            }
            // Install package(s)
            install_package_helper_live(snapshot, &tmp, &pkgs, noconfirm)?;
        }

        // Read commands section in configuration file
        if profconf.sections().contains(&"install-commands".to_string()) {
            for cmd in profconf.get_map().unwrap().get("install-commands").unwrap().keys() {
                chroot_exec(&format!("/.snapshots/rootfs/snapshot-{}", snapshot), cmd)?;
            }
        }
    } else {
        return Err(Error::new(ErrorKind::Interrupted,
                              format!("System update failed.")));
    }

    // Umounts tmp
    ash_umounts(&tmp, "").unwrap();

    Ok(())
}

// Triage functions for argparse method
pub fn install_triage(snapshot: &str, live: bool, pkgs: Vec<String>, profile: &str, force: bool,
                      user_profile: &str, noconfirm: bool, secondary: bool) -> Result<(), Error> {
    // Switch between profile and user_profile
    let p = if user_profile.is_empty() {
        profile
    } else {
        user_profile
    };

    if !live {
        // Install profile
        if !profile.is_empty() {
            let excode = install_profile(snapshot, profile, force, secondary, user_profile, true);
            match excode {
                Ok(secondary) => {
                    if post_transactions(snapshot).is_ok() {
                        println!("Profile {} installed in snapshot {} successfully.", p,snapshot);
                        println!("Deploying snapshot {}.", snapshot);
                        if deploy(snapshot, secondary, false).is_ok() {
                            println!("Snapshot {} deployed to '/'.", snapshot);
                        }
                    } else {
                        chr_delete(snapshot)?;
                        eprintln!("Install failed and changes discarded!");
                    }
                },
                Err(_) => {
                    chr_delete(snapshot)?;
                    eprintln!("Install failed and changes discarded!");
                },
            }

        } else if !pkgs.is_empty() {
            // Install package
            let excode = install(snapshot, &pkgs, noconfirm);
            match excode {
                Ok(_) => println!("Package(s) {pkgs:?} installed in snapshot {} successfully.", snapshot),
                Err(e) => eprintln!("{}", e),
            }

        } else if !user_profile.is_empty() {
            // Install user_profile
            let excode = install_profile(snapshot, profile, force, secondary, user_profile, true);
            match excode {
                Ok(secondary) => {
                    if post_transactions(snapshot).is_ok() {
                        println!("Profile {} installed in snapshot {} successfully.", p,snapshot);
                        println!("Deploying snapshot {}.", snapshot);
                        if deploy(snapshot, secondary, false).is_ok() {
                            println!("Snapshot {} deployed to '/'.", snapshot);
                        }
                    } else {
                        chr_delete(snapshot)?;
                        eprintln!("Install failed and changes discarded!");
                    }
                },
                Err(_) => {
                    chr_delete(snapshot)?;
                    eprintln!("Install failed and changes discarded!");
                },
            }
        }

    } else if live && snapshot != get_current_snapshot() {
        // Prevent live option if snapshot is not current snapshot
        eprintln!("Can't use the live option with any other snapshot than the current one.");
    } else if live && snapshot == get_current_snapshot() {
        // Do live install only if: live flag is used OR target snapshot is current
        if !profile.is_empty() {
            // Live profile installation
            let excode = install_profile_live(snapshot, profile, force, user_profile, true);
            match excode {
                Ok(_) => println!("Profile {} installed in current/live snapshot.", p),
                Err(e) => eprintln!("{}", e),
            }

        } else if !pkgs.is_empty() {
            // Live package installation
            let excode = install_live(&pkgs, noconfirm);
            match excode {
                Ok(_) => println!("Package(s) {pkgs:?} installed in snapshot {} successfully.", snapshot),
                Err(e) => eprintln!("{}", e),
            }

        } else if !user_profile.is_empty() {
            // Live user_profile installation
            let excode = install_profile_live(snapshot, profile, force, user_profile, true);
            match excode {
                Ok(_) => println!("Profile {} installed in current/live snapshot.", p),
                Err(e) => eprintln!("{}", e),
            }
        }
    }
    Ok(())
}

// Check EFI
pub fn is_efi() -> bool {
    let is_efi = Path::new("/sys/firmware/efi").try_exists().unwrap();
    is_efi
}

// Check if path is mounted
fn is_mounted(path: &Path) -> bool {
    let mount_iter = MountIter::new().unwrap();
    for mount in mount_iter {
        if let Ok(mount) = mount {
            if mount.dest == path {
                return true;
            }
        }
    }
    false
}

// Return if package installed in snapshot
pub fn is_pkg_installed(snapshot: &str, pkg: &str) -> bool {
    if pkg_list(snapshot, "").contains(&pkg.to_string()) {
        return true;
    } else {
        return false;
    }
}

// Package list
pub fn list(snapshot: &str, chr: &str) -> Vec<String> {
    pkg_list(snapshot, chr)
}

// List sub-volumes for the booted distro only
pub fn list_subvolumes() {
    let distro_id = detect::distro_id();
    let args = format!("btrfs sub list / | grep -i {} | sort -f -k 9",
                       &get_distro_suffix(&distro_id));
    Command::new("sh").arg("-c").arg(args).status().unwrap();
}

// Live unlocked shell
pub fn live_unlock() -> Result<(), Error> {
    let tmp = get_tmp();
    ash_mounts(&tmp, "")?;
    chroot_in(&format!("/.snapshots/rootfs/snapshot-{}", tmp))?;
    ash_umounts(&tmp, "")?;
    Ok(())
}

// Auto update
pub fn noninteractive_update(snapshot: &str) -> Result<(), Error> {
    auto_upgrade(snapshot)
}

// Post transaction function, copy from chroot dirs back to read only snapshot dir
pub fn post_transactions(snapshot: &str) -> Result<(), Error> {
    // Some operations were moved below to fix hollow functionality
    let tmp = get_tmp();

    //File operations in snapshot-chr
    remove_dir_content(&format!("/.snapshots/boot/boot-chr{}", snapshot))?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/boot/.", snapshot))
                      .arg(format!("/.snapshots/boot/boot-chr{}", snapshot))
                      .output()?;
    remove_dir_content(&format!("/.snapshots/etc/etc-chr{}", snapshot))?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/etc/.", snapshot))
                      .arg(format!("/.snapshots/etc/etc-chr{}", snapshot))
                      .output()?;

    // Keep package manager's cache after installing packages
    // This prevents unnecessary downloads for each snapshot when upgrading multiple snapshots
    cache_copy(snapshot)?;

    // Delete old snapshot
    delete_subvolume(Path::new(&format!("/.snapshots/boot/boot-{}", snapshot)),
                     DeleteSubvolumeFlags::empty()).unwrap();
    delete_subvolume(Path::new(&format!("/.snapshots/etc/etc-{}", snapshot)),
                     DeleteSubvolumeFlags::empty()).unwrap();
    delete_subvolume(Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)),
                     DeleteSubvolumeFlags::empty()).unwrap();

    // Create mutable or immutable snapshot
    // Mutable
    if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/usr/share/ash/mutable", snapshot)).try_exists().unwrap() {
        create_snapshot(format!("/.snapshots/boot/boot-chr{}", snapshot),
                        format!("/.snapshots/boot/boot-{}", snapshot),
                        CreateSnapshotFlags::empty(), None).unwrap();
        create_snapshot(format!("/.snapshots/etc/etc-chr{}", snapshot),
                        format!("/.snapshots/etc/etc-{}", snapshot),
                        CreateSnapshotFlags::empty(), None).unwrap();
        // Copy init system files to shared
        init_system_copy(&tmp, "post_transactions")?;
        create_snapshot(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot),
                        format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                        CreateSnapshotFlags::empty(), None).unwrap();
    } else {
        // Immutable
        create_snapshot(format!("/.snapshots/boot/boot-chr{}", snapshot),
                        format!("/.snapshots/boot/boot-{}", snapshot),
                        CreateSnapshotFlags::READ_ONLY, None).unwrap();
        create_snapshot(format!("/.snapshots/etc/etc-chr{}", snapshot),
                        format!("/.snapshots/etc/etc-{}", snapshot),
                        CreateSnapshotFlags::READ_ONLY, None).unwrap();
        // Copy init system files to shared
        init_system_copy(&tmp, "post_transactions")?;
        create_snapshot(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot),
                        format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                        CreateSnapshotFlags::READ_ONLY, None).unwrap();
    }

    // Special mutable directories
    let options = snapshot_config_get(snapshot);
    let mutable_dirs: Vec<&str> = options.get("mutable_dirs")
                                                .map(|dirs| {dirs.split(',').flat_map(|dir| {
                                                    if let Some(index) = dir.find("::") {
                                                        vec![&dir[..index], &dir[index + 2..]]
                                                    } else {
                                                        vec![dir]
                                                    }
                                                }).filter(|dir| !dir.trim().is_empty()).collect()})
                                                .unwrap_or_else(|| Vec::new());
    let mutable_dirs_shared: Vec<&str> = options.get("mutable_dirs_shared")
                                                .map(|dirs| {dirs.split(',').flat_map(|dir| {
                                                    if let Some(index) = dir.find("::") {
                                                        vec![&dir[..index], &dir[index + 2..]]
                                                    } else {
                                                        vec![dir]
                                                    }
                                                }).filter(|dir| !dir.trim().is_empty()).collect()})
                                                .unwrap_or_else(|| Vec::new());

    if !mutable_dirs.is_empty() {
        for mount_path in mutable_dirs {
            if is_mounted(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path))) {
                umount2(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path)),
                        MntFlags::MNT_DETACH).unwrap();
            }
        }
    }
    if !mutable_dirs_shared.is_empty() {
        for mount_path in mutable_dirs_shared {
            if is_mounted(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path))) {
                umount2(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path)),
                        MntFlags::MNT_DETACH).unwrap();
            }
        }
    }

    // Fix for hollow functionality
    ash_umounts(snapshot, "chr")?;
    chr_delete(snapshot)?;

    Ok(())
}

// TODO IMPORTANT review 2023 older to revert if hollow introduces issues //NOTE systemd dependent!
pub fn posttrans(snapshot: &str) -> Result<(), Error> {
    let etc = snapshot;
    let tmp = get_tmp();
    ash_umounts(snapshot, "chr")?;
    delete_subvolume(Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)),
                     DeleteSubvolumeFlags::empty()).unwrap();
    remove_dir_content(&format!("/.snapshots/etc/etc-chr{}", snapshot))?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/etc/*", snapshot))
                      .arg(format!("/.snapshots/boot/etc-chr{}", snapshot))
                      .output()?;
    remove_dir_content(&format!("/.snapshots/var/var-chr{}", snapshot))?;
    DirBuilder::new().recursive(true)
                     .create(format!("/.snapshots/var/var-chr{}/lib/systemd", snapshot))?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/lib/systemd/*", snapshot))
                      .arg(format!("/.snapshots/var/var-chr{}/lib/systemd", snapshot))
                      .output()?;
    Command::new("cp").args(["-r", "-n", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/cache/pacman/pkg/*", snapshot))
                      .arg("/var/cache/pacman/pkg/")
                      .output()?;
    remove_dir_content(&format!("/.snapshots/boot/boot-chr{}",  snapshot))?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/boot/*", snapshot))
                      .arg(format!("/.snapshots/boot/boot-chr{}", snapshot))
                      .output()?;
    delete_subvolume(Path::new(&format!("/.snapshots/etc/etc-{}", etc)),
                     DeleteSubvolumeFlags::empty()).unwrap();
    delete_subvolume(Path::new(&format!("/.snapshots/var/var-{}", etc)),
                     DeleteSubvolumeFlags::empty()).unwrap();
    delete_subvolume(Path::new(&format!("/.snapshots/boot/boot-{}", etc)),
                     DeleteSubvolumeFlags::empty()).unwrap();
    create_snapshot(format!("/.snapshots/etc/etc-chr{}", snapshot),
                    format!("/.snapshots/etc/etc-{}", etc),
                    CreateSnapshotFlags::READ_ONLY, None).unwrap();
    create_subvolume(format!("/.snapshots/var/var-{}", etc), CreateSubvolumeFlags::empty(), None).unwrap();
    DirBuilder::new().recursive(true)
                     .create(format!("/.snapshots/var/var-{}/lib/systemd", etc))?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/var/var-chr{}/lib/systemd/*", snapshot))
                      .arg(format!("/.snapshots/var/var-{}/lib/systemd", etc))
                      .output()?;
    remove_dir_content("/var/lib/systemd/")?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-{}/var/lib/systemd/*", tmp))
                      .arg("/var/lib/systemd")
                      .output()?;
    create_snapshot(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot),
                    format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                    CreateSnapshotFlags::READ_ONLY, None).unwrap();
    create_snapshot(format!("/.snapshots/boot/boot-chr{}", snapshot),
                    format!("/.snapshots/boot/boot-{}", etc),
                    CreateSnapshotFlags::READ_ONLY, None).unwrap();
    chr_delete(snapshot)?;
    Ok(())
}

// Prepare snapshot to chroot directory to install or chroot into
pub fn prepare(snapshot: &str) -> Result<(), Error> {
    chr_delete(snapshot)?;
    let snapshot_chr = format!("/.snapshots/rootfs/snapshot-chr{}", snapshot);

    // Create chroot directory
    create_snapshot(format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                    &snapshot_chr,
                    CreateSnapshotFlags::empty(), None).unwrap();

    // Pacman gets weird when chroot directory is not a mountpoint, so the following mount is necessary
    ash_mounts(snapshot, "chr")?;

    // Special mutable directories
    let options = snapshot_config_get(snapshot);
    let mutable_dirs: Vec<&str> = options.get("mutable_dirs")
                                                .map(|dirs| {dirs.split(',').flat_map(|dir| {
                                                    if let Some(index) = dir.find("::") {
                                                        vec![&dir[..index], &dir[index + 2..]]
                                                    } else {
                                                        vec![dir]
                                                    }
                                                }).filter(|dir| !dir.trim().is_empty()).collect()})
                                                .unwrap_or_else(|| Vec::new());
    let mutable_dirs_shared: Vec<&str> = options.get("mutable_dirs_shared")
                                                .map(|dirs| {dirs.split(',').flat_map(|dir| {
                                                    if let Some(index) = dir.find("::") {
                                                        vec![&dir[..index], &dir[index + 2..]]
                                                    } else {
                                                        vec![dir]
                                                    }
                                                }).filter(|dir| !dir.trim().is_empty()).collect()})
                                                .unwrap_or_else(|| Vec::new());

    if !mutable_dirs.is_empty() {
        for mount_path in mutable_dirs {
            // Create mouth_path directory in snapshot
            DirBuilder::new().recursive(true)
                             .create(format!("/.snapshots/mutable_dirs/snapshot-{}/{}", snapshot,mount_path))?;
            // Create mouth_path directory in snapshot-chr
            DirBuilder::new().recursive(true)
                             .create(format!("{}/{}", snapshot_chr,mount_path))?;
            // Use mount_path
            mount(Some(format!("/.snapshots/mutable_dirs/snapshot-{}/{}", snapshot,mount_path).as_str()),
                  format!("{}/{}", snapshot_chr,mount_path).as_str(),
                  Some("btrfs"), MsFlags::MS_BIND , None::<&str>)?;
        }
    }
    if !mutable_dirs_shared.is_empty() {
        for mount_path in mutable_dirs_shared {
            // Create mouth_path directory in snapshot
            DirBuilder::new().recursive(true)
                             .create(format!("/.snapshots/mutable_dirs/{}", mount_path))?;
            // Create mouth_path directory in snapshot-chr
            DirBuilder::new().recursive(true)
                             .create(format!("{}/{}", snapshot_chr,mount_path))?;
            // Use mount_path
            mount(Some(format!("/.snapshots/mutable_dirs/{}", mount_path).as_str()),
                  format!("{}/{}", snapshot_chr,mount_path).as_str(),
                  Some("btrfs"), MsFlags::MS_BIND , None::<&str>)?;
        }
    }

    // File operations for snapshot-chr
    create_snapshot(format!("/.snapshots/boot/boot-{}", snapshot),
                    format!("/.snapshots/boot/boot-chr{}", snapshot),
                    CreateSnapshotFlags::empty(), None).unwrap();
    create_snapshot(format!("/.snapshots/etc/etc-{}", snapshot),
                    format!("/.snapshots/etc/etc-chr{}", snapshot),
                    CreateSnapshotFlags::empty(), None).unwrap();
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/boot/boot-chr{}/.", snapshot))
                      .arg(format!("{}/boot", snapshot_chr))
                      .output()?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/etc/etc-chr{}/.", snapshot))
                      .arg(format!("{}/etc", snapshot_chr))
                      .output()?;

    // Clean init system
    init_system_clean(snapshot, "prepare")?;

    // Copy ash related configurations
    if Path::new("/etc/systemd").try_exists().unwrap() {
        // Machine-id is a Systemd thing
        copy("/etc/machine-id", format!("{}/etc/machine-id", snapshot_chr))?;
    }
    DirBuilder::new().recursive(true)
                     .create(format!("{}/.snapshots/ash", snapshot_chr))?;
    copy("/.snapshots/ash/fstree", format!("{}/.snapshots/ash/fstree", snapshot_chr))?;

   Ok(())
}

// Refresh snapshot
pub fn refresh(snapshot: &str) -> Result<(), Error> {
    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot refresh as snapshot {} doesn't exist.", snapshot);

        // Make sure snapshot is not in use by another ash process
        } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.",
                  snapshot,snapshot);

        // Make sure snapshot is not base snapshot
        } else if snapshot == "0" {
        eprintln!("Changing base snapshot is not allowed.");

    } else {
        sync_time()?;
        prepare(snapshot)?;
        let excode = refresh_helper(snapshot);
        if excode.success() {
            post_transactions(snapshot)?;
            println!("Snapshot {} refreshed successfully.", snapshot);
        } else {
            chr_delete(snapshot)?;
            eprintln!("Refresh failed and changes discarded.")
        }
    }
    Ok(())
}

// Remove directory contents
pub fn remove_dir_content(dir_path: &str) -> Result<(), Error> {
    // Specify the path to the directory to remove contents from
    let path = PathBuf::from(dir_path);

    // Iterate over the directory entries using the read_dir function
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        // Check if the entry is a file or a directory
        if path.is_file() {
            // If it's a file, remove it using the remove_file function
            std::fs::remove_file(path)?;
        } else if path.is_symlink() {
            // If it's a symlink, remove it using the remove_file function
            std::fs::remove_file(path)?;
        } else if path.is_dir() {
            // If it's a directory, recursively remove its contents using the remove_dir_all function
            std::fs::remove_dir_all(path)?;
        }
    }
    Ok(())
}

// Recursively remove package in tree
pub fn remove_from_tree(treename: &str, pkgs: &Vec<String>, profiles: &Vec<String>, user_profiles: &Vec<String>) -> Result<(), Error> {
    // Make sure treename exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
        eprintln!("Cannot remove as tree {} doesn't exist.", treename);

    } else {
        // Import tree value
        let tree = fstree().unwrap();
        // Remove packages
        if !pkgs.is_empty() {
            for pkg in pkgs {
                uninstall(treename, &vec![pkg.to_string()], true)?;
                let mut order = recurse_tree(&tree, treename);
                if order.len() > 2 {
                    order.remove(0);
                    order.remove(0);
                }
                loop {
                    if order.len() < 2 {
                        break;
                    }
                    let arg = &order[0];
                    let sarg = &order[1];
                    println!("{}, {}", arg,sarg);
                    uninstall(sarg, &vec![pkg.to_string()], true)?;
                    order.remove(0);
                    order.remove(0);
                }
            }
        } else if !profiles.is_empty() {
            // Remove profiles
            for profile in profiles {
                let user_profile = "";
                uninstall_profile(treename, &profile, &user_profile, true)?;
                let mut order = recurse_tree(&tree, treename);
                if order.len() > 2 {
                    order.remove(0);
                    order.remove(0);
                }
                loop {
                    if order.len() < 2 {
                        break;
                    }
                    let arg = &order[0];
                    let sarg = &order[1];
                    println!("{}, {}", arg,sarg);
                    if uninstall_profile(sarg, &profile, &user_profile, true).is_ok() {
                        post_transactions(sarg)?;
                    } else {
                        chr_delete(sarg)?;
                        return Err(Error::new(ErrorKind::Other,
                                              format!("Failed to remove and changes discarded.")));
                    }
                    order.remove(0);
                    order.remove(0);
                }
            }
        } else if !user_profiles.is_empty() {
            // Remove profiles
            for user_profile in user_profiles {
                let profile = "";
                uninstall_profile_live(treename, &profile, &user_profile, true)?;
                let mut order = recurse_tree(&tree, treename);
                if order.len() > 2 {
                    order.remove(0);
                    order.remove(0);
                }
                loop {
                    if order.len() < 2 {
                        break;
                    }
                    let arg = &order[0];
                    let sarg = &order[1];
                    println!("{}, {}", arg,sarg);
                    if uninstall_profile(sarg, &profile, &user_profile, true).is_ok() {
                        post_transactions(sarg)?;
                    } else {
                        chr_delete(sarg)?;
                        return Err(Error::new(ErrorKind::Other,
                                              format!("Failed to remove and changes discarded.")));
                    }
                    order.remove(0);
                    order.remove(0);
                }
            }
        }
    }
    Ok(())
}

// System reset
pub fn reset() -> Result<(), Error> {
    let current_snapshot = get_current_snapshot();
    let msg = "All snapshots will be permanently deleted and cannot be retrieved, are you absolutely certain you want to continue?";
    let reset_msg = "The system will restart automatically to complete the reset. Do you want to continue?";
    if yes_no(msg) && yes_no(reset_msg) {
        // Collect snapshots
        let mut snapshots = read_dir("/.snapshots/rootfs")
            .unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();

        // Ignore deploy and deploy-aux
        snapshots.retain(|s| s != &Path::new("/.snapshots/rootfs/snapshot-deploy").to_path_buf());
        snapshots.retain(|s| s != &Path::new("/.snapshots/rootfs/snapshot-deploy-aux").to_path_buf());

        // Ignore base snapshot
        snapshots.retain(|s| s != &Path::new("/.snapshots/rootfs/snapshot-0").to_path_buf());

        // Prepare base snapshot
        prepare("0")?;
        copy("/.snapshots/rootfs/snapshot-chr0/etc/rc.local", "/.snapshots/rootfs/snapshot-chr0/etc/rc.local.bak")?;
        let start = "#!/bin/sh";
        let del_snap = format!("/usr/sbin/ash del -q -n -s {}", current_snapshot);
        let cp_rc = "cp /etc/rc.local.bak /etc/rc.local";
        let end = "exit 0";
        let mut file = OpenOptions::new().truncate(true)
                                         .read(true)
                                         .write(true)
                                         .open("/.snapshots/rootfs/snapshot-chr0/etc/rc.local")?;
        let new_content = format!("{}\n{}\n{}\n{}\nexit 0", start,del_snap,cp_rc,end);
        file.write_all(new_content.as_bytes())?;
        post_transactions("0")?;

        // Deploy the base snapshot and remove all the other snapshots
        if deploy("0", false, true).is_ok() {
            let mut snapshot = snapshots.len();
            while snapshot > 0 {
                // Delete snapshot if exist
                if Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap()
                && snapshot.to_string() != current_snapshot {
                    delete_node(&vec![snapshot.to_string()], true, true)?;
                }
                snapshot -= 1;
            }
        } else {
            eprintln!("Failed to deploy base snapshot");
        }
    }
    Ok(())
}

// Rollback last booted deployment
pub fn rollback() -> Result<(), Error> {
    let tmp = get_tmp();
    let i = find_new();
    clone_as_tree(&tmp, "")?;
    write_desc(&i.to_string(), " rollback ", false)?;
    deploy(&i.to_string(), false, false)?;
    Ok(())
}

// Enable service(s) (Systemd, OpenRC, etc.) //TODO
//fn service_enable(snapshot: &str, profile: &str, tmp_prof: &str) -> std::io::Result<()> {
    //if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        //return Err(Error::new(ErrorKind::NotFound,
                              //format!("Cannot enable services as snapshot {} doesn't exist.", snapshot)));

    //} else {
        //loop {
            //let postinst: Vec<String> = String::from_utf8(Command::new("sh")
                                                          //.arg("-c")
                                                          //.arg(format!("cat {}/packages.txt | grep -E -w '^&' | sed 's|& ||'", tmp_prof))
                                                          //.output()
                                                          //.unwrap()
                                                          //.stdout).unwrap()
                                                                  //.trim()
                                                                  //.split('\n')
                                                                  //.map(|s| s.to_string()).collect();

            //for cmd in postinst.into_iter().filter(|cmd| !cmd.is_empty()) {
                //Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{} {}", snapshot,cmd)).status().unwrap();
            //}

            //let services: Vec<String> = String::from_utf8(Command::new("sh")
                                                          //.arg("-c")
                                                          //.arg(format!("cat {}/packages.txt | grep -E -w '^%' | sed 's|% ||'", tmp_prof))
                                                          //.output()
                                                          //.unwrap().stdout).unwrap()
                                                                           //.trim()
                                                                           //.split('\n')
                                                                           //.map(|s| s.to_string()).collect();

            //for cmd in services.into_iter().filter(|cmd| !cmd.is_empty()) {
                //let excode = Command::new("chroot")
                    //.arg(format!("/.snapshots/rootfs/snapshot-chr{} {}",snapshot,cmd))
                    //.status().unwrap();
                //if excode.success() {
                    //println!("Failed to enable service(s) from {}.", profile);
                //} else {
                    //println!("Installed service(s) from {}.", profile);
                //}
            //}
        //}
    //}
//}

// Creates new tree from base file
pub fn snapshot_base_new(desc: &str) -> Result<i32, Error> {
    // Immutability toggle not used as base should always be immutable
    let i = find_new();
    create_snapshot("/.snapshots/boot/boot-0",
                    format!("/.snapshots/boot/boot-{}", i),
                    CreateSnapshotFlags::READ_ONLY, None).unwrap();
     create_snapshot("/.snapshots/etc/etc-0",
                    format!("/.snapshots/etc/etc-{}", i),
                    CreateSnapshotFlags::READ_ONLY, None).unwrap();
     create_snapshot("/.snapshots/rootfs/snapshot-0",
                    format!("/.snapshots/rootfs/snapshot-{}", i),
                    CreateSnapshotFlags::READ_ONLY, None).unwrap();

    // Import tree file
    let tree = fstree().unwrap();

    // Add to root tree
    append_base_tree(&tree, i).unwrap();
    // Write description
    if desc.is_empty() {
        write_desc(&i.to_string(), "clone of base.", true).unwrap();
    } else {
        write_desc(&i.to_string(), desc, true).unwrap();
    }
    Ok(i)
}

// Edit per-snapshot configuration
pub fn snapshot_config_edit(snapshot: &str, /*skip_prep: bool, skip_post: bool*/) -> Result<(), Error> {
    // Make sure snapshot exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot chroot as snapshot {} doesn't exist.", snapshot);
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        // Make sure snapshot is not in use by another ash process
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snapshot,snapshot)

    } else if snapshot == "0" {
        // Make sure is not base snapshot
        eprintln!("Changing base snapshot is not allowed.")

    } else {
        // Edit ash config
        //if !skip_prep {
        prepare(snapshot)?;
        //}
        if std::env::var("EDITOR").is_ok() {
        Command::new("sh").arg("-c")
                          .arg(format!("$EDITOR /.snapshots/rootfs/snapshot-chr{}/etc/ash.conf", snapshot))
                          .status()?;
            } else {
            // nano available
            println!("You can use the default editor by running 'sudo -E ash edit'.");
            if Command::new("sh").arg("-c")
                                 .arg("[ -x \"$(command -v nano)\" ]")
                                 .status().unwrap().success() {
                                     Command::new("sh").arg("-c")
                                                       .arg(format!("nano /.snapshots/rootfs/snapshot-chr{}/etc/ash.conf", snapshot))
                                                       .status()?;
                                 }
            // vi available
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v vi)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("vi /.snapshots/rootfs/snapshot-chr{}/etc/ash.conf", snapshot))
                                                            .status()?;
                                      }
            // vim available
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v vim)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("vim /.snapshots/rootfs/snapshot-chr{}/etc/ash.conf", snapshot))
                                                            .status()?;
                                      }
            // neovim
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v nvim)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("nvim /.snapshots/rootfs/snapshot-chr{}/etc/ash.conf", snapshot))
                                                            .status()?;
                                      }
            // micro
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v micro)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("micro /.snapshots/rootfs/snapshot-chr{}/etc/ash.conf", snapshot))
                                                            .status()?;
                                      }
            else {
                eprintln!("No text editor available!");
            }
            // if !skip_post {
            post_transactions(snapshot)?;
            //}
        }
    }
    Ok(())
}

// Get per-snapshot configuration options
pub fn snapshot_config_get(snapshot: &str) -> HashMap<String, String> {
    let mut options = HashMap::new();

    if !Path::new(&format!("/.snapshots/etc/etc-{}/ash.conf", snapshot)).try_exists().unwrap() {
        // Defaults here
        options.insert(String::from("aur"), String::from("False"));
        options.insert(String::from("mutable_dirs"), String::new());
        options.insert(String::from("mutable_dirs_shared"), String::new());
        return options;
    } else {
        let optfile = File::open(format!("/.snapshots/etc/etc-{}/ash.conf", snapshot)).unwrap();
        let reader = BufReader::new(optfile);

        for line in reader.lines() {
            let mut line = line.unwrap();
            // Skip line if there's no option set
            if comment_after_hash(&mut line).contains("::") {
                // Split options with '::'
                let (left, right) = line.split_once("::").unwrap();
                // Remove newline here
                options.insert(left.to_string(), right.trim_end().to_string().split("#").next().unwrap().to_string());
            }
        }
        return options;
    }
}

// Remove temporary chroot for specified snapshot only
// This unlocks the snapshot for use by other functions
pub fn snapshot_unlock(snapshot: &str) -> Result<(), Error> {
    let print_path = format!("/.snapshots/rootfs/snapshot-chr{}", snapshot);
    let path = Path::new(&print_path);
    if path.try_exists().unwrap() {
        // Make sure snapshot is not mounted
        if !is_mounted(path) {
            delete_subvolume(&format!("/.snapshots/boot/boot-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(&format!("/.snapshots/etc/etc-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
        } else {
            eprintln!("{} is busy.", path.to_str().unwrap());
        }
    }
    Ok(())
}

// Switch between distros
pub fn switch_distro() -> Result<(), Error>{
    loop {
        let map_tmp_output = Command::new("cat")
            .arg("/boot/efi/EFI/map.txt")
            .arg("|")
            .arg("awk 'BEGIN { FS = \"'==='\" } ; { print $1 }'")
            .output();

        let map_tmp = match map_tmp_output {
            Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
            Err(error) => {
                println!("Failed to read map.txt: {}", error);
                continue;
            }
        };

        println!("Type the name of a distribution to switch to: (type 'list' to list them, 'q' to quit)");
        let mut next_distro = String::new();
        stdin().read_line(&mut next_distro)?;

        next_distro = next_distro.trim().to_string();

        if next_distro == "q" {
            break;
        } else if next_distro == "list" {
            println!("{}", map_tmp);
        } else if map_tmp.contains(&next_distro) {
            if let Ok(file) = std::fs::File::open("/boot/efi/EFI/map.txt") {
                let mut reader = csv::ReaderBuilder::new()
                    .delimiter(b',')
                    .quoting(false)
                    .from_reader(file);

                let mut found = false;

                for row in reader.records() {
                    let record = row.unwrap();
                    let distro = record.get(0).unwrap();

                    if distro == next_distro {
                        let boot_order_output = Command::new("efibootmgr")
                            .arg("|")
                            .arg("grep BootOrder")
                            .arg("|")
                            .arg("awk '{print $2}'")
                            .output();

                        let boot_order = match boot_order_output {
                            Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
                            Err(error) => {
                                eprintln!("Failed to get boot order: {}", error);
                                continue;
                            }
                        };

                        let temp = boot_order.replace(&format!("{},", record.get(1).unwrap()), "");
                        let new_boot_order = format!("{},{}", record.get(1).unwrap(), temp);

                        let efibootmgr_output = Command::new("efibootmgr")
                            .arg("--bootorder")
                            .arg(&new_boot_order)
                            .output();

                        if let Err(error) = efibootmgr_output {
                            eprintln!("Failed to switch distros: {}", error);
                        } else {
                            println!("Done! Please reboot whenever you would like to switch to {}", next_distro);
                        }

                        found = true;
                        break;
                    }
                }

                if found {
                    break;
                }
            } else {
                eprintln!("Failed to open map.txt");
                continue;
            }
        } else {
            eprintln!("Invalid distribution!");
        }
    }
    Ok(())
}

// Switch between /tmp deployments
pub fn switch_tmp(secondary: bool, reset: bool) -> Result<(), Error> {
    let distro_name = detect::distro_name();
    let distro_id = detect::distro_id();
    let distro_suffix = &get_distro_suffix(&distro_id);
    let grub = get_grub().unwrap();
    let part = get_part();
    let tmp_boot = TempDir::new_in("/.snapshots/tmp")?;

    // Mount boot partition for writing
    mount(Some(part.as_str()), tmp_boot.path().as_os_str(),
          Some("btrfs"), MsFlags::empty(), Some(format!("subvol=@boot{}", distro_suffix).as_bytes()))?;

    // Swap deployment subvolumes: deploy <-> deploy-aux
    let source_dep = get_tmp();
    let target_dep = get_aux_tmp(source_dep.to_string(), secondary);
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-{}/boot/grub", target_dep))
                      .arg(format!("{}", tmp_boot.path().to_str().unwrap()))
                      .output()?;

    // Overwrite grub config boot subvolume
    let tmp_grub_cfg = format!("{}/{}/grub.cfg", tmp_boot.path().to_str().unwrap(),grub);
    // Read the contents of the file into a string
    let mut contents = String::new();
    let mut file = File::open(&tmp_grub_cfg)?;
    file.read_to_string(&mut contents)?;
    let modified_tmp_contents = contents.replace(&format!("@.snapshots{}/rootfs/snapshot-{}", distro_suffix,source_dep),
                                                 &format!("@.snapshots{}/rootfs/snapshot-{}", distro_suffix,target_dep));
    // Write the modified contents back to the file
    let mut file = File::create(tmp_grub_cfg)?;
    file.write_all(modified_tmp_contents.as_bytes())?;

    let grub_cfg = format!("/.snapshots/rootfs/snapshot-{}/boot/{}/grub.cfg", target_dep,grub);
    // Read the contents of the file into a string
    let mut contents = String::new();
    let mut file = File::open(&grub_cfg)?;
    file.read_to_string(&mut contents)?;
    let modified_cfg_contents = contents.replace(&format!("@.snapshots{}/rootfs/snapshot-{}", distro_suffix,source_dep),
                                                 &format!("@.snapshots{}/rootfs/snapshot-{}", distro_suffix,target_dep));
    // Write the modified contents back to the file
    let mut file = File::create(grub_cfg)?;
    file.write_all(modified_cfg_contents.as_bytes())?;

    // Update fstab for new deployment
    let fstab_file = format!("/.snapshots/rootfs/snapshot-{}/etc/fstab", target_dep);
    // Read the contents of the file into a string
    let mut contents = String::new();
    let mut file = File::open(&fstab_file)?;
    file.read_to_string(&mut contents)?;
    let modified_boot_contents = contents.replace(&format!("@.snapshots{}/boot/boot-{}", distro_suffix,source_dep),
                                                  &format!("@.snapshots{}/boot/boot-{}", distro_suffix,target_dep));
    let modified_etc_contents = modified_boot_contents.replace(&format!("@.snapshots{}/etc/etc-{}", distro_suffix,source_dep),
                                                               &format!("@.snapshots{}/etc/etc-{}", distro_suffix,target_dep));
    let modified_rootfs_contents = modified_etc_contents.replace(&format!("@.snapshots{}/rootfs/snapshot-{}", distro_suffix,source_dep),
                                                                 &format!("@.snapshots{}/rootfs/snapshot-{}", distro_suffix,target_dep));
    // Write the modified contents back to the file
    let mut file = File::create(fstab_file)?;
    file.write_all(modified_rootfs_contents.as_bytes())?;

    let src_file = File::open(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", source_dep))?;
    let mut reader = BufReader::new(src_file);
    let mut sfile = String::new();
    reader.read_line(&mut sfile)?;
    let snap = sfile.replace(" ", "").replace('\n', "");

    // Recovery GRUB configurations
    if !reset {
        for boot_location in ["/.snapshots/rootfs/snapshot-deploy-aux/boot", &tmp_boot.path().to_str().unwrap()] {
            // Get grub configurations
            let file_path = format!("{}/{}/grub.cfg", boot_location, grub);
            let file = File::open(&file_path)?;
            let reader = BufReader::new(file);
            let mut gconf = String::new();
            let mut in_10_linux = false;
            for line in reader.lines() {
                let line = line?;
                if line.contains("BEGIN /etc/grub.d/10_linux") {
                    in_10_linux = true;
                } else if in_10_linux {
                    if line.contains("}") {
                        gconf.push_str(&line);
                        gconf.push_str("\n### END /etc/grub.d/41_custom ###");
                        break;
                    } else {
                        gconf.push_str(&format!("\n{}",&line));
                    }
                }
            }

            // Switch tmp
            if gconf.contains("snapshot-deploy-aux") {
                gconf = gconf.replace("snapshot-deploy-aux", "snapshot-deploy");
            } else if gconf.contains("snapshot-deploy") {
                gconf = gconf.replace("snapshot-deploy", "snapshot-deploy-aux");
            } else if gconf.contains("snapshot-deploy-aux-secondary") {
                gconf = gconf.replace("snapshot-deploy-aux-secondary", "snapshot-deploy-secondary");
            } else if gconf.contains("snapshot-deploy-secondary") {
                gconf = gconf.replace("snapshot-deploy-secondary", "snapshot-deploy-aux-secondary");
            }

            if gconf.contains(&distro_name) {
                // Remove snapshot number
                let prefix = gconf.split("snapshot ").next().unwrap();
                let suffix = gconf.split("snapshot ").skip(1).next().unwrap();
                let snapshot_num = suffix.split(' ').next().unwrap();
                let suffix = suffix.replacen(snapshot_num, "", 1);
                gconf = format!("{}{}", prefix, suffix);

                // Replace with last booted deployment entry
                gconf = gconf.replace(&distro_name, &format!("{} last booted deployment (snapshot {})",
                                                             &distro_name, snap));
            }

            // Remove last line
            let contents = read_to_string(&file_path)?;
            let lines: Vec<&str> = contents.lines().collect();
            let new_contents = lines[..lines.len() - 1].join("\n");
            std::fs::write(&file_path, new_contents)?;

            // Open the file in read and write mode
            let mut file = OpenOptions::new().read(true).write(true).append(true).open(&file_path)?;

            // Write the modified content back to the file
            file.write_all(format!("\n\n{}", gconf).as_bytes())?;
        }
    }

    // Umount boot partition
    umount2(Path::new(&format!("{}", tmp_boot.path().as_os_str().to_str().unwrap())),
            MntFlags::MNT_DETACH)?;

    Ok(())
}

// No comment
pub fn switch_to_windows() -> std::process::ExitStatus {
    Command::new("efibootmgr").args(["-c", "-L"])
                      .arg(format!("'Windows' -l '\\EFI\\BOOT\\BOOTX64.efi'")).status().unwrap()
}

// Sync time
pub fn sync_time() -> Result<(), Error> {
    // curl --tlsv1.3 --proto =https -I https://google.com
    let mut easy = Easy::new();
    easy.url("https://google.com")?;

    easy.ssl_version(SslVersion::Tlsv13)?;
    easy.http_version(HttpVersion::V2)?;

    let mut headers = List::new();
    headers.append("Accept: */*")?;
    easy.http_headers(headers)?;
    easy.show_header(true)?;

    let mut response_headers = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|data| {
            response_headers.extend_from_slice(data);
            Ok(data.len())
        }).unwrap();
        transfer.perform()?;
    }

    let response_headers_str = String::from_utf8_lossy(&response_headers);

    let date_header = response_headers_str
        .lines()
        .find(|line| line.starts_with("date:"))
        .expect("Date header not found");

    let date_str: Vec<&str> = date_header.split_whitespace().collect();
    let date = &date_str[2..6].join(" ");

    // Set time
    Command::new("date").arg("-s").arg(format!("\"({})Z\"", date)).output()?;
    Ok(())
}

// Clear all temporary snapshots
pub fn temp_snapshots_clear() -> Result<(), Error> {
    // Collect snapshots numbers
    let boots = read_dir("/.snapshots/boot")
        .unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
    let etcs = read_dir("/.snapshots/etc")
        .unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
    //let vars = read_dir("/.snapshots/var")
        //.unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>(); // Can this be deleted?
    let mut snapshots = read_dir("/.snapshots/rootfs")
        .unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
    snapshots.append(&mut etcs.clone());
    //snapshots.append(&mut vars.clone());
    snapshots.append(&mut boots.clone());

    // Clear temp if exist
    for snapshot in snapshots {
        if snapshot.to_str().unwrap().contains("snapshot-chr") {
            // Make sure the path isn't being used
            if !is_mounted(&snapshot) {
                delete_subvolume(&snapshot, DeleteSubvolumeFlags::empty()).unwrap();
            } else {
                eprintln!("{} is busy.", snapshot.to_str().unwrap());
            }
        } else if snapshot.to_str().unwrap().contains("etc-chr") {
            // Make sure the path isn't being used
            if !is_mounted(&snapshot) {
                delete_subvolume(&snapshot, DeleteSubvolumeFlags::empty()).unwrap();
            } else {
                eprintln!("{} is busy.", snapshot.to_str().unwrap());
            }
        //} else if snapshot.to_str().unwrap().contains("var") {
        } else if snapshot.to_str().unwrap().contains("boot-chr") {
            // Make sure the path isn't being used
            if !is_mounted(&snapshot) {
                delete_subvolume(&snapshot, DeleteSubvolumeFlags::empty()).unwrap();
            } else {
                eprintln!("{} is busy.", snapshot.to_str().unwrap());
            }
        }
    }
    Ok(())
}

// Clean tmp dirs
pub fn tmp_delete(secondary: bool) -> Result<(), Error> {
    // Get tmp
    let tmp = get_tmp();
    let tmp = get_aux_tmp(tmp, secondary);

    // Clean tmp
    delete_subvolume(&format!("/.snapshots/boot/boot-{}", tmp), DeleteSubvolumeFlags::RECURSIVE).unwrap();
    delete_subvolume(&format!("/.snapshots/etc/etc-{}", tmp), DeleteSubvolumeFlags::RECURSIVE).unwrap();
    delete_subvolume(&format!("/.snapshots/rootfs/snapshot-{}", tmp), DeleteSubvolumeFlags::RECURSIVE).unwrap();
    Ok(())
}

// Recursively run a command in tree
pub fn tree_run(treename: &str, cmd: &str) -> Result<(), Error> {
    // Make sure treename exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
                return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot update as tree {} doesn't exist.", treename)));

    } else {
        // Run command
        prepare(treename)?;
        chroot_exec(&format!("/.snapshots/rootfs/snapshot-chr{}", treename), cmd)?;
        post_transactions(treename)?;

        // Import tree file
        let tree = fstree().unwrap();

        let mut order = recurse_tree(&tree, treename);
        if order.len() > 2 {
            order.remove(0);
            order.remove(0);
        }
        loop {
            if order.len() < 2 {
                break;
            }
            let arg = &order[0];
            let sarg = &order[1];
            println!("{}, {}", arg,sarg);
            if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", sarg)).try_exists().unwrap() {
                eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", sarg,sarg);
                eprintln!("Tree command canceled.");
            } else {
                prepare(&sarg)?;
                chroot_exec(&format!("/.snapshots/rootfs/snapshot-chr{}", sarg), cmd)?;
                post_transactions(&sarg)?;
            }
            order.remove(0);
            order.remove(0);
        }
    }
    Ok(())
}

// Calls print function
pub fn tree_show() {
    // Import tree file
    let tree = fstree().unwrap();
    tree_print(&tree);
}

// Sync tree and all its snapshots
pub fn tree_sync(treename: &str, force_offline: bool, live: bool) -> Result<(), Error> {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot sync as tree {} doesn't exist.", treename)));

    } else {
        // Syncing tree automatically updates it, unless 'force-sync' is used
        if !force_offline {
            tree_upgrade(treename)?;
        }

        // Import tree file
        let tree = fstree().unwrap();

        let mut order = recurse_tree(&tree, treename);
        if order.len() > 2 {
            order.remove(0);
            order.remove(0);
        }
        loop {
            if order.len() < 2 {
                break;
            }
            let snap_from = &order[0];
            let snap_to = &order[1];
            println!("{}, {}", snap_from, snap_to);
            if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snap_to)).try_exists().unwrap() {
                eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snap_to,snap_to);
                eprintln!("Tree sync canceled.");
            } else {
                prepare(snap_to)?;
                // Pre-sync
                tree_sync_helper(snap_from, snap_to, "chr")?;
                // Live sync
                if live && snap_to == &get_current_snapshot() {
                    // Post-sync
                    tree_sync_helper(snap_from, &get_tmp(), "")?;
                }
                // Moved here from the line immediately after first sync_tree_helper
                post_transactions(snap_to).unwrap();
            }
            order.remove(0);
            order.remove(0);
        }
    }
    Ok(())
}

// Recursively run an update in tree
pub fn tree_upgrade(treename: &str) -> Result<(), Error> {
    // Make sure treename exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot update as tree {} doesn't exist.", treename)));
    } else {
        // Run update
        auto_upgrade(treename)?;

        // Import tree file
        let tree = fstree().unwrap();

        let mut order = recurse_tree(&tree, treename);
        if order.len() > 2 {
            order.remove(0);
            order.remove(0);
        }
        loop {
            if order.len() < 2 {
                break;
            } else {
                let arg = &order[0];
                let sarg = &order[1];
                println!("{}, {}", arg, sarg);
                auto_upgrade(&sarg).unwrap();
                order.remove(0);
                order.remove(0);
            }
        }
    }
    Ok(())
}

// Uninstall package(s)
pub fn uninstall(snapshot: &str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot remove as snapshot {} doesn't exist.", snapshot)));

        // Make sure snapshot is not in use by another ash process
        } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        return Err(
            Error::new(ErrorKind::Unsupported,
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.",
                               snapshot,snapshot)));

        // Make sure snapshot is not base snapshot
        } else if snapshot == "0" {
        return Err(Error::new(ErrorKind::Unsupported,
                              format!("Changing base snapshot is not allowed.")));

    } else {
        // Uninstall package
        prepare(snapshot)?;
        let excode = uninstall_package_helper(snapshot, &pkgs, noconfirm);
        if excode.is_ok() {
            post_transactions(snapshot)?;
            println!("Package(s) {pkgs:?} removed from snapshot {} successfully.", snapshot);
        } else {
            chr_delete(snapshot).unwrap();
            eprintln!("Remove failed and changes discarded.");
        }
    }
    Ok(())
}

// Uninstall live
pub fn uninstall_live(pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    let tmp = get_tmp();
    ash_mounts(&tmp, "").unwrap();
    uninstall_package_helper_live(&tmp, &pkgs, noconfirm)?;
    ash_umounts(&tmp, "").unwrap();
    Ok(())
}

// Uninstall a profile from a text file
fn uninstall_profile(snapshot: &str, profile: &str, user_profile: &str, noconfirm: bool) -> Result<(), Error> {
    // Get some values
    let dist = detect::distro_id();
    let cfile = format!("/usr/share/ash/profiles/{}/{}.conf", profile,dist);

    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot uninstall as snapshot {} doesn't exist.", snapshot)));

        // Make sure snapshot is not in use by another ash process
        } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        return Err(
            Error::new(ErrorKind::Unsupported,
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.",
                               snapshot,snapshot)));

        // Make sure snapshot is not base snapshot
        } else if snapshot == "0" {
        return Err(Error::new(ErrorKind::Unsupported,
                              format!("Changing base snapshot is not allowed.")));
    } else {
        // Uninstall profile
        // Prepare
        prepare(snapshot)?;

        // profile configurations
        let mut profconf = Ini::new();
        profconf.set_comment_symbols(&['#']);
        profconf.set_multiline(true);
        // Load profile if exist
        if !Path::new(&cfile).try_exists().unwrap() && user_profile.is_empty() {
            profconf.load(&cfile).unwrap();
        } else if !user_profile.is_empty() {
            profconf.load(user_profile).unwrap();
        }

        // Read packages section in configuration file
        if profconf.sections().contains(&"packages".to_string()) {
            let mut pkgs: Vec<String> = Vec::new();
            for pkg in profconf.get_map().unwrap().get("packages").unwrap().keys() {
                pkgs.push(pkg.to_string());
            }
            // Install package(s)
            uninstall_package_helper(snapshot, &pkgs, noconfirm)?;
        }

        // Read commands section in configuration file
        if profconf.sections().contains(&"uninstall-commands".to_string()) {
            for cmd in profconf.get_map().unwrap().get("uninstall-commands").unwrap().keys() {
                chroot_exec(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot), cmd)?;
            }
        }
    }
    Ok(())
}

// Uninstall profile in live snapshot
fn uninstall_profile_live(snapshot: &str,profile: &str, user_profile: &str, noconfirm: bool) -> Result<(), Error> {
    // Get some values
    let dist = detect::distro_id();
    let cfile = format!("/usr/share/ash/profiles/{}/{}.conf", profile,dist);
    let tmp = get_tmp();

    // Prepare
    ash_mounts(&tmp, "")?;

    // profile configurations
    let mut profconf = Ini::new();
    profconf.set_comment_symbols(&['#']);
    profconf.set_multiline(true);

    // Load profile if exist
    if !Path::new(&cfile).try_exists().unwrap() && user_profile.is_empty() {
        profconf.load(&cfile).unwrap();
    } else if !user_profile.is_empty() {
        profconf.load(user_profile).unwrap();
    }

    // Read packages section in configuration file
    if profconf.sections().contains(&"packages".to_string()) {
        let mut pkgs: Vec<String> = Vec::new();
        for pkg in profconf.get_map().unwrap().get("packages").unwrap().keys() {
            pkgs.push(pkg.to_string());
        }
        // Install package(s)
        uninstall_package_helper_live(&tmp, &pkgs, noconfirm)?;
    }

    // Read commands section in configuration file
    if profconf.sections().contains(&"uninstall-commands".to_string()) {
        for cmd in profconf.get_map().unwrap().get("uninstall-commands").unwrap().keys() {
            chroot_exec(&format!("/.snapshots/rootfs/snapshot-{}", snapshot), cmd)?;
        }
    }

    // Umounts tmp
    ash_umounts(&tmp, "").unwrap();

    Ok(())
}

// Triage functions for argparse method
pub fn uninstall_triage(snapshot: &str, live: bool, pkgs: Vec<String>, profile: &str,
                        user_profile: &str, noconfirm: bool) -> Result<(), Error> {
    // Switch between profile and user_profile
    let p = if user_profile.is_empty() {
        profile
    } else {
        user_profile
    };

    if !live {
        // Uninstall profile
        if !profile.is_empty() {
            let excode = uninstall_profile(snapshot, profile, user_profile, true);
            match excode {
                Ok(_) => {
                    if post_transactions(snapshot).is_ok() {
                        println!("Profile {} removed from snapshot {} successfully.", p,snapshot);
                    } else {
                        chr_delete(snapshot)?;
                        eprintln!("Uninstall failed and changes discarded!");
                    }
                },
                Err(_) => {
                    chr_delete(snapshot)?;
                    eprintln!("Uninstall failed and changes discarded!");
                },
            }

        } else if !pkgs.is_empty() {
            // Uninstall package
            uninstall(snapshot, &pkgs, noconfirm)?;

        } else if !user_profile.is_empty() {
            // Uninstall user_profile
            let excode = uninstall_profile(snapshot, profile, user_profile, true);
            match excode {
                Ok(_) => {
                    if post_transactions(snapshot).is_ok() {
                        println!("Profile {} removed from snapshot {} successfully.", p,snapshot);
                    } else {
                        chr_delete(snapshot)?;
                        eprintln!("Uninstall failed and changes discarded!");
                    }
                },
                Err(_) => {
                    chr_delete(snapshot)?;
                    eprintln!("uninstall failed and changes discarded!");
                },
            }
        }

    } else if live && snapshot != get_current_snapshot() {
        // Prevent live Uninstall except for current snapshot
        eprintln!("Can't use the live option with any other snapshot than the current one.");

    } else if live && snapshot == get_current_snapshot() {
        // Do live uninstall only if: live flag is used OR target snapshot is current
        if !profile.is_empty() {
            // Live profile uninstall
            let excode = uninstall_profile_live(snapshot, profile, user_profile, true);
            match excode {
                Ok(_) => println!("Profile {} removed from current/live snapshot.", p),
                Err(e) => eprintln!("{}", e),
            }

        } else if !pkgs.is_empty() {
            // Live package uninstall
            let excode = uninstall_live(&pkgs, noconfirm);
            match excode {
                Ok(_) => println!("Package(s) {pkgs:?} removed from snapshot {} successfully.", snapshot),
                Err(e) => eprintln!("{}", e),
            }

        } else if !user_profile.is_empty() {
            // Live user_profile uninstall
            let excode = uninstall_profile_live(snapshot, profile, user_profile, true);
            match excode {
                Ok(_) => println!("Profile {} removed from current/live snapshot.", p),
                Err(e) => eprintln!("{}", e),
            }
        }
    }
    Ok(())
}

// Update boot
pub fn update_boot(snapshot: &str, secondary: bool) -> Result<(), Error> {
    // Path to grub directory
    let grub = get_grub().unwrap();

    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot update boot as snapshot {} doesn't exist.", snapshot)));

    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        // Make sure snapshot is not in use by another ash process
        return Err(
            Error::new(
                ErrorKind::Unsupported,
                format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.",
                        snapshot,snapshot)));

    } else {
        // Get tmp
        let tmp = get_tmp();
        let tmp = if secondary {
            if tmp.contains("secondary") {
                tmp.replace("-secondary", "")
            } else {
                format!("{}-secondary", tmp)
            }
        } else {
            tmp
        };

        // Partition path
        let part = get_part();

        // Prepare for update
        prepare(snapshot)?;
        // Remove grub configurations older than 30 days
        if Path::new(&format!("/boot/{}/BAK/", grub)).try_exists().unwrap() {
            delete_old_grub_files(&format!("/boot/{}", grub).as_str())?;
        }
        // Get current time
        let time = time::OffsetDateTime::now_utc();
        let formatted = time.format(&time::format_description::parse("[year][month][day]-[hour][minute][second]").unwrap()).unwrap();
        // Copy backup
        copy(format!("/boot/{}/grub.cfg", grub), format!("/boot/{}/BAK/grub.cfg.{}", grub,formatted))?;

        // Run update commands in chroot
        let distro_name = detect::distro_name();
        let mkconfig = format!("grub-mkconfig {} -o /boot/{}/grub.cfg", part,grub);
        let sed_snap = format!("sed -i 's|snapshot-chr{}|snapshot-{}|g' /boot/{}/grub.cfg", snapshot,tmp,grub);
        let sed_distro = format!("sed -i '0,\\|{}| s||{} snapshot {}|' /boot/{}/grub.cfg",
                                 distro_name,distro_name,snapshot,grub);
        let cmds = [mkconfig, sed_snap, sed_distro];
        for cmd in cmds {
            chroot_exec(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot), &cmd)?;
        }

        // Finish the update
        post_transactions(snapshot)?;
    }
    Ok(())
}

// Saves changes made to /etc to snapshot
pub fn update_etc() -> Result<(), Error> {
    let snapshot = get_current_snapshot();
    let tmp = get_tmp();

    // Remove old /etc
    delete_subvolume(&format!("/.snapshots/etc/etc-{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();

    // Check mutability
    let immutability: CreateSnapshotFlags = if check_mutability(&snapshot) {
        CreateSnapshotFlags::empty()
    } else {
        CreateSnapshotFlags::READ_ONLY
    };

    // Create new /etc
    create_snapshot(format!("/.snapshots/etc/etc-{}", tmp),
                    format!("/.snapshots/etc/etc-{}", snapshot),
                    immutability, None).unwrap();
    Ok(())
}

// Upgrade snapshot
pub fn upgrade(snapshot:  &str, baseup: bool) -> Result<(), Error> {
    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot upgrade as snapshot {} doesn't exist.", snapshot)));

    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        // Make sure snapshot is not in use by another ash process
        return Err(
            Error::new(ErrorKind::Unsupported,
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.",
                               snapshot,snapshot)));

    } else if snapshot == "0" && !baseup {
        // Make sure snapshot is not base snapshot
        return Err(Error::new(ErrorKind::Unsupported,
                              format!("Changing base snapshot is not allowed.")));

    } else {
        // Default upgrade behaviour is now "safe" update, meaning failed updates get fully discarded
        let excode = upgrade_helper(snapshot);
        if excode.success() {
            if post_transactions(snapshot).is_ok() {
                println!("Snapshot {} upgraded successfully.", snapshot);
            }
        } else {
            chr_delete(snapshot).unwrap();
            return Err(Error::new(ErrorKind::Other,
                                  format!("Upgrade failed and changes discarded.")));
        }
    }
    Ok(())
}

// Return snapshot that has a package
pub fn which_snapshot_has(pkgs: Vec<String>) {
    // Collect snapshots
    let mut snapshots = read_dir("/.snapshots/rootfs")
        .unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
    // Ignore deploy and deploy-aux
    snapshots.retain(|s| s != &Path::new("/.snapshots/rootfs/snapshot-deploy").to_path_buf());
    snapshots.retain(|s| s != &Path::new("/.snapshots/rootfs/snapshot-deploy-aux").to_path_buf());

    // Search snapshots for package
    let i_max = snapshots.len();
    for pkg in pkgs {
        let mut snapshot: Vec<String> = Vec::new();
        let mut i = 0;
        while i < i_max {
            if is_pkg_installed(&i.to_string(), &pkg) {
                snapshot.push(format!("snapshot-{}", i.to_string()));
            }
            i += 1;
        }
        if !snapshot.is_empty() {
            println!("package {} installed in {snapshot:?}", pkg);
        }
    }
}

// Write new description (default) or append to an existing one (i.e. toggle immutability)
pub fn write_desc(snapshot: &str, desc: &str, overwrite: bool) -> Result<(), Error> {
    let mut descfile = if overwrite {
        OpenOptions::new().truncate(true)
                          .create(true)
                          .read(true)
                          .write(true)
                          .open(format!("/.snapshots/ash/snapshots/{}-desc", snapshot))?
    } else {
        OpenOptions::new().append(true)
                          .create(true)
                          .read(true)
                          .open(format!("/.snapshots/ash/snapshots/{}-desc", snapshot))?
    };
    descfile.write_all(desc.as_bytes())?;
    Ok(())
}

// Generic yes no prompt
pub fn yes_no(msg: &str) -> bool {
    loop {
        println!("{} (y/n)", msg);
        let mut reply = String::new();
        stdin().read_line(&mut reply).unwrap();
        match reply.trim().to_lowercase().as_str() {
            "yes" | "y" => return true,
            "no" | "n" => return false,
            _ => eprintln!("Invalid choice!"),
        }
    }
}
