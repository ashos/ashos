extern crate lib;
mod cli;

use cli::*;
use lib::*;
use nix::unistd::Uid;
// Directexplicitories
// All snapshots share one /var
// Global boot is always at @boot
// *-deploy and *-deploy-aux         : temporary directories used to boot deployed snapshot
// *-deploy[-aux]-secondary          : temporary directories used to boot secondary deployed snapshot
// *-chr                             : temporary directories used to chroot into snapshot or copy snapshots around
// /.snapshots/ash/ash               : symlinked to /usr/sbin/ash
// /.snapshots/etc/etc-*             : individual /etc for each snapshot
// /.snapshots/boot/boot-*           : individual /boot for each snapshot
// /.snapshots/rootfs/snapshot-*     : snapshots
// /.snapshots/ash/snapshots/*-desc  : descriptions
// /usr/share/ash                    : files that store current snapshot info
// /usr/share/ash/db                 : package database
// /var/lib/ash(/fstree)             : ash files, stores fstree, symlink to /.snapshots/ash

fn main() {
    if !Uid::effective().is_root() {
        panic!("sudo/doas is required to run ash!");
    } else if chroot_check() {
        panic!("Please don't use ash inside a chroot!");
    } else {
        // Call cli matches
        let matches = cli().get_matches();
        // Call relevant functions
        match matches.subcommand() {
            Some(("auto-upgrade", _matches)) => {
            }
            Some(("base-update", _matches)) => {
            }
            Some(("branch", barnch_matches)) => {
                let snapshot  = barnch_matches.get_one::<i32>("snapshot").unwrap();
                if barnch_matches.contains_id("desc") {
                    let desc = barnch_matches.get_one::<String>("desc").map(|s| s.as_str()).unwrap();
                    extend_branch(format!("{}", snapshot).as_str(), desc);
                } else {
                    let desc = String::new();
                    extend_branch(format!("{}", snapshot).as_str(), desc.as_str());
                }
            }
            Some(("check", _matches)) => {
                check_update().unwrap();
            }
            // Chroot
            Some(("chroot", chroot_matches)) => {
                // Get snapshot value
                let snapshot  = if chroot_matches.contains_id("SNAPSHOT") {
                    let snap = chroot_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Get description value
                let cmd = if chroot_matches.contains_id("COMMAND") {
                    let cmd = chroot_matches.get_one::<String>("COMMAND").map(|s| s.as_str()).unwrap().to_string();
                    cmd
                } else {
                    let cmd = String::new();
                    cmd
                };

                // Run chroot
                chroot(format!("{}", snapshot).as_str(), cmd.as_str()).unwrap();
            }
            // Clone
            Some(("clone", clone_matches)) => {
                // Get snapshot value
                let snapshot = if clone_matches.contains_id("SNAPSHOT") {
                    let snap = clone_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Get description value
                let desc = if clone_matches.contains_id("DESCRIPTION") {
                    let desc = clone_matches.get_one::<String>("DESCRIPTION").map(|s| s.as_str()).unwrap().to_string();
                    desc
                } else {
                    let desc = String::new();
                    desc
                };

                // Run clone
                let run = clone_as_tree(snapshot.as_str(), desc.as_str());
                match run {
                    Ok(snapshot_num) => println!("Tree {} cloned from {}.", snapshot_num,snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Clone branch
            Some(("clone-branch", clone_branch_matches)) => {
                // Get snapshot value
                let snapshot = if clone_branch_matches.contains_id("SNAPSHOT") {
                    let snap = clone_branch_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run clone-branch
                let run = clone_branch(snapshot.as_str());
                match run {
                    Ok(snapshot_num) => println!("Branch {} added to parent of {}.", snapshot_num,snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            // Clone tree
            Some(("clone-tree", clone_tree_matches)) => {
                // Get snapshot value
                let snapshot = if clone_tree_matches.contains_id("SNAPSHOT") {
                    let snap = clone_tree_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run clone-tree
                let run = clone_recursive(snapshot.as_str());
                match run {
                    Ok(_) => println!("Snapshot {} was cloned recursively.", snapshot),
                    Err(e) => eprintln!("{}", e),
                }
                clone_recursive(snapshot.as_str()).unwrap();
            }
            Some(("clone-under", clone_under_matches)) => {
                // Get snapshot value
                let snap = clone_under_matches.get_one::<i32>("SNAPSHOT").unwrap();
                let snapshot = format!("{}", snap);

                // Get branch value
                let branch_i32 = clone_under_matches.get_one::<i32>("BRANCH").unwrap();
                let branch = format!("{}", branch_i32);

                // Run clone-under
                let run = clone_under(snapshot.as_str(), branch.as_str());
                match run {
                    Ok(snapshot_num) => println!("Branch {} added to parent of {}.", snapshot_num,snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            Some(("current", _matches)) => {
                println!("{}", get_current_snapshot());
            }
            Some(("del", del_matches)) => {
                // Get snapshot value
                let snapshots: Vec<_> = del_matches.get_many::<i32>("SNAPSHOT").unwrap().map(|s| format!("{}", s)).collect();

                // Optional values
                let quiet = del_matches.get_flag("quiet");
                let nuke = del_matches.get_flag("nuke");

                // Run delelte_node
                let run = delete_node(&snapshots, quiet, nuke);
                match run {
                    Ok(_) => println!("Snapshot {:?} removed.", snapshots),
                    Err(e) => eprintln!("{}", e),
                }
            }
            Some(("dist", _matches)) => {
            }
            Some(("etc-update", _matches)) => {
                update_etc();
            }
            Some(("live-chroot", _matches)) => {
                live_unlock();
            }
            Some(("rollback", _matches)) => {
            }
            Some(("subs", _matches)) => {
                list_subvolumes();
            }
            Some(("tree", _matches)) => {
            }
            Some(("tmp", _matches)) => {
                tmp_clear();
            }
            Some(("version", _matches)) => {
                match ash_version() {
                    Some(version) => println!("ash version: {}", version),
                    None => eprintln!("Ash not found"),
                }
            }
            Some(("whichtmp", _matches)) => {
                println!("{}", get_tmp());
            }
            _=> unreachable!(), // If all subcommands called, anything else is unreachable
        }
    }
}
