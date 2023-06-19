mod detect_distro;
mod distros;
mod tree;

use crate::detect_distro as detect;
use crate::distros::*;
use nix::mount::{mount, MsFlags};
use tree::*;
use std::collections::HashMap;
use std::fs::{copy, File, metadata, OpenOptions, read_dir, read_to_string};
use std::io::{BufRead, BufReader, Read, stdin, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

// Ash chroot mounts
pub fn ash_mounts(i: &str, chr: &str) -> nix::Result<()> {
    let snapshot_path = format!("/.snapshots/rootfs/snapshot-{}{}", chr, i);
    // Mount snapshot to itself as a bind mount
    mount(Some(snapshot_path.as_str()), snapshot_path.as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>).unwrap();
    // Mount /dev
    mount(Some("/dev"), format!("{}/dev", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE, None::<&str>).unwrap();
    // Mount /etc
    mount(Some("/etc"), format!("{}/etc", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>).unwrap();
    // Mount /home
    mount(Some("/home"), format!("{}/home", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>).unwrap();
    // Mount /proc
    mount(Some("/proc"), format!("{}/proc", snapshot_path).as_str(),
          Some("proc"), MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV, None::<&str>).unwrap();
    // Mount /run
    mount(Some("/run"), format!("{}/run", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>).unwrap();
    // Mount /sys
    mount(Some("/sys"), format!("{}/sys", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE, None::<&str>).unwrap();
    // Mount /tmp
    mount(Some("/tmp"), format!("{}/tmp", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>).unwrap();
    // Mount /var
    mount(Some("/var"), format!("{}/var", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>).unwrap();
    if is_efi() {
        // Mount /sys/firmware/efi/efivars
        mount(Some("/sys/firmware/efi/efivars"), format!("{}/sys/firmware/efi/efivars", snapshot_path).as_str(),
              Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE, None::<&str>).unwrap();
        // Copy /etc/resolv.conf
        let etc_path = format!("{}/etc/resolv.conf", snapshot_path);
        copy("/etc/resolv.conf", etc_path).unwrap();
    }
    Ok(())
}

// Ash version
pub fn ash_version() {
    //let ash_bin_path = std::env::current_exe().unwrap(); //https://doc.rust-lang.org/std/env/fn.current_exe.html#security
    let ash_bin_path = Path::new("/usr/sbin/ash");
    let metadata = metadata(ash_bin_path).unwrap();
    let time = metadata.modified().unwrap();
    let duration = time.duration_since(UNIX_EPOCH).unwrap();
    let utc = time::OffsetDateTime::UNIX_EPOCH +
        time::Duration::try_from(duration).unwrap();
    let local = utc.to_offset(time::UtcOffset::local_offset_at(utc).unwrap());
    local.format_into(
        &mut std::io::stdout().lock(),
        time::macros::format_description!(
            "Last modified on: [weekday] [day]-[month]-[year] [hour repr:12]:[minute]:[second] [period]\n"
        ),
    ).unwrap();
}

// Copy cache of downloaded packages to shared
pub fn cache_copy(snapshot: &str) {
    Command::new("cp").args(["-n", "-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/cache/pacman/pkg/.", snapshot))
                      .arg("/var/cache/pacman/pkg/").status().unwrap();
}

// Check if snapshot is mutable
pub fn check_mutability(snapshot: &str) -> bool {
    Path::new(&format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", snapshot))
        .try_exists().unwrap()
}

// Check if last update was successful
pub fn check_update() {
    let upstate = File::open("/.snapshots/ash/upstate").unwrap();
    let buf_read = BufReader::new(upstate);
    let mut read = buf_read.lines();
    let line = read.next().unwrap().unwrap();
    let data = read.next().unwrap().unwrap();
    if line.contains("1") {
        eprintln!("Last update on {} failed.", data);
    }
    if line.contains("0") {
        print!("Last update on {} completed successfully.", data)
    }
}

// Clean chroot mount directories for a snapshot
pub fn chr_delete(snapshot: &str) {
    if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        Command::new("btrfs").args(["sub", "del"])
                             .arg(format!("/.snapshots/boot/boot-chr{}", snapshot))
                             .output().expect(&format!("Failed to delete chroot snapshot {}", snapshot));
        Command::new("btrfs").args(["sub", "del"])
                             .arg(format!("/.snapshots/etc/etc-chr{}", snapshot))
                             .output().expect(&format!("Failed to delete chroot snapshot {}", snapshot));
        Command::new("btrfs").args(["sub", "del"])
                             .arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                             .output().expect(&format!("Failed to delete chroot snapshot {}", snapshot));
        }
}

// Run command in snapshot
pub fn chr_run(snapshot: &str, cmd: &str) {
    // make cmd to cmds (IMPORTANT for install_profile)
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot chroot as snapshot {} doesn't exist.", snapshot);
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        // Make sure snapshot is not in use by another ash process
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snapshot,snapshot)
    } else if snapshot == "0" {
        eprintln!("Changing base snapshot is not allowed.")
    } else {
        prepare(snapshot);
        Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                              .arg("sh")
                              .arg("-c")
                              .arg(cmd)
                              .status().unwrap();
        post_transactions(snapshot);
    }
}

// Chroot into snapshot
pub fn chroot(snapshot: &str) {
    // make cmd to cmds (IMPORTANT for install_profile)
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot chroot as snapshot {} doesn't exist.", snapshot);
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        // Make sure snapshot is not in use by another ash process
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snapshot,snapshot)
    } else if snapshot == "0" {
        eprintln!("Changing base snapshot is not allowed.")
    } else {
        prepare(snapshot);
        Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                              .status().unwrap();
        post_transactions(snapshot);
    }
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
pub fn clone_as_tree(snapshot: &str, desc: &str) {
    let i = find_new();
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot clone as snapshot {} doesn't exist.", snapshot);
    } else {
        if check_mutability(snapshot) {
            let immutability = "";
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/boot/boot-{}", snapshot))
                                 .arg(format!("/.snapshots/boot/boot-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/etc/etc-{}", snapshot))
                                 .arg(format!("/.snapshots/etc/etc-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", i)).status().unwrap();
            Command::new("touch").arg(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", i))
                                 .status().unwrap();
        } else {
            let immutability = "-r";
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/boot/boot-{}", snapshot))
                                 .arg(format!("/.snapshots/boot/boot-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/etc/etc-{}", snapshot))
                                 .arg(format!("/.snapshots/etc/etc-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", i)).status().unwrap();
        }
        append_base_tree(i).unwrap();
        write_tree().unwrap();
        if desc.is_empty() {
            let description = format!("clone of {}", snapshot);
            write_desc(i.to_string().as_str(), &description).unwrap();
        } else {
            let description = desc.split("").collect::<Vec<&str>>().join(" ");
            write_desc(i.to_string().as_str(), &description).unwrap();
        }
        println!("Tree {} cloned from {}.", i,snapshot);
    }
}

// Clone branch under same parent
pub fn clone_branch(snapshot: &str) -> i32 {
    let i = find_new();
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot clone as snapshot {} doesn't exist.", snapshot);
    } else {
        if check_mutability(snapshot) {
            let immutability = "";
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/boot/boot-{}", snapshot))
                                 .arg(format!("/.snapshots/boot/boot-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/etc/etc-{}", snapshot))
                                 .arg(format!("/.snapshots/etc/etc-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", i)).status().unwrap();
            Command::new("touch").arg(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", i))
                                 .status().unwrap();
        } else {
            let immutability = "-r";
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/boot/boot-{}", snapshot))
                                 .arg(format!("/.snapshots/boot/boot-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/etc/etc-{}", snapshot))
                                 .arg(format!("/.snapshots/etc/etc-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", i)).status().unwrap();
        }
        add_node_to_level(snapshot, i).unwrap();
        write_tree().unwrap();
        let desc = format!("clone of {}", snapshot);
        write_desc(i.to_string().as_str(), &desc).unwrap();
        println!("Branch {} added to parent of {}.", i,snapshot);
    }
    return i;
}

// Recursively clone an entire tree
pub fn clone_recursive(snapshot: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        println!("Cannot clone as tree {} doesn't exist.", snapshot);
    } else {
        let mut children = return_children(snapshot);
        let ch = children.clone();
        children.insert(0, snapshot.to_string());
        let ntree = clone_branch(snapshot);
        let mut new_children = ch.clone();
        new_children.insert(0, ntree.to_string());
        for child in ch {
            let parent = get_parent(&child).unwrap().to_string();
            let index = children.iter().position(|x| x == &parent).unwrap();
            let i = clone_under(&new_children[index], &child);
            new_children[index] = i.to_string();
        }
    }
}

// Clone under specified parent
pub fn clone_under(snapshot: &str, branch: &str) -> i32 {
    let i = find_new();
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot clone as snapshot {} doesn't exist.", snapshot);
        } else if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", branch)).try_exists().unwrap() {
        eprintln!("Cannot clone as snapshot {} doesn't exist.", branch);
        } else {
        if check_mutability(snapshot) {
            let immutability = "";
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/boot/boot-{}", branch))
                                 .arg(format!("/.snapshots/boot/boot-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/etc/etc-{}", branch))
                                 .arg(format!("/.snapshots/etc/etc-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", branch))
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", i)).status().unwrap();
            Command::new("touch").arg(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", i))
                                 .status().unwrap();
        } else {
            let immutability = "-r";
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/boot/boot-{}", branch))
                                 .arg(format!("/.snapshots/boot/boot-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/etc/etc-{}", branch))
                                 .arg(format!("/.snapshots/etc/etc-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", branch))
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", i)).status().unwrap();
        }
        add_node_to_parent(snapshot, i).unwrap();
        write_tree().unwrap();
        let desc = format!("clone of {}", branch);
        write_desc(i.to_string().as_str(), desc.as_str()).unwrap();
        println!("Branch {} added under snapshot {}.", i,snapshot);
    }
    return i;
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
pub fn delete_node(snapshots: &str, quiet: bool) {
    let snapshots: Vec<&str> = snapshots.split_whitespace().collect();
    for snapshot in snapshots {
        let run = if !quiet {
            let mut answer = String::new();
            println!("Are you sure you want to delete snapshot {}? (y/n)", snapshot);
            stdin().read_line(&mut answer).unwrap();
            let choice: String = answer.trim().parse().unwrap();
            let selected_run = if choice != "y".to_string() {
                false
            } else {
                true
            };
            selected_run
        } else {
            true
        };
        if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() && run != false {
            eprintln!("Cannot delete as snapshot {} doesn't exist.", snapshot);
        } else if snapshot == "0" && run != false {
            eprintln!("Changing base snapshot is not allowed.");
        } else if snapshot == &get_current_snapshot() && run != false {
            eprintln!("Cannot delete booted snapshot.");
        } else if snapshot == &get_next_snapshot() && run != false {
            eprintln!("Cannot delete deployed snapshot.");
        } else if run == true {
            let children = return_children(&snapshot);
            write_desc(&snapshot, "").unwrap(); // Clear descriptio
            Command::new("btrfs").args(["sub", "del"])
                                 .arg(format!("/.snapshots/boot/boot-{}", snapshot))
                                 .status().unwrap();
            Command::new("btrfs").args(["sub", "del"])
                                 .arg(format!("/.snapshots/etc/etc-{}", snapshot))
                                 .status().unwrap();
            Command::new("btrfs").args(["sub", "del"])
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                                 .status().unwrap();
            // Make sure temporary chroot directories are deleted as well
            if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
                Command::new("btrfs").args(["sub", "del"])
                                     .arg(format!("/.snapshots/boot/boot-chr{}", snapshot))
                                     .status().unwrap();
                Command::new("btrfs").args(["sub", "del"])
                                     .arg(format!("/.snapshots/etc/etc-chr{}", snapshot))
                                     .status().unwrap();
                Command::new("btrfs").args(["sub", "del"])
                                     .arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                                     .status().unwrap();
            }
            for child in children { // This deletes the node itself along with its children
                write_desc(&snapshot, "").unwrap();
                Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/boot/boot-{}", child)).status().unwrap();
                Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/etc/etc-{}", child)).status().unwrap();
                Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/rootfs/snapshot-{}", child)).status().unwrap();
                if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", child)).try_exists().unwrap() {
                    Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/boot/boot-chr{}", child)).status().unwrap();
                    Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/etc/etc-chr{}", child)).status().unwrap();
                    Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/rootfs/snapshot-chr{}", child)).status().unwrap();
                }
            }
            if remove_node(&snapshot).is_ok() && write_tree().is_ok() { // Remove node from tree or root
                println!("Snapshot {} removed.", snapshot); //REVIEW
            }
        } else {
            println!("Aborted");
        }
    }
}

// Deploy snapshot //REVIEW
pub fn deploy(snapshot: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot deploy as snapshot {} doesn't exist.", snapshot);
    } else {
        update_boot(snapshot);
        let tmp = get_tmp();
        Command::new("btrfs").args(["sub", "set-default"])
                             .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                             .status().unwrap(); // Set default volume
        tmp_delete();
        let tmp = if tmp.contains("deploy-aux") {
            "deploy"
        } else {
            "deploy-aux"
        };
        // Special mutable directories
        let options = snapshot_config_get(snapshot);
        let mutable_dirs: Vec<&str> = options.get("mutable_dirs")
                                             .map(|dirs| dirs.split(',').filter(|dir| !dir.is_empty()).collect())
                                             .unwrap_or_else(|| Vec::new());
        let mutable_dirs_shared: Vec<&str> = options.get("mutable_dirs_shared")
                                                    .map(|dirs| dirs.split(',').filter(|dir| !dir.is_empty()).collect())
                                                    .unwrap_or_else(|| Vec::new());
        // btrfs snapshot operations
        Command::new("btrfs").args(["sub", "snap"])
                             .arg(format!("/.snapshots/boot/boot-{}", snapshot))
                             .arg(format!("/.snapshots/boot/boot-{}", tmp))
                             .status().unwrap();
        Command::new("btrfs").args(["sub", "snap"])
                             .arg(format!("/.snapshots/etc/etc-{}", snapshot))
                             .arg(format!("/.snapshots/etc/etc-{}", tmp))
                             .status().unwrap();
        Command::new("btrfs").args(["sub", "snap"])
                             .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                             .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                             .status().unwrap();
        Command::new("mkdir").arg("-p")
                             .arg(format!("/.snapshots/rootfs/snapshot-{}/boot", tmp))
                             .status().unwrap();
        Command::new("mkdir").arg("-p")
                             .arg(format!("/.snapshots/rootfs/snapshot-{}/etc", tmp))
                             .status().unwrap();
        Command::new("rm").arg("-rf")
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/var", tmp))
                          .status().unwrap();
        Command::new("cp").args(["-r", "--reflink=auto"])
                          .arg(format!("/.snapshots/boot/boot-{}/.", snapshot))
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/boot", tmp))
                          .status().unwrap();
        Command::new("cp").args(["-r", "--reflink=auto"])
                          .arg(format!("/.snapshots/etc/etc-{}/.", snapshot))
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/etc", tmp))
                          .status().unwrap();
        // If snapshot is mutable, modify '/' entry in fstab to read-write
        if check_mutability(snapshot) {
            Command::new("sh").arg("-c")
                              .arg(format!("sed -i '0,/snapshot-{}/ s|,ro||' /.snapshots/rootfs/snapshot-{}/etc/fstab", tmp,tmp)) // ,rw
                              .status().unwrap();
        }
        // Add special user-defined mutable directories as bind-mounts into fstab
        if !mutable_dirs.is_empty() {
            for mount_path in mutable_dirs {
                let source_path = format!("/.snapshots/mutable_dirs/snapshot-{}/{}", snapshot,mount_path);
                Command::new("mkdir").arg("-p")
                                     .arg(format!("/.snapshots/mutable_dirs/snapshot-{}/{}", snapshot,mount_path))
                                     .status().unwrap();
                Command::new("mkdir").arg("-p")
                                     .arg(format!("/.snapshots/rootfs/snapshot-{}/{}", tmp,mount_path))
                                     .status().unwrap();
                Command::new("sh")
                    .arg("-c")
                    .arg(format!("echo '{} /{} none defaults,bind 0 0' >> /.snapshots/rootfs/snapshot-{}/etc/fstab", source_path,mount_path,tmp))
                    .status().unwrap();
            }
        }
        // Same thing but for shared directories
        if mutable_dirs_shared.is_empty() {
            for mount_path in mutable_dirs_shared {
                let source_path = format!("/.snapshots/mutable_dirs/{}", mount_path);
                Command::new("mkdir").arg("-p")
                                     .arg(format!("/.snapshots/mutable_dirs/{}", mount_path))
                                     .status().unwrap();
                Command::new("mkdir").arg("-p")
                                     .arg(format!("/.snapshots/rootfs/snapshot-{}/{}", tmp,mount_path))
                                     .status().unwrap();
                Command::new("sh").arg("-c")
                                  .arg(format!("echo '{} /{} none defaults,bind 0 0' >> /.snapshots/rootfs/snapshot-{}/etc/fstab", source_path,mount_path,tmp))
                                  .status().unwrap();
                Command::new("btrfs").args(["sub", "snap"])
                                     .arg("/var")
                                     .arg(format!("/.snapshots/rootfs/snapshot-{}/var", tmp)).status().unwrap(); // Is this needed?
                Command::new("sh").arg("-c")
                                  .arg(format!("echo '{}' > /.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", snapshot,tmp))
                                  .status().unwrap();
            }
            switch_tmp();
            init_system_clean(tmp, "deploy");
            let excode = Command::new("btrfs").args(["sub", "set-default"])
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                                 .status().unwrap(); // Set default volume
            if excode.success() {
                println!("Snapshot {} deployed to /.", snapshot) //REVIEW
            }
        }
    }
}

// Add node to branch
pub fn extend_branch(snapshot: &str, desc: &str) {
    let i = find_new();
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot branch as snapshot {} doesn't exist.", snapshot);
    } else {
        if check_mutability(snapshot) {
            let immutability = "";
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/boot/boot-{}", snapshot))
                                 .arg(format!("/.snapshots/boot/boot-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/etc/etc-{}", snapshot))
                                 .arg(format!("/.snapshots/etc/etc-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", i)).status().unwrap();
            Command::new("touch").arg(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", i))
                                 .status().unwrap();
       } else {
            let immutability = "-r";
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/boot/boot-{}", snapshot))
                                 .arg(format!("/.snapshots/boot/boot-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/etc/etc-{}", snapshot))
                                 .arg(format!("/.snapshots/etc/etc-{}", i)).status().unwrap();
            Command::new("btrfs").args(["sub", "snap"])
                                 .arg(immutability)
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                                 .arg(format!("/.snapshots/rootfs/snapshot-{}", i)).status().unwrap();
        }
    }
    add_node_to_parent(snapshot, i).unwrap();
    write_tree().unwrap();
    if desc.is_empty() {
        print!("Branch {} added under snapshot {}.", i,snapshot);
    } else {
        write_desc(i.to_string().as_str(), desc).unwrap();
        print!("Branch {} added under snapshot {}.", i,snapshot);
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

// Get deployed snapshot
pub fn get_next_snapshot() -> String {
    let d = if get_tmp().contains("deploy-aux") {
        "deploy"
    } else {
        "deploy-aux"
    };
    if Path::new(&format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", d)).try_exists().unwrap() {// Make sure next snapshot exists
        let mut file = File::open(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", d)).unwrap();
        let mut contents = String::new();
        let csnapshot = file.read_to_string(&mut contents).unwrap();
        return csnapshot.to_string().trim().to_string();
    } else {
        return "".to_string() // Return empty string in case no snapshot is deployed
        }
}

// Get drive partition
pub fn get_part() -> String {
    let mut file = File::open("/.snapshots/ash/part").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let output = Command::new("sh").arg("-c")
                                   .arg(format!("blkid | grep '{}' | awk -F: '{{print $1}}'", contents.trim_end()))
                                   .output()
                                   .unwrap();
    let cpart = String::from_utf8(output.stdout).unwrap().trim().to_string();
    return cpart;
}

// Get tmp partition state
pub fn get_tmp() -> &'static str {
    // By default just return which deployment is running
    let mount_exec = Command::new("cat")
        .args(["/proc/mounts", "|", "grep", "' / btrfs'"])
        .output().unwrap();
    let mount = String::from_utf8_lossy(&mount_exec.stdout).to_string();
    if mount.contains("deploy-aux") {
        let r = "deploy-aux";
        return r;
    } else {
        let r = "deploy";
        return r;
    }
}

// Make a snapshot vulnerable to be modified even further (snapshot should be deployed as mutable) //REVIEW
pub fn hollow(s: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", s)).try_exists().unwrap() {
        println!("Cannot make hollow as snapshot {} doesn't exist.", s);
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", s)).try_exists().unwrap() { // Make sure snapshot is not in use by another ash process
        println!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", s,s)
    } else if s == "0" {
        println!("Changing base snapshot is not allowed.");
    } else {
        // AUR step might be needed and if so make a distro_specific function with steps similar to install_package().
        // Call it hollow_helper and change this accordingly().
        prepare(s);
        Command::new("mount").arg("--rbind")
                             .arg("--make-rslave")
                             .arg("/")
                             .arg(format!("/.snapshots/rootfs/snapshot-chr{}", s)).status().unwrap();
        println!("Snapshot {} is now hollow! When done, type YES (in capital):", s);
        let mut answer = String::new();
        stdin().read_line(&mut answer).unwrap();
        let choice: String = answer.trim().parse().unwrap();
        let replay = if choice == "YES".to_string() {
            true
        } else {
            false
        };
        while replay == true {
            post_transactions(s);
            immutability_enable(s);
            deploy(s);
            println!("Snapshot {} hollow operation succeeded. Please reboot!", s);
            break;
        }
    }
}

// Make a node mutable
pub fn immutability_disable(snapshot: &str) {
    if snapshot != "0" {
        if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
            eprintln!("Snapshot {} doesn't exist.", snapshot);
        } else {
            if check_mutability(snapshot) {
                println!("Snapshot {} is already mutable.", snapshot);
            } else {
                let excode1 = Command::new("btrfs").arg("property")
                                                   .arg("set")
                                                   .arg("-ts")
                                                   .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                                                   .arg("ro")
                                                   .arg("false")
                                                   .status().unwrap();
                let excode2 = Command::new("touch").arg(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", snapshot))
                                                   .status().unwrap();
                if excode1.success() && excode2.success() {
                    println!("Snapshot {} successfully made mutable.", snapshot);
                }
                write_desc(snapshot, " MUTABLE").unwrap();
            }
        }
    } else {
        eprintln!("Snapshot 0 (base) should not be modified.");
    }
}

//Make a node immutable
pub fn immutability_enable(snapshot: &str) {
    if snapshot != "0" {
        if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
            eprintln!("Snapshot {} doesn't exist.", snapshot);
        } else {
            if !check_mutability(snapshot) {
                println!("Snapshot {} is already immutable.", snapshot);
            } else {
                let excode1 = Command::new("rm").arg(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", snapshot))
                                                .status().unwrap();
                let excode2 = Command::new("btrfs").arg("property")
                                                   .arg("set")
                                                   .arg("-ts")
                                                   .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                                                   .arg("ro")
                                                   .arg("true")
                                                   .status().unwrap();
                if excode1.success() && excode2.success() {
                    println!("Snapshot {} successfully made immutable.", snapshot);
                }
                Command::new("sed").arg("-i")
                                   .arg("s|MUTABLE||g")
                                   .arg(format!("/.snapshots/ash/snapshots/{}-desc", snapshot))
                                   .status().unwrap();
            }
        }
    } else {
        eprintln!("Snapshot 0 (base) should not be modified.");
    }
}

// Install packages
pub fn install(snapshot: &str, pkg: &str) -> i32 {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot install as snapshot {} doesn't exist.", snapshot);
        return 1;
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() { // Make sure snapshot is not in use by another ash process
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snapshot,snapshot);
        return 1;
    } else if snapshot == "0" {
        eprintln!("Changing base snapshot is not allowed.");
        return 1;
    } else {
        let excode = install_package(snapshot, pkg);
        if excode == 0 {
            post_transactions(snapshot);
            println!("Package(s) {} installed in snapshot {} successfully.", pkg,snapshot);
            return 1;
        } else {
            chr_delete(snapshot);
            eprintln!("Install failed and changes discarded.");
            return 0;
        }
    }
}

// Install live
pub fn install_live(snapshot: &str, pkg: &str) {
    let tmp = get_tmp();
    Command::new("mount").arg("--bind")
                         .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                         .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                         .status().unwrap();
    Command::new("mount").arg("--bind")
                         .arg("/home")
                         .arg(format!("/.snapshots/rootfs/snapshot-{}/home", tmp))
                         .status().unwrap();
    Command::new("mount").arg("--bind")
                         .arg("/var")
                         .arg(format!("/.snapshots/rootfs/snapshot-{}/var",tmp))
                         .status().unwrap();
    Command::new("mount").arg("--bind")
                         .arg("/etc")
                         .arg(format!("/.snapshots/rootfs/snapshot-{}/etc", tmp))
                         .status().unwrap();
    Command::new("mount").arg("--bind")
                         .arg("/tmp")
                         .arg(format!("/.snapshots/rootfs/snapshot-{}/tmp", tmp))
                         .status().unwrap();
    ash_mounts(tmp, "").unwrap();
    println!("Please wait as installation is finishing.");
    let excode = install_package_live(snapshot, tmp, pkg);
    Command::new("umount").arg(format!("/.snapshots/rootfs/snapshot-{}/*", tmp)).status().unwrap();
    Command::new("umount").arg(format!("/.snapshots/rootfs/snapshot-{}", tmp)).status().unwrap();
    if excode.success() {
        println!("Package(s) {} live installed in snapshot {} successfully.", pkg,snapshot);
    } else {
        eprintln!("Live installation failed!");
    }
}

// Install a profile from a text file //REVIEW error handling
fn install_profile(snapshot: &str, profile: &str) -> i32 {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot install as snapshot {} doesn't exist.", snapshot);
        return 1;
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() { // Make sure snapshot is not in use by another ash process
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snapshot,snapshot);
        return 1;
    } else if snapshot == "0" {
        eprintln!("Changing base snapshot is not allowed.");
        return 1;
    } else {
        println!("Updating the system before installing profile {}.", profile);
        auto_upgrade(snapshot);
        let tmp_prof = String::from_utf8(Command::new("sh")
                                         .arg("-c")
                                         .arg("mktemp -d -p /tmp ashpk_profile.XXXXXXXXXXXXXXXX")
                                         .output().unwrap().stdout).unwrap().trim().to_string();
        Command::new("sh") //REVIEW change this?
        .arg("-c")
        .arg(format!("curl --fail -o {}/packages.txt -LO https://raw.githubusercontent.com/ashos/ashos/main/src/profiles/{}/packages{}.txt",
             tmp_prof,profile,get_distro_suffix(&detect::distro_id().as_str()))).status().unwrap();
        prepare(snapshot);
        loop { // Ignore empty lines or ones starting with # [ % &
            let pkg = String::from_utf8(Command::new("sh")
                                        .arg("-c")
                                        .arg(r"cat {tmp_prof}/packages.txt | grep -E -v '^#|^\[|^%|^&|^$'")
                                        .output().unwrap().stdout).unwrap().trim().replace("\n", " ").to_string();
            let excode1 = install_package(snapshot, pkg.as_str());
            let excode2 = service_enable(snapshot, profile, tmp_prof.as_str());
            if excode1 == 1 && excode2 == 1 {
                chr_delete(snapshot);
                println!("Install failed and changes discarded.");
                break 1;
            } else {
                post_transactions(snapshot);
                println!("Profile {} installed in snapshot {} successfully.", profile,snapshot);
                println!("Deploying snapshot {}.", snapshot);
                deploy(snapshot);
                break 0;
            }
        }
    }
}

// Install profile in live snapshot //REVIEW
fn install_profile_live(snapshot: &str,profile: &str) -> i32 {
    let tmp = get_tmp();
    ash_mounts(tmp, "").unwrap();
    println!("Updating the system before installing profile {}.", profile);
    auto_upgrade(tmp);
    let tmp_prof = String::from_utf8(Command::new("sh").arg("-c")
                                     .arg("mktemp -d -p /tmp ashpk_profile.XXXXXXXXXXXXXXXX")
                                     .output().unwrap().stdout).unwrap().trim().to_string();
    Command::new("sh").arg("-c") // REVIEW
                      .arg(format!("curl --fail -o {}/packages.txt -LO https://raw.githubusercontent.com/ashos/ashos/main/src/profiles/{}/packages{}.txt", tmp_prof,profile,get_distro_suffix(&detect::distro_id().as_str()))).status().unwrap();
    // Ignore empty lines or ones starting with # [ % &
    let pkg = String::from_utf8(Command::new("sh").arg("-c")
                                     .arg(r"cat {tmp_prof}/packages.txt | grep -E -v '^#|^\[|^%|^$'")
                                     .output().unwrap().stdout).unwrap().trim().replace("\n", " ").to_string();
    let excode1 = install_package_live(snapshot, tmp, pkg.as_str());
    let excode2 = service_enable(tmp, profile, tmp_prof.as_str());
    Command::new("umount").arg(format!("/.snapshots/rootfs/snapshot-{}/*", tmp)).status().unwrap();
    Command::new("umount").arg(format!("/.snapshots/rootfs/snapshot-{}", tmp)).status().unwrap();
    if excode1.success() && excode2 == 0 {
        println!("Profile {} installed in current/live snapshot.", profile);
        return 0;
    } else {
        println!("Install failed and changes discarded.");
        return 1;
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
    append_base_tree(i).unwrap();
    let excode = write_tree();
    if desc.is_empty() {
        write_desc(i.to_string().as_str(), "clone of base").unwrap();
    } else {
        write_desc(i.to_string().as_str(), desc).unwrap();
    }
    if excode.is_ok() {
        println!("New tree {} created.", i);
    }
}

// Post transaction function, copy from chroot dirs back to read only snapshot dir
pub fn post_transactions(snapshot: &str) {
    let tmp = get_tmp();
    // Some operations were moved below to fix hollow functionality ###
    //File operations in snapshot-chr
    Command::new("rm").arg("-rf")
                      .arg(format!("/.snapshots/boot/boot-chr{}/*", snapshot))
                      .status().unwrap();
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/boot/.", snapshot))
                      .arg(format!("/.snapshots/boot/boot-chr{}", snapshot))
                      .status().unwrap();
    Command::new("rm").arg("-rf")
                      .arg(format!("/.snapshots/etc/etc-chr{}/*", snapshot))
                      .status().unwrap();
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/etc/.", snapshot))
                      .arg(format!("/.snapshots/etc/etc-chr{}", snapshot))
                      .status().unwrap();
    // Keep package manager's cache after installing packages. This prevents unnecessary downloads for each snapshot when upgrading multiple snapshots
    cache_copy(snapshot);
    Command::new("btrfs").args(["sub", "del"])
                         .arg(format!("/.snapshots/boot/boot-{}", snapshot))
                         .status().unwrap();
    Command::new("btrfs").args(["sub", "del"])
                         .arg(format!("/.snapshots/etc/etc-{}", snapshot))
                         .status().unwrap();
    Command::new("btrfs").args(["sub", "del"])
                         .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                         .status().unwrap();
    if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/usr/share/ash/mutable", snapshot)).try_exists().unwrap() {
        Command::new("btrfs").args(["sub", "snap"])
                             .arg(format!("/.snapshots/boot/boot-chr{}", snapshot))
                             .arg(format!("/.snapshots/boot/boot-{}", snapshot)).status().unwrap();
        Command::new("btrfs").args(["sub", "snap"])
                             .arg(format!("/.snapshots/etc/etc-chr{}", snapshot))
                             .arg(format!("/.snapshots/etc/etc-{}", snapshot)).status().unwrap();
        // Copy init system files to shared
        init_system_copy(tmp, "post_transactions");
        Command::new("btrfs").args(["sub", "snap"])
                             .arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                             .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                             .status().unwrap();
    } else {
        let immutability = "-r";
        Command::new("btrfs").args(["sub", "snap"])
                             .arg(format!("{}", immutability))
                             .arg(format!("/.snapshots/boot/boot-chr{}", snapshot))
                             .arg(format!("/.snapshots/boot/boot-{}", snapshot)).status().unwrap();
        Command::new("btrfs").args(["sub", "snap"])
                             .arg(format!("{}", immutability))
                             .arg(format!("/.snapshots/etc/etc-chr{}", snapshot))
                             .arg(format!("/.snapshots/etc/etc-{}", snapshot)).status().unwrap();
        // Copy init system files to shared
        init_system_copy(tmp, "post_transactions");
        Command::new("btrfs").args(["sub", "snap"])
                             .arg(format!("{}", immutability))
                             .arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                             .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                             .status().unwrap();
    }
    // fix for hollow functionality
    // Unmount in reverse order
    Command::new("umount").arg(format!("/.snapshots/rootfs/snapshot-chr{}/etc/resolv.conf", snapshot))
                          .status().unwrap();
    Command::new("umount").arg("-R")
                          .arg(format!("/.snapshots/rootfs/snapshot-chr{}/dev", snapshot))
                          .status().unwrap();
    Command::new("umount").arg("-R")
                          .arg(format!("/.snapshots/rootfs/snapshot-chr{}/home", snapshot))
                          .status().unwrap();
    Command::new("umount").arg("-R")
                          .arg(format!("/.snapshots/rootfs/snapshot-chr{}/proc", snapshot))
                          .status().unwrap();
    Command::new("umount").arg("-R")
                          .arg(format!("/.snapshots/rootfs/snapshot-chr{}/root", snapshot))
                          .status().unwrap();
    Command::new("umount").arg("-R")
                          .arg(format!("/.snapshots/rootfs/snapshot-chr{}/run", snapshot))
                          .status().unwrap();
    Command::new("umount").arg("-R")
                          .arg(format!("/.snapshots/rootfs/snapshot-chr{}/sys", snapshot))
                          .status().unwrap();
    Command::new("umount").arg("-R")
                          .arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                          .status().unwrap();
    // Special mutable directories
    let options = snapshot_config_get(snapshot);
    let mutable_dirs: Vec<&str> = options.get("mutable_dirs")
                                         .map(|dirs| dirs.split(',').filter(|dir| !dir.is_empty()).collect())
                                         .unwrap_or_else(|| Vec::new());
    let mutable_dirs_shared: Vec<&str> = options.get("mutable_dirs_shared")
                                         .map(|dirs| dirs.split(',').filter(|dir| !dir.is_empty()).collect())
                                         .unwrap_or_else(|| Vec::new());
    if !mutable_dirs.is_empty() {
        for mount_path in mutable_dirs {
            Command::new("umount").arg("-R")
                                  .arg(format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path))
                                  .status().unwrap();
        }
    }
    if !mutable_dirs_shared.is_empty() {
        for mount_path in mutable_dirs_shared {
            Command::new("umount").arg("-R")
                                  .arg(format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path))
                                  .status().unwrap();
        }
    }
    // fix for hollow functionality
    chr_delete(snapshot);
}

// Prepare snapshot to chroot dir to install or chroot into
pub fn prepare(snapshot: &str) {
    chr_delete(snapshot);
    Command::new("btrfs").args(["sub", "snap"])
                         .arg(format!("/.snapshots/rootfs/snapshot-{}", snapshot))
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                         .status().unwrap();
    // Pacman gets weird when chroot directory is not a mountpoint, so the following mount is necessary
    Command::new("mount").args(["--bind", "--make-slave"])
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                         .status().unwrap();
    Command::new("mount").args(["--rbind", "--make-rslave", "/dev"])
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}/dev", snapshot))
                         .status().unwrap();
    Command::new("mount").args(["--bind", "--make-slave", "/home"])
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}/home", snapshot))
                         .status().unwrap();
    Command::new("mount").args(["--rbind", "--make-rslave", "/proc"])
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}/proc", snapshot))
                         .status().unwrap();
    Command::new("mount").args(["--bind", "--make-slave", "/root"])
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}/root", snapshot))
                         .status().unwrap();
    Command::new("mount").args(["--rbind", "--make-rslave", "/run"])
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}/run", snapshot))
                         .status().unwrap();
    Command::new("mount").args(["--rbind", "--make-rslave", "/sys"])
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}/sys", snapshot))
                         .status().unwrap();
    Command::new("mount").args(["--rbind", "--make-rslave", "/tmp"])
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}/tmp", snapshot))
                         .status().unwrap();
    Command::new("mount").args(["--bind", "--make-slave", "/var"])
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}/var", snapshot))
                         .status().unwrap();
    Command::new("mount").args(["--bind", "--make-slave"])
                         .arg("/etc/resolv.conf")
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}/etc/resolv.conf", snapshot))
                         .status().unwrap();
    // File operations for snapshot-chr
    Command::new("btrfs").args(["sub", "snap"])
                         .arg(format!("/.snapshots/boot/boot-{}", snapshot))
                         .arg(format!("/.snapshots/boot/boot-chr{}", snapshot))
                         .status().unwrap();
    Command::new("btrfs").args(["sub", "snap"])
                         .arg(format!("/.snapshots/etc/etc-{}", snapshot))
                         .arg(format!("/.snapshots/etc/etc-chr{}", snapshot))
                         .status().unwrap();
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/boot/boot-chr{}/.", snapshot))
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/boot", snapshot))
                      .status().unwrap();
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/etc/etc-chr{}/.", snapshot))
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/etc", snapshot)).status().unwrap();
    init_system_clean(snapshot, "prepare");
    Command::new("cp").arg("/etc/machine-id")
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/etc/machine-id", snapshot))
                      .status().unwrap();
    Command::new("mkdir").arg("-p")
                         .arg(format!("/.snapshots/rootfs/snapshot-chr{}/.snapshots/ash", snapshot)).status().unwrap();
    Command::new("cp").arg("-f")
                      .arg("/.snapshots/ash/fstree")
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/.snapshots/ash/", snapshot))
                      .status().unwrap();
    // Special mutable directories
    let options = snapshot_config_get(snapshot);
    let mutable_dirs: Vec<&str> = options.get("mutable_dirs")
                                         .map(|dirs| dirs.split(',').filter(|dir| !dir.is_empty()).collect())
                                         .unwrap_or_else(|| Vec::new());
    let mutable_dirs_shared: Vec<&str> = options.get("mutable_dirs_shared")
                                         .map(|dirs| dirs.split(',').filter(|dir| !dir.is_empty()).collect())
                                         .unwrap_or_else(|| Vec::new());
    if !mutable_dirs.is_empty() {
        for mount_path in mutable_dirs {
            Command::new("mkdir").arg("-p")
                                 .arg(format!("/.snapshots/mutable_dirs/snapshot-{}/{}", snapshot,mount_path))
                                 .status().unwrap();
            Command::new("mkdir").arg("-p")
                                 .arg(format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path))
                                 .status().unwrap();
            //Command::new("mount").arg("--bind")
                                 //.arg(format!("/.snapshots/mutable_dirs/snapshot-{}/{}", snapshot,mount_path))
                                 //.arg(format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path))
                                 //.status().unwrap(); //REVIEW
        }
    }
    if !mutable_dirs_shared.is_empty() {
        for mount_path in mutable_dirs_shared {
            Command::new("mkdir").arg("-p")
                                 .arg(format!("/.snapshots/mutable_dirs/{}", mount_path))
                                 .status().unwrap();
            Command::new("mkdir").arg("-p")
                                 .arg(format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path))
                                 .status().unwrap();
            //Command::new("mount").arg("--bind")
                                 //.arg(format!("/.snapshots/mutable_dirs/{}", mount_path))
                                 //.arg(format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path))
                                 //.status().unwrap(); //REVIEW
        }
    }
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
        prepare(snapshot);
        let excode = refresh_helper(snapshot);
        if excode.success() {
            post_transactions(snapshot);
            println!("Snapshot {} refreshed successfully.", snapshot);
        } else {
            chr_delete(snapshot);
            eprintln!("Refresh failed and changes discarded.")
        }
    }
}

// Recursively remove package in tree //REVIEW
pub fn remove_from_tree(treename: &str, pkg: &str, profile: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
        eprintln!("Cannot update as tree {} doesn't exist.", treename);
    } else {
        if !pkg.is_empty() {
            uninstall_package(treename, pkg);
            let mut order = recurse_tree(treename);
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
    clone_as_tree(tmp, ""); // REVIEW clone_as_tree(tmp, "rollback") will do.
    write_desc(i.to_string().as_str(), "rollback").unwrap();
    deploy(i.to_string().as_str());
}

// Recursively run an update in tree //REVIEW
pub fn run_tree(treename: &str, cmd: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
        eprintln!("Cannot update as tree {} doesn't exist.", treename);
    } else {
        prepare(treename);
        Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{} {}", treename,cmd)).status().unwrap();
        post_transactions(treename);
        let mut order = recurse_tree(treename);
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
                prepare(sarg.as_str());
                Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{} {}", sarg,cmd)).status().unwrap();
                post_transactions(sarg.as_str());
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
    print_tree();
}

// Read snap file
//pub fn snap() -> String {
    //let source_dep = get_tmp();
    //let sfile = File::open(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", source_dep)).unwrap();
    //let mut buf_read = BufReader::new(sfile);
    //let mut snap_value = String::new();
    //buf_read.read_line(&mut snap_value).unwrap();
    //let snap = snap_value.replace(" ", "").replace("\n", "");
    //snap
//}

// Edit per-snapshot configuration
pub fn snapshot_config_edit(snapshot: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot chroot as snapshot {} doesn't exist.", snapshot);
    } else if snapshot == "0" {
        eprintln!("Changing base snapshot is not allowed.")
    } else {
        prepare(snapshot);
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
            post_transactions(snapshot);
        }
    }
}

// Get per-snapshot configuration options
pub fn snapshot_config_get(snap: &str) -> HashMap<String, String> {
    let mut options = HashMap::new();

    if !Path::new(&format!("/.snapshots/etc/etc-{}/ash.conf", snap)).try_exists().unwrap() {
        // defaults here
        options.insert(String::from("aur"), String::from("False"));
        options.insert(String::from("mutable_dirs"), String::new());
        options.insert(String::from("mutable_dirs_shared"), String::new());
        return options;
    } else {
        let optfile = File::open(format!("/.snapshots/etc/etc-{}/ash.conf", snap)).unwrap();
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

// Show diff of packages between 2 snapshots
pub fn snapshot_diff(snap1: &str, snap2: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snap1)).try_exists().unwrap() {
        println!("Snapshot {} not found.", snap1);
    } else if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snap2)).try_exists().unwrap() {
        println!("Snapshot {} not found.", snap2);
    } else {
        Command::new("bash")
                .arg("-c")
                .arg(format!("diff <(ls /.snapshots/rootfs/snapshot-{}/usr/share/ash/db/local)\\
 <(ls /.snapshots/rootfs/snapshot-{}/usr/share/ash/db/local) | grep '^>\\|^<' | sort", snap1, snap2))
                .status().unwrap();
    }
}

// Remove temporary chroot for specified snapshot only
// This unlocks the snapshot for use by other functions
pub fn snapshot_unlock(snap: &str) {
    Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/boot/boot-chr{}", snap)).status().unwrap();
    Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/etc/etc-chr{}", snap)).status().unwrap();
    Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/rootfs/snapshot-chr{}", snap)).status().unwrap();
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

// Switch between /tmp deployments //REVIEW
pub fn switch_tmp() {
    let distro_suffix = get_distro_suffix(&detect::distro_id().as_str());
    let grub =  String::from_utf8(Command::new("sh").arg("-c")
                                  .arg("ls /boot | grep grub")
                                  .output().unwrap().stdout).unwrap().trim().to_string();
    let part = get_part();
    let tmp_boot = String::from_utf8(Command::new("sh").arg("-c")
                                     .arg("mktemp -d -p /.snapshots/tmp boot.XXXXXXXXXXXXXXXX")
                                     .output().unwrap().stdout).unwrap().trim().to_string();
    Command::new("sh").arg("-c").arg("mount").arg(format!("{} -o subvol=@boot{} {}", part,distro_suffix,tmp_boot)).status().unwrap(); // Mount boot partition for writing
    // Swap deployment subvolumes: deploy <-> deploy-aux
    let (source_dep, target_dep) = if get_tmp().contains("deploy-aux") {
        ("deploy-aux", "deploy")
    } else {
        ("deploy", "deploy-aux")
    };
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-{}/boot/.", target_dep))
                      .arg(format!("{}", tmp_boot)).status().unwrap();
    Command::new("sed")
        .arg("-i")
        .arg(format!("s|@.snapshots{}/rootfs/snapshot-{}|@.snapshots{}/rootfs/snapshot-{}|g", distro_suffix,source_dep,distro_suffix,target_dep))
        .arg(format!("{}/{}/grub.cfg", tmp_boot,grub)).status().unwrap(); // Overwrite grub config boot subvolume
    Command::new("sed")
        .arg("-i")
        .arg(format!("s|@.snapshots{}/rootfs/snapshot-{}|@.snapshots{}/rootfs/snapshot-{}|g", distro_suffix,source_dep,distro_suffix,target_dep))
        .arg(format!("/.snapshots/rootfs/snapshot-{}/boot/{}/grub.cfg", target_dep,grub)).status().unwrap();
    Command::new("sed")
        .arg("-i")
        .arg(format!("s|@.snapshots{}/boot/boot-{}|@.snapshots{}/boot/boot-{}|g",distro_suffix,source_dep,distro_suffix,target_dep))
        .arg(format!("/.snapshots/rootfs/snapshot-{}/etc/fstab", target_dep)).status().unwrap(); // Update fstab for new deployment
    Command::new("sed")
        .arg("-i")
        .arg(format!("s|@.snapshots{}/etc/etc-{}|@.snapshots{}/etc/etc-{}|g", distro_suffix,source_dep,distro_suffix,target_dep))
        .arg(format!("/.snapshots/rootfs/snapshot-{}/etc/fstab", target_dep)).status().unwrap();
    Command::new("sed")
        .arg("-i")
        .arg(format!("s|@.snapshots{}/rootfs/snapshot-{}|@.snapshots{}/rootfs/snapshot-{}|g", distro_suffix,source_dep,distro_suffix,target_dep))
        .arg(format!("/.snapshots/rootfs/snapshot-{}/etc/fstab", target_dep)).status().unwrap();
    let file = File::open(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", source_dep)).unwrap();
    let mut reader = BufReader::new(file);
    let mut sfile = String::new();
    reader.read_line(&mut sfile).unwrap();
    let snap = sfile.replace(" ", "").replace('\n', "");
    // Update GRUB configurations
    for boot_location in ["/.snapshots/rootfs/snapshot-deploy-aux/boot", &tmp_boot] {
        let file = File::open(format!("{}/{}/grub.cfg", boot_location,grub)).unwrap();
        let mut reader = BufReader::new(file);
        let mut grubconf = String::new();
        let line = reader.read_line(&mut grubconf).unwrap().to_string();
        let gconf = if line.contains("}") {
            "".to_owned() + &line
        } else {
            "".to_owned()
        };
        let gconf = if gconf.contains("snapshot-deploy-aux") {
            gconf.replace("snapshot-deploy-aux", "snapshot-deploy")
        } else {
            gconf.replace("snapshot-deploy", "snapshot-deploy-aux")
        };
        let gconf = if gconf.contains(&detect::distro_name()) {
            //gconf.replace(r"snapshot \d", "");
            gconf.replace(&detect::distro_name(), &format!("{} last booted deployment (snapshot {})", detect::distro_name(), snap))
        } else {
            gconf
        };
        Command::new("sh").arg("-c")
                          .arg(format!("sed -i '$ d' {}/{}/grub.cfg", boot_location,grub))
                          .status().unwrap();
        let mut grubconf_file = std::fs::OpenOptions::new()
            .append(true)
            .open(format!("{}/{}/grub.cfg", boot_location,grub)).unwrap();
        grubconf_file.write_all(gconf.as_bytes()).unwrap();
        grubconf_file.write_all(b"}\n").unwrap();
        grubconf_file.write_all(b"### END /etc/grub.d/41_custom ###").unwrap();
        drop(grubconf_file);
        Command::new("umount").arg(format!("{}", tmp_boot)).status().unwrap();
    };
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
        let mut order = recurse_tree(treename);
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
                prepare(snap_to_order);
                sync_tree_helper("chr", snap_from_order, snap_to_order).unwrap(); // Pre-sync
                if live && snap_to_order == get_current_snapshot().as_str() { // Live sync
                    sync_tree_helper("", snap_from_order, get_tmp()).unwrap(); // Post-sync
                    }
                post_transactions(snap_to_order); // Moved here from the line immediately after first sync_tree_helper
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
pub fn tmp_delete() {
    let tmp = get_tmp();
    if tmp.contains("deploy-aux") {
        let tmp = "deploy";
        Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/boot/boot-{}", tmp)).output().unwrap();
        Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/etc/etc-{}", tmp)).output().unwrap();
        Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/rootfs/snapshot-{}/*", tmp)).output().unwrap();
        Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/rootfs/snapshot-{}", tmp)).output().unwrap();
    } else {
        let tmp = "deploy-aux";
        Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/boot/boot-{}", tmp)).output().unwrap();
        Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/etc/etc-{}", tmp)).output().unwrap();
        Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/rootfs/snapshot-{}/*", tmp)).output().unwrap();
        Command::new("btrfs").args(["sub", "del"]).arg(format!("/.snapshots/rootfs/snapshot-{}", tmp)).output().unwrap();
    }
}

// Triage functions for argparse method //REVIEW
pub fn triage_install(snapshot: &str, live: bool, profile: &str, pkg: &str) {
    if !profile.is_empty() {
        install_profile(snapshot, profile);
    } else if !pkg.is_empty() {
        let package = pkg.to_string() + " ";
        install(snapshot, &package);
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
            install_live(snapshot, &package);
        }
    }
}
pub fn triage_uninstall(snapshot: &str, profile: &str, pkg: &str) { // TODO add live, not_live
    if !profile.is_empty() {
        //let excode = install_profile(snapshot, profile);
        println!("TODO");
    } else if !pkg.is_empty() {
        let package = pkg.to_string() + " ";
        uninstall_package(snapshot,  &package);
    }
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
        prepare(snapshot);
        let excode = uninstall_package_helper(snapshot, pkg);
        if excode.success() {
            post_transactions(snapshot);
            println!("Package {} removed from snapshot {} successfully.", pkg,snapshot);
        } else {
            chr_delete(snapshot);
            eprintln!("Remove failed and changes discarded.");
        }
    }
}

// Update boot
pub fn update_boot(snapshot: &str) {
    let grub =  String::from_utf8(Command::new("sh").arg("-c")
                                  .arg("ls /boot | grep grub")
                                  .output().unwrap().stdout).unwrap().trim().to_string();
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot update boot as snapshot {} doesn't exist.", snapshot)
    } else {
        let tmp = get_tmp();
        let part = get_part();
        prepare(snapshot);
        if Path::new(&format!("/boot/{}/BAK/", grub)).try_exists().unwrap() {
            Command::new("sh").arg("-c")
                              .arg("find")
                              .arg(format!(r"/boot/{}/BAK/. -mtime +30 -exec rm -rf' + ' {} \;", grub,"{}"))
                              .status().unwrap(); // Delete 30-day-old grub.cfg.DATE files
        }
        Command::new("cp").arg(format!("/boot/{}/grub.cfg", grub))
                          .arg(format!("/boot/{}/BAK/grub.cfg.`date '+%Y%m%d-%H%M%S'`", grub))
                          .status().unwrap();
        Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                              .arg("sh")
                              .arg("-c")
                              .arg(format!("{}-mkconfig {} -o /boot/{}/grub.cfg", grub,part,grub))
                              .status().unwrap();
        Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                              .arg("sh")
                              .arg("-c")
                              .arg(format!("sed -i 's|snapshot-chr{}|snapshot-{}|g' /boot/{}/grub.cfg", snapshot,tmp,grub))
                              .status().unwrap();
        Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot))
                              .arg("sh")
                              .arg("-c")
                              .arg(format!(r"sed -i '0,\|{}| s||{} snapshot {}|' /boot/{}/grub.cfg", detect::distro_name(),detect::distro_name(),snapshot,grub))
                              .status().unwrap();
        post_transactions(snapshot);
    }
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
        let mut order = recurse_tree(treename);
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
                auto_upgrade(snapshot);
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
            post_transactions(snapshot);
            println!("Snapshot {} upgraded successfully.", snapshot);
            return 0;
        } else {
            chr_delete(snapshot);
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
                                         .open(format!("/.snapshots/ash/snapshots/{}-desc", snapshot))
                                         .unwrap();
    descfile.write_all(desc.as_bytes()).unwrap();
    Ok(())
}
