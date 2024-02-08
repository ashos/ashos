/*/use crate::{check_mutability, chr_delete, chroot_exec, get_current_snapshot, get_tmp,
            immutability_disable, immutability_enable, is_system_pkg, is_system_locked,
            post_transactions, prepare, remove_dir_content, snapshot_config_get, sync_time};

use configparser::ini::{Ini, WriteOptions};
use rustix::path::Arg;
use std::fs::{DirBuilder, File, metadata, OpenOptions, read_dir};
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Write};
use std::path::Path;
use std::process::{Command, ExitStatus};
use tempfile::TempDir;
use users::get_user_by_name;
use users::os::unix::UserExt;
use walkdir::WalkDir;

// Noninteractive update
pub fn auto_upgrade(snapshot: &str) -> Result<(), Error> {
    Ok(())
}

// Copy cache of downloaded packages to shared
pub fn cache_copy(snapshot: &str, prepare: bool) -> Result<(), Error> {
   Ok(())
}

// Clean apt cache
pub fn clean_cache(snapshot: &str) -> Result<(), Error> {
   Ok(())
}

// Uninstall all packages in snapshot
pub fn clean_chroot(snapshot: &str, profconf: &Ini) -> Result<(), Error> {
    Ok(())
}

// Fix signature invalid error
pub fn fix_package_db(snapshot: &str) -> Result<(), Error> {
    Ok(())
}

// Install atomic-operation
pub fn install_package_helper(snapshot:&str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
   Ok(())
}

// Install atomic-operation
pub fn install_package_helper_chroot(snapshot:&str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    Ok(())
}

// Install atomic-operation in live snapshot
pub fn install_package_helper_live(snapshot: &str, tmp: &str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
    Ok(())
}

// Check if service enabled
pub fn is_service_enabled(snapshot: &str, service: &str) -> bool {
    if Path::new("/var/lib/systemd/").try_exists().unwrap() {
        let excode = Command::new("sh").arg("-c")
                                       .arg(format!("chroot /.snapshots/rootfs/snapshot-chr{} systemctl is-enabled {}", snapshot,service))
                                       .output().unwrap();
        let stdout = String::from_utf8_lossy(&excode.stdout).trim().to_string();
        if stdout == "enabled" {
            return true;
        } else {
            return false;
        }
    } else {
        // TODO add OpenRC
        return false;
    }
}

// Prevent system packages from being automatically removed
pub fn lockpkg(snapshot:&str, profconf: &Ini) -> Result<(), Error> {
    Ok(())
}

// Get list of installed packages and exclude packages installed as dependencies
pub fn no_dep_pkg_list(snapshot: &str, chr: &str) /*-> Vec<String>*/ {
}

// Reinstall base packages in snapshot
pub fn bootstrap(snapshot: &str) -> Result<(), Error> {
   Ok(())
}

// Get list of packages installed in a snapshot
pub fn pkg_list(snapshot: &str, chr: &str) /*-> Vec<String>*/ {
}

// APT query
pub fn pkg_query(pkg: &str) /*-> Result<ExitStatus, Error>*/ {
}

// Refresh snapshot atomic-operation
pub fn refresh_helper(snapshot: &str) -> Result<(), Error> {
   Ok(())
}

// Disable service(s) (Systemd, OpenRC, etc.)
pub fn service_disable(snapshot: &str, services: &Vec<String>, chr: &str) -> Result<(), Error> {
    for service in services {
        // Systemd
        if Path::new("/var/lib/systemd/").try_exists()? {
            let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-{}{}", chr,snapshot))
                                               .arg("systemctl")
                                               .arg("disable")
                                               .arg(&service).status()?;
            if !excode.success() {
                return Err(Error::new(ErrorKind::Other,
                                      format!("Failed to disable {}.", service)));
            }
        } //TODO add OpenRC
    }
    Ok(())
}

// Enable service(s) (Systemd, OpenRC, etc.)
pub fn service_enable(snapshot: &str, services: &Vec<String>, chr: &str) -> Result<(), Error> {
    for service in services {
        // Systemd
        if Path::new("/var/lib/systemd/").try_exists()? {
            let excode = Command::new("chroot").arg(format!("/.snapshots/rootfs/snapshot-{}{}", chr,snapshot))
                                               .arg("systemctl")
                                               .arg("enable")
                                               .arg(&service).status()?;
            if !excode.success() {
                return Err(Error::new(ErrorKind::Other,
                                      format!("Failed to enable {}.", service)));
            }
        } //TODO add OspenRC
    }
    Ok(())
}

// Show diff of packages between 2 snapshots
pub fn snapshot_diff(snapshot1: &str, snapshot2: &str) -> Result<(), Error> {
    Ok(())
}

// Copy system configurations to new snapshot
pub fn system_config(snapshot: &str, profconf: &Ini) -> Result<(), Error> {
   Ok(())
}

// Sync tree helper function
pub fn tree_sync_helper(s_f: &str, s_t: &str, chr: &str) -> Result<(), Error>  {
   Ok(())
}

// Uninstall package(s) atomic-operation
pub fn uninstall_package_helper(snapshot: &str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
   Ok(())
}

// Uninstall package(s) atomic-operation
pub fn uninstall_package_helper_chroot(snapshot: &str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
   Ok(())
}

// Uninstall package(s) atomic-operation live snapshot
pub fn uninstall_package_helper_live(tmp: &str, pkgs: &Vec<String>, noconfirm: bool) -> Result<(), Error> {
   Ok(())
}

// Upgrade snapshot atomic-operation
pub fn upgrade_helper(snapshot: &str, noconfirm: bool) -> Result<(), Error> {
   Ok(())
}

// Live upgrade snapshot atomic-operation
pub fn upgrade_helper_live(tmp: &str, noconfirm: bool) -> Result<(), Error> {
   Ok(())
}*/
