#!/usr/bin/env python3
"""Use QXL for the x64 software-rendered test desktop; run with UTM closed."""
import pathlib
import plistlib
import shutil
import subprocess

processes = subprocess.check_output(['ps', '-axo', 'command'], text=True)
assert not any(p.rstrip().endswith('/Contents/MacOS/UTM') or (p.startswith('/Applications/') and '/QEMULauncher ' in p) for p in processes.splitlines()), 'Shut down the test guests and quit UTM first'
path = pathlib.Path('/Volumes/AllOfIt/PeakRunner-VMs/PeakRunner Linux x64.utm/config.plist')
backup = path.with_name('config.before-qxl.plist')
if not backup.exists():
    shutil.copyfile(path, backup)
with path.open('rb') as stream:
    config = plistlib.load(stream)
assert config['Information']['Name'] == 'PeakRunner Linux x64'
config['Display'][0]['Hardware'] = 'qxl-vga'
with path.open('wb') as stream:
    plistlib.dump(config, stream)
