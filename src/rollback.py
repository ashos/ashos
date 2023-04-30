#!/usr/bin/env python3

import os
import sys

if os.path.exists("/dev/mapper/luks_root"):
    is_luks = True
    os_root = "/dev/mapper/luks_root"
else:
    is_luks = True
    os_root = sys.argv[1]

#   Unmount everything
def rollback():
    os.system(f"umount --recursive /mnt")
    os.system(f"mount {os_root} -o subvolid=0 /mnt")
    os.system(f"btrfs sub del /mnt/@*")
    os.system(f"umount --recursive /mnt")
    if is_luks:
        os.system(f"cryptsetup close luks_root")

rollback()
print("Changes reverted!")

