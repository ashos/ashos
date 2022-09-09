#!/usr/bin/python3

import os
import time
import subprocess

snapshot = subprocess.check_output("/usr/bin/ash c", shell=True)
while True:
    if os.path.exists(f"/.snapshots/rootfs/snapshot-chr{snapshot}"):
        time.sleep(20)
    else:
        os.system("/usr/bin/ash clone $(/usr/bin/ash c)")
        os.system("/usr/bin/ash auto-upgrade")
        os.system("/usr/bin/ash base-update")
        break

upstate = open("/.snapshots/ash/upstate")
line = upstate.readline()
upstate.close()
if "1" not in line:
    os.system("/usr/bin/ash deploy $(/usr/bin/ash c)")

