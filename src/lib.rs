mod detect_distro;
mod distros;
mod tree;

use crate::detect_distro as detect;
use crate::distros::*;
use tree::*;
use std::collections::HashMap;
use std::fs::{File, OpenOptions, read_dir, read_to_string};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

// Ash version
//pub fn ash_version() {
    //let ash_bin_path = Path::new("/usr/sbin/ash");
    //let metadata = metadata(ash_bin_path).unwrap();
    //let time = metadata.modified().unwrap();
    //let duration = time.duration_since(UNIX_EPOCH).unwrap();
    //let utc = time::OffsetDateTime::UNIX_EPOCH +
        //time::Duration::try_from(duration).unwrap();
    //let local = utc.to_offset(time::UtcOffset::local_offset_at(utc).unwrap());
    //local.format_into(
        //&mut std::io::stdout().lock(),
        //time::macros::format_description!(
            //"[day]-[month repr:short]-[year] [hour]:[minute]:[second]\n"
        //),
    //).unwrap();
//}

// Ash chroot mounts
pub fn ash_chroot_mounts(i: &str) {
    let chr = "";
    Command::new("mount").arg("--bind")
                         .arg("--make-slave")
                         .arg(format!("/.snapshots/rootfs/snapshot-{}{}", chr,i))
                         .arg(format!("/.snapshots/rootfs/snapshot-{}{}", chr,i)).status().unwrap();
    Command::new("mount").args(["--rbind", "--make-rslave", "/dev"])
                         .arg(format!("/.snapshots/rootfs/snapshot-{}{}/dev", chr,i)).status().unwrap();
    Command::new("mount").args(["--bind", "--make-slave", "/etc"])
                         .arg(format!("/.snapshots/rootfs/snapshot-{}{}/etc", chr,i)).status().unwrap();
    Command::new("mount").args(["--bind", "--make-slave", "/home"])
                         .arg(format!("/.snapshots/rootfs/snapshot-{}{}/home", chr,i)).status().unwrap();
    Command::new("mount").args(["--types", "proc", "/proc"])
                         .arg(format!("/.snapshots/rootfs/snapshot-{}{}/proc", chr,i)).status().unwrap();
    Command::new("mount").args(["--bind", "--make-slave", "/run"])
                         .arg(format!("/.snapshots/rootfs/snapshot-{}{}/run", chr,i)).status().unwrap();
    Command::new("mount").args(["--rbind", "--make-rslave", "/sys"])
                         .arg(format!("/.snapshots/rootfs/snapshot-{}{}/sys", chr,i)).status().unwrap();
    Command::new("mount").args(["--bind", "--make-slave", "/tmp"])
                         .arg(format!("/.snapshots/rootfs/snapshot-{}{}/tmp", chr,i)).status().unwrap();
    Command::new("mount").args(["--bind", "--make-slave", "/var"])
                         .arg(format!("/.snapshots/rootfs/snapshot-{}{}/var", chr,i)).status().unwrap();
    if is_efi() {
        Command::new("mount").args(["--rbind", "--make-rslave", "/sys/firmware/efi/efivars"])
                             .arg(format!("/.snapshots/rootfs/snapshot-{}{}/sys/firmware/efi/efivars", chr,i)).status().unwrap();
        Command::new("cp").args(["--dereference", "/etc/resolv.conf"])
                          .arg(format!("/.snapshots/rootfs/snapshot-{}{}/etc/", chr,i)).status().unwrap();
        }
}

// Ash version
pub fn ash_version() {
    let version = String::from_utf8_lossy(&Command::new("date").arg("-r")
                                      .arg("/usr/sbin/ash")
                                      .arg("+%Y%m%d-%H%M%S")
                                      .output().unwrap().stdout).to_string();
    println!("{}", version);
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

/*#   Delete tree or branch
def delete_node(snapshots, quiet):
    for snapshot in snapshots:
        if not quiet: ### NEWLY ADDED
            print(f"Are you sure you want to delete snapshot {snapshot}? (y/n)")
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
            write_desc(snapshot, "") # Clear description
            os.system(f"btrfs sub del /.snapshots/boot/boot-{snapshot}{DEBUG}")
            os.system(f"btrfs sub del /.snapshots/etc/etc-{snapshot}{DEBUG}")
            os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{snapshot}{DEBUG}")
            # Make sure temporary chroot directories are deleted as well
            if (os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}")):
                os.system(f"btrfs sub del /.snapshots/boot/boot-chr{snapshot}{DEBUG}")
                os.system(f"btrfs sub del /.snapshots/etc/etc-chr{snapshot}{DEBUG}")
                os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{snapshot}{DEBUG}")
            for child in children: # This deletes the node itself along with its children
                write_desc(snapshot, "")
                os.system(f"btrfs sub del /.snapshots/boot/boot-{child}{DEBUG}")
                os.system(f"btrfs sub del /.snapshots/etc/etc-{child}{DEBUG}")
                os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-{child}{DEBUG}")
                if (os.path.exists(f"/.snapshots/rootfs/snapshot-chr{child}")):
                    os.system(f"btrfs sub del /.snapshots/boot/boot-chr{child}{DEBUG}")
                    os.system(f"btrfs sub del /.snapshots/etc/etc-chr{child}{DEBUG}")
                    os.system(f"btrfs sub del /.snapshots/rootfs/snapshot-chr{child}{DEBUG}")
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
        os.system(f"btrfs sub set-default /.snapshots/rootfs/snapshot-{tmp}{DEBUG}") # Set default volume
        tmp_delete()
        if "deploy-aux" in tmp:
            tmp = "deploy"
        else:
            tmp = "deploy-aux"
      # Special mutable directories
        options = snapshot_config_get(snapshot)
        mutable_dirs = options["mutable_dirs"].split(',').remove('')
        mutable_dirs_shared = options["mutable_dirs_shared"].split(',').remove('')
      # btrfs snapshot operations
        os.system(f"btrfs sub snap /.snapshots/boot/boot-{snapshot} /.snapshots/boot/boot-{tmp}{DEBUG}")
        os.system(f"btrfs sub snap /.snapshots/etc/etc-{snapshot} /.snapshots/etc/etc-{tmp}{DEBUG}")
        os.system(f"btrfs sub snap /.snapshots/rootfs/snapshot-{snapshot} /.snapshots/rootfs/snapshot-{tmp}{DEBUG}")
        os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{tmp}/boot{DEBUG}")
        os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{tmp}/etc{DEBUG}")
        os.system(f"rm -rf /.snapshots/rootfs/snapshot-{tmp}/var{DEBUG}")
        os.system(f"cp -r --reflink=auto /.snapshots/boot/boot-{snapshot}/. /.snapshots/rootfs/snapshot-{tmp}/boot{DEBUG}")
        os.system(f"cp -r --reflink=auto /.snapshots/etc/etc-{snapshot}/. /.snapshots/rootfs/snapshot-{tmp}/etc{DEBUG}")
      # If snapshot is mutable, modify '/' entry in fstab to read-write
        if check_mutability(snapshot):
            os.system(f"sed -i '0,/snapshot-{tmp}/ s|,ro||' /.snapshots/rootfs/snapshot-{tmp}/etc/fstab") ### ,rw
      # Add special user-defined mutable directories as bind-mounts into fstab
        if mutable_dirs:
            for mount_path in mutable_dirs:
                source_path = f"/.snapshots/mutable_dirs/snapshot-{snapshot}/{mount_path}"
                os.system(f"mkdir -p /.snapshots/mutable_dirs/snapshot-{snapshot}/{mount_path}")
                os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{tmp}/{mount_path}")
                os.system(f"echo '{source_path} /{mount_path} none defaults,bind 0 0' >> /.snapshots/rootfs/snapshot-{tmp}/etc/fstab")
      # Same thing but for shared directories
        if mutable_dirs_shared:
            for mount_path in mutable_dirs_shared:
                source_path = f"/.snapshots/mutable_dirs/{mount_path}"
                os.system(f"mkdir -p /.snapshots/mutable_dirs/{mount_path}")
                os.system(f"mkdir -p /.snapshots/rootfs/snapshot-{tmp}/{mount_path}")
                os.system(f"echo '{source_path} /{mount_path} none defaults,bind 0 0' >> /.snapshots/rootfs/snapshot-{tmp}/etc/fstab")
        os.system(f"btrfs sub snap /var /.snapshots/rootfs/snapshot-{tmp}/var{DEBUG}") ### Is this needed?
        os.system(f"echo '{snapshot}' > /.snapshots/rootfs/snapshot-{tmp}/usr/share/ash/snap")
        switch_tmp()
        init_system_clean(tmp, "deploy")
        os.system(f"btrfs sub set-default /.snapshots/rootfs/snapshot-{tmp}") # Set default volume
        print(f"Snapshot {snapshot} deployed to /.")*/

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

/*#   Make a snapshot vulnerable to be modified even further (snapshot should be deployed as mutable)
def hollow(s):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{s}"):
        print(f"F: Cannot make hollow as snapshot {s} doesn't exist.")
    elif os.path.exists(f"/.snapshots/rootfs/snapshot-chr{s}"): # Make sure snapshot is not in use by another ash process
        print(f"F: Snapshot {s} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {s}'.")
    elif s == "0":
        print("F: Changing base snapshot is not allowed.")
    else:
        ### AUR step might be needed and if so make a distro_specific function with steps similar to install_package(). Call it hollow_helper and change this accordingly().
        prepare(s)
        os.system(f"mount --rbind --make-rslave / /.snapshots/rootfs/snapshot-chr{s}")
        print(f"Snapshot {s} is now hollow! When done, type YES (in capital):")
        while True:
            reply = input("> ")
            if reply == "YES":
                post_transactions(s)
                #os.system(f"umount -R /.snapshots/rootfs/snapshot-chr{s}") OR os.system(f"umount -R /") ### REVIEW NEED to unmount this a second time?! (I BELIEVE NOT NEEDED)
                immutability_enable(s)
                deploy(s)
                print(f"Snapshot {s} hollow operation succeeded. Please reboot!")
                break*/

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
pub fn install(snapshot: &str, pkg: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot install as snapshot {} doesn't exist.", snapshot);
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() { // Make sure snapshot is not in use by another ash process
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {}'.", snapshot,snapshot);
    } else if snapshot == "0" {
        eprintln!("Changing base snapshot is not allowed.");
    } else {
        let excode = install_package(snapshot, pkg);
        if excode == 0 {
            post_transactions(snapshot);
            println!("Package(s) {} installed in snapshot {} successfully.", pkg,snapshot);
        } else {
            chr_delete(snapshot);
            eprintln!("Install failed and changes discarded.");
        }
    }
}

//#   Install live
//def install_live(snapshot, pkg):
    //tmp = get_tmp()
    //#options = get_persnap_options(tmp) ### moved this to install_package_live
    //os.system(f"mount --bind /.snapshots/rootfs/snapshot-{tmp} /.snapshots/rootfs/snapshot-{tmp}{DEBUG}")
    //os.system(f"mount --bind /home /.snapshots/rootfs/snapshot-{tmp}/home{DEBUG}")
    //os.system(f"mount --bind /var /.snapshots/rootfs/snapshot-{tmp}/var{DEBUG}")
    //os.system(f"mount --bind /etc /.snapshots/rootfs/snapshot-{tmp}/etc{DEBUG}")
    //os.system(f"mount --bind /tmp /.snapshots/rootfs/snapshot-{tmp}/tmp{DEBUG}")
    //ash_chroot_mounts(tmp) ### REVIEW Not having this was the culprit for live install to fail for Arch and derivative. Now, does having this here Ok or does it cause errors in NON-Arch distros? If so move it to ashpk.py
    //print("Please wait as installation is finishing.")
    //excode = install_package_live(snapshot, tmp, pkg)
    //os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/*{DEBUG}")
    //os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}{DEBUG}") ### REVIEW not safe
    //if not excode:
        //print(f"Package(s) {pkg} live installed in snapshot {snapshot} successfully.")
    //else:
        //print("F: Live installation failed!")

/*   Install a profile from a text file
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
        except subprocess.CalledProcessError:
            chr_delete(snapshot)
            print("F: Install failed and changes discarded.")
            sys.exit(1)
        else:
            post_transactions(snapshot)
            print(f"Profile {profile} installed in snapshot {snapshot} successfully.")
            print(f"Deploying snapshot {snapshot}.")
            deploy(snapshot)*/

//#   Install profile in live snapshot
//def install_profile_live(profile):
    //tmp = get_tmp()
    //ash_chroot_mounts(tmp)
    //print(f"Updating the system before installing profile {profile}.")
    //auto_upgrade(tmp)
    //tmp_prof = subprocess.check_output("mktemp -d -p /tmp ashpk_profile.XXXXXXXXXXXXXXXX", shell=True, encoding='utf-8').strip()
    //subprocess.check_output(f"curl --fail -o {tmp_prof}/packages.txt -LO https://raw.githubusercontent.com/ashos/ashos/main/src/profiles/{profile}/packages{get_distro_suffix()}.txt", shell=True)
  //# Ignore empty lines or ones starting with # [ % &
    //pkg = subprocess.check_output(f"cat {tmp_prof}/packages.txt | grep -E -v '^#|^\[|^%|^$'", shell=True).decode('utf-8').strip().replace('\n', ' ')
    //excode1 = install_package_live(tmp, pkg) ### REVIEW snapshot argument needed
    //excode2 = service_enable(tmp, profile, tmp_prof)
    //if excode1 == 0 and excode2 == 0:
        //print(f"Profile {profile} installed in current/live snapshot.") ### REVIEW
        //return 0
    //else:
        //print("F: Install failed and changes discarded.")
        //return 1
    //os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}/*{DEBUG}")
    //os.system(f"umount /.snapshots/rootfs/snapshot-{tmp}{DEBUG}")*/

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

/*#   Recursively remove package in tree
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
            print("TODO") ### REVIEW*/

/*#   Rollback last booted deployment
def rollback():
    tmp = get_tmp()
    i = find_new()
###    clone_as_tree(tmp)
    clone_as_tree(tmp, "") ### REVIEW clone_as_tree(tmp, "rollback") will do.
    write_desc(i, "rollback")
    deploy(i)*/

/*#   Recursively run an update in tree
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
        except subprocess.CalledProcessError:
            print(f"F: Failed to enable service(s) from {profile}.")
            return 1
        else:
            print(f"Installed service(s) from {profile}.")
            return 0*/

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

/*#   Switch between distros
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
                gconf = sub('snapshot \d', '', gconf)
                gconf = gconf.replace(f"{distro_name}", f"{distro_name} last booted deployment (snapshot {snap})")
        os.system(f"sed -i '$ d' {boot_location}/{GRUB}/grub.cfg")
        with open(f"{boot_location}/{GRUB}/grub.cfg", "a") as grubconf:
            grubconf.write(gconf)
            grubconf.write("}\n")
            grubconf.write("### END /etc/grub.d/41_custom ###")
    os.system(f"umount {tmp_boot}{DEBUG}")*/

// Sync time
pub fn sync_time() {
    Command::new("sh")
        .arg("-c")
        .arg("date -s \"$(curl --tlsv1.3 --proto =https -I https://google.com 2>&1 | grep Date: | cut -d\" \" -f3-6)Z\"")
        .status().unwrap();
}

/*   Sync tree and all its snapshots
def sync_tree(tree, treename, force_offline, live):
    if not os.path.exists(f"/.snapshots/rootfs/snapshot-{treename}"):
        print(f"F: Cannot sync as tree {treename} doesn't exist.")
    else:
        if not force_offline: # Syncing tree automatically updates it, unless 'force-sync' is used
            update_tree(tree, treename)
        order = recurse_tree(tree, treename)
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
                print(f"F: Snapshot {snap_to} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock {snap_to}'.")
                print("Tree sync canceled.")
                return
            else:
                prepare(snap_to)
                sync_tree_helper("chr", snap_from, snap_to) # Pre-sync
                if live and int(snap_to) == int(get_current_snapshot()): # Live sync
                    sync_tree_helper("", snap_from, get_tmp()) # Post-sync
                post_transactions(snap_to) ### Moved here from the line immediately after first sync_tree_helper
        print(f"Tree {treename} synced.")*/

//#   Sync tree helper function ### REVIEW might need to put it in distro-specific ashpk.py
//def sync_tree_helper(CHR, s_f, s_t):
    //os.system("mkdir -p /.snapshots/tmp-db/local/") ### REVIEW Still resembling Arch pacman folder structure!
    //os.system("rm -rf /.snapshots/tmp-db/local/*") ### REVIEW
    //pkg_list_to = pkg_list(CHR, s_t)
    //pkg_list_from = pkg_list("", s_f)
  //# Get packages to be inherited
    //pkg_list_from = [j for j in pkg_list_from if j not in pkg_list_to]
    //os.system(f"cp -r /.snapshots/rootfs/snapshot-{CHR}{s_t}/usr/share/ash/db/local/. /.snapshots/tmp-db/local/") ### REVIEW
    //os.system(f"cp -n -r --reflink=auto /.snapshots/rootfs/snapshot-{s_f}/. /.snapshots/rootfs/snapshot-{CHR}{s_t}/{DEBUG}")
    //os.system(f"rm -rf /.snapshots/rootfs/snapshot-{CHR}{s_t}/usr/share/ash/db/local/*") ### REVIEW
    //os.system(f"cp -r /.snapshots/tmp-db/local/. /.snapshots/rootfs/snapshot-{CHR}{s_t}/usr/share/ash/db/local/") ### REVIEW
    //for entry in pkg_list_from:
        //os.system(f"bash -c 'cp -r /.snapshots/rootfs/snapshot-{s_f}/usr/share/ash/db/local/{entry}-[0-9]* /.snapshots/rootfs/snapshot-{CHR}{s_t}/usr/share/ash/db/local/'") ### REVIEW
    //os.system("rm -rf /.snapshots/tmp-db/local/*") ### REVIEW (originally inside the loop, but I took it out

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

/*def triage_install(snapshot, live, profile, pkg, not_live):
    if profile:
        excode = install_profile(snapshot, profile)
    elif pkg:
        excode = install(snapshot, " ".join(pkg))
  # If installing into current snapshot and no not_live flag, use live install
    if snapshot == int(get_current_snapshot()) and not not_live:
        live = True
  # Perform the live install only if install above was successful
    if live and not excode:
        if profile:
            install_profile_live(profile)
        elif pkg:
            install_live(snapshot, " ".join(pkg))

def triage_uninstall(snapshot, profile, pkg, live, not_live): ### TODO add live, not_live
    if profile:
        #excode = install_profile(snapshot, profile)
        print("TODO")
    elif pkg:
        uninstall_package(snapshot, " ".join(pkg))*/

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

// Recursively run an update in tree
/*pub fn update_tree(treename: &str) {
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
        eprintln!("Cannot update as tree {} doesn't exist.", treename);
    } else {
        //upgrade(treename)
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
            }
            //auto_upgrade(sarg);
        }
        println!("Tree {} updated.", treename)
    }
}*/

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
