use libbtrfs::subvolume::{new, remove, set_default_subvol, set_readonly, snapshot};
use std::io::Error;

// Create btrfs snapshot
pub fn create_snapshot(snapvol: &str, pathname: &str, readonly: bool) -> Result<(), Error> {
    snapshot(snapvol, pathname, readonly)?;
    Ok(())
}

// Create btrfs subvolume
pub fn create_subvolume(pathname: &str) -> Result<(), Error> {
    new(pathname)?;
    Ok(())
}

// Delete btrfs subvolume
pub fn delete_subvolume(pathname: &str) -> Result<(), Error> {
    remove(pathname)?;
    Ok(())
}

// Set default subvolume
pub fn set_default(subvol: &str) -> Result<(), Error> {
    set_default_subvol(subvol)?;
    Ok(())
}

// Set read only flag for subvolume
pub fn set_subvolume_read_only(subv: &str, readonly: bool) -> Result<(), Error> {
    set_readonly(subv, readonly)?;
    Ok(())
}
