#!/usr/bin/env python3
"""
generate_wxs.py
Automated stage scanner and deterministic WiX v5 Package.wxs generator for Throne.

Supports relative staging paths with portable $(var.StageDir) variable resolution.
"""

import os
import sys
import uuid
import re
import argparse

def clean_id(s):
    # Valid WiX ID: [A-Za-z_][A-Za-z0-9_.]*
    s = re.sub(r"[^A-Za-z0-9_.]", "_", s)
    if s and s[0].isdigit():
        s = "_" + s
    return s[:64]

def generate_wxs(stage_dir="stage", output_wxs="Package.wxs", version="1.4.2.0", upgrade_code="29950A94-3C8D-4043-9E00-36AB69F78042"):
    # Deterministic namespace for consistent component GUIDs across builds
    NS = uuid.UUID(upgrade_code.lower())

    # Canonical directory map
    dirs_map = {
        ".": ("INSTALLFOLDER", []),
        os.path.join("config", "dashboard"): ("DashboardFolder", []),
        os.path.join("config", "sb-dashboard"): ("SbDashboardFolder", []),
        os.path.join("config", "sb-dashboard", "assets"): ("SbAssetsFolder", []),
        os.path.join("config", "sb-dashboard", "licenses"): ("SbLicensesFolder", []),
    }

    if not os.path.exists(stage_dir):
        print(f"Error: Staging directory not found: {stage_dir}", file=sys.stderr)
        sys.exit(1)

    for root, dirs, files in os.walk(stage_dir):
        rel = os.path.relpath(root, stage_dir)
        if rel in dirs_map:
            folder_id, file_list = dirs_map[rel]
            for f in sorted(files):
                file_list.append((f, os.path.join(rel, f) if rel != "." else f))

    guid_shortcut = str(uuid.uuid5(NS, "ApplicationShortcut"))
    guid_desktop = str(uuid.uuid5(NS, "DesktopShortcut"))
    guid_reg = str(uuid.uuid5(NS, "InstallPathRegistry"))

    wxs = f"""<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs"
     xmlns:ui="http://wixtoolset.org/schemas/v4/wxs/ui"
     xmlns:util="http://wixtoolset.org/schemas/v4/wxs/util"
     xmlns:firewall="http://wixtoolset.org/schemas/v4/wxs/firewall">

  <?ifndef StageDir?>
    <?define StageDir = "stage"?>
  <?endif?>

  <Package Name="Throne"
           Manufacturer="Throne Project"
           Version="{version}"
           UpgradeCode="{upgrade_code.upper()}"
           Scope="perMachine"
           Language="1033"
           InstallerVersion="500">

    <MajorUpgrade DowngradeErrorMessage="A newer version of [ProductName] is already installed." AllowSameVersionUpgrades="yes" />

    <MediaTemplate EmbedCab="yes" CompressionLevel="high" />

    <Icon Id="ThroneIcon" SourceFile="Throne.ico" />
    <Property Id="ARPPRODUCTICON" Value="ThroneIcon" />
    <Property Id="ARPHELPLINK" Value="https://github.com/throneproj/Throne" />
    <Property Id="ARPURLINFOABOUT" Value="https://github.com/throneproj/Throne" />
    <Property Id="ARPCOMMENTS" Value="Cross-platform GUI proxy utility empowered by sing-box" />

    <!-- Close running processes on install/upgrade -->
    <util:CloseApplication Id="CloseThrone" Target="Throne.exe" CloseMessage="yes" RebootPrompt="no" PromptToContinue="yes" Description="Throne is currently running and must be closed to proceed." />
    <util:CloseApplication Id="CloseThroneCore" Target="ThroneCore.exe" CloseMessage="yes" RebootPrompt="no" PromptToContinue="yes" Description="Throne Core is currently running and must be closed to proceed." />

    <!-- Directory Structure -->
    <StandardDirectory Id="ProgramFiles64Folder">
      <Directory Id="INSTALLFOLDER" Name="Throne">
        <Directory Id="ConfigFolder" Name="config">
          <Directory Id="DashboardFolder" Name="dashboard" />
          <Directory Id="SbDashboardFolder" Name="sb-dashboard">
            <Directory Id="SbAssetsFolder" Name="assets" />
            <Directory Id="SbLicensesFolder" Name="licenses" />
          </Directory>
        </Directory>
      </Directory>
    </StandardDirectory>

    <StandardDirectory Id="ProgramMenuFolder">
      <Component Id="ApplicationShortcutComponent" Guid="{guid_shortcut}">
        <Shortcut Id="ApplicationStartMenuShortcut"
                  Name="Throne"
                  Description="Cross-platform GUI proxy utility"
                  Target="[!ThroneExe]"
                  WorkingDirectory="INSTALLFOLDER"
                  Icon="ThroneIcon" />
        <RegistryValue Root="HKCU" Key="Software\\Throne" Name="StartMenuShortcut" Type="integer" Value="1" KeyPath="yes" />
      </Component>
    </StandardDirectory>

    <StandardDirectory Id="DesktopFolder">
      <Component Id="ApplicationDesktopShortcutComponent" Guid="{guid_desktop}">
        <Shortcut Id="ApplicationDesktopShortcut"
                  Name="Throne"
                  Description="Cross-platform GUI proxy utility"
                  Target="[!ThroneExe]"
                  WorkingDirectory="INSTALLFOLDER"
                  Icon="ThroneIcon" />
        <RegistryValue Root="HKCU" Key="Software\\Throne" Name="DesktopShortcut" Type="integer" Value="1" KeyPath="yes" />
      </Component>
    </StandardDirectory>

    <!-- Components -->
    <Component Id="InstallPathRegistryComponent" Directory="INSTALLFOLDER" Guid="{guid_reg}">
      <RegistryKey Root="HKLM" Key="Software\\Throne">
        <RegistryValue Name="InstallPath" Type="string" Value="[INSTALLFOLDER]" KeyPath="yes" />
        <RegistryValue Name="Version" Type="string" Value="{version}" />
        <RegistryValue Name="Executable" Type="string" Value="[INSTALLFOLDER]Throne.exe" />
      </RegistryKey>
    </Component>
"""

    components_list = [
        "ApplicationShortcutComponent",
        "ApplicationDesktopShortcutComponent",
        "InstallPathRegistryComponent"
    ]

    # Process file components
    total_files = 0
    for rel_dir, (folder_id, file_list) in dirs_map.items():
        wxs += f"\n    <!-- Folder: {rel_dir} ({folder_id}) -->\n"
        for fname, rel_path in file_list:
            total_files += 1
            comp_id = clean_id(f"cmp_{folder_id}_{fname}")
            file_id = clean_id(f"fil_{folder_id}_{fname}")
            comp_guid = str(uuid.uuid5(NS, f"{rel_dir}/{fname}"))
            components_list.append(comp_id)

            # Portable StageDir reference
            win_rel_path = rel_path.replace("/", "\\")
            src_attr = f"$(var.StageDir)\\{win_rel_path}"

            if fname == "Throne.exe":
                file_id = "ThroneExe"
                wxs += f"""    <Component Id="{comp_id}" Directory="{folder_id}" Guid="{comp_guid}">
      <File Id="{file_id}" Source="{src_attr}" KeyPath="yes">
        <firewall:FirewallException Id="ThroneFw" Name="Throne" Scope="any" IgnoreFailure="yes" />
      </File>
    </Component>\n"""
            elif fname == "ThroneCore.exe":
                file_id = "ThroneCoreExe"
                wxs += f"""    <Component Id="{comp_id}" Directory="{folder_id}" Guid="{comp_guid}">
      <File Id="{file_id}" Source="{src_attr}" KeyPath="yes">
        <firewall:FirewallException Id="ThroneCoreFw" Name="Throne Core (sing-box)" Scope="any" IgnoreFailure="yes" />
      </File>
    </Component>\n"""
            else:
                wxs += f"""    <Component Id="{comp_id}" Directory="{folder_id}" Guid="{comp_guid}">
      <File Id="{file_id}" Source="{src_attr}" KeyPath="yes" />
    </Component>\n"""

    # Features
    wxs += """
    <!-- Features -->
    <Feature Id="MainFeature" Title="Throne Application" Level="1" Display="expand" ConfigurableDirectory="INSTALLFOLDER">
"""

    for cid in components_list:
        wxs += f'      <ComponentRef Id="{cid}" />\n'

    wxs += """    </Feature>

    <!-- Setup UI -->
    <ui:WixUI Id="WixUI_InstallDir" InstallDirectory="INSTALLFOLDER" />
    <WixVariable Id="WixUILicenseRtf" Value="License.rtf" />

    <!-- Launch on Exit -->
    <Property Id="WIXUI_EXITDIALOGOPTIONALCHECKBOXTEXT" Value="Launch Throne now" />
    <Property Id="WIXUI_EXITDIALOGOPTIONALCHECKBOX" Value="1" />
    <Property Id="WixShellExecTarget" Value="[#ThroneExe]" />
    <CustomAction Id="LaunchApplication" DllEntry="WixShellExec" Impersonate="yes" BinaryRef="Wix4UtilCA_X64" />

    <UI>
      <Publish Dialog="ExitDialog" Control="Finish" Event="DoAction" Value="LaunchApplication" Condition="WIXUI_EXITDIALOGOPTIONALCHECKBOX = 1 and NOT Installed" />
    </UI>

  </Package>
</Wix>
"""

    with open(output_wxs, "w", encoding="utf-8") as f:
        f.write(wxs)

    print(f"Generated {output_wxs}: {total_files} files mapped, {len(components_list)} components configured.")
    return len(components_list), total_files

def main():
    parser = argparse.ArgumentParser(description="Generate WiX v5 Package.wxs for Throne")
    parser.add_argument("--stage-dir", default="stage", help="Path to staging directory containing build artifacts (default: stage)")
    parser.add_argument("--output", default="Package.wxs", help="Output WXS file path (default: Package.wxs)")
    parser.add_argument("--version", default="1.4.2.0", help="Product version (default: 1.4.2.0)")
    parser.add_argument("--upgrade-code", default="29950A94-3C8D-4043-9E00-36AB69F78042", help="MSI UpgradeCode GUID")

    args = parser.parse_args()
    generate_wxs(stage_dir=args.stage_dir, output_wxs=args.output, version=args.version, upgrade_code=args.upgrade_code)

if __name__ == "__main__":
    main()
