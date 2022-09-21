#!/usr/bin/python3

import subprocess
import sys

args = list(sys.argv[0:4]) # just first 3 arguments (exclude distro arguments)
use_arch_iso_for_this = False # Set True if using arch iso to install other distro

try: # If distro to be installed does not match live environment, use argument 4 and 5 (examples: README.md)
    distro = sys.argv[4]
    distro_name = sys.argv[5]
#    override_distro = True
except IndexError:
    distro = subprocess.check_output(['./src/detect_os.sh', 'id']).decode('utf-8').replace('"', "").strip()
    distro_name = subprocess.check_output(['./src/detect_os.sh', 'name']).decode('utf-8').replace('"', "").strip()
#    override_distro = False

if distro:
    try: # CAUTION: comment lines 19-26 if prefer to prepare manually
        if use_arch_iso_for_this:
            subprocess.check_output([f'./src/prep/arch_live.sh', f'{args[1]}', f'{args[2]}', f'{args[3]}'])
        else:
            subprocess.check_output([f'./src/prep/{distro}_live.sh', f'{args[1]}', f'{args[2]}', f'{args[3]}'])
    except subprocess.CalledProcessError as e:
        print(f"F: There was an error in prep steps! {e.output.decode('utf-8')}")
    else:
        __import__(f"src.distros.{distro}.installer")
else:
    print("F: Distribution could not be detected!")

