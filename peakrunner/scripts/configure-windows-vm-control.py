#!/usr/bin/env python3
"""Enable local QA controls. Run ONLY while this VM and UTM are stopped."""
import pathlib
import plistlib
import shutil
import subprocess
import argparse

parser = argparse.ArgumentParser()
parser.add_argument('--disable', action='store_true', help='Remove the temporary localhost QA listener')
parser.add_argument('--eject-install-media', action='store_true', help='Detach only the two known setup CD drives, preserving their image files')
args = parser.parse_args()

processes = subprocess.check_output(['ps', '-axo', 'command'], text=True)
if any(line.rstrip().endswith('/Contents/MacOS/UTM') or (line.startswith('/Applications/') and '/QEMULauncher ' in line) for line in processes.splitlines()):
    raise SystemExit('Stop the test guests and quit UTM before editing its configuration.')
path = pathlib.Path('/Volumes/AllOfIt/PeakRunner-VMs/PeakRunner Windows 11 ARM.utm/config.plist')
backup = path.with_name('config.before-qa-control.plist')
if not backup.exists():
    shutil.copyfile(path, backup)
with path.open('rb') as stream:
    config = plistlib.load(stream)
assert config['Information']['Name'] == 'PeakRunner Windows 11 ARM'
endpoint = '-qmp tcp:127.0.0.1:4445,server=on,wait=off'
arguments = [a for a in config['QEMU']['AdditionalArguments'] if a and a != endpoint]
if not args.disable:
    if any('-qmp' in a for a in arguments):
        raise SystemExit('Review existing QMP configuration instead of overwriting it.')
    arguments.append(endpoint)
config['QEMU']['AdditionalArguments'] = arguments
if args.eject_install_media:
    config['Drive'] = [drive for drive in config['Drive'] if drive.get('Identifier') not in ('wininstall', 'winsetup')]
with path.open('wb') as stream:
    plistlib.dump(config, stream)
