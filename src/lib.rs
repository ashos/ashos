mod detect_distro;
mod distros;
mod tree;

use crate::detect_distro as detect;
use crate::distros::*;
use libbtrfsutil::{create_snapshot, CreateSnapshotFlags, delete_subvolume, DeleteSubvolumeFlags};
use mktemp::Temp;
use nix::mount::{mount, MntFlags, MsFlags, umount2};
use partition_identity::{PartitionID, PartitionSource};
use std::collections::HashMap;
use std::fs::{copy, DirBuilder, File, metadata, OpenOptions, read_dir, read_to_string};
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, stdin, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;
use tree::*;
use walkdir::{DirEntry, WalkDir};

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
pub fn ash_version() -> std::io::Result<String> {
    //let ash_bin_path = std::env::current_exe().unwrap(); //https://doc.rust-lang.org/std/env/fn.current_exe.html#security
    let ash_bin_path = if Path::new("/usr/sbin/ash").try_exists().unwrap() {
        Path::new("/usr/sbin/ash")
    } else {
        Path::new("/.snapshots/ash/ash")
    };
    let metadata = metadata(ash_bin_path).unwrap();
    let time = metadata.modified().unwrap();
    let duration = time.duration_since(UNIX_EPOCH).unwrap();
    let utc = time::OffsetDateTime::UNIX_EPOCH +
        time::Duration::try_from(duration).unwrap();
    let local = utc.to_offset(time::UtcOffset::local_offset_at(utc).unwrap());
    let version = local.format_into(
        &mut std::io::stdout().lock(),
        time::macros::format_description!(
            "Last modified on: [weekday] [day]-[month]-[year] [hour repr:12]:[minute]:[second] [period]\n"
        ),
    ).unwrap().to_string();
    Ok(version)
}

// Add node to branch
pub fn branch_create(snapshot: &str, desc: &str) -> std::io::Result<i32> {
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
            let description = desc.split("").collect::<Vec<&str>>().join(" ");
            write_desc(i.to_string().as_str(), &description)?;
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
pub fn check_update() -> std::io::Result<()> {
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
pub fn chr_delete(snapshot: &str) -> std::io::Result<()> {
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
pub fn chroot(snapshot: &str, cmd: &str) -> std::io::Result<()> {
    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists()? {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot clone as snapshot {} doesn't exist.", snapshot)));

    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists()? {
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
        if prepare(snapshot).is_ok() && !cmd.is_empty() {
            // Chroot to snapshot path
            let chroot_and_exec = Command::new("sh").arg("-c")
                                                    .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} {}", snapshot,cmd))
                                                    .status().unwrap();
            if chroot_and_exec.success() {
                // Exit chroot
                Command::new("sh").arg("-c").arg("exit").output().unwrap();
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
                Command::new("sh").arg("-c").arg("exit").output().unwrap();
                chr_delete(snapshot)?;
            }
        } else if prepare(snapshot).is_ok() {
            // chroot
            let chroot = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                               .status().unwrap();
            if chroot.code().is_some() {
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

// Clone tree
pub fn clone_as_tree(snapshot: &str, desc: &str) -> std::io::Result<i32> {
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
            write_desc(i.to_string().as_str(), &description)?;
        } else {
            let description = desc.split("").collect::<Vec<&str>>().join(" ");
            write_desc(i.to_string().as_str(), &description)?;
        }
    }
    Ok(i)
}

// Clone branch under same parent
pub fn clone_branch(snapshot: &str) -> std::io::Result<i32> {
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
        write_desc(i.to_string().as_str(), &desc)?;
    }
    Ok(i)
}

// Recursively clone an entire tree
pub fn clone_recursive(snapshot: &str) -> std::io::Result<()> {
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
pub fn clone_under(snapshot: &str, branch: &str) -> std::io::Result<i32> {
    // Find the next available snapshot number
    let i = find_new();

    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot clone as snapshot {} doesn't exist.", snapshot)));

        // Make sure branch does exist
        } else if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", branch)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot clone as snapshot {} doesn't exist.", snapshot)));

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
        write_desc(i.to_string().as_str(), desc.as_str())?;
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

// Delete tree or branch
pub fn delete_node(snapshots: &Vec<String>, quiet: bool, nuke: bool) -> std::io::Result<()> {
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
                } else if !quiet && !yes_no(format!("Are you sure you want to delete snapshot {}?", snapshot).as_str()) {
                return Err(Error::new(ErrorKind::Interrupted, format!(
                    "Aborted.")));
            } else {
                run = true;
            }
        }

        if nuke | run {
            // Delete snapshot
            let tree = fstree().unwrap();
            let children = return_children(&tree, snapshot.as_str());
            write_desc(snapshot, "")?;
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
                write_desc(snapshot, "")?;
                delete_subvolume(format!("/.snapshots/boot/boot{}", child), DeleteSubvolumeFlags::empty()).unwrap();
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

// Delete tree or branch
pub fn delete_deploys() -> std::io::Result<()> {
    for snap in ["deploy", "deploy-aux"] {
        delete_subvolume(format!("/.snapshots/boot/boot-{}", snap), DeleteSubvolumeFlags::empty()).unwrap();
        delete_subvolume(format!("/.snapshots/etc/etc-{}", snap), DeleteSubvolumeFlags::empty()).unwrap();
        delete_subvolume(format!("/.snapshots/rootfs/snapshot-{}", snap), DeleteSubvolumeFlags::empty()).unwrap();
        // Make sure temporary chroot directories are deleted as well
        if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snap)).try_exists().unwrap() {
            delete_subvolume(format!("/.snapshots/boot/boot-chr{}", snap), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(format!("/.snapshots/etc/etc-chr{}", snap), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(format!("/.snapshots/rootfs/snapshot-chr{}", snap), DeleteSubvolumeFlags::empty()).unwrap();
        }
        //print(f"Snapshot {snap} removed.")
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
pub fn deploy(snapshot: &str, secondary: bool) -> std::io::Result<()> {
    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot deploy as snapshot {} doesn't exist.", snapshot)));

    } else {
        println!("The snapshot {} is being deployed, it may take some time, please be patient.", snapshot);
        update_boot(snapshot, secondary)?;
        let tmp = get_tmp();
        // Set default volume
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
                          .arg(format!("/.snapshots/boot/boot-{}/", snapshot))
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/boot", tmp))
                          .output()?;
        Command::new("cp").args(["-r", "--reflink=auto"])
                          .arg(format!("/.snapshots/etc/etc-{}/", snapshot))
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
        if mutable_dirs_shared.is_empty() {
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
        switch_tmp(secondary)?;
        init_system_clean(tmp.as_str(), "deploy")?;

        // Set default volume
        Command::new("btrfs").args(["sub", "set-default"])
                             .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                             .output()?;
    }
    Ok(())
}

// Show diff of packages between 2 snapshots
pub fn diff(snapshot1:  &str, snapshot2: &str) {
    snapshot_diff(snapshot1, snapshot2).unwrap();
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
    return tmp;
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
                        return Some(entry.path().to_string_lossy().into_owned());
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
        // Return empty string in case no snapshot is deployed
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
    if mount.contains(&"deploy-aux".to_string()) {
        let r = String::from("deploy-aux");
        return r;
    } else {
        let r = String::from("deploy");
            return r;
    }
}

// Make a snapshot vulnerable to be modified even further (snapshot should be deployed as mutable)
pub fn hollow(snapshot: &str) -> std::io::Result<()> {
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
        // AUR step might be needed and if so make a distro_specific function with steps similar to install_package().
        // Call it hollow_helper and change this accordingly().
        prepare(snapshot)?;
        // Mount root
        mount(Some("/"), format!("/.snapshots/rootfs/snapshot-chr{}", snapshot).as_str(),
              Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE, None::<&str>)?;
        // Deploy or not
        if yes_no(format!("Snapshot {} is now hollow! Whenever done, type yes to deploy and no to discard", snapshot).as_str()) {
            post_transactions(snapshot)?;
            immutability_enable(snapshot)?;
            deploy(snapshot, false)?;
        } else {
            chr_delete(snapshot)?;
            return Err(Error::new(ErrorKind::Interrupted,
                                  format!("No changes applied on snapshot {}.", snapshot)));
        }
    }
    Ok(())
}

// Make a node mutable
pub fn immutability_disable(snapshot: &str) -> std::io::Result<()> {
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
                Command::new("btrfs").arg("property")
                                     .arg("set")
                                     .arg("-ts")
                                     .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                                     .arg("ro")
                                     .arg("false")
                                     .output()?;
                File::create(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", snapshot))?;
                write_desc(snapshot, " MUTABLE ")?;
            }
        }

    } else {
        // Base snapshot unsupported
        return Err(Error::new(ErrorKind::Unsupported, format!("Snapshot 0 (base) should not be modified.")));
    }
    Ok(())
}

//Make a node immutable
pub fn immutability_enable(snapshot: &str) -> std::io::Result<()> {
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
                Command::new("btrfs").arg("property")
                                     .arg("set")
                                     .arg("-ts")
                                     .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                                     .arg("ro")
                                     .arg("true")
                                     .output()?;
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
pub fn install(snapshot: &str, pkg: &str) -> std::io::Result<()> {
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
        if install_package(snapshot, pkg).is_ok() {
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
pub fn install_live(snapshot: &str, pkg: &str) -> std::io::Result<()> {
    let tmp = get_tmp();
    ash_mounts(&tmp.as_str(), "").unwrap();
    println!("Please wait as installation is finishing.");
    install_package_live(snapshot, tmp.as_str(), pkg)?;
    ash_umounts(tmp.as_str(), "").unwrap();
    Ok(())
}

// Install a profile from a text file //REVIEW
fn install_profile(snapshot: &str, profile: &str, force: bool, secondary: bool, section_only: Option<String>) -> std::io::Result<()> {
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
        auto_upgrade(snapshot)?;
        prepare(snapshot)?;
        let pkgs = "";
        let tmp_prof = Temp::new_dir_in("/.snapshots/tmp")?;
        Command::new("sh") //REVIEW change this?
        .arg("-c")
        .arg(format!("curl --fail -o {}/packages.txt -LO https://raw.githubusercontent.com/ashos/ashos/main/src/profiles/{}/packages{}.txt",
             tmp_prof.as_os_str().to_str().unwrap(),profile,get_distro_suffix(&detect::distro_id().as_str()))).status().unwrap();
        loop {
            // Ignore empty lines or ones starting with # [ % &
            let pkg = String::from_utf8(Command::new("sh")
                                        .arg("-c")
                                        .arg(r"cat {tmp_prof}/packages.txt | grep -E -v '^#|^\[|^%|^&|^$'")
                                        .output().unwrap().stdout).unwrap().trim().replace("\n", " ").to_string();
            let excode1 = install_package(snapshot, pkg.as_str());
            let excode2 = service_enable(snapshot, profile, tmp_prof.as_os_str().to_str().unwrap());
            if excode1.is_ok() && excode2 == 1 {
                chr_delete(snapshot).unwrap();
                return Err(Error::new(ErrorKind::Unsupported,
                              format!("Install failed and changes discarded.")));
            } else {
                post_transactions(snapshot)?;
                deploy(snapshot, secondary)?;
            }
        }
    }
    Ok(())
}

// Install profile in live snapshot //REVIEW
fn install_profile_live(snapshot: &str,profile: &str) -> i32 {
    let tmp = get_tmp();
    ash_mounts(tmp.as_str(), "").unwrap();
    println!("Updating the system before installing profile {}.", profile);
    auto_upgrade(tmp.as_str()).unwrap();
    let tmp_prof = String::from_utf8(Command::new("sh").arg("-c")
                                     .arg("mktemp -d -p /tmp ashpk_profile.XXXXXXXXXXXXXXXX")
                                     .output().unwrap().stdout).unwrap().trim().to_string();
    Command::new("sh").arg("-c") // REVIEW
                      .arg(format!("curl --fail -o {}/packages.txt -LO https://raw.githubusercontent.com/ashos/ashos/main/src/profiles/{}/packages{}.txt", tmp_prof,profile,get_distro_suffix(&detect::distro_id().as_str()))).status().unwrap();
    // Ignore empty lines or ones starting with # [ % &
    let pkg = String::from_utf8(Command::new("sh").arg("-c")
                                     .arg(r"cat {tmp_prof}/packages.txt | grep -E -v '^#|^\[|^%|^$'")
                                     .output().unwrap().stdout).unwrap().trim().replace("\n", " ").to_string();
    let excode1 = install_package_live(snapshot, tmp.as_str(), pkg.as_str());
    let excode2 = service_enable(tmp.as_str(), profile, tmp_prof.as_str());
    Command::new("umount").arg(format!("/.snapshots/rootfs/snapshot-{}/*", tmp)).status().unwrap();
    Command::new("umount").arg(format!("/.snapshots/rootfs/snapshot-{}", tmp)).status().unwrap();
    if excode1.is_ok() && excode2 == 0 {
        println!("Profile {} installed in current/live snapshot.", profile);
        return 0;
    } else {
        println!("Install failed and changes discarded.");
        return 1;
    }
}

// Triage functions for argparse method //REVIEW
pub fn install_triage(not_live: bool, live: bool, pkg: &str, profile: &str, snapshot: &str, force: bool) {
    if !profile.is_empty() {
        //install_profile(snapshot, profile, force);
    } else if !pkg.is_empty() {
        let package = pkg.to_string() + " ";
        install(snapshot, &package).unwrap();
    }
    // If installing into current snapshot and no not_live flag, use live install
    let live = if snapshot == get_current_snapshot() && !live {
        true
    } else {
        false
    };
    // Perform the live install only if install above was successful
    if live {
        if !profile.is_empty() {
            install_profile_live(snapshot, profile);
        } else if !pkg.is_empty() {
            let package = pkg.to_string() + " ";
            install_live(snapshot, &package).unwrap();
        }
    }
}

// Check EFI
pub fn is_efi() -> bool {
    let is_efi = Path::new("/sys/firmware/efi").try_exists().unwrap();
    is_efi
}

// List sub-volumes for the booted distro only
pub fn list_subvolumes() {
    let args = format!("btrfs sub list / | grep -i {} | sort -f -k 9",
                       get_distro_suffix(&detect::distro_id()).as_str());
    Command::new("sh").arg("-c").arg(args).status().unwrap();
}

// Live unlocked shell
pub fn live_unlock() {
    let tmp = get_tmp();
    Command::new("mount").arg("--bind")
                         .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                         .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp)).status().unwrap();
    Command::new("mount").arg("--bind")
                         .arg("/etc")
                         .arg(format!("/.snapshots/rootfs/snapshot-{}/etc", tmp)).status().unwrap();
    Command::new("mount").arg("--bind")
                         .arg("/home")
                         .arg(format!("/.snapshots/rootfs/snapshot-{}/home", tmp)).status().unwrap();
    Command::new("mount").arg("--bind")
                         .arg("/tmp")
                         .arg(format!("/.snapshots/rootfs/snapshot-{}/tmp", tmp)).status().unwrap();
    Command::new("mount").arg("--bind")
                         .arg("/var")
                         .arg(format!("/.snapshots/rootfs/snapshot-{}/var", tmp)).status().unwrap();
    Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-{}", tmp)).status().unwrap();
    Command::new("sh").arg("-c")
                        .arg(format!("umount /.snapshots/rootfs/snapshot-{}/*", tmp)).output().unwrap();
    Command::new("umount").arg(format!("/.snapshots/rootfs/snapshot-{}", tmp)).status().unwrap();
    // TODO prevent unlock if ash chroot is active
}

// Creates new tree from base file
pub fn new_snapshot(desc: &str) {
    // immutability toggle not used as base should always be immutable
    let i = find_new();
    Command::new("btrfs").args(["sub", "snap", "-r"])
                         .arg("/.snapshots/boot/boot-0")
                         .arg(format!("/.snapshots/boot/boot-{}", i))
                         .output().unwrap();
    Command::new("btrfs").args(["sub", "snap", "-r"])
                         .arg("/.snapshots/etc/etc-0")
                         .arg(format!("/.snapshots/etc/etc-{}", i))
                         .output().unwrap();
    Command::new("btrfs").args(["sub", "snap", "-r"])
                         .arg("/.snapshots/rootfs/snapshot-0")
                         .arg(format!("/.snapshots/rootfs/snapshot-{}", i))
                         .output().unwrap();
    // Import tree file
    let tree = fstree().unwrap();

    append_base_tree(&tree, i).unwrap();
    let excode = write_tree(&tree);
    if desc.is_empty() {
        write_desc(i.to_string().as_str(), "clone of base.").unwrap();
    } else {
        write_desc(i.to_string().as_str(), desc).unwrap();
    }
    if excode.is_ok() {
        println!("New tree {} created.", i);
    }
}

// Post transaction function, copy from chroot dirs back to read only snapshot dir
pub fn post_transactions(snapshot: &str) -> std::io::Result<()> {
    // Some operations were moved below to fix hollow functionality
    let tmp = get_tmp();
    //File operations in snapshot-chr
    remove_dir_content(format!("/.snapshots/boot/boot-chr{}", snapshot).as_str()).unwrap();
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/boot/", snapshot))
                      .arg(format!("/.snapshots/boot/boot-chr{}", snapshot))
                      .output().unwrap();
    remove_dir_content(format!("/.snapshots/etc/etc-chr{}", snapshot).as_str()).unwrap();
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/etc/", snapshot))
                      .arg(format!("/.snapshots/etc/etc-chr{}", snapshot))
                      .output().unwrap();

    // Keep package manager's cache after installing packages. This prevents unnecessary downloads for each snapshot when upgrading multiple snapshots
    cache_copy(snapshot).unwrap();

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
        init_system_copy(tmp.as_str(), "post_transactions").unwrap();
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
        init_system_copy(tmp.as_str(), "post_transactions").unwrap();
        create_snapshot(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot),
                        format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                        CreateSnapshotFlags::READ_ONLY, None).unwrap();
    }

    // fix for hollow functionality
    ash_umounts(snapshot, "chr")?;

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
            umount2(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path)),
                    MntFlags::MNT_DETACH).unwrap();
        }
    }
    if !mutable_dirs_shared.is_empty() {
        for mount_path in mutable_dirs_shared {
            umount2(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path)),
                    MntFlags::MNT_DETACH).unwrap();
        }
    }

    // fix for hollow functionality
    chr_delete(snapshot)?;
    Ok(())
}

// Prepare snapshot to chroot directory to install or chroot into
pub fn prepare(snapshot: &str) -> std::io::Result<()> {
    chr_delete(snapshot)?;
    let snapshot_chr = format!("/.snapshots/rootfs/snapshot-chr{}", snapshot);

    // create chroot directory
    create_snapshot(format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                    &snapshot_chr,
                    CreateSnapshotFlags::empty(), None).unwrap();

    // Pacman gets weird when chroot directory is not a mountpoint, so the following mount is necessary
    ash_mounts(snapshot, "chr")?;

    // File operations for snapshot-chr
    create_snapshot(format!("/.snapshots/boot/boot-{}", snapshot),
                    format!("/.snapshots/boot/boot-chr{}", snapshot),
                    CreateSnapshotFlags::empty(), None).unwrap();
    create_snapshot(format!("/.snapshots/etc/etc-{}", snapshot),
                    format!("/.snapshots/etc/etc-chr{}", snapshot),
                    CreateSnapshotFlags::empty(), None).unwrap();
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/boot/boot-chr{}/", snapshot))
                      .arg(format!("{}/boot", snapshot_chr))
                      .output()?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/etc/etc-chr{}/", snapshot))
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
    Ok(())
}

// Refresh snapshot
pub fn refresh(snapshot: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot refresh as snapshot {} doesn't exist.", snapshot);
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        eprintln!("F: Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snapshot,snapshot)
    } else if snapshot == "0" {
        eprintln!("Changing base snapshot is not allowed.");
    } else {
        //sync_time() // REVIEW At least required in virtualbox, otherwise error in package db update
        prepare(snapshot).unwrap();
        let excode = refresh_helper(snapshot);
        if excode.success() {
            post_transactions(snapshot).unwrap();
            println!("Snapshot {} refreshed successfully.", snapshot);
        } else {
            chr_delete(snapshot).unwrap();
            eprintln!("Refresh failed and changes discarded.")
        }
    }
}

// Remove directory contents
fn remove_dir_content(dir_path: &str) -> std::io::Result<()> {
    // Specify the path to the directory to remove contents from
    let path = PathBuf::from(dir_path);

    // Iterate over the directory entries using the `read_dir` function
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        // Check if the entry is a file or a directory
        if path.is_file() {
            // If it's a file, remove it using the `remove_file` function
            std::fs::remove_file(path)?;
        } else if path.is_symlink() {
            // If it's a symlink, remove it using the `remove_file` function
            std::fs::remove_file(path)?;
        } else if path.is_dir() {
            // If it's a directory, recursively remove its contents using the `remove_dir_all` function
            std::fs::remove_dir_all(path)?;
        }
    }
    Ok(())
}

// Recursively remove package in tree //REVIEW
pub fn remove_from_tree(treename: &str, pkg: &str, profile: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
        eprintln!("Cannot update as tree {} doesn't exist.", treename);
    } else {
        if !pkg.is_empty() {
            // Import tree file
            let tree = fstree().unwrap();
            uninstall_package(treename, pkg);
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
                order.remove(0);
                order.remove(0);
                let snapshot = &order[1];
                uninstall_package(snapshot, pkg);
            }
            println!("Tree {} updated.", treename);
        } else if !profile.is_empty() {
            println!("profile unsupported"); //TODO
        }
    }
}

// Rollback last booted deployment
pub fn rollback() {
    let tmp = get_tmp();
    let i = find_new();
    clone_as_tree(tmp.as_str(), "").unwrap(); // REVIEW clone_as_tree(tmp, "rollback") will do.
    write_desc(i.to_string().as_str(), "rollback").unwrap();
    deploy(i.to_string().as_str(), false).unwrap();
}

// Recursively run an update in tree //REVIEW
pub fn run_tree(treename: &str, cmd: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
        eprintln!("Cannot update as tree {} doesn't exist.", treename);
    } else {
        prepare(treename).unwrap();
        Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{} {}", treename,cmd)).status().unwrap();
        post_transactions(treename).unwrap();
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
            order.remove(0);
            order.remove(0);
            let snapshot = &order[1];
            if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
                eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snapshot,snapshot);
                eprintln!("Tree command canceled.");
                break;
            } else {
                let sarg = &order[1];
                prepare(sarg.as_str()).unwrap();
                Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{} {}", sarg,cmd)).status().unwrap();
                post_transactions(sarg.as_str()).unwrap();
                println!("Tree {} updated.", treename);
                break;
            }
        }
    }
}

// Enable service(s) (Systemd, OpenRC, etc.) //REVIEW error handling
fn service_enable(snapshot: &str, profile: &str, tmp_prof: &str) -> i32 {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot enable services as snapshot {} doesn't exist.", snapshot);
        return 1;
    } else { // No need for other checks as this function is not exposed to user
        loop {
            let postinst: Vec<String> = String::from_utf8(Command::new("sh").arg("-c")
                                             .arg(format!("cat {}/packages.txt | grep -E -w '^&' | sed 's|& ||'", tmp_prof))
                                             .output().unwrap().stdout).unwrap().trim().split('\n')
                                                                                       .map(|s| s.to_string()).collect(); //REVIEW
            for cmd in postinst.into_iter().filter(|cmd| !cmd.is_empty()) {// remove '' from [''] if no postinstalls
                Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{} {}", snapshot,cmd)).status().unwrap();
            }
            let services: Vec<String> = String::from_utf8(Command::new("sh").arg("-c")
                                             .arg(format!("cat {}/packages.txt | grep -E -w '^%' | sed 's|% ||'", tmp_prof))
                                             .output().unwrap().stdout).unwrap().trim().split('\n')
                                                                                       .map(|s| s.to_string()).collect();//REVIEW
            for cmd in services.into_iter().filter(|cmd| !cmd.is_empty()) { // remove '' from [''] if no services
                let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{} {}",snapshot,cmd)).status().unwrap();
                if excode.success() {
                    println!("Failed to enable service(s) from {}.", profile);
                } else {
                    println!("Installed service(s) from {}.", profile);
                }
            }
            break 0;
        }
    }
}

// Calls print function
pub fn show_fstree() {
    // Import tree file
    let tree = fstree().unwrap();
    print_tree(&tree);
}

// Edit per-snapshot configuration
pub fn snapshot_config_edit(snapshot: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot chroot as snapshot {} doesn't exist.", snapshot);
    } else if snapshot == "0" {
        eprintln!("Changing base snapshot is not allowed.")
    } else {
        prepare(snapshot).unwrap();
        if std::env::var_os("EDITOR").is_some() { // REVIEW always return None
        Command::new("sh").arg("-c")
                          .arg(format!("$EDITOR /.snapshots/rootfs/snapshot-chr{}/etc/ash.conf", snapshot))
                          .status().unwrap();// usage: sudo -E ash edit X
            } else {
            // nano available
            if Command::new("sh").arg("-c")
                                 .arg("[ -x \"$(command -v nano)\" ]")
                                 .status().unwrap().success() {
                                     Command::new("sh").arg("-c")
                                                       .arg(format!("nano /.snapshots/rootfs/snapshot-chr{}/etc/ash.conf", snapshot))
                                                       .status().unwrap();
                                 }
            // vi available
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v vi)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("vi /.snapshots/rootfs/snapshot-chr{}/etc/ash.conf", snapshot))
                                                            .status().unwrap();
                                      }
            // vim available
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v vim)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("vim /.snapshots/rootfs/snapshot-chr{}/etc/ash.conf", snapshot))
                                                            .status().unwrap();
                                      }
            // neovim
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v nvim)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("nvim /.snapshots/rootfs/snapshot-chr{}/etc/ash.conf", snapshot))
                                                            .status().unwrap();
                                      }
            // micro
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v micro)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("micro /.snapshots/rootfs/snapshot-chr{}/etc/ash.conf", snapshot))
                                                            .status().unwrap();
                                      }
            else {
                eprintln!("No text editor available!");
            }
            post_transactions(snapshot).unwrap();
        }
    }
}

// Get per-snapshot configuration options
pub fn snapshot_config_get(snapshot: &str) -> HashMap<String, String> {
    let mut options = HashMap::new();

    if !Path::new(&format!("/.snapshots/etc/etc-{}/ash.conf", snapshot)).try_exists().unwrap() {
        // defaults here
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
pub fn snapshot_unlock(snapshot: &str) {
    Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/boot/boot-chr{}", snapshot)).status().unwrap();
    Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/etc/etc-chr{}", snapshot)).status().unwrap();
    Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).status().unwrap();
}

// Switch between distros
fn switch_distro() -> std::io::Result<()> {
    let map_output = Command::new("sh")
        .arg("-c")
        .arg(r#"cat /boot/efi/EFI/map.txt | awk 'BEGIN { FS = "'"'" === "'"'" } ; { print $1 }'"#)
        .output().unwrap();
    let map_tmp = String::from_utf8(map_output.stdout).unwrap().trim().to_owned();

    loop {
        println!("Type the name of a distro to switch to: (type 'list' to list them, 'q' to quit)");
        let mut next_distro = String::new();
        stdin().lock().read_line(&mut next_distro).unwrap();
        let next_distro = next_distro.trim();

        if next_distro == "q" {
            break;
        } else if next_distro == "list" {
            println!("{}", map_tmp);
        } else if map_tmp.contains(next_distro) {
            let file = std::fs::File::open("/boot/efi/EFI/map.txt").unwrap();
            let mut input_file = csv::ReaderBuilder::new()
                .delimiter(b',')
                .quote(b'\0')
                .from_reader(file);
            for row in input_file.records() {
                let record = row.unwrap();
                if record.get(0) == Some(&next_distro.to_owned()) {
                    let boot_order_output = Command::new("sh")
                        .arg("-c")
                        .arg(r#"efibootmgr | grep BootOrder | awk '{print $2}'"#)
                        .output().unwrap();
                    let boot_order = String::from_utf8(boot_order_output.stdout).unwrap().trim().to_owned();
                    let temp = boot_order.replace(&format!("{},", record[1].to_string().as_str()), "");
                    let new_boot_order = format!("{},{}", record[1].to_string().as_str(), temp);
                    Command::new("sh")
                        .arg("-c")
                        .arg(&format!("efibootmgr --bootorder {}", new_boot_order))
                        .output().unwrap();
                    println!("Done! Please reboot whenever you would like switch to {}", next_distro);
                    break;
                }
            }
            break;
        } else {
            println!("Invalid distro!");
            continue;
        }
    }
    Ok(())
}

// Switch between /tmp deployments
pub fn switch_tmp(secondary: bool) -> std::io::Result<()> {
    let distro_suffix = get_distro_suffix(&detect::distro_id().as_str());
    let grub = get_grub().unwrap();
    let part = get_part();
    let tmp_boot = Temp::new_dir_in("/.snapshots/tmp")?;

    // Mount boot partition for writing
    mount(Some(part.as_str()), tmp_boot.as_os_str(),
          Some("btrfs"), MsFlags::empty(), Some(format!("subvol=@boot{}", distro_suffix).as_bytes()))?;

    // Swap deployment subvolumes: deploy <-> deploy-aux
    let source_dep = get_tmp();
    let target_dep = get_aux_tmp(source_dep.to_string(), secondary);
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-{}/boot/", target_dep))
                      .arg(format!("{}", tmp_boot.to_str().unwrap()))
                      .output().unwrap();

    // Overwrite grub config boot subvolume
    let tmp_grub_cfg = format!("{}/{}/grub.cfg", tmp_boot.to_str().unwrap(),grub);
    // Read the contents of the file into a string
    let mut contents = String::new();
    let mut file = File::open(&tmp_grub_cfg)?;
    file.read_to_string(&mut contents)?;
    let modified_tmp_contents = contents.replace(format!("@.snapshots{}/rootfs/snapshot-{}", distro_suffix,source_dep).as_str(),
                                             format!("@.snapshots{}/rootfs/snapshot-{}", distro_suffix,target_dep).as_str());
    // Write the modified contents back to the file
    let mut file = File::create(tmp_grub_cfg)?;
    file.write_all(modified_tmp_contents.as_bytes())?;

    let grub_cfg = format!("/.snapshots/rootfs/snapshot-{}/{}/grub.cfg", target_dep,grub);
    // Read the contents of the file into a string
    let mut contents = String::new();
    let mut file = File::open(&grub_cfg)?;
    file.read_to_string(&mut contents)?;
    let modified_cfg_contents = contents.replace(format!("@.snapshots{}/rootfs/snapshot-{}", distro_suffix,source_dep).as_str(),
                                             format!("@.snapshots{}/rootfs/snapshot-{}", distro_suffix,target_dep).as_str());
    // Write the modified contents back to the file
    let mut file = File::create(grub_cfg)?;
    file.write_all(modified_cfg_contents.as_bytes())?;

    // Update fstab for new deployment
    let fstab_file = format!("/.snapshots/rootfs/snapshot-{}/etc/fstab", target_dep);
    // Read the contents of the file into a string
    let mut contents = String::new();
    let mut file = File::open(&fstab_file)?;
    file.read_to_string(&mut contents)?;
    let modified_boot_contents = contents.replace(format!("@.snapshots{}/boot/boot-{}", distro_suffix,source_dep).as_str(),
                                                  format!("@.snapshots{}/boot/boot-{}", distro_suffix,target_dep).as_str());
    let modified_etc_contents = contents.replace(format!("@.snapshots{}/etc/etc-{}", distro_suffix,source_dep).as_str(),
                                                 format!("@.snapshots{}/etc/etc-{}", distro_suffix,target_dep).as_str());
    let modified_rootfs_contents = contents.replace(format!("@.snapshots{}/rootfs/snapshot-{}", distro_suffix,source_dep).as_str(),
                                                    format!("@.snapshots{}/rootfs/snapshot-{}", distro_suffix,target_dep).as_str());
    // Write the modified contents back to the file
    let mut file = File::create(fstab_file)?;
    file.write_all(modified_boot_contents.as_bytes())?;
    file.write_all(modified_etc_contents.as_bytes())?;
    file.write_all(modified_rootfs_contents.as_bytes())?;

    let file = File::open(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", source_dep)).unwrap();
    let mut reader = BufReader::new(file);
    let mut sfile = String::new();
    reader.read_line(&mut sfile).unwrap();
    let snap = sfile.replace(" ", "").replace('\n', "");

    // Update GRUB configurations
    for boot_location in ["/.snapshots/rootfs/snapshot-deploy-aux/", &tmp_boot.to_str().unwrap()] {
        let file_path = format!("{}/{}/grub.cfg", boot_location, grub);
        let file = File::open(&file_path)?;
        let reader = BufReader::new(file);
        let mut gconf = String::new();
        let mut in_10_linux = false;
        let begin_str = "BEGIN /etc/grub.d/10_linux";
        let end_str = "}";
        let snapshot_str = "snapshot-";
        for line in reader.lines() {
            let line = line?;
            if line.contains(begin_str) {
                in_10_linux = true;
            } else if in_10_linux {
                if line.contains(end_str) {
                    gconf.push_str(&line);
                    break;
                } else {
                    gconf.push_str(&line);
                }
            }
        }
        if gconf.contains(snapshot_str) {
            if gconf.contains("snapshot-deploy-aux") {
                gconf = gconf.replace("snapshot-deploy-aux", "snapshot-deploy");
            } else if gconf.contains("snapshot-deploy") {
                gconf = gconf.replace("snapshot-deploy", "snapshot-deploy-aux");
            }
            gconf = gconf.replace(&detect_distro::distro_name(), &format!("{} last booted deployment (snapshot {})",
                                                                          &detect_distro::distro_name(), snap));
            while gconf.contains("snapshot ") {
                let prefix = gconf.split("snapshot ").next().unwrap();
                let suffix = gconf.split("snapshot ").skip(1).next().unwrap();
                let snapshot_num = suffix.split(' ').next().unwrap();
                let suffix = suffix.replacen(snapshot_num, "", 1);
                gconf = format!("{}{}", prefix, suffix);
            }
        }
        let mut file = File::create(&file_path)?;
        for line in BufReader::new(gconf.as_bytes()).lines() {
            writeln!(file, "{}", line?)?;
        }
        writeln!(file, "}}")?;
        writeln!(file, "### END /etc/grub.d/41_custom ###")?;
    }

    // Umount boot partition
    umount2(Path::new(&format!("{}", tmp_boot.as_os_str().to_str().unwrap())),
            MntFlags::MNT_DETACH)?;

    Ok(())
}

// Sync time
pub fn sync_time() {
    Command::new("sh")
        .arg("-c")
        .arg("date -s \"$(curl --tlsv1.3 --proto =https -I https://google.com 2>&1 | grep Date: | cut -d\" \" -f3-6)Z\"")
        .status().unwrap();
}

// Sync tree and all its snapshots //REVIEW
pub fn sync_tree(treename: &str, force_offline: bool, live: bool) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
        println!("Cannot sync as tree {} doesn't exist.", treename);
    } else {
        if !force_offline { // Syncing tree automatically updates it, unless 'force-sync' is used
            update_tree(treename);
        }
        // Import tree file
        let tree = fstree().unwrap();

        let mut order = recurse_tree(&tree, treename);
        if order.len() > 2 {
            order.remove(0); // TODO: Better way instead of these repetitive removes
            order.remove(0);
        }
        loop {
            if order.len() < 2 {
                break;
            }
            let snap_from = &order[0];
            let snap_to = &order[1];
            println!("{}, {}", snap_from, snap_to);
            order.remove(0);
            order.remove(0);
            let snap_from_order = &order[0];
            let snap_to_order = &order[1];
            if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snap_to_order)).try_exists().unwrap() {
                println!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snap_to_order,snap_to_order);
                println!("Tree sync canceled.");
            } else {
                prepare(snap_to_order).unwrap();
                sync_tree_helper("chr", snap_from_order, snap_to_order).unwrap(); // Pre-sync
                if live && snap_to_order == get_current_snapshot().as_str() { // Live sync
                    sync_tree_helper("", snap_from_order, get_tmp().as_str()).unwrap(); // Post-sync
                    }
                post_transactions(snap_to_order).unwrap(); // Moved here from the line immediately after first sync_tree_helper
            }
            println!("Tree {} synced.", treename);
            break;
        }
    }
}

// Sync tree helper function // REVIEW might need to put it in distro-specific ashpk.py
fn sync_tree_helper(chr: &str, s_f: &str, s_t: &str) -> std::io::Result<()>  {
    Command::new("mkdir").arg("-p").arg("/.snapshots/tmp-db/local/").status().unwrap(); // REVIEW Still resembling Arch pacman folder structure!
    Command::new("rm").arg("-rf").arg("/.snapshots/tmp-db/local/*").status().unwrap(); // REVIEW
    let pkg_list_to = pkg_list(chr, s_t);
    let pkg_list_from = pkg_list("", s_f);
    // Get packages to be inherited
    let mut pkg_list_new = Vec::new();
    for j in pkg_list_from {
        if !pkg_list_to.contains(&j) {
            pkg_list_new.push(j);
        }
    }
    let pkg_list_from = pkg_list_new;
    Command::new("cp").arg("-r")
                      .arg(format!("/.snapshots/rootfs/snapshot-{}{}/usr/share/ash/db/local/.", chr,s_t))
                      .arg("/.snapshots/tmp-db/local/").status().unwrap(); // REVIEW
    Command::new("cp").args(["-n", "-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-{}/.", s_f))
                      .arg(format!("/.snapshots/rootfs/snapshot-{}{}/", chr,s_t))
                      .status().unwrap();
    Command::new("rm").arg("-rf")
                      .arg(format!("/.snapshots/rootfs/snapshot-{}{}/usr/share/ash/db/local/*", chr,s_t))
                      .status().unwrap(); // REVIEW
    Command::new("cp").arg("-r")
                      .arg("/.snapshots/tmp-db/local/.")
                      .arg(format!("/.snapshots/rootfs/snapshot-{}{}/usr/share/ash/db/local/", chr,s_t))
                      .status().unwrap(); // REVIEW
    for entry in pkg_list_from {
        Command::new("sh").arg("-c")
                          .arg(format!("cp -r /.snapshots/rootfs/snapshot-{}/usr/share/ash/db/local/{}-[0-9]*", s_f,entry))
                          .arg(format!("/.snapshots/rootfs/snapshot-{}{}/usr/share/ash/db/local/'", chr,s_t)).status().unwrap();// REVIEW
        }
    Command::new("rm").arg("-rf").arg("/.snapshots/tmp-db/local/*").status().unwrap(); // REVIEW (originally inside the loop, but I took it out
    Ok(())
}

// Clear all temporary snapshots
pub fn tmp_clear() {
    Command::new("sh").arg("-c")
                        .arg(format!("btrfs sub del /.snapshots/boot/boot-chr*"))
                        .status().unwrap();
    Command::new("sh").arg("-c")
                        .arg(format!("btrfs sub del /.snapshots/etc/etc-chr*"))
                        .status().unwrap();
    Command::new("sh").arg("-c")
                        .arg(format!("btrfs sub del '/.snapshots/rootfs/snapshot-chr*/*'"))
                        .status().unwrap();
    Command::new("sh").arg("-c")
                        .arg(format!("btrfs sub del /.snapshots/rootfs/snapshot-chr*"))
                        .status().unwrap();
}

// Clean tmp dirs
pub fn tmp_delete(secondary: bool) -> std::io::Result<()> {
    // Get tmp
    let tmp = get_tmp();
    let tmp = get_aux_tmp(tmp, secondary);

    // Clean tmp
    if remove_dir_content(format!("/.snapshots/boot/boot-{}", tmp).as_str()).is_ok() {
        delete_subvolume(&format!("/.snapshots/boot/boot-{}", tmp), DeleteSubvolumeFlags::empty()).unwrap();
    }
    if remove_dir_content(format!("/.snapshots/etc/etc-{}", tmp).as_str()).is_ok() {
        delete_subvolume(&format!("/.snapshots/etc/etc-{}", tmp), DeleteSubvolumeFlags::empty()).unwrap();
    }
    if remove_dir_content(format!("/.snapshots/rootfs/snapshot-{}", tmp).as_str()).is_ok() {
        delete_subvolume(&format!("/.snapshots/rootfs/snapshot-{}", tmp), DeleteSubvolumeFlags::empty()).unwrap();
    }
    Ok(())
}

// Uninstall package(s)
pub fn uninstall_package(snapshot: &str, pkg: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot remove as snapshot {} doesn't exist.", snapshot);
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snapshot,snapshot);
    } else if snapshot == "0" {
        eprintln!("Changing base snapshot is not allowed.");
    } else {
        prepare(snapshot).unwrap();
        let excode = uninstall_package_helper(snapshot, pkg);
        if excode.success() {
            post_transactions(snapshot).unwrap();
            println!("Package {} removed from snapshot {} successfully.", pkg,snapshot);
        } else {
            chr_delete(snapshot).unwrap();
            eprintln!("Remove failed and changes discarded.");
        }
    }
}

pub fn uninstall_triage(snapshot: &str, profile: &str, pkg: &str) { // TODO add live, not_live
    if !profile.is_empty() {
        //let excode = install_profile(snapshot, profile);
        println!("TODO");
    } else if !pkg.is_empty() {
        let package = pkg.to_string() + " ";
        uninstall_package(snapshot,  &package);
    }
}

// Update boot
pub fn update_boot(snapshot: &str, secondary: bool) -> std::io::Result<()> {
    // Path to grub directory
    let grub = get_grub().unwrap();

    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot update boot as snapshot {} doesn't exist.", snapshot)));

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
        // Remove grub configurations older than 30 days.
        if Path::new(&format!("{}/BAK/", grub)).try_exists().unwrap() {
            delete_old_grub_files(&format!("{}", grub).as_str())?;
        }
        // Get current time
        let time = time::OffsetDateTime::now_utc();
        let formatted = time.format(&time::format_description::parse("[year][month][day]-[hour][minute][second]").unwrap()).unwrap();
        // Copy backup
        copy(format!("{}/grub.cfg", grub), format!("{}/BAK/grub.cfg.{}", grub,formatted))?;

        // Run update commands in chroot
        let mkconfig = format!("grub-mkconfig {} -o {}/grub.cfg", part,grub);
        let sed_snap = format!("sed -i 's|snapshot-chr{}|snapshot-{}|g' {}/grub.cfg", snapshot,tmp,grub);
        let sed_distro = format!("sed -i '0,\\|{}| s||{} snapshot {}|' {}/grub.cfg",
                                 detect::distro_name(),detect::distro_name(),snapshot,grub);
        let cmds = [mkconfig, sed_snap, sed_distro];
        for cmd in cmds {
            Command::new("sh").arg("-c")
                              .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} {}", snapshot,cmd))
                              .output()?;
        }

        // Finish the update
        post_transactions(snapshot)?;
    }
    Ok(())
}

// Saves changes made to /etc to snapshot
pub fn update_etc() {
    let snapshot = get_current_snapshot();
    let tmp = get_tmp();
    Command::new("btrfs").args(["sub", "del"])
                         .arg(format!("/.snapshots/etc/etc-{}", snapshot)).output().unwrap();
    if check_mutability(&snapshot) {
        let immutability = "";
        Command::new("btrfs").args(["sub", "snap"]).arg(format!("{}", immutability))
                                                   .arg(format!("/.snapshots/etc/etc-{}", tmp))
                                                   .arg(format!("/.snapshots/etc/etc-{}", snapshot)).output().unwrap();
    } else {
        let immutability = "-r";
        Command::new("btrfs").args(["sub", "snap"]).arg(format!("{}", immutability))
                                                   .arg(format!("/.snapshots/etc/etc-{}", tmp))
                                                   .arg(format!("/.snapshots/etc/etc-{}", snapshot)).output().unwrap();
    }
}

// Recursively run an update in tree //REVIEW
pub fn update_tree(treename: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
        eprintln!("Cannot update as tree {} doesn't exist.", treename);
    } else {
        upgrade(treename);
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
                order.remove(0);
                order.remove(0);
                let snapshot = &order[1];
                auto_upgrade(snapshot).unwrap();
            }
        }
        println!("Tree {} updated.", treename)
    }
}

// Upgrade snapshot
pub fn upgrade(snapshot:  &str) ->i32 {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot upgrade as snapshot {} doesn't exist.", snapshot);
        return 1;
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snapshot,snapshot);
        return 1;
    } else if snapshot == "0" {
        eprintln!("Changing base snapshot is not allowed.");
        return 1;
    } else {
        // prepare(snapshot) // REVIEW Moved to a distro-specific function as it needs to go after setup_aur_if_enabled()
        // Default upgrade behaviour is now "safe" update, meaning failed updates get fully discarded
        let excode = upgrade_helper(snapshot);
        if excode.success() {
            post_transactions(snapshot).unwrap();
            println!("Snapshot {} upgraded successfully.", snapshot);
            return 0;
        } else {
            chr_delete(snapshot).unwrap();
            eprintln!("Upgrade failed and changes discarded.");
            return 1;
        }
    }
}

// Write new description (default) or append to an existing one (i.e. toggle immutability)
pub fn write_desc(snapshot: &str, desc: &str) -> std::io::Result<()> {
    let mut descfile = OpenOptions::new().append(true)
                                         .create(true)
                                         .read(true)
                                         .open(format!("/.snapshots/ash/snapshots/{}-desc", snapshot))?;
    descfile.write_all(desc.as_bytes())?;
    Ok(())
}

// Generic yes no prompt
fn yes_no(msg: &str) -> bool {
    loop {
        println!("{} (y/n)", msg);
        let mut reply = String::new();
        stdin().read_line(&mut reply).expect("Failed to read input");
        match reply.trim().to_lowercase().as_str() {
            "yes" | "y" => return true,
            "no" | "n" => return false,
            _ => println!("F: Invalid choice!"),
        }
    }
}
