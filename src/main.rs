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
            Some(("auto-upgrade", auto_upgrade_matches)) => {
                // Get snapshot value
                let snapshot = if auto_upgrade_matches.contains_id("SNAPSHOT") {
                    let snap = auto_upgrade_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run auto_upgrade
                noninteractive_update(snapshot.as_str()).unwrap();
            }
            Some(("base-update", _matches)) => {
            }
            Some(("branch", branch_matches)) => {
                // Get snapshot value
                let snapshot = if branch_matches.contains_id("SNAPSHOT") {
                    let snap = branch_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Get description value
                let desc = if branch_matches.contains_id("DESCRIPTION") {
                    let desc = branch_matches.get_one::<String>("DESCRIPTION").map(|s| s.as_str()).unwrap().to_string();
                    desc
                } else {
                    let desc = String::new();
                    desc
                };

                // Run barnch_create
                let run = branch_create(snapshot.as_str(), desc.as_str());
                match run {
                    Ok(snapshot_num) => println!("Branch {} added under snapshot {}.", snapshot_num,snapshot),
                    Err(e) => eprintln!("{}", e),
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

                // Run clone_branch
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

                // Run clone_recursive
                let run = clone_recursive(snapshot.as_str());
                match run {
                    Ok(_) => println!("Snapshot {} was cloned recursively.", snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            Some(("clone-under", clone_under_matches)) => {
                // Get snapshot value
                let snap = clone_under_matches.get_one::<i32>("SNAPSHOT").unwrap();
                let snapshot = format!("{}", snap);

                // Get branch value
                let branch_i32 = clone_under_matches.get_one::<i32>("BRANCH").unwrap();
                let branch = format!("{}", branch_i32);

                // Run clone_under
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
            Some(("deploy", deploy_matches)) => {
                // Get snapshot value
                let snapshot = if deploy_matches.contains_id("SNAPSHOT") {
                    let snap = deploy_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Optional values
                let secondary = deploy_matches.get_flag("secondary");

                // Run deploy
                let run = deploy(snapshot.as_str(), secondary);
                match run {
                    Ok(_) => println!("Snapshot {} deployed to /.", snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            Some(("diff", diff_matches)) => {
                // Get snapshot one value
                let snap1 = diff_matches.get_one::<i32>("SNAPSHOT-1").unwrap();
                let snapshot1 = format!("{}", snap1);

                // Get snapshot two value
                let snap2 = diff_matches.get_one::<i32>("SNAPSHOT-2").unwrap();
                let snapshot2 = format!("{}", snap2);

                // Run diff
                diff(snapshot1.as_str(), snapshot2.as_str());
            }
            Some(("dist", _matches)) => {
            }
            Some(("etc-update", _matches)) => {
                update_etc();
            }
            Some(("fixdb", fixdb_matches)) => {
                // Get snapshot value
                let snapshot = if fixdb_matches.contains_id("SNAPSHOT") {
                    let snap = fixdb_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                //Run fixdb
                let run = fixdb(snapshot.as_str());
                match run {
                    Ok(_) => {
                        if post_transactions(snapshot.as_str()).is_ok() {
                            println!("Snapshot {}'s package manager database fixed successfully.", snapshot);
                        } else {
                            eprintln!("Fixing package manager database failed.");
                        }
                    },
                    Err(e) => {
                        chr_delete(snapshot.as_str()).unwrap();
                        eprintln!("Fixing package manager database failed due to: {}", e);
                    },
                }
            }
            Some(("hollow", hollow_matches)) => {
                // Get snapshot value
                let snapshot = if hollow_matches.contains_id("SNAPSHOT") {
                    let snap = hollow_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run hollow
                let run = hollow(snapshot.as_str());
                match run {
                    Ok(_) => println!("Snapshot {} hollow operation succeeded. Please reboot!", snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            Some(("immdis", immdis_matches)) => {
                // Get snapshot value
                let snapshot = if immdis_matches.contains_id("SNAPSHOT") {
                    let snap = immdis_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run immutability_disable
                let run = immutability_disable(snapshot.as_str());
                match run {
                    Ok(_) => println!("Snapshot {} successfully made mutable.", snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            Some(("immen", immen_matches)) => {
                // Get snapshot value
                let snapshot = if immen_matches.contains_id("SNAPSHOT") {
                    let snap = immen_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run immutability_enable
                let run = immutability_enable(snapshot.as_str());
                match run {
                    Ok(_) => println!("Snapshot {} successfully made immutable.", snapshot),
                    Err(e) => eprintln!("{}", e),
                }
            }
            Some(("list", list_matches)) => {
                // Get snapshot value
                let snapshot = if list_matches.contains_id("SNAPSHOT") {
                    let snap = list_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // chr value
                let chr = "";

                // Run list
                let run = list(snapshot.as_str(), chr);
                for pkg in run {
                    println!("{}", pkg);
                }
            }
            Some(("live-chroot", _matches)) => {
                live_unlock().unwrap();
            }
            Some(("refresh", refresh_matches)) => {
                // Get snapshot value
                let snapshot = if refresh_matches.contains_id("SNAPSHOT") {
                    let snap = refresh_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Run refresh
                refresh(snapshot.as_str()).unwrap();
            }
            Some(("rollback", _matches)) => {
                rollback().unwrap();
            }
            Some(("sub", _matches)) => {
                list_subvolumes();
            }
            Some(("tree", _matches)) => {
            }
            Some(("tmp", _matches)) => {
                tmp_clear();
            }
            Some(("tremove", tremove_matches)) => {
                // Get treename value
                let treename = if tremove_matches.contains_id("SNAPSHOT") {
                    let snap = tremove_matches.get_one::<i32>("SNAPSHOT").unwrap();
                    let snap_to_string = format!("{}", snap);
                    snap_to_string
                } else {
                    let snap = get_current_snapshot();
                    snap
                };

                // Get pkg value
                let pkgs = if tremove_matches.contains_id("PACKAGE") {
                    let pkgs: Vec<String> = tremove_matches.get_many::<String>("PACKAGE").unwrap().map(|s| format!("{}", s)).collect();
                    pkgs
                } else {
                    let pkgs: Vec<String> = Vec::new();
                    pkgs
                };

                // Get profile value
                let profiles = if tremove_matches.contains_id("PROFILE") {
                    let profiles: Vec<String> = tremove_matches.get_many::<String>("PROFILE").unwrap().map(|s| format!("{}", s)).collect();
                    profiles
                } else {
                    let profiles: Vec<String> = Vec::new();
                    profiles
                };

                // Run remove_from_tree
                remove_from_tree(treename.as_str(), pkgs, profiles).unwrap();
            }
            Some(("version", _matches)) => {
                ash_version().unwrap();
            }
            Some(("whichtmp", _matches)) => {
                println!("{}", get_tmp());
            }
            _=> unreachable!(), // If all subcommands called, anything else is unreachable
        }
    }
}
