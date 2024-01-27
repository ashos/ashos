mod detect_distro;
mod distros;
mod tree;

use crate::detect_distro as detect;

use chrono::{NaiveDateTime, Local};
use configparser::ini::Ini;
use curl::easy::{Easy, HttpVersion, List, SslVersion};
use libbtrfsutil::{create_snapshot, CreateSnapshotFlags, delete_subvolume, DeleteSubvolumeFlags, set_subvolume_read_only};
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
    //TODO make feature conflict
    if #[cfg(feature = "arch")] {
        use distros::arch::ashpk::*;
    }
}

// Check if directory mutable
fn allow_dir_mut(mount_path: &str) -> bool {
    let mount_str = format!("/{}", mount_path);
    let path = Path::new(&mount_str);
    let not_allowed = vec!["bin", "dev", "lib", "lib64", "proc", "sbin", "usr", "usr/bin", "usr/lib", "usr/lib64", "usr/sbin"];
    if path.is_dir() && !not_allowed.contains(&mount_path.trim_end_matches("/")) && !mount_path.starts_with("/") {
        return true;
    } else {
        return false;
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
    if chr != "chr" {
        mount(Some("/etc"), format!("{}/etc", snapshot_path).as_str(),
              Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>)?;
    }
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
    if chr != "chr" {
    mount(Some("/var"), format!("{}/var", snapshot_path).as_str(),
          Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_SLAVE, None::<&str>)?;
    }

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
    if chr != "chr" {
        umount2(Path::new(&format!("{}/var", snapshot_path)),
                MntFlags::empty())?;
    }
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
    if chr != "chr" {
        umount2(Path::new(&format!("{}/etc", snapshot_path)),
                MntFlags::MNT_DETACH)?;
    }
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
        create_snapshot(format!("/.snapshots/var/var-{}", snapshot),
                        format!("/.snapshots/var/var-{}", i),
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

// Check if snapshot profile was changed
fn check_profile(snapshot: &str) -> Result<(), Error> {
    // Get values before edit
    let old_cfile = format!("/.snapshots/rootfs/snapshot-{}/etc/ash/profile", snapshot);
    let mut old_profconf = Ini::new();
    old_profconf.set_comment_symbols(&['#']);
    old_profconf.set_multiline(true);
    old_profconf.load(&old_cfile).unwrap();
    let mut old_system_pkgs: Vec<String> = Vec::new();
    if old_profconf.sections().contains(&"system-packages".to_string()) {
        for pkg in old_profconf.get_map().unwrap().get("system-packages").unwrap().keys() {
            old_system_pkgs.push(pkg.to_string());
        }
    }
    let mut old_pkgs: Vec<String> = Vec::new();
    if old_profconf.sections().contains(&"profile-packages".to_string()) {
        for pkg in old_profconf.get_map().unwrap().get("profile-packages").unwrap().keys() {
            old_pkgs.push(pkg.to_string());
        }
    }
    let mut old_enable: Vec<String> = Vec::new();
    if old_profconf.sections().contains(&"enable-services".to_string()) {
        for service in old_profconf.get_map().unwrap().get("enable-services").unwrap().keys() {
            old_enable.push(service.to_string());
        }
    }
    let mut old_disable: Vec<String> = Vec::new();
    if old_profconf.sections().contains(&"disable-services".to_string()) {
        for service in old_profconf.get_map().unwrap().get("disable-services").unwrap().keys() {
            old_disable.push(service.to_string());
        }
    }

    // Get new values
    let cfile = format!("/.snapshots/rootfs/snapshot-chr{}/etc/ash/profile", snapshot);
    let mut profconf = Ini::new();
    profconf.set_comment_symbols(&['#']);
    profconf.set_multiline(true);
    profconf.load(&cfile).unwrap();
    let mut new_system_pkgs: Vec<String> = Vec::new();
    if profconf.sections().contains(&"system-packages".to_string()) {
        for pkg in profconf.get_map().unwrap().get("system-packages").unwrap().keys() {
            new_system_pkgs.push(pkg.to_string());
        }
    }
    let mut new_pkgs: Vec<String> = Vec::new();
    if profconf.sections().contains(&"profile-packages".to_string()) {
        for pkg in profconf.get_map().unwrap().get("profile-packages").unwrap().keys() {
            new_pkgs.push(pkg.to_string());
        }
    }
    let mut new_enable: Vec<String> = Vec::new();
    if profconf.sections().contains(&"enable-services".to_string()) {
        for service in profconf.get_map().unwrap().get("enable-services").unwrap().keys() {
            new_enable.push(service.to_string());
        }
    }
    let mut new_disable: Vec<String> = Vec::new();
    if profconf.sections().contains(&"disable-services".to_string()) {
        for service in profconf.get_map().unwrap().get("disable-services").unwrap().keys() {
            new_disable.push(service.to_string());
        }
    }

    // Apply changes
    // Install new system package(s)
    let mut system_pkgs_to_install: Vec<String> = Vec::new();
    for pkg in &new_system_pkgs {
        if !old_system_pkgs.contains(&pkg) {
            system_pkgs_to_install.push(pkg.to_string());
        }
    }
    if !system_pkgs_to_install.is_empty() && !is_system_locked() {
        install_package_helper_chroot(snapshot, &system_pkgs_to_install, true)?;
        holdpkg(snapshot, &profconf)?;
    } else if !system_pkgs_to_install.is_empty() && is_system_locked() {
        // Prevent install new system package(s)
        return Err(Error::new(ErrorKind::Unsupported, format!("Install system package(s) is not allowed.")));
    }

    // Install new profile package(s)
    let mut pkgs_to_install: Vec<String> = Vec::new();
    for pkg in &new_pkgs {
        if !old_pkgs.contains(&pkg) {
            pkgs_to_install.push(pkg.to_string());
        }
    }
    if !pkgs_to_install.is_empty() {
        install_package_helper_chroot(snapshot, &pkgs_to_install, true)?;
    }

    // Disable removed service(s)
    let mut services_to_disable: Vec<String> = Vec::new();
    for service in &old_enable {
        if !new_enable.contains(&service) {
            services_to_disable.push(service.to_string());
        }
    }
    for service in new_disable {
        if !old_disable.contains(&service) {
            services_to_disable.push(service.to_string());
        }
    }
    if !services_to_disable.is_empty() {
        service_disable(snapshot, &services_to_disable, "chr")?;
    }

    // Enable new service(s)
    let mut services_to_enable: Vec<String> = Vec::new();
    for service in new_enable {
        if !old_enable.contains(&service) {
            services_to_enable.push(service.to_string());
        }
    }
    if !services_to_enable.is_empty() {
        service_enable(snapshot, &services_to_enable, "chr")?;
    }

    // Uninstall package(s) not in the new system-packages list
    let mut system_pkgs_to_uninstall: Vec<String> = Vec::new();
    for pkg in old_system_pkgs {
        if !new_system_pkgs.contains(&pkg) && !new_pkgs.contains(&pkg) {
            system_pkgs_to_uninstall.push(pkg.to_string());
        }
    }
    if !system_pkgs_to_uninstall.is_empty() && !is_system_locked() {
        uninstall_package_helper_chroot(snapshot, &system_pkgs_to_uninstall, true)?;
        holdpkg(snapshot, &profconf)?;
    } else if !system_pkgs_to_uninstall.is_empty() && is_system_locked() {
        // Prevent remove of system package from profile if not installed
        return Err(Error::new(ErrorKind::Unsupported, format!("Remove system package(s) is not allowed.")));
    }

    // Uninstall package(s) not in the new profile-packages list
    let mut pkgs_to_uninstall: Vec<String> = Vec::new();
    for pkg in old_pkgs {
        if !new_pkgs.contains(&pkg) && !new_system_pkgs.contains(&pkg) {
            pkgs_to_uninstall.push(pkg.to_string());
        }
    }
    if !pkgs_to_uninstall.is_empty() {
        uninstall_package_helper_chroot(snapshot, &pkgs_to_uninstall, true)?;
    }
    // Prevent duplication
    for pkg in new_system_pkgs {
        if new_pkgs.contains(&pkg) {
            profconf.remove_key("profile-packages", &pkg);
            profconf.write(&cfile)?;
        }
    }

    // Check pacman database
    let pkg_list = no_dep_pkg_list(snapshot, "chr");
    for pkg in &pkg_list {
        let mut pkgs: Vec<String> = Vec::new();
        if profconf.sections().contains(&"system-packages".to_string()) {
            for pkg in profconf.get_map().unwrap().get("system-packages").unwrap().keys() {
                // Remove package from profile if not installed and lock feature is not enabled
                if !pkg_list.contains(pkg) && !is_system_locked() {
                    profconf.remove_key("system-packages", pkg);
                } else if !pkg_list.contains(pkg) && is_system_locked() {
                    // Prevent remove of system package from profile if not installed
                    return Err(Error::new(ErrorKind::Unsupported, format!("Remove system package(s) is not allowed.")));
                }
            }
            profconf.write(&cfile)?;
        }
        if profconf.sections().contains(&"profile-packages".to_string()) {
            for pkg in profconf.get_map().unwrap().get("profile-packages").unwrap().keys() {
                // Remove package from profile if not installed
                if !pkg_list.contains(pkg) {
                    profconf.remove_key("profile-packages", pkg);
                }
                pkgs.push(pkg.to_string());
            }
            profconf.write(&cfile)?;
        }
        // Add package to profile if installed
        if !pkgs.contains(&pkg) {
            pkgs.push(pkg.to_string());
        }
        for key in pkgs {
            profconf.remove_key("profile-packages", &key);
            profconf.set("profile-packages", &key, None);
        }
        profconf.write(&cfile)?;
    }

    // Check services
    // Add service(s) enabled by systemctl
    if profconf.sections().contains(&"enable-services".to_string()) {
        for service in profconf.get_map().unwrap().get("enable-services").unwrap().keys() {
            if !is_service_enabled(snapshot, service) {
                profconf.remove_key("enable-services", &service);
            }
        }
        profconf.write(&cfile)?;
    }
    // Remove service(s) disabled by systemctl
    if profconf.sections().contains(&"disable-services".to_string()) {
        for service in profconf.get_map().unwrap().get("disable-services").unwrap().keys() {
            if is_service_enabled(snapshot, service) {
                profconf.remove_key("disable-services", &service);
            }
        }
        profconf.write(&cfile)?;
    }

    Ok(())
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
    // Path to var mount directory
    let var_path = format!("/.snapshots/var/var-chr{}", snapshot);
    // Path to snapshot mount directory
    let snapshot_path = format!("/.snapshots/rootfs/snapshot-chr{}", snapshot);

    // Delete boot,etc and snapshot subvolumes
    if Path::new(&snapshot_path).try_exists()? {
        delete_subvolume(&boot_path, DeleteSubvolumeFlags::empty()).unwrap();
        delete_subvolume(&etc_path, DeleteSubvolumeFlags::empty()).unwrap();
        delete_subvolume(&var_path, DeleteSubvolumeFlags::empty()).unwrap();
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
                    format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
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
                    if chroot_exec(&path, &cmd).is_ok() {
                        // Check for profile changes
                        if check_profile(snapshot).is_ok() {
                            // Make sure post_transactions exit properly
                            match post_transactions(snapshot) {
                                Ok(()) => {
                                    // Do nothing
                                }
                                Err(error) => {
                                    eprintln!("post_transactions error: {}", error);
                                    // Clean chroot mount directories if command failed
                                    chr_delete(snapshot)?;
                                }
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
                // Check for profile changes
                if check_profile(snapshot).is_ok() {
                    // Make sure post_transactions exit properly
                    match post_transactions(snapshot) {
                        Ok(()) => {
                            // Do nothig
                            }
                        Err(error) => {
                            eprintln!("post_transactions error: {}", error);
                            // Clean chroot mount directories if command failed
                            chr_delete(snapshot)?;

                        }
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
pub fn chroot_exec(path: &str,cmd: &str) -> Result<(), Error> {
    let exocde = Command::new("sh").arg("-c").arg(format!("chroot {} {}", path,cmd)).status()?;
    if !exocde.success() {
        return Err(
            Error::new(
                ErrorKind::Other,
                format!("Failed to run {}.", cmd)));
    }
    Ok(())
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
        create_snapshot(format!("/.snapshots/var/var-{}", snapshot),
                        format!("/.snapshots/var/var-{}", i),
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
        create_snapshot(format!("/.snapshots/var/var-{}", snapshot),
                        format!("/.snapshots/var/var-{}", i),
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
        create_snapshot(format!("/.snapshots/var/var-{}", snapshot),
                        format!("/.snapshots/var/var-{}", i),
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

        // Make sure snapshot does exist
        } else if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
            return Err(Error::new(ErrorKind::NotFound, format!(
                "Cannot delete as snapshot {} doesn't exist.", snapshot)));
        }

        if !nuke {
            // Make sure snapshot is not current working snapshot
            if snapshot == &current_snapshot {
                return Err(Error::new(ErrorKind::Unsupported, format!(
                    "Cannot delete booted snapshot.")));
            // Make sure snapshot is not deploy snapshot
            } else if snapshot == &next_snapshot { // REVIEW
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
            if Path::new(&desc_path).try_exists().unwrap() {
                std::fs::remove_file(desc_path)?;
            }
            if Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
                delete_subvolume(format!("/.snapshots/boot/boot-{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
                delete_subvolume(format!("/.snapshots/etc/etc-{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
                delete_subvolume(format!("/.snapshots/var/var-{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
                delete_subvolume(format!("/.snapshots/rootfs/snapshot-{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
            }

            // Make sure temporary chroot directories are deleted as well
            if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
                delete_subvolume(format!("/.snapshots/boot/boot-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
                delete_subvolume(format!("/.snapshots/etc/etc-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
                delete_subvolume(format!("/.snapshots/var/var-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
                delete_subvolume(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
            }

            for child in children {
                // This deletes the node itself along with its children
                let desc_path = format!("/.snapshots/ash/snapshots/{}-desc", child);
                if Path::new(&desc_path).try_exists().unwrap() {
                    std::fs::remove_file(desc_path)?;
                }
                if Path::new(&format!("/.snapshots/rootfs/snapshot-{}", child)).try_exists().unwrap() {
                    delete_subvolume(&format!("/.snapshots/boot/boot-{}", child), DeleteSubvolumeFlags::empty()).unwrap();
                    delete_subvolume(format!("/.snapshots/etc/etc-{}", child), DeleteSubvolumeFlags::empty()).unwrap();
                    delete_subvolume(format!("/.snapshots/var/var-{}", child), DeleteSubvolumeFlags::empty()).unwrap();
                    delete_subvolume(format!("/.snapshots/rootfs/snapshot-{}", child), DeleteSubvolumeFlags::empty()).unwrap();
                }
                if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", child)).try_exists().unwrap() {
                    delete_subvolume(format!("/.snapshots/boot/boot-chr{}", child), DeleteSubvolumeFlags::empty()).unwrap();
                    delete_subvolume(format!("/.snapshots/etc/etc-chr{}", child), DeleteSubvolumeFlags::empty()).unwrap();
                    delete_subvolume(format!("/.snapshots/var/var-chr{}", child), DeleteSubvolumeFlags::empty()).unwrap();
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
    for entry in WalkDir::new(bak_path) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.to_str().unwrap().contains("grub.cfg") {
            let file_ext = path.extension().unwrap().to_str().unwrap();
            let file_create_time = NaiveDateTime::parse_from_str(file_ext, "%Y%m%d-%H%M%S").unwrap();
            let cutoff_time = Local::now().naive_local().signed_duration_since(file_create_time);
            if cutoff_time.num_days() >= 30 {
                std::fs::remove_file(path)?;
            }
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
        create_snapshot(format!("/.snapshots/var/var-{}", snapshot),
                        format!("/.snapshots/var/var-{}", tmp),
                        CreateSnapshotFlags::empty(), None).unwrap();
        create_snapshot(format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                        format!("/.snapshots/rootfs/snapshot-{}", tmp),
                        CreateSnapshotFlags::empty(), None).unwrap();
        DirBuilder::new().recursive(true)
                         .create(format!("/.snapshots/rootfs/snapshot-{}/boot", tmp))?;
        DirBuilder::new().recursive(true)
                         .create(format!("/.snapshots/rootfs/snapshot-{}/etc", tmp))?;
        DirBuilder::new().recursive(true)
                         .create(format!("/.snapshots/rootfs/snapshot-{}/var", tmp))?;
        Command::new("cp").args(["-r", "--reflink=auto"])
                          .arg(format!("/.snapshots/boot/boot-{}/.", snapshot))
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/boot", tmp))
                          .output()?;
        Command::new("cp").args(["-r", "--reflink=auto"])
                          .arg(format!("/.snapshots/etc/etc-{}/.", snapshot))
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/etc", tmp))
                          .output()?;
        Command::new("cp").args(["-r", "--reflink=auto"])
                          .arg(format!("/.snapshots/var/var-{}/.", snapshot))
                          .arg(format!("/.snapshots/rootfs/snapshot-{}/var", tmp))
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

        let snap_num = format!("{}", snapshot);
        let mut snap_file = OpenOptions::new().truncate(true)
                                              .create(true)
                                              .read(true)
                                              .write(true)
                                              .open(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", tmp))?;
        snap_file.write_all(snap_num.as_bytes())?;
        switch_tmp(secondary, reset)?;

        // Set default volume
        Command::new("btrfs").args(["sub", "set-default"])
                             .arg(format!("/.snapshots/rootfs/snapshot-{}", tmp))
                             .output()?;
    }
    Ok(())
}

// Deploy recovery snapshot
fn deploy_recovery() -> Result<(), Error> {
    let tmp = get_recovery_tmp();

    // Update boot
    update_boot("0", false)?;

    // Special mutable directories
    let options = snapshot_config_get("0");
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

    // Change recovery tmp
    let tmp = get_recovery_aux_tmp(&tmp);

    // Clean tmp
    if Path::new(&format!("/.snapshots/rootfs/snapshot-{}", tmp)).try_exists().unwrap() {
        delete_subvolume(format!("/.snapshots/boot/boot-{}", tmp), DeleteSubvolumeFlags::RECURSIVE).unwrap();
        delete_subvolume(format!("/.snapshots/etc/etc-{}", tmp), DeleteSubvolumeFlags::RECURSIVE).unwrap();
        delete_subvolume(format!("/.snapshots/var/var-{}", tmp), DeleteSubvolumeFlags::RECURSIVE).unwrap();
        delete_subvolume(format!("/.snapshots/rootfs/snapshot-{}", tmp), DeleteSubvolumeFlags::RECURSIVE).unwrap();
    }

    // btrfs snapshot operations
    create_snapshot("/.snapshots/boot/boot-0",
                    format!("/.snapshots/boot/boot-{}", tmp),
                    CreateSnapshotFlags::empty(), None).unwrap();
    create_snapshot("/.snapshots/etc/etc-0",
                    format!("/.snapshots/etc/etc-{}", tmp),
                    CreateSnapshotFlags::empty(), None).unwrap();
    create_snapshot("/.snapshots/var/var-0",
                    format!("/.snapshots/var/var-{}", tmp),
                    CreateSnapshotFlags::empty(), None).unwrap();
    create_snapshot("/.snapshots/rootfs/snapshot-0",
                    format!("/.snapshots/rootfs/snapshot-{}", tmp),
                    CreateSnapshotFlags::empty(), None).unwrap();
    DirBuilder::new().recursive(true)
                     .create(format!("/.snapshots/rootfs/snapshot-{}/boot", tmp))?;
    DirBuilder::new().recursive(true)
                     .create(format!("/.snapshots/rootfs/snapshot-{}/etc", tmp))?;
    DirBuilder::new().recursive(true)
                     .create(format!("/.snapshots/rootfs/snapshot-{}/var", tmp))?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg("/.snapshots/boot/boot-0/.")
                      .arg(format!("/.snapshots/rootfs/snapshot-{}/boot", tmp))
                      .output()?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg("/.snapshots/etc/etc-0/.")
                      .arg(format!("/.snapshots/rootfs/snapshot-{}/etc", tmp))
                      .output()?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg("/.snapshots/var/var-0/.")
                      .arg(format!("/.snapshots/rootfs/snapshot-{}/var", tmp))
                      .output()?;

    // Update fstab for new deployment
    let fstab_file = format!("/.snapshots/rootfs/snapshot-{}/etc/fstab", tmp);
    // Read the contents of the file into a string
    let mut contents = String::new();
    let mut file = File::open(&fstab_file)?;
    file.read_to_string(&mut contents)?;

    let src_tmp = if contents.contains("deploy-aux") && !contents.contains("secondary") {
        "deploy-aux"
    } else if contents.contains("secondary") && !contents.contains("aux") {
        "deploy-secondary"
    } else if contents.contains("aux-secondary") {
        "deploy-aux-secondary"
    } else {
        "deploy"
    };

    let modified_boot_contents = contents.replace(&format!("@.snapshots_linux/boot/boot-{}", src_tmp),
                                                  &format!("@.snapshots_linux/boot/boot-{}", tmp));
    let modified_etc_contents = modified_boot_contents.replace(&format!("@.snapshots_linux/etc/etc-{}", src_tmp),
                                                               &format!("@.snapshots_linux/etc/etc-{}", tmp));
    let modified_var_contents = modified_etc_contents.replace(&format!("@.snapshots_linux/var/var-{}", src_tmp),
                                                               &format!("@.snapshots_linux/var/var-{}", tmp));
    let modified_rootfs_contents = modified_var_contents.replace(&format!("@.snapshots_linux/rootfs/snapshot-{}", src_tmp),
                                                                 &format!("@.snapshots_linux/rootfs/snapshot-{}", tmp));

    // Write the modified contents back to the file
    let mut file = File::create(fstab_file)?;
    file.write_all(modified_rootfs_contents.as_bytes())?;

    // If snapshot is mutable, modify '/' entry in fstab to read-write
    if check_mutability("0") {
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
            let source_path = format!("/.snapshots/mutable_dirs/snapshot-0/{}", mount_path);
            DirBuilder::new().recursive(true)
                             .create(format!("/.snapshots/mutable_dirs/snapshot-0/{}", mount_path))?;
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

    let snap_num = "0";
    let mut snap_file = OpenOptions::new().truncate(true)
                                          .create(true)
                                          .read(true)
                                          .write(true)
                                          .open(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/snap", tmp))?;
    snap_file.write_all(snap_num.as_bytes())?;

    // Update recovery tmp
    switch_recovery_tmp()?;
    prepare("0")?;
    let mut recovery_tmp = OpenOptions::new().truncate(true)
                                             .create(true)
                                             .read(true)
                                             .write(true)
                                             .open("/.snapshots/rootfs/snapshot-chr0/usr/share/ash/rec-tmp")?;
    recovery_tmp.write_all(tmp.as_bytes())?;
    post_transactions("0")?;

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
    let vars = read_dir("/.snapshots/var")
        .unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
    let mut snapshots = read_dir("/.snapshots/rootfs")
        .unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
    snapshots.append(&mut vars.clone());
    snapshots.append(&mut etcs.clone());
    snapshots.append(&mut boots.clone());

    loop {
        i += 1;
        if !snapshots.contains
            (&PathBuf::from(format!("/.snapshots/rootfs/snapshot-{}", i))) && !snapshots
            .contains
            (&PathBuf::from(format!("/.snapshots/var/var-{}", i))) && !snapshots
            .contains
            (&PathBuf::from(format!("/.snapshots/etc/etc-{}", i))) && !snapshots
            .contains
            (&PathBuf::from(format!("/.snapshots/boot/boot-{}", i))) {
                break i;
        }
    }
}

// FixDB
pub fn fixdb(snapshot: &str) -> Result<(), Error> {
    fix_package_db(snapshot)?;
    Ok(())
}

// Get aux tmp
pub fn get_aux_tmp(tmp: String, secondary: bool) -> String {
    let tmp = if secondary {
        if tmp == "deploy-aux-secondary" {
            tmp.replace("deploy-aux-secondary", "deploy-secondary")
        } else if tmp == "deploy-aux" {
            tmp.replace("deploy-aux", "deploy-aux-secondary")
        } else if tmp == "deploy" {
            tmp.replace("deploy", "deploy-secondary")
        } else {
            tmp.replace("deploy-secondary", "deploy-aux-secondary")
        }
    } else {
        if tmp == "deploy-aux" {
            tmp.replace("deploy-aux", "deploy")
        } else if tmp == "deploy-aux-secondary" {
            tmp.replace("deploy-aux-secondary", "deploy-aux")
        } else if tmp == "deploy-secondary" {
            tmp.replace("deploy-secondary", "deploy")
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

// Get deployed snapshot // REVIEW
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

// Get recovery aux tmp
fn get_recovery_aux_tmp(tmp: &str) -> String {
    let tmp = if tmp == "recovery-deploy-aux" {
        tmp.replace("recovery-deploy-aux", "recovery-deploy")
    } else {
        tmp.replace("recovery-deploy", "recovery-deploy-aux")
    };
    tmp
}

// Get recovery tmp state
fn get_recovery_tmp() -> String {
    if Path::new("/.snapshots/rootfs/snapshot-0/usr/share/ash/rec-tmp").try_exists().unwrap() {
        let mut file = File::open("/.snapshots/rootfs/snapshot-0/usr/share/ash/rec-tmp").unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        let tmp = contents.trim().to_string();
        return tmp;
    } else {
        return "recovery-deploy".to_string();
    }
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
                format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
                        snapshot,snapshot)));

        // Make sure snapshot is not  base snapshot
        } else if snapshot == "0" {
        return Err(Error::new(ErrorKind::Unsupported, format!("Changing base snapshot is not allowed.")));

    } else {
        prepare(snapshot)?;
        // Mount root
        mount(Some("/"), format!("/.snapshots/rootfs/snapshot-chr{}", snapshot).as_str(),
              Some("btrfs"), MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_SLAVE, None::<&str>)?;
        // Deploy or not
        if yes_no(&format!("Snapshot {} is now hollow! Whenever done, type yes to deploy and no to discard.", snapshot)) {
            posttrans(snapshot)?;
            if check_mutability(snapshot) {
                immutability_enable(snapshot)?;
            }
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
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
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
    ash_umounts(&tmp, "").unwrap();
    Ok(())
}

// Install a profile from a text file
fn install_profile(snapshot: &str, profile: &str, force: bool, secondary: bool,
                   user_profile: &str, noconfirm: bool) -> Result<bool, Error> {
    // Get some values
    let distro = detect::distro_id();
    let dist_name = if distro.contains("_ashos") {
        distro.replace("_ashos", "")
    } else {
        distro
    };
    let cfile = format!("/usr/share/ash/profiles/{}/{}", profile,dist_name);

    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot install as snapshot {} doesn't exist.", snapshot)));

        // Make sure snapshot is not in use by another ash process
        } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        return Err(
            Error::new(ErrorKind::Unsupported,
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
                               snapshot,snapshot)));

        // Make sure snapshot is not base snapshot
        } else if snapshot == "0" {
        return Err(Error::new(ErrorKind::Unsupported,
                              format!("Changing base snapshot is not allowed.")));
    } else {
        // Install profile
        println!("Updating the system before installing {} profile...", profile);
        // Prepare
        auto_upgrade(snapshot)?;
        prepare(snapshot)?;

        // Profile configurations
        let mut profconf = Ini::new();
        profconf.set_comment_symbols(&['#']);
        profconf.set_multiline(true);
        // Load profile if exist
        if Path::new(&cfile).try_exists().unwrap() && !force && user_profile.is_empty() {
            profconf.load(&cfile).unwrap();
        } else if force {
            println!("Installing AshOS profiles...");
            install_package_helper(snapshot, &vec!["ash-profiles".to_string()], true)?;
            profconf.load(&cfile).unwrap();
        } else if !user_profile.is_empty() {
            profconf.load(user_profile).unwrap();
        } else if !Path::new(&cfile).try_exists().unwrap() && !force {
            return Err(Error::new(ErrorKind::NotFound,
                                  format!("Please install ash-profiles package.")));
        }

        // Read presets section in configuration file
        if profconf.sections().contains(&"presets".to_string()) {
            if !aur_check(snapshot) {
                return Err(Error::new(ErrorKind::Unsupported,
                                      format!("Please enable AUR.")));
            }
        }

        // Read packages section in configuration file
        if profconf.sections().contains(&"profile-packages".to_string()) {
            let mut pkgs: Vec<String> = Vec::new();
            for pkg in profconf.get_map().unwrap().get("profile-packages").unwrap().keys() {
                pkgs.push(pkg.to_string());
            }
            // Install package(s)
            install_package_helper(snapshot, &pkgs, noconfirm)?;
        }

        // Read disable services section in configuration file
        if profconf.sections().contains(&"disable-services".to_string()) {
            let mut services: Vec<String> = Vec::new();
            for service in profconf.get_map().unwrap().get("disable-services").unwrap().keys() {
                services.push(service.to_string());
            }
            // Disable service(s)
            service_disable(snapshot, &services, "chr")?;
        }

        // Read enable services section in configuration file
        if profconf.sections().contains(&"enable-services".to_string()) {
            let mut services: Vec<String> = Vec::new();
            for service in profconf.get_map().unwrap().get("enable-services").unwrap().keys() {
                services.push(service.to_string());
            }
            // Enable service(s)
            service_enable(snapshot, &services, "chr")?;
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
    let distro = detect::distro_id();
    let dist_name = if distro.contains("_ashos") {
        distro.replace("_ashos", "")
    } else {
        distro
    };
    let cfile = format!("/usr/share/ash/profiles/{}/{}", profile,dist_name);
    let tmp = get_tmp();

    // Prepare
    if user_profile.is_empty() {
        println!("Updating the system before installing {} profile...", profile);
    } else {
        println!("Updating the system before installing {} profile...", user_profile);
    }
    // Mount tmp
    ash_mounts(&tmp, "")?;
    // Upgrade
    if upgrade_helper_live(&tmp, noconfirm).is_ok() {

        // Profile configurations
        let mut profconf = Ini::new();
        profconf.set_comment_symbols(&['#']);
        profconf.set_multiline(true);

        // Load profile if exist
        if Path::new(&cfile).try_exists().unwrap() && !force && user_profile.is_empty() {
            profconf.load(&cfile).unwrap();
        } else if force {
            println!("Installing AshOS profiles...");
            install_package_helper_live(snapshot, &tmp, &vec!["ash-profiles".to_string()], true)?;
            profconf.load(&cfile).unwrap();
        } else if !user_profile.is_empty() {
            profconf.load(user_profile).unwrap();
        } else if !Path::new(&cfile).try_exists().unwrap() && !force {
            return Err(Error::new(ErrorKind::NotFound,
                                  format!("Please install ash-profiles package.")));
        }

        // Read presets section in configuration file
        if profconf.sections().contains(&"presets".to_string()) {
            if !aur_check(snapshot) {
                return Err(Error::new(ErrorKind::Unsupported,
                                      format!("Please enable AUR.")));
            }
        }

        // Read packages section in configuration file
        if profconf.sections().contains(&"profile-packages".to_string()) {
            let mut pkgs: Vec<String> = Vec::new();
            for pkg in profconf.get_map().unwrap().get("profile-packages").unwrap().keys() {
                pkgs.push(pkg.to_string());
            }
            // Install package(s)
            install_package_helper_live(snapshot, &tmp, &pkgs, noconfirm)?;
        }

        // Read disable services section in configuration file
        if profconf.sections().contains(&"disable-services".to_string()) {
            let mut services: Vec<String> = Vec::new();
            for service in profconf.get_map().unwrap().get("disable-services").unwrap().keys() {
                services.push(service.to_string());
            }
            // Disable service(s)
            service_disable(snapshot, &services, "chr")?;
        }

        // Read enable services section in configuration file
        if profconf.sections().contains(&"enable-services".to_string()) {
            let mut services: Vec<String> = Vec::new();
            for service in profconf.get_map().unwrap().get("enable-services").unwrap().keys() {
                services.push(service.to_string());
            }
            // Enable service(s)
            service_enable(snapshot, &services, "chr")?;
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

    // Umount tmp
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
            let excode = install_profile(snapshot, profile, force, secondary, user_profile, noconfirm);
            match excode {
                Ok(secondary) => {
                    if post_transactions(snapshot).is_ok() {
                        println!("Profile {} installed in snapshot {} successfully.", p,snapshot);
                        if yes_no(
                            &format!
                                ("Would you like to proceed with the deployment of snapshot {}?", snapshot)) {
                            if deploy(snapshot, secondary, false).is_ok() {
                                println!("Snapshot {} deployed to '/'.", snapshot);
                            }
                        }
                    } else {
                        chr_delete(snapshot)?;
                        eprintln!("Install failed and changes discarded!");
                    }
                },
                Err(e) => {
                    eprintln!("{}",e);
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
            let excode = install_profile(snapshot, profile, force, secondary, user_profile, noconfirm);
            match excode {
                Ok(secondary) => {
                    if post_transactions(snapshot).is_ok() {
                        println!("Profile {} installed in snapshot {} successfully.", p,snapshot);
                        if yes_no(&format!("Would you like to proceed with the deployment of snapshot {}?", snapshot)) {
                            if deploy(snapshot, secondary, false).is_ok() {
                                println!("Snapshot {} deployed to '/'.", snapshot);
                            }
                        }
                    } else {
                        chr_delete(snapshot)?;
                        eprintln!("Install failed and changes discarded!");
                    }
                },
                Err(e) => {
                    eprintln!("{}",e);
                    chr_delete(snapshot)?;
                    eprintln!("Install failed and changes discarded!");
                },
            }
        }

    } else if live && snapshot != get_current_snapshot() {
        // Prevent live option if snapshot is not current snapshot
        eprintln!("Can't use the live option with any other snapshot than the current one.");

    // Do live install only if: live flag is used OR target snapshot is current
    } else if live && snapshot == get_current_snapshot() {
        if !profile.is_empty() {
            // Live profile installation
            let excode = install_profile_live(snapshot, profile, force, user_profile, noconfirm);
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
            let excode = install_profile_live(snapshot, profile, force, user_profile, noconfirm);
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

// Check if system packages is locked
fn is_system_locked() -> bool {
    cfg_if::cfg_if! {
        if #[cfg(feature = "lock")] {
            return true;
        } else {
            return false;
        }
    }
}

// Check if package in "system-packages" list
fn is_system_pkg(profconf: &Ini, pkg: String) -> bool {
    let mut pkg_list: Vec<String> = Vec::new();
    if profconf.sections().contains(&"system-packages".to_string()) {
        for system_pkg in profconf.get_map().unwrap().get("system-packages").unwrap().keys() {
            pkg_list.push(system_pkg.to_string());
        }
    }
    if pkg_list.contains(&pkg) {
        return true;
    } else {
        return false;
    }
}

// Package list
pub fn list(snapshot: &str, chr: &str, exclude: bool) -> Vec<String> {
    let list = if exclude {
        no_dep_pkg_list(snapshot, chr)
    } else {
        pkg_list(snapshot, chr)
    };
    list
}

// List sub-volumes for the booted distro only
pub fn list_subvolumes() {
    let args = "btrfs sub list / | grep -i _linux | sort -f -k 9";
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
    cache_copy(snapshot, false)?;
    // Clean cache for base snapshot
    if snapshot == "0" {
        remove_dir_content(&format!("/.snapshots/rootfs/snapshot-chr{}/var/cache/pacman/pkg", snapshot))?;
    }
    remove_dir_content(&format!("/.snapshots/var/var-chr{}", snapshot))?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/.", snapshot))
                      .arg(format!("/.snapshots/var/var-chr{}", snapshot))
                      .output()?;

    // Delete old snapshot
    delete_subvolume(Path::new(&format!("/.snapshots/boot/boot-{}", snapshot)),
                     DeleteSubvolumeFlags::empty()).unwrap();
    delete_subvolume(Path::new(&format!("/.snapshots/etc/etc-{}", snapshot)),
                     DeleteSubvolumeFlags::empty()).unwrap();
    delete_subvolume(Path::new(&format!("/.snapshots/var/var-{}", snapshot)),
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
        create_snapshot(format!("/.snapshots/var/var-chr{}", snapshot),
                        format!("/.snapshots/var/var-{}", snapshot),
                        CreateSnapshotFlags::empty(), None).unwrap();
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
        create_snapshot(format!("/.snapshots/var/var-chr{}", snapshot),
                        format!("/.snapshots/var/var-{}", snapshot),
                        CreateSnapshotFlags::READ_ONLY, None).unwrap();
        create_snapshot(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot),
                        format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                        CreateSnapshotFlags::READ_ONLY, None).unwrap();
    }

    // Unmount in reverse order
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
            if !allow_dir_mut(mount_path) {
                return Err(Error::new(ErrorKind::InvalidInput,
                                      format!("Please insert valid value for mutable_dirs in /.snapshots/etc/etc-{}/ash/ash.conf", snapshot)));
            }
            if is_mounted(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path))) {
                umount2(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path)),
                        MntFlags::MNT_DETACH).unwrap();
            }
        }
    }
    if !mutable_dirs_shared.is_empty() {
        for mount_path in mutable_dirs_shared {
            if !allow_dir_mut(mount_path) {
                return Err(Error::new(ErrorKind::InvalidInput,
                                      format!("Please insert valid value for mutable_dirs_shared in /.snapshots/etc/etc-{}/ash/ash.conf", snapshot)));
            }
            if is_mounted(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path))) {
                umount2(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snapshot,mount_path)),
                        MntFlags::MNT_DETACH).unwrap();
            }
        }
    }

    // Clean chroot
    chr_delete(snapshot)?;

    Ok(())
}

// Hollow  Post transaction function
pub fn posttrans(snapshot: &str) -> Result<(), Error> {
    delete_subvolume(Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)),
                     DeleteSubvolumeFlags::empty()).unwrap();
    remove_dir_content(&format!("/.snapshots/etc/etc-chr{}", snapshot))?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/etc/*", snapshot))
                      .arg(format!("/.snapshots/etc/etc-chr{}", snapshot))
                      .output()?;
    remove_dir_content(&format!("/.snapshots/var/var-chr{}", snapshot))?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/var/*", snapshot))
                      .arg(format!("/.snapshots/var/var-chr{}", snapshot))
                      .output()?;
    remove_dir_content(&format!("/.snapshots/boot/boot-chr{}",  snapshot))?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-chr{}/boot/*", snapshot))
                      .arg(format!("/.snapshots/boot/boot-chr{}", snapshot))
                      .output()?;
    delete_subvolume(Path::new(&format!("/.snapshots/etc/etc-{}", snapshot)),
                     DeleteSubvolumeFlags::empty()).unwrap();
    delete_subvolume(Path::new(&format!("/.snapshots/var/var-{}", snapshot)),
                     DeleteSubvolumeFlags::empty()).unwrap();
    delete_subvolume(Path::new(&format!("/.snapshots/boot/boot-{}", snapshot)),
                     DeleteSubvolumeFlags::empty()).unwrap();
    create_snapshot(format!("/.snapshots/etc/etc-chr{}", snapshot),
                    format!("/.snapshots/etc/etc-{}", snapshot),
                    CreateSnapshotFlags::READ_ONLY, None).unwrap();
    create_snapshot(format!("/.snapshots/var/var-chr{}", snapshot),
                    format!("/.snapshots/var/var-{}", snapshot),
                    CreateSnapshotFlags::READ_ONLY, None).unwrap();
    create_snapshot(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot),
                    format!("/.snapshots/rootfs/snapshot-{}", snapshot),
                    CreateSnapshotFlags::READ_ONLY, None).unwrap();
    create_snapshot(format!("/.snapshots/boot/boot-chr{}", snapshot),
                    format!("/.snapshots/boot/boot-{}", snapshot),
                    CreateSnapshotFlags::READ_ONLY, None).unwrap();
    umount2(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/.snapshots/rootfs/snapshot-chr{}/", snapshot,snapshot)),
            MntFlags::MNT_DETACH).unwrap();
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
        // Clean old mutable_dirs
        if Path::new("/.snapshots/mutable_dirs/snapshot-{}").try_exists().unwrap() {
            remove_dir_content("/.snapshots/mutable_dirs/snapshot-{}")?;
        }
        for mount_path in mutable_dirs {
            if allow_dir_mut(mount_path) {
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
    }
    if !mutable_dirs_shared.is_empty() {
        // Clean old mutable_dirs_shared
        if Path::new("/.snapshots/mutable_dirs/").try_exists().unwrap() {
            remove_dir_content("/.snapshots/mutable_dirs/")?;
        }
        for mount_path in mutable_dirs_shared {
            if allow_dir_mut(mount_path) {
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
    }

    // File operations for snapshot-chr
    create_snapshot(format!("/.snapshots/boot/boot-{}", snapshot),
                    format!("/.snapshots/boot/boot-chr{}", snapshot),
                    CreateSnapshotFlags::empty(), None).unwrap();
    create_snapshot(format!("/.snapshots/etc/etc-{}", snapshot),
                    format!("/.snapshots/etc/etc-chr{}", snapshot),
                    CreateSnapshotFlags::empty(), None).unwrap();
    create_snapshot(format!("/.snapshots/var/var-{}", snapshot),
                    format!("/.snapshots/var/var-chr{}", snapshot),
                    CreateSnapshotFlags::empty(), None).unwrap();
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/boot/boot-chr{}/.", snapshot))
                      .arg(format!("{}/boot", snapshot_chr))
                      .output()?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/etc/etc-chr{}/.", snapshot))
                      .arg(format!("{}/etc", snapshot_chr))
                      .output()?;
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/var/var-chr{}/.", snapshot))
                      .arg(format!("{}/var", snapshot_chr))
                      .output()?;

    // Copy ash related configurations
    if Path::new("/etc/systemd").try_exists().unwrap() {
        // Machine-id is a Systemd thing
        copy("/etc/machine-id", format!("{}/etc/machine-id", snapshot_chr))?;
    }
    DirBuilder::new().recursive(true)
                     .create(format!("{}/.snapshots/ash", snapshot_chr))?;
    copy("/.snapshots/ash/fstree", format!("{}/.snapshots/ash/fstree", snapshot_chr))?;

    // Keep package manager's cache after installing packages
    // This prevents unnecessary downloads for each snapshot when upgrading multiple snapshots
    if snapshot != "0" {
        cache_copy(snapshot, true)?;
    }

    Ok(())
}

// Show tmp partition state
pub fn print_tmp() -> String {
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
    if mount.iter().any(|element| element.contains("recovery-deploy-aux")) {
        let r = String::from("recovery-deploy-aux");
        return r;
     } else if mount.iter().any(|element| element.contains("recovery-deploy")) {
        let r = String::from("recovery-deploy");
        return r;
     } else if mount.iter().any(|element| element.contains("deploy-aux-secondary")) {
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

// Rebuild snapshot
pub fn rebuild(snapshot: &str, desc: &str) -> Result<i32, Error> {
    let snap_num = find_new();
    // Make sure snapshot does exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound, format!("Cannot rebuild as snapshot {} doesn't exist.", snapshot)));

    // Make sure snapshot is not in use by another ash process
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(
            ErrorKind::NotFound,
            format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
                    snapshot,snapshot)));

    // Make sure snapshot is not base snapshot
    } else if snapshot == "0" {
        return Err(Error::new(ErrorKind::NotFound, "Changing base snapshot is not allowed."));

    } else {
        // Make snapshot mutable or immutable
        let immutability: CreateSnapshotFlags = if check_mutability(snapshot) {
            CreateSnapshotFlags::empty()
        } else {
            CreateSnapshotFlags::READ_ONLY
        };

        // prepare
        rebuild_prep(snapshot)?;

        // Create snapshot
        create_snapshot(format!("/.snapshots/rootfs/snapshot-chr{}", snapshot),
                        format!("/.snapshots/rootfs/snapshot-chr{}", snap_num),
                        immutability, None).unwrap();
        create_snapshot(format!("/.snapshots/boot/boot-chr{}", snapshot),
                        format!("/.snapshots/boot/boot-chr{}", snap_num),
                        immutability, None).unwrap();
        create_snapshot(format!("/.snapshots/etc/etc-chr{}", snapshot),
                        format!("/.snapshots/etc/etc-chr{}", snap_num),
                        immutability, None).unwrap();
        // Keep package manager's cache after installing packages
        // This prevents unnecessary downloads for each snapshot when upgrading multiple snapshots
        cache_copy(snapshot, false)?;
        create_snapshot(format!("/.snapshots/var/var-chr{}", snapshot),
                        format!("/.snapshots/var/var-chr{}", snap_num),
                        immutability, None).unwrap();

        // Unmount in reverse order
        ash_umounts(snapshot, "chr")?;

        // Delete old snapshot chroot
        chr_delete(snapshot)?;

        // Create mutable or immutable snapshot
        // Mutable
        if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/usr/share/ash/mutable", snap_num)).try_exists().unwrap() {
            create_snapshot(format!("/.snapshots/boot/boot-chr{}", snap_num),
                            format!("/.snapshots/boot/boot-{}", snap_num),
                            CreateSnapshotFlags::empty(), None).unwrap();
            create_snapshot(format!("/.snapshots/etc/etc-chr{}", snap_num),
                            format!("/.snapshots/etc/etc-{}", snap_num),
                            CreateSnapshotFlags::empty(), None).unwrap();
            create_snapshot(format!("/.snapshots/var/var-chr{}", snap_num),
                            format!("/.snapshots/var/var-{}", snap_num),
                            CreateSnapshotFlags::empty(), None).unwrap();
            create_snapshot(format!("/.snapshots/rootfs/snapshot-chr{}", snap_num),
                            format!("/.snapshots/rootfs/snapshot-{}", snap_num),
                            CreateSnapshotFlags::empty(), None).unwrap();
        } else {
            // Immutable
            create_snapshot(format!("/.snapshots/boot/boot-chr{}", snap_num),
                            format!("/.snapshots/boot/boot-{}", snap_num),
                            CreateSnapshotFlags::READ_ONLY, None).unwrap();
            create_snapshot(format!("/.snapshots/etc/etc-chr{}", snap_num),
                            format!("/.snapshots/etc/etc-{}", snap_num),
                            CreateSnapshotFlags::READ_ONLY, None).unwrap();
            create_snapshot(format!("/.snapshots/var/var-chr{}", snap_num),
                            format!("/.snapshots/var/var-{}", snap_num),
                            CreateSnapshotFlags::READ_ONLY, None).unwrap();
            create_snapshot(format!("/.snapshots/rootfs/snapshot-chr{}", snap_num),
                            format!("/.snapshots/rootfs/snapshot-{}", snap_num),
                            CreateSnapshotFlags::READ_ONLY, None).unwrap();
        }

        // Special mutable directories
        let options = snapshot_config_get(&format!("{}", snap_num));
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
                if !allow_dir_mut(mount_path) {
                    return Err(Error::new(ErrorKind::InvalidInput,
                                          format!("Please insert valid value for mutable_dirs in /.snapshots/etc/etc-{}/ash/ash.conf", snapshot)));
                }
                if is_mounted(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snap_num,mount_path))) {
                    umount2(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snap_num,mount_path)),
                            MntFlags::MNT_DETACH).unwrap();
                }
            }
        }
        if !mutable_dirs_shared.is_empty() {
            for mount_path in mutable_dirs_shared {
                if !allow_dir_mut(mount_path) {
                    return Err(Error::new(ErrorKind::InvalidInput,
                                          format!("Please insert valid value for mutable_dirs_shared in /.snapshots/etc/etc-{}/ash/ash.conf", snapshot)));
                }
                if is_mounted(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snap_num,mount_path))) {
                    umount2(Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/{}", snap_num,mount_path)),
                            MntFlags::MNT_DETACH).unwrap();
                }
            }
        }

        // Clean chroot
        chr_delete(&format!("{}", snap_num))?;

        // Mark newly created snapshot as mutable
        if immutability ==  CreateSnapshotFlags::empty() {
            File::create(format!("/.snapshots/rootfs/snapshot-{}/usr/share/ash/mutable", snap_num))?;
        }

        // Import tree file
        let tree = fstree().unwrap();
        // Add to root tree
        append_base_tree(&tree, snap_num).unwrap();
        // Save tree to fstree
        write_tree(&tree)?;
        // Write description for snapshot
        if desc.is_empty() {
            let description = format!("rebuild of {}.", snapshot);
            write_desc(&snap_num.to_string(), &description, true)?;
        } else {
            write_desc(&snap_num.to_string(), &desc, true)?;
        }
    }
    Ok(snap_num)
}

// Rebuild base snapshot
pub fn rebuild_base() -> Result<(), Error> {
    let snapshot = "0";
    if rebuild_prep(snapshot).is_ok() {
        post_transactions(snapshot)?;
    }
    Ok(())
}

// Prepare rebuild process
pub fn rebuild_prep(snapshot: &str) -> Result<(), Error> {
    //Profile configurations
    let cfile = format!("/.snapshots/rootfs/snapshot-{}/etc/ash/profile", snapshot);
    let mut profconf = Ini::new();
    profconf.set_comment_symbols(&['#']);
    profconf.set_multiline(true);
    // Load profile
    profconf.load(&cfile).unwrap();

    // Remove packages
    prepare(snapshot)?;
    clean_chroot(snapshot)?;

    // Reinstall base
    pacstrap(snapshot)?;
    system_config(snapshot, &profconf)?;

    Ok(())
}

// Refresh snapshot
pub fn refresh(snapshot: &str) -> Result<(), Error> {
    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot refresh as snapshot {} doesn't exist.", snapshot);

        // Make sure snapshot is not in use by another ash process
        } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
                  snapshot,snapshot);

        // Make sure snapshot is not base snapshot
        } else if snapshot == "0" {
        eprintln!("Changing base snapshot is not allowed.");

    } else {
        sync_time()?;
        prepare(snapshot)?;
        let excode = refresh_helper(snapshot);
        if excode.is_ok() {
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

// System reset
pub fn reset() -> Result<(), Error> {
    let current_snapshot = get_current_snapshot();
    let msg = "All snapshots will be permanently deleted and cannot be retrieved, are you absolutely certain you want to continue?";
    let reset_msg = "The system will restart automatically to complete the reset. Do you want to continue?";
    if yes_no(msg) && yes_no(reset_msg) {
        // Collect snapshots
        let tree = fstree().unwrap();
        let snapshots = return_children(&tree, "root");

        // Prepare rc.local
        prepare("0")?;
        copy("/.snapshots/rootfs/snapshot-chr0/etc/rc.local", "/.snapshots/rootfs/snapshot-chr0/etc/rc.local.bak")?;
        let start = "#!/bin/sh";
        let del_snap = format!("/usr/sbin/ash del -q -n -s {}", current_snapshot);
        let cp_rc = "cp /etc/rc.local.bak /etc/rc.local";
        let mut file = OpenOptions::new().truncate(true)
                                         .read(true)
                                         .write(true)
                                         .open("/.snapshots/rootfs/snapshot-chr0/etc/rc.local")?;
        let new_content = format!("{}\n{}\n{}\nexit 0", start,del_snap,cp_rc);
        file.write_all(new_content.as_bytes())?;
        post_transactions("0")?;

        // Deploy the base snapshot and remove all the other snapshots
        if deploy("0", false, true).is_ok() {
            // Revers rc.local
            prepare("0")?;
            copy("/.snapshots/rootfs/snapshot-chr0/etc/rc.local.bak", "/.snapshots/rootfs/snapshot-chr0/etc/rc.local")?;
            post_transactions("0")?;
            for snapshot in snapshots {
                // Delete snapshot if exist
                if Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap()
                && snapshot.to_string() != current_snapshot && snapshot.to_string() != "0" {
                    delete_node(&vec![snapshot.to_string()], true, true)?;
                }
            }
        } else {
            return Err(Error::new(ErrorKind::Other,
                                  "Failed to deploy base snapshot."));
        }
    } else {
        return Err(Error::new(ErrorKind::Interrupted,
                              "Aborted."));
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
    create_snapshot("/.snapshots/var/var-0",
                    format!("/.snapshots/var/var-{}", i),
                    CreateSnapshotFlags::READ_ONLY, None).unwrap();
    create_snapshot("/.snapshots/rootfs/snapshot-0",
                    format!("/.snapshots/rootfs/snapshot-{}", i),
                    CreateSnapshotFlags::READ_ONLY, None).unwrap();

    // Import tree file
    let tree = fstree().unwrap();

    // Add to root tree
    append_base_tree(&tree, i).unwrap();
    // Save tree to fstree
    write_tree(&tree)?;
    // Write description
    if desc.is_empty() {
        write_desc(&i.to_string(), "clone of base.", true).unwrap();
    } else {
        write_desc(&i.to_string(), desc, true).unwrap();
    }
    Ok(i)
}

// Edit per-snapshot configuration
pub fn snapshot_config_edit(snapshot: &str) -> Result<(), Error> {
    // Make sure snapshot exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot chroot as snapshot {} doesn't exist.", snapshot);
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        // Make sure snapshot is not in use by another ash process
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.", snapshot,snapshot)

    } else if snapshot == "0" {
        // Make sure is not base snapshot
        eprintln!("Changing base snapshot is not allowed.")

    } else {
        // Edit ash config
        prepare(snapshot)?;
        if std::env::var("EDITOR").is_ok() {
        Command::new("sh").arg("-c")
                          .arg(format!("$EDITOR /.snapshots/rootfs/snapshot-chr{}/etc/ash/ash.conf", snapshot))
                          .status()?;
            } else {
            // nano available
            if Command::new("sh").arg("-c")
                                 .arg("[ -x \"$(command -v nano)\" ]")
                                 .status().unwrap().success() {
                                     Command::new("sh").arg("-c")
                                                       .arg(format!("nano /.snapshots/rootfs/snapshot-chr{}/etc/ash/ash.conf", snapshot))
                                                       .status()?;
                                 }
            // vi available
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v vi)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("vi /.snapshots/rootfs/snapshot-chr{}/etc/ash/ash.conf", snapshot))
                                                            .status()?;
                                      }
            // vim available
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v vim)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("vim /.snapshots/rootfs/snapshot-chr{}/etc/ash/ash.conf", snapshot))
                                                            .status()?;
                                      }
            // neovim
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v nvim)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("nvim /.snapshots/rootfs/snapshot-chr{}/etc/ash/ash.conf", snapshot))
                                                            .status()?;
                                      }
            // micro
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v micro)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("micro /.snapshots/rootfs/snapshot-chr{}/etc/ash/ash.conf", snapshot))
                                                            .status()?;
                                      }
            else {
                eprintln!("No text editor available!");
            }
            post_transactions(snapshot)?;
        }
    }
    Ok(())
}

// Get per-snapshot configuration options
pub fn snapshot_config_get(snapshot: &str) -> HashMap<String, String> {
    let mut options = HashMap::new();

    if !Path::new(&format!("/.snapshots/etc/etc-{}/ash/ash.conf", snapshot)).try_exists().unwrap() {
        // Defaults here
        options.insert(String::from("aur"), String::from("False"));
        options.insert(String::from("mutable_dirs"), String::new());
        options.insert(String::from("mutable_dirs_shared"), String::new());
        return options;
    } else {
        let optfile = File::open(format!("/.snapshots/etc/etc-{}/ash/ash.conf", snapshot)).unwrap();
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

// Edit per-snapshot profile
pub fn snapshot_profile_edit(snapshot: &str) -> Result<(), Error> {
    // Make sure snapshot exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        eprintln!("Cannot chroot as snapshot {} doesn't exist.", snapshot);
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        // Make sure snapshot is not in use by another ash process
        eprintln!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.", snapshot,snapshot)

    } else if snapshot == "0" {
        // Make sure is not base snapshot
        eprintln!("Changing base snapshot is not allowed.")

    } else {
        // Edit profile
        prepare(snapshot)?;

        // Launch editor
        if std::env::var("EDITOR").is_ok() {
        Command::new("sh").arg("-c")
                          .arg(format!("$EDITOR /.snapshots/rootfs/snapshot-chr{}/etc/ash/profile", snapshot))
                          .status()?;
            } else {
            // nano available
            if Command::new("sh").arg("-c")
                                 .arg("[ -x \"$(command -v nano)\" ]")
                                 .status().unwrap().success() {
                                     Command::new("sh").arg("-c")
                                                       .arg(format!("nano /.snapshots/rootfs/snapshot-chr{}/etc/ash/profile", snapshot))
                                                       .status()?;
                                 }
            // vi available
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v vi)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("vi /.snapshots/rootfs/snapshot-chr{}/etc/ash/profile", snapshot))
                                                            .status()?;
                                      }
            // vim available
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v vim)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("vim /.snapshots/rootfs/snapshot-chr{}/etc/ash/profile", snapshot))
                                                            .status()?;
                                      }
            // neovim
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v nvim)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("nvim /.snapshots/rootfs/snapshot-chr{}/etc/ash/profile", snapshot))
                                                            .status()?;
                                      }
            // micro
            else if Command::new("sh").arg("-c")
                                      .arg("[ -x \"$(command -v micro)\" ]")
                                      .status().unwrap().success() {
                                          Command::new("sh").arg("-c")
                                                            .arg(format!("micro /.snapshots/rootfs/snapshot-chr{}/etc/ash/profile", snapshot))
                                                            .status()?;
                                      }
            else {
                return Err(Error::new(ErrorKind::NotFound,
                                      "No text editor available!"));
            }
        }
        if check_profile(snapshot).is_err() {
            return Err(Error::new(ErrorKind::Other,
                                  "Failed to apply changes."));
        }
    }
    Ok(())
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
            delete_subvolume(&format!("/.snapshots/var/var-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
        } else {
            umount2(Path::new(path),
                    MntFlags::MNT_DETACH)?;
            delete_subvolume(&format!("/.snapshots/boot/boot-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(&format!("/.snapshots/etc/etc-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(&format!("/.snapshots/var/var-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
            delete_subvolume(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot), DeleteSubvolumeFlags::empty()).unwrap();
        }
    }
    Ok(())
}

// Switch between distros //REVIEW
pub fn efi_boot_order() -> Result<(), Error>{
    /*loop {
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
    }*/
    println!("This feature is currently under construction.");
    Ok(())
}

// Switch between /recovery-tmp deployments
fn switch_recovery_tmp() -> Result<(), Error> {
    let grub = get_grub().unwrap();
    let part = get_part();
    let tmp_boot = TempDir::new_in("/.snapshots/tmp")?;

    // Mount boot partition for writing
    mount(Some(part.as_str()), tmp_boot.path().as_os_str(),
          Some("btrfs"), MsFlags::empty(), Some("subvol=@boot_linux".as_bytes()))?;

    // Swap deployment subvolumes: deploy <-> deploy-aux
    let source_dep = get_recovery_tmp();
    let target_dep = get_recovery_aux_tmp(&source_dep);
    let boot_location = tmp_boot.path().to_str().unwrap();

    // Read the contents of the file into a string
    let grub_cfg = format!("{}/{}/grub.cfg", boot_location, grub);
    let src_file_path = format!("/.snapshots/rootfs/snapshot-{}/boot/{}/grub.cfg", source_dep,grub);
    let sfile = if Path::new(&src_file_path).try_exists().unwrap() {
        File::open(&src_file_path)?
    } else {
        File::open(format!("/.snapshots/rootfs/snapshot-{}/boot/{}/grub.cfg", target_dep,grub))?
    };
    let reader = BufReader::new(sfile);
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

    // Remove old recovery
    let gfile = File::open(&grub_cfg)?;
    let reader = BufReader::new(gfile);
    let mut ngrub_cfg = String::new();
    let mut recovery_mode = false;
    for line in reader.lines() {
        let line = line?;
        if line.contains("menuentry 'Recovery Mode'") {
            recovery_mode = true;
        } else if recovery_mode {
            if line.contains("}") {
                ngrub_cfg.retain(|s| s.to_string() != line);
                break;
            } else {
                ngrub_cfg.retain(|s| s.to_string() != line);
            }
        } else {
            ngrub_cfg.push_str(&format!("\n{}",&line));
        }
    }
    let mut gfile = File::create(&grub_cfg)?;
    gfile.write_all(ngrub_cfg.as_bytes())?;

    // Remove END of 41_custom
    let mut contents = String::new();
    let mut sfile = File::open(&grub_cfg)?;
    sfile.read_to_string(&mut contents)?;
    let modified_grub_contents = contents.replace("### END /etc/grub.d/41_custom ###", "");
    let mut nfile = File::create(&grub_cfg)?;
    nfile.write_all(modified_grub_contents.as_bytes())?;

    // Change recovery tmp
    let first_quote_index = gconf.find('\'').unwrap_or(0);
    let second_quote_index = gconf[first_quote_index + 1..].find('\'').unwrap_or(gconf.len()) + first_quote_index + 1;
    let updated_line = gconf.replace(&gconf[first_quote_index + 1..second_quote_index], "Recovery Mode");
    let modified_cfg_contents = if updated_line.contains(&source_dep) {
        updated_line.replace(&format!("@.snapshots_linux/rootfs/snapshot-{}", source_dep),
                             &format!("@.snapshots_linux/rootfs/snapshot-{}", target_dep))
    } else {
        let src_tmp = if updated_line.contains("deploy-aux") && !updated_line.contains("secondary") {
            "deploy-aux"
        } else if updated_line.contains("secondary") && !updated_line.contains("aux") {
            "deploy-secondary"
        } else if updated_line.contains("aux-secondary") {
            "deploy-aux-secondary"
        } else {
            "deploy"
        };

        updated_line.replace(&format!("@.snapshots_linux/rootfs/snapshot-{}", src_tmp),
                             &format!("@.snapshots_linux/rootfs/snapshot-{}", target_dep))
    };

    // Write the modified contents back to the file
    let mut file = OpenOptions::new().append(true)
                                     .create(true)
                                     .read(true)
                                     .open(grub_cfg)?;
    file.write_all(modified_cfg_contents.as_bytes())?;

    // Umount boot partition
    umount2(Path::new(&format!("{}", tmp_boot.path().as_os_str().to_str().unwrap())),
            MntFlags::MNT_DETACH)?;

    Ok(())
}

// Switch between /tmp deployments
pub fn switch_tmp(secondary: bool, reset: bool) -> Result<(), Error> {
    let grub = get_grub().unwrap();
    let part = get_part();
    let rec_tmp = get_recovery_tmp();
    let tmp_boot = TempDir::new_in("/.snapshots/tmp")?;

    // Mount boot partition for writing
    mount(Some(part.as_str()), tmp_boot.path().as_os_str(),
          Some("btrfs"), MsFlags::empty(), Some("subvol=@boot_linux".as_bytes()))?;

    // Swap deployment subvolumes: deploy <-> deploy-aux
    let source_dep = get_tmp();
    let target_dep = get_aux_tmp(source_dep.to_string(), secondary);
    Command::new("cp").args(["-r", "--reflink=auto"])
                      .arg(format!("/.snapshots/rootfs/snapshot-{}/boot/grub", target_dep))
                      .arg(format!("{}", tmp_boot.path().to_str().unwrap()))
                      .output()?;

    // Update fstab for new deployment
    let fstab_file = format!("/.snapshots/rootfs/snapshot-{}/etc/fstab", target_dep);
    // Read the contents of the file into a string
    let mut contents = String::new();
    let mut file = File::open(&fstab_file)?;
    file.read_to_string(&mut contents)?;
    let modified_boot_contents = contents.replace(&format!("@.snapshots_linux/boot/boot-{}", source_dep),
                                                  &format!("@.snapshots_linux/boot/boot-{}", target_dep));
    let modified_etc_contents = modified_boot_contents.replace(&format!("@.snapshots_linux/etc/etc-{}", source_dep),
                                                               &format!("@.snapshots_linux/etc/etc-{}", target_dep));
    let modified_var_contents = modified_etc_contents.replace(&format!("@.snapshots_linux/var/var-{}", source_dep),
                                                               &format!("@.snapshots_linux/var/var-{}", target_dep));
    let modified_rootfs_contents = modified_var_contents.replace(&format!("@.snapshots_linux/rootfs/snapshot-{}", source_dep),
                                                                 &format!("@.snapshots_linux/rootfs/snapshot-{}", target_dep));
    // Write the modified contents back to the file
    let mut file = File::create(fstab_file)?;
    file.write_all(modified_rootfs_contents.as_bytes())?;

    // Recovery GRUB configurations
    if !reset {
        for boot_location in [&tmp_boot.path().to_str().unwrap()] {
            // Get old grub configurations
            let grub_path = format!("{}/{}/grub.cfg", boot_location, grub);
            let src_file_path = format!("/.snapshots/rootfs/snapshot-{}/boot/{}/grub.cfg", source_dep,grub);
            let sfile = File::open(&src_file_path)?;
            let reader = BufReader::new(sfile);
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

            // Remove END of 41_custom
            let mut contents = String::new();
            let mut sfile = File::open(&grub_path)?;
            sfile.read_to_string(&mut contents)?;
            let modified_grub_contents = contents.replace("### END /etc/grub.d/41_custom ###", "");
            let mut nfile = File::create(&grub_path)?;
            nfile.write_all(modified_grub_contents.as_bytes())?;

            // Open the file in read and write mode
            let mut file = OpenOptions::new().read(true).write(true).append(true).open(&grub_path)?;

            // Write the modified content back to the file
            file.write_all(format!("\n\n{}", &gconf).as_bytes())?;

            // Add recovery mode
            if Path::new(&format!("/.snapshots/rootfs/snapshot-{}", rec_tmp)).try_exists().unwrap() {
                // Remove END of 41_custom
                let mut contents = String::new();
                let mut sfile = File::open(&grub_path)?;
                sfile.read_to_string(&mut contents)?;
                let modified_grub_contents = contents.replace("### END /etc/grub.d/41_custom ###", "");
                let mut nfile = File::create(&grub_path)?;
                nfile.write_all(modified_grub_contents.as_bytes())?;

                let first_quote_index = gconf.find('\'').unwrap_or(0);
                let second_quote_index = gconf[first_quote_index + 1..].find('\'').unwrap_or(gconf.len()) + first_quote_index + 1;
                let updated_line = gconf.replace(&gconf[first_quote_index + 1..second_quote_index], "Recovery Mode");

                // Change recovery tmp
                let gconf = updated_line.replace(&format!("@.snapshots_linux/rootfs/snapshot-{}", source_dep),
                                                 &format!("@.snapshots_linux/rootfs/snapshot-{}", rec_tmp));

                // Open the file in read and write mode
                let mut file = OpenOptions::new().read(true).write(true).append(true).open(&grub_path)?;

                // Write the modified content back to the file
                file.write_all(format!("\n\n{}", &gconf).as_bytes())?;
            } else {
                deploy_recovery()?;
            }
        }
    } else {
        deploy_recovery()?;
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
        .expect("Date header not found.");

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
    let vars = read_dir("/.snapshots/var")
        .unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
    let mut snapshots = read_dir("/.snapshots/rootfs")
        .unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
    snapshots.append(&mut vars.clone());
    snapshots.append(&mut etcs.clone());
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
        } else if snapshot.to_str().unwrap().contains("var") {
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
    if Path::new(&format!("/.snapshots/rootfs/snapshot-{}", tmp)).try_exists().unwrap() {
        delete_subvolume(&format!("/.snapshots/boot/boot-{}", tmp), DeleteSubvolumeFlags::RECURSIVE).unwrap();
        delete_subvolume(&format!("/.snapshots/etc/etc-{}", tmp), DeleteSubvolumeFlags::RECURSIVE).unwrap();
        delete_subvolume(&format!("/.snapshots/var/var-{}", tmp), DeleteSubvolumeFlags::RECURSIVE).unwrap();
        delete_subvolume(&format!("/.snapshots/rootfs/snapshot-{}", tmp), DeleteSubvolumeFlags::RECURSIVE).unwrap();
    }
    Ok(())
}

// Recursively install package in tree
pub fn tree_install(treename: &str, pkgs: &Vec<String>, profiles: &Vec<String>, force: bool
                    ,user_profiles: &Vec<String>, noconfirm: bool, secondary: bool) -> Result<(), Error> {
    // Make sure treename exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
        eprintln!("Cannot remove as tree {} doesn't exist.", treename);

    } else {
        // Import tree value
        let tree = fstree().unwrap();
        // Install packages
        if !pkgs.is_empty() {
            for pkg in pkgs {
                install(treename, &vec![pkg.to_string()], noconfirm)?;
                let order = recurse_tree(&tree, treename);
                for branch in order {
                    if branch != treename {
                        println!("{}, {}", treename,branch);
                        install(&branch, &vec![pkg.to_string()], noconfirm)?;
                    }
                }
            }
        } else if !profiles.is_empty() {
            // Install profiles
            for profile in profiles {
                let user_profile = "";
                if install_profile(treename, &profile, force, secondary, &user_profile, noconfirm).is_ok() {
                    post_transactions(treename)?;
                } else {
                    chr_delete(treename)?;
                    return Err(Error::new(ErrorKind::Other,
                                          format!("Failed to install and changes discarded.")));
                }
                let order = recurse_tree(&tree, treename);
                for branch in order {
                    if branch != treename {
                        println!("{}, {}", treename,branch);
                        if install_profile(&branch, &profile, force, secondary, &user_profile, noconfirm).is_ok() {
                            post_transactions(&branch)?;
                        } else {
                            chr_delete(&branch)?;
                            return Err(Error::new(ErrorKind::Other,
                                                  format!("Failed to install and changes discarded.")));
                        }
                    }
                }
            }
        } else if !user_profiles.is_empty() {
            // Install profiles
            for user_profile in user_profiles {
                let profile = "";
                if install_profile(treename, &profile, force, secondary, &user_profile, noconfirm).is_ok() {
                    post_transactions(treename)?;
                } else {
                    chr_delete(treename)?;
                    return Err(Error::new(ErrorKind::Other,
                                          format!("Failed to install and changes discarded.")));
                }
                let order = recurse_tree(&tree, treename);
                for branch in order {
                    if branch != treename {
                        println!("{}, {}", treename,branch);
                        if install_profile(&branch, &profile, force, secondary, &user_profile, noconfirm).is_ok() {
                            post_transactions(&branch)?;
                        } else {
                            chr_delete(&branch)?;
                            return Err(Error::new(ErrorKind::Other,
                                                  format!("Failed to install and changes discarded.")));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

// Recursively remove package in tree
pub fn tree_remove(treename: &str, pkgs: &Vec<String>, profiles: &Vec<String>, user_profiles: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    // Make sure treename exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
        eprintln!("Cannot remove as tree {} doesn't exist.", treename);

    } else {
        // Import tree value
        let tree = fstree().unwrap();
        // Remove packages
        if !pkgs.is_empty() {
            for pkg in pkgs {
                uninstall(treename, &vec![pkg.to_string()], noconfirm)?;
                let order = recurse_tree(&tree, treename);
                for branch in order {
                    if branch != treename {
                        println!("{}, {}", treename,branch);
                        uninstall(&branch, &vec![pkg.to_string()], noconfirm)?;
                    }
                }
            }
        } else if !profiles.is_empty() {
            // Remove profiles
            for profile in profiles {
                let user_profile = "";
                if uninstall_profile(treename, &profile, &user_profile, noconfirm).is_ok() {
                    post_transactions(treename)?;
                } else {
                    chr_delete(treename)?;
                    return Err(Error::new(ErrorKind::Other,
                                          format!("Failed to remove and changes discarded.")));
                }
                let order = recurse_tree(&tree, treename);
                for branch in order {
                    if branch != treename {
                        println!("{}, {}", treename,branch);
                        if uninstall_profile(&branch, &profile, &user_profile, noconfirm).is_ok() {
                            post_transactions(&branch)?;
                        } else {
                            chr_delete(&branch)?;
                            return Err(Error::new(ErrorKind::Other,
                                                  format!("Failed to remove and changes discarded.")));
                        }
                    }
                }
            }
        } else if !user_profiles.is_empty() {
            // Remove profiles
            for user_profile in user_profiles {
                let profile = "";
                if uninstall_profile(treename, &profile, &user_profile, noconfirm).is_ok() {
                    post_transactions(treename)?;
                } else {
                    chr_delete(treename)?;
                    return Err(Error::new(ErrorKind::Other,
                                          format!("Failed to remove and changes discarded.")));
                }
                let order = recurse_tree(&tree, treename);
                for branch in order {
                    if branch != treename {
                        println!("{}, {}", treename,branch);
                        if uninstall_profile(&branch, &profile, &user_profile, noconfirm).is_ok() {
                            post_transactions(&branch)?;
                        } else {
                            chr_delete(&branch)?;
                            return Err(Error::new(ErrorKind::Other,
                                                  format!("Failed to remove and changes discarded.")));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

// Recursively run a command in tree
pub fn tree_run(treename: &str, cmd: &str) -> Result<(), Error> {
    // Make sure treename exist
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", treename)).try_exists().unwrap() {
                return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot update as tree {} doesn't exist.", treename)));

    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", treename)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::Other,
                              format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}.",
                                      treename,treename)));
    } else {
        // Run command
        prepare(treename)?;
        chroot_exec(&format!("/.snapshots/rootfs/snapshot-chr{}", treename), cmd)?;
        post_transactions(treename)?;

        // Import tree file
        let tree = fstree().unwrap();

        let order = recurse_tree(&tree, treename);
        for branch in order {
            if branch != treename {
                println!("{}, {}", treename,branch);
                if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", branch)).try_exists().unwrap() {
                    return Err(Error::new(ErrorKind::Other,
                                          format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}.",
                                                  branch,branch)));
                } else {
                    prepare(&branch)?;
                    chroot_exec(&format!("/.snapshots/rootfs/snapshot-chr{}", branch), cmd)?;
                    post_transactions(&branch)?;
                }
            }
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

    // Make sure snapshot is not in use by another ash process
    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", treename)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::Other,
                              format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}.",
                                      treename,treename)));
    } else {
        // Syncing tree automatically updates it, unless 'force-sync' is used
        if !force_offline {
            if tree_upgrade(treename).is_err() {
                return Err(Error::new(ErrorKind::Other,
                                      format!("Failed to upgrade tree {}.", treename)));
            }
        }

        // Import tree file
        let tree = fstree().unwrap();

        let order = recurse_tree(&tree, treename);
        for branch in order {
            if branch != treename {
                println!("{}, {}", treename,branch);
                if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", branch)).try_exists().unwrap() {
                    return Err(Error::new(ErrorKind::Other,
                                          format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}.",
                                                  branch,branch)));
                } else {
                    prepare(&branch)?;
                    // Pre-sync
                    tree_sync_helper(treename, &branch, "chr")?;
                    // Live sync
                    if live && &branch == &get_current_snapshot() {
                        // Post-sync
                        tree_sync_helper(&branch, &get_tmp(), "")?;
                    }
                    // Moved here from the line immediately after first sync_tree_helper
                    post_transactions(&branch)?;
                }
            }
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

        // Make sure snapshot is not in use by another ash process
        } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", treename)).try_exists().unwrap() {
        return Err(
            Error::new(ErrorKind::Unsupported,
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
                               treename,treename)));

    } else {
        // Run update
        if auto_upgrade(treename).is_err() {
            return Err(Error::new(ErrorKind::Other,
                                  format!("Failed to auto upgrade tree {}.", treename)));
        };

        // Import tree file
        let tree = fstree().unwrap();

        let order = recurse_tree(&tree, treename);

        // Auto upgrade braches in sync_tree
        for branch in order {
            if branch != treename {
                println!("{}, {}", treename,branch);
                // Make sure snapshot is not in use by another ash process
                if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", branch)).try_exists().unwrap() {
                    return Err(
                        Error::new(ErrorKind::Unsupported,
                                   format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
                                           branch,branch)));
                }
                // Run update
                if auto_upgrade(&branch).is_err() {
                    return Err(Error::new(ErrorKind::Other,
                                          format!("Failed to auto upgrade tree {}.",  branch)));
                }
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
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
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
    let distro = detect::distro_id();
    let dist_name = if distro.contains("_ashos") {
        distro.replace("_ashos", "")
    } else {
        distro
    };
    let cfile = format!("/usr/share/ash/profiles/{}/{}", profile,dist_name);

    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot uninstall as snapshot {} doesn't exist.", snapshot)));

        // Make sure snapshot is not in use by another ash process
        } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        return Err(
            Error::new(ErrorKind::Unsupported,
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
                               snapshot,snapshot)));

        // Make sure snapshot is not base snapshot
        } else if snapshot == "0" {
        return Err(Error::new(ErrorKind::Unsupported,
                              format!("Changing base snapshot is not allowed.")));
    } else {
        // Uninstall profile
        // Prepare
        prepare(snapshot)?;

        // Profile configurations
        let mut profconf = Ini::new();
        profconf.set_comment_symbols(&['#']);
        profconf.set_multiline(true);
        // Load profile if exist
        if Path::new(&cfile).try_exists().unwrap() && user_profile.is_empty() {
            profconf.load(&cfile).unwrap();
        } else if !user_profile.is_empty() {
            profconf.load(user_profile).unwrap();
        }

        // Read packages section in configuration file
        if profconf.sections().contains(&"profile-packages".to_string()) {
            let mut pkgs: Vec<String> = Vec::new();
            for pkg in profconf.get_map().unwrap().get("profile-packages").unwrap().keys() {
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
    let distro = detect::distro_id();
    let dist_name = if distro.contains("_ashos") {
        distro.replace("_ashos", "")
    } else {
        distro
    };
    let cfile = format!("/usr/share/ash/profiles/{}/{}", profile,dist_name);
    let tmp = get_tmp();

    // Prepare
    ash_mounts(&tmp, "")?;

    // Profile configurations
    let mut profconf = Ini::new();
    profconf.set_comment_symbols(&['#']);
    profconf.set_multiline(true);

    // Load profile if exist
    if Path::new(&cfile).try_exists().unwrap() && user_profile.is_empty() {
        profconf.load(&cfile).unwrap();
    } else if !user_profile.is_empty() {
        profconf.load(user_profile).unwrap();
    }

    // Read packages section in configuration file
    if profconf.sections().contains(&"profile-packages".to_string()) {
        let mut pkgs: Vec<String> = Vec::new();
        for pkg in profconf.get_map().unwrap().get("profile-packages").unwrap().keys() {
            pkgs.push(pkg.to_string());
        }
        // Uninstall package(s)
        uninstall_package_helper_live(&tmp, &pkgs, noconfirm)?;
    }

    // Read commands section in configuration file
    if profconf.sections().contains(&"uninstall-commands".to_string()) {
        for cmd in profconf.get_map().unwrap().get("uninstall-commands").unwrap().keys() {
            chroot_exec(&format!("/.snapshots/rootfs/snapshot-{}", snapshot), cmd)?;
        }
    }

    // Umount tmp
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
            let excode = uninstall_profile(snapshot, profile, user_profile, noconfirm);
            match excode {
                Ok(_) => {
                    if post_transactions(snapshot).is_ok() {
                        println!("Profile {} removed from snapshot {} successfully.", p,snapshot);
                    } else {
                        chr_delete(snapshot)?;
                        eprintln!("Uninstall failed and changes discarded!");
                    }
                },
                Err(e) => {
                    eprintln!("{}",e);
                    chr_delete(snapshot)?;
                    eprintln!("Uninstall failed and changes discarded!");
                },
            }

        } else if !pkgs.is_empty() {
            // Uninstall package
            uninstall(snapshot, &pkgs, noconfirm)?;

        } else if !user_profile.is_empty() {
            // Uninstall user_profile
            let excode = uninstall_profile(snapshot, profile, user_profile, noconfirm);
            match excode {
                Ok(_) => {
                    if post_transactions(snapshot).is_ok() {
                        println!("Profile {} removed from snapshot {} successfully.", p,snapshot);
                    } else {
                        chr_delete(snapshot)?;
                        eprintln!("Uninstall failed and changes discarded!");
                    }
                },
                Err(e) => {
                    eprintln!("{}",e);
                    chr_delete(snapshot)?;
                    eprintln!("uninstall failed and changes discarded!");
                },
            }
        }

    } else if live && snapshot != get_current_snapshot() {
        // Prevent live Uninstall except for current snapshot
        eprintln!("Can't use the live option with any other snapshot than the current one.");

    // Do live uninstall only if: live flag is used OR target snapshot is current
    } else if live && snapshot == get_current_snapshot() {
        if !profile.is_empty() {
            // Live profile uninstall
            let excode = uninstall_profile_live(snapshot, profile, user_profile, noconfirm);
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
            let excode = uninstall_profile_live(snapshot, profile, user_profile, noconfirm);
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
                format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
                        snapshot,snapshot)));

    } else {
        // Get tmp
        let tmp = get_tmp();
        let tmp = get_aux_tmp(tmp, secondary);

        // Partition path
        let part = get_part();

        // Prepare for update
        prepare(snapshot)?;

        // Remove grub configurations older than 30 days
        if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}/boot/{}/BAK/", snapshot,grub)).try_exists().unwrap() && snapshot != "0" {
            delete_old_grub_files(&format!("/.snapshots/rootfs/snapshot-chr{}/boot/{}", snapshot,grub).as_str())?;
        }

        // Get current time
        let time = Local::now().naive_local();
        let formatted = time.format("%Y%m%d-%H%M%S").to_string();

        // Copy backup
        if snapshot != "0" {
            copy(format!("/.snapshots/rootfs/snapshot-chr{}/boot/{}/grub.cfg", snapshot,grub),
                 format!("/.snapshots/rootfs/snapshot-chr{}/boot/{}/BAK/grub.cfg.{}", snapshot,grub,formatted))?;
        }

        // Run update commands in chroot
        let distro_name = detect::distro_name(snapshot);
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

    // Make sure snapshot is not base snapshot
    if snapshot == "0" {
        return Err(Error::new(ErrorKind::Unsupported,
                              format!("Changing base snapshot is not allowed.")));
    } else {
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
    }
    Ok(())
}

// Upgrade snapshot
pub fn upgrade(snapshot:  &str, baseup: bool, noconfirm: bool) -> Result<(), Error> {
    // Make sure snapshot exists
    if !Path::new(&format!("/.snapshots/rootfs/snapshot-{}", snapshot)).try_exists().unwrap() {
        return Err(Error::new(ErrorKind::NotFound,
                              format!("Cannot upgrade as snapshot {} doesn't exist.", snapshot)));

    } else if Path::new(&format!("/.snapshots/rootfs/snapshot-chr{}", snapshot)).try_exists().unwrap() {
        // Make sure snapshot is not in use by another ash process
        return Err(
            Error::new(ErrorKind::Unsupported,
                       format!("Snapshot {} appears to be in use. If you're certain it's not in use, clear lock with 'ash unlock -s {}'.",
                               snapshot,snapshot)));

    } else if snapshot == "0" && !baseup {
        // Make sure snapshot is not base snapshot
        return Err(Error::new(ErrorKind::Unsupported,
                              format!("Changing base snapshot is not allowed.")));

    } else {
        // Default upgrade behaviour is now "safe" update, meaning failed updates get fully discarded
        let excode = upgrade_helper(snapshot, noconfirm);
        if excode.is_ok() {
            if post_transactions(snapshot).is_ok() {
                if baseup {
                    if deploy_recovery().is_err() {
                        return Err(Error::new(ErrorKind::Other,
                                              format!("Failed to deploy recovery snapshot.")));
                    }
                }
                println!("Snapshot {} upgraded successfully.", snapshot);
            }
        } else {
            chr_delete(snapshot)?;
            return Err(Error::new(ErrorKind::Other,
                                  format!("Upgrade failed and changes discarded.")));
        }
    }
    Ok(())
}

// Return snapshot that has a package
pub fn which_snapshot_has(pkgs: Vec<String>) {
    // Collect snapshots
    let tree = fstree().unwrap();
    let snapshots = return_children(&tree, "root");

    // Search snapshots for package
    for pkg in pkgs {
        let mut snapshot: Vec<String> = Vec::new();
        for snap in &snapshots {
            if is_pkg_installed(&&snap.to_string(), &pkg) {
                snapshot.push(format!("{}", snap.to_string()));
            }
        }
        if !snapshot.is_empty() {
            println!("package {} installed in {snapshot:?}.", pkg);
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
