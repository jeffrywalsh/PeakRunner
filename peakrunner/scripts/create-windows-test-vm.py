#!/usr/bin/env python3
"""Create a NEW Windows 11 ARM UTM test VM on AllOfIt, never overwrite one.

Requires the official ISO, mounted UTM guest tools and Homebrew qemu-img.
Windows licensing/activation remains the owner's responsibility.
"""
import hashlib
import os
import pathlib
import plistlib
import secrets
import shutil
import subprocess
import uuid
from xml.sax.saxutils import escape

root = pathlib.Path('/Volumes/AllOfIt/PeakRunner-VMs')
iso = root / 'Media/Win11_25H2_English_Arm64_v2.iso'
with iso.open('rb') as stream:
    assert hashlib.file_digest(stream, 'sha256').hexdigest() == '638aa2c88e94385b00f4f178d071e3df0b7d9e335577a83bd533b7f2eb65adf0'
bundle = root / 'PeakRunner Windows 11 ARM.utm'
assert not bundle.exists(), 'Refusing to overwrite an existing VM'
data = bundle / 'Data'
data.mkdir(parents=True)
subprocess.run(['qemu-img', 'create', '-f', 'qcow2', str(data / 'windows.qcow2'), '128G'], check=True)
os.link(iso, data / 'windows-install.iso')
seed = root / 'Windows-seed'
seed.mkdir(mode=0o700, exist_ok=False)
shutil.copyfile(pathlib.Path(__file__).resolve().parent.parent / 'deploy/setup-windows-qa.ps1', seed / 'setup-windows-qa.ps1')
password = secrets.token_urlsafe(24) + '9aA!'
credentials = root / 'windows-test-account.txt'
with credentials.open('x') as stream:
    os.chmod(credentials, 0o600)
    stream.write('Local VM account: tester\nPassword: ' + password + '\nWindows activation is not configured.\n')
tools = pathlib.Path('/Volumes/UTM Guest Tools')
shutil.copyfile(tools / 'utm-guest-tools-0.1.271.exe', seed / 'utm-guest-tools.exe')
for driver in ['viostor', 'vioscsi', 'vioserial', 'NetKVM', 'Balloon']:
    shutil.copytree(tools / 'Drivers' / driver / 'w11/ARM64', seed / 'Drivers' / driver)
attrs = 'processorArchitecture="arm64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS"'
driver_xml = ''.join(f'<PathAndCredentials wcm:action="add" wcm:keyValue="{n}"><Path>E:\\Drivers\\{d}</Path></PathAndCredentials>' for n, d in enumerate(['viostor', 'vioscsi', 'vioserial', 'NetKVM', 'Balloon'], 1))
xml = f'''<?xml version="1.0" encoding="utf-8"?>
<unattend xmlns="urn:schemas-microsoft-com:unattend" xmlns:wcm="http://schemas.microsoft.com/WMIConfig/2002/State">
<settings pass="windowsPE">
 <component name="Microsoft-Windows-International-Core-WinPE" {attrs}>
  <SetupUILanguage><UILanguage>en-US</UILanguage></SetupUILanguage><InputLocale>en-US</InputLocale><SystemLocale>en-US</SystemLocale><UILanguage>en-US</UILanguage><UserLocale>en-US</UserLocale>
 </component>
 <component name="Microsoft-Windows-Setup" {attrs}>
  <DiskConfiguration><Disk wcm:action="add"><DiskID>0</DiskID><WillWipeDisk>true</WillWipeDisk>
   <CreatePartitions>
    <CreatePartition wcm:action="add"><Order>1</Order><Type>EFI</Type><Size>260</Size></CreatePartition>
    <CreatePartition wcm:action="add"><Order>2</Order><Type>MSR</Type><Size>16</Size></CreatePartition>
    <CreatePartition wcm:action="add"><Order>3</Order><Type>Primary</Type><Extend>true</Extend></CreatePartition>
   </CreatePartitions>
   <ModifyPartitions>
    <ModifyPartition wcm:action="add"><Order>1</Order><PartitionID>1</PartitionID><Format>FAT32</Format><Label>System</Label></ModifyPartition>
    <ModifyPartition wcm:action="add"><Order>2</Order><PartitionID>3</PartitionID><Format>NTFS</Format><Label>Windows</Label><Letter>C</Letter></ModifyPartition>
   </ModifyPartitions>
  </Disk><WillShowUI>OnError</WillShowUI></DiskConfiguration>
  <ImageInstall><OSImage><InstallFrom><MetaData wcm:action="add"><Key>/IMAGE/NAME</Key><Value>Windows 11 Pro</Value></MetaData></InstallFrom><InstallTo><DiskID>0</DiskID><PartitionID>3</PartitionID></InstallTo><WillShowUI>OnError</WillShowUI></OSImage></ImageInstall>
  <UserData><AcceptEula>true</AcceptEula><FullName>PeakRunner QA</FullName><Organization>PeakRunner</Organization><ProductKey><WillShowUI>OnError</WillShowUI></ProductKey></UserData>
 </component>
 <component name="Microsoft-Windows-PnpCustomizationsWinPE" {attrs}><DriverPaths>{driver_xml}</DriverPaths></component>
</settings>
<settings pass="specialize"><component name="Microsoft-Windows-Shell-Setup" {attrs}><ComputerName>PEAKRUNNER-QA</ComputerName><TimeZone>Central Standard Time</TimeZone></component></settings>
<settings pass="oobeSystem">
 <component name="Microsoft-Windows-International-Core" {attrs}><InputLocale>en-US</InputLocale><SystemLocale>en-US</SystemLocale><UILanguage>en-US</UILanguage><UserLocale>en-US</UserLocale></component>
 <component name="Microsoft-Windows-Shell-Setup" {attrs}>
  <OOBE><HideEULAPage>true</HideEULAPage><HideOnlineAccountScreens>true</HideOnlineAccountScreens><HideWirelessSetupInOOBE>true</HideWirelessSetupInOOBE><ProtectYourPC>3</ProtectYourPC></OOBE>
  <UserAccounts><LocalAccounts><LocalAccount wcm:action="add"><Name>tester</Name><DisplayName>PeakRunner Tester</DisplayName><Group>Administrators</Group><Password><Value>{escape(password)}</Value><PlainText>true</PlainText></Password></LocalAccount></LocalAccounts></UserAccounts>
  <AutoLogon><Username>tester</Username><Enabled>true</Enabled><LogonCount>2</LogonCount><Password><Value>{escape(password)}</Value><PlainText>true</PlainText></Password></AutoLogon>
  <FirstLogonCommands><SynchronousCommand wcm:action="add"><Order>1</Order><Description>Install UTM guest tools</Description><CommandLine>cmd /c for %d in (D E F G H I J K) do @if exist %d:\\utm-guest-tools.exe start /wait "" %d:\\utm-guest-tools.exe /S</CommandLine></SynchronousCommand></FirstLogonCommands>
 </component>
</settings></unattend>
'''
(seed / 'Autounattend.xml').write_text(xml)
subprocess.run(['hdiutil', 'makehybrid', '-iso', '-joliet', '-default-volume-name', 'PEAKRUNNER_QA', '-o', str(data / 'setup.iso'), str(seed)], check=True)
with (root / 'PeakRunner Linux x64.utm/config.plist').open('rb') as stream:
    config = plistlib.load(stream)
config['Information'] = {'Name': 'PeakRunner Windows 11 ARM', 'UUID': str(uuid.uuid4()).upper(), 'Icon': 'windows', 'IconCustom': False, 'Notes': 'Windows 11 ARM64; tests Windows x64 client through Windows emulation. No accelerated GPU performance claims. Test account credentials in AllOfIt/PeakRunner-VMs/windows-test-account.txt. Owner supplies Windows license.'}
config['System'].update(Architecture='aarch64', CPU='host', CPUCount=4, MemorySize=8192, Target='virt')
config['QEMU'].update(Hypervisor=True, PS2Controller=False, RTCLocalTime=True, TPMDevice=True, BalloonDevice=False)
config['QEMU']['AdditionalArguments'] = []
config['Display'][0]['Hardware'] = 'virtio-ramfb'
config['Network'][0].update(MacAddress='52:54:00:50:52:02', PortForward=[])
config['Drive'] = [
 {'Identifier': 'wininstall', 'ImageName': 'windows-install.iso', 'ImageType': 'CD', 'Interface': 'USB', 'InterfaceVersion': 0, 'ReadOnly': True},
 {'Identifier': 'winsetup', 'ImageName': 'setup.iso', 'ImageType': 'CD', 'Interface': 'USB', 'InterfaceVersion': 0, 'ReadOnly': True},
 {'Identifier': 'windisk', 'ImageName': 'windows.qcow2', 'ImageType': 'Disk', 'Interface': 'NVMe', 'InterfaceVersion': 0, 'ReadOnly': False},
]
config['Sound'] = [{'Hardware': 'intel-hda'}]
with (bundle / 'config.plist').open('wb') as stream:
    plistlib.dump(config, stream)
print(bundle)
print(config['Information']['UUID'])
