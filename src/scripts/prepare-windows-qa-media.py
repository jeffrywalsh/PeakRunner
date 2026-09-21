#!/usr/bin/env python3
"""Refresh ONLY the generated QA seed ISO while the Windows VM is stopped."""
import pathlib
import shutil
import subprocess

root = pathlib.Path('/Volumes/AllOfIt/PeakRunner-VMs')
processes = subprocess.check_output(['ps', '-axo', 'command'], text=True)
assert not any('QEMULauncher' in p and 'PeakRunner Windows 11 ARM' in p for p in processes.splitlines()), 'Stop Windows VM first'
seed = root / 'Windows-seed'
assert (seed / 'Autounattend.xml').exists()
shutil.copyfile(pathlib.Path(__file__).resolve().parent.parent / 'deploy/setup-windows-qa.ps1', seed / 'setup-windows-qa.ps1')
(seed / 'startup.nsh').write_text('fs0:\\efi\\boot\\bootaa64.efi\r\n')
target = root / 'PeakRunner Windows 11 ARM.utm/Data/setup.iso'
subprocess.run(['hdiutil', 'makehybrid', '-iso', '-joliet', '-default-volume-name', 'PEAKRUNNER_QA', '-ov', '-o', str(target), str(seed)], check=True)
