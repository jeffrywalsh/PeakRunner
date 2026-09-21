#!/usr/bin/env python3
"""Create a NEW, isolated UTM x64 compatibility-test VM on AllOfIt.

Uses a verified official Debian cloud image; never modifies an existing VM.
Run on macOS with UTM installed. Images and the cloud-init seed stay off Git.
"""
import hashlib
import pathlib
import plistlib
import shutil
import subprocess
import uuid

root = pathlib.Path('/Volumes/AllOfIt/PeakRunner-VMs')
image = root / 'Media/debian-13-generic-amd64.qcow2'
expected = 'a733e7d49442a03e70d03e4eb5aaf3967f3efc69ef70952f9bb10fc1ee2c4876eb95956b5ad2d31350e5fada768feb651352535fb8cd1233f61998a5a7d2e93c'
with image.open('rb') as stream:
    assert hashlib.file_digest(stream, 'sha512').hexdigest() == expected, 'Image checksum mismatch'
bundle = root / 'PeakRunner Linux x64.utm'
assert not bundle.exists(), 'Refusing to overwrite an existing VM'
public_key = (pathlib.Path.home() / '.ssh/id_ed25519.pub').read_text().strip()
assert public_key.startswith('ssh-ed25519 ')
data = bundle / 'Data'
data.mkdir(parents=True)
shutil.copyfile(image, data / 'linux.qcow2')
subprocess.run(['qemu-img', 'resize', str(data / 'linux.qcow2'), '80G'], check=True)
seed = root / 'Linux-seed'
seed.mkdir(exist_ok=False)
(seed / 'meta-data').write_text('instance-id: peakrunner-linux-x64-20260919\nlocal-hostname: peakrunner-linux\n')
(seed / 'network-config').write_text('''version: 2
ethernets:
  enp0s1:
    dhcp4: true
    dhcp6: false
    dhcp4-overrides:
      use-dns: false
    nameservers:
      addresses: [1.1.1.1, 1.0.0.1]
''')
(seed / 'user-data').write_text('''#cloud-config
hostname: peakrunner-linux
manage_etc_hosts: true
users:
  - name: tester
    groups: [sudo, audio, video, render]
    shell: /bin/bash
    lock_passwd: true
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - ''' + public_key + '''
ssh_pwauth: false
disable_root: true
package_update: true
packages:
  - qemu-guest-agent
  - spice-vdagent
  - xfce4
  - lightdm
  - dbus-x11
  - xserver-xorg
  - mesa-vulkan-drivers
  - vulkan-tools
  - libasound2t64
  - libudev1
  - libxkbcommon-x11-0
  - libwayland-client0
  - curl
  - ca-certificates
  - unzip
write_files:
  - path: /etc/lightdm/lightdm.conf.d/50-peakrunner-test.conf
    content: |
      [Seat:*]
      autologin-user=tester
      autologin-user-timeout=0
      user-session=xfce
  - path: /etc/ssh/sshd_config.d/00-test-vm.conf
    content: |
      PasswordAuthentication no
      PermitRootLogin no
runcmd:
  - [systemctl, enable, --now, qemu-guest-agent]
  - [systemctl, set-default, graphical.target]
  - [systemctl, restart, lightdm]
final_message: 'PeakRunner Linux test desktop provisioning complete.'
''')
subprocess.run(['hdiutil', 'makehybrid', '-iso', '-joliet', '-default-volume-name', 'cidata', '-o', str(data / 'seed.iso'), str(seed)], check=True)
config = {
    'Backend': 'QEMU', 'ConfigurationVersion': 4,
    'Information': {'Name': 'PeakRunner Linux x64', 'UUID': str(uuid.uuid4()).upper(), 'Icon': 'debian', 'IconCustom': False,
                    'Notes': 'Debian 13 x64 compatibility QA. Emulated on Apple Silicon; software graphics are not a native performance benchmark. Local tester desktop autologin; SSH key-only on localhost:2222.'},
    'System': {'Architecture': 'x86_64', 'CPU': 'default', 'CPUCount': 1, 'CPUFlagsAdd': [], 'CPUFlagsRemove': [], 'ForceMulticore': False, 'JITCacheSize': 0, 'MemorySize': 6144, 'Target': 'q35'},
    'QEMU': {'AdditionalArguments': [], 'BalloonDevice': True, 'DebugLog': False, 'Hypervisor': False, 'PS2Controller': True, 'RNGDevice': True, 'RTCLocalTime': False, 'TPMDevice': False, 'TSO': False, 'UEFIBoot': True},
    'Display': [{'Hardware': 'qxl-vga', 'DynamicResolution': True, 'NativeResolution': False, 'DownscalingFilter': 'Linear', 'UpscalingFilter': 'Linear'}],
    'Drive': [
        {'Identifier': 'disk0', 'ImageName': 'linux.qcow2', 'ImageType': 'Disk', 'Interface': 'VirtIO', 'InterfaceVersion': 0, 'ReadOnly': False},
        {'Identifier': 'seed', 'ImageName': 'seed.iso', 'ImageType': 'CD', 'Interface': 'IDE', 'InterfaceVersion': 0, 'ReadOnly': True},
    ],
    'Input': {'MaximumUsbShare': 3, 'UsbBusSupport': '3.0', 'UsbSharing': False},
    'Network': [{'Hardware': 'virtio-net-pci', 'IsolateFromHost': False, 'MacAddress': '52:54:00:50:52:01', 'Mode': 'Emulated',
                 'PortForward': [{'Protocol': 'TCP', 'HostAddress': '127.0.0.1', 'HostPort': 2222, 'GuestAddress': '', 'GuestPort': 22}]}],
    'Serial': [], 'Sharing': {'ClipboardSharing': True, 'DirectoryShareMode': 'None', 'DirectoryShareReadOnly': True},
    'Sound': [{'Hardware': 'intel-hda'}],
}
with (bundle / 'config.plist').open('wb') as stream:
    plistlib.dump(config, stream)
print(bundle)
print(config['Information']['UUID'])
