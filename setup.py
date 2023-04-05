#!/usr/bin/python3

import os.path
import subprocess
import sys

is_efi = os.path.exists("/sys/firmware/efi")
use_other_iso = "" # if using different iso to install target OS, use its id

try:
    if is_efi:
        args = list(sys.argv[0:4]) # just first 3 arguments (exclude distro arguments)
        distro = sys.argv[4]
        distro_name = sys.argv[5]
    else:
        args = list(sys.argv[0:3]) # just first 2 arguments (exclude distro arguments)
        distro = sys.argv[3]
        distro_name = sys.argv[4]
except IndexError:
    distro = subprocess.check_output(['./src/detect_os.sh', 'id']).decode('utf-8').replace('"', "").strip()
    distro_name = subprocess.check_output(['./src/detect_os.sh', 'name']).decode('utf-8').replace('"', "").strip()

if distro:
    if use_other_iso != "":
        use_distro = use_other_iso
    else:
        use_distro = distro
    try: # CAUTION: comment lines 28-35 & unindent line 36 if prepared manually
        if is_efi:
            subprocess.check_output([f'./src/prep/{use_distro}_live.sh', f'{args[1]}', f'{args[2]}', f'{args[3]}'])
        else:
            subprocess.check_output([f'./src/prep/{use_distro}_live.sh', f'{args[1]}', f'{args[2]}'])
    except subprocess.CalledProcessError as e:
        print(f"F: There was an error in prep steps! {e.output.decode('utf-8')}")
    else:
        __import__(f"src.distros.{distro}.installer")
else:
    print("F: Distribution could not be detected!")

