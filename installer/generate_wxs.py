#!/usr/bin/env python3
"""Generate deterministic WiX v5 Package.wxs from staged Throne-Rust payloads."""

import argparse
import hashlib
import os
import sys
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path

NAMESPACE_THRONE_RUST = uuid.UUID("a4b9c8d2-1e34-4f5a-8b7c-901234567890")
UPGRADE_CODE = "7D4E89B2-4F1A-4D7E-9A8B-5E6C3F1A8D02"


def stable_guid(tag: str) -> str:
    return str(uuid.uuid5(NAMESPACE_THRONE_RUST, f"throne-rust:{tag}")).upper()


def safe_id(prefix: str, rel: str) -> str:
    digest = hashlib.sha1(rel.encode("utf-8")).hexdigest()[:10]
    cleaned = "".join(c if (c.isalnum() or c == "_") else "_" for c in rel)
    if len(cleaned) > 36:
        cleaned = cleaned[-36:]
    return f"{prefix}_{cleaned}_{digest}"


def add_dir_tree(parent_elem: ET.Element, src_dir: Path, rel_prefix: str, comp_ids: list[str]) -> None:
    entries = sorted(src_dir.iterdir(), key=lambda p: (not p.is_file(), p.name.lower()))
    for entry in entries:
        rel = f"{rel_prefix}/{entry.name}" if rel_prefix else entry.name
        if entry.is_file():
            comp_id = safe_id("Cmp", rel)
            file_id = safe_id("Fil", rel)
            comp = ET.SubElement(
                parent_elem,
                "Component",
                Id=comp_id,
                Guid=stable_guid(rel),
                Bitness="always64",
            )
            file_attrs = {
                "Id": file_id,
                "Source": str(entry.resolve()),
                "KeyPath": "yes",
            }
            if rel.lower() == "throne.exe":
                file_attrs["Id"] = "ThroneMainExe"
            ET.SubElement(comp, "File", **file_attrs)
            comp_ids.append(comp_id)
        elif entry.is_dir():
            dir_id = safe_id("Dir", rel)
            sub_dir = ET.SubElement(parent_elem, "Directory", Id=dir_id, Name=entry.name)
            add_dir_tree(sub_dir, entry, rel, comp_ids)


def build_wxs(stage_dir: Path, out_wxs: Path, version: str, icon_path: Path, license_path: Path) -> None:
    ET.register_namespace("", "http://wixtoolset.org/schemas/v4/wxs")
    ET.register_namespace("ui", "http://wixtoolset.org/schemas/v4/wxs/ui")

    wix = ET.Element("{http://wixtoolset.org/schemas/v4/wxs}Wix")

    pkg = ET.SubElement(
        wix,
        "Package",
        Name="Throne-Rust",
        Manufacturer="IMAN-Py / Throne Contributors",
        Version=version,
        UpgradeCode=UPGRADE_CODE,
        Scope="perMachine",
        InstallerVersion="500",
        Language="1033",
    )

    ET.SubElement(
        pkg,
        "MajorUpgrade",
        DowngradeErrorMessage="A newer version of [ProductName] is already installed.",
        AllowSameVersionUpgrades="yes",
    )
    ET.SubElement(pkg, "MediaTemplate", EmbedCab="yes", CompressionLevel="high")
    ET.SubElement(pkg, "Icon", Id="ThroneIcon", SourceFile=str(icon_path.resolve()))
    ET.SubElement(pkg, "Property", Id="ARPPRODUCTICON", Value="ThroneIcon")
    ET.SubElement(pkg, "Property", Id="ARPURLINFOABOUT", Value="https://github.com/IMAN-Py/Throne-Rust")
    ET.SubElement(pkg, "Property", Id="ARPHELPLINK", Value="https://github.com/IMAN-Py/Throne-Rust/issues")
    ET.SubElement(pkg, "WixVariable", Id="WixUILicenseRtf", Value=str(license_path.resolve()))
    ET.SubElement(pkg, "{http://wixtoolset.org/schemas/v4/wxs/ui}WixUI", Id="WixUI_InstallDir", InstallDirectory="INSTALLFOLDER")

    prog_files = ET.SubElement(pkg, "StandardDirectory", Id="ProgramFiles64Folder")
    install_dir = ET.SubElement(prog_files, "Directory", Id="INSTALLFOLDER", Name="Throne-Rust")

    comp_ids: list[str] = []
    add_dir_tree(install_dir, stage_dir, "", comp_ids)

    # Start Menu shortcut
    pm_dir = ET.SubElement(pkg, "StandardDirectory", Id="ProgramMenuFolder")
    app_menu = ET.SubElement(pm_dir, "Directory", Id="AppMenuFolder", Name="Throne-Rust")
    sm_cmp = ET.SubElement(app_menu, "Component", Id="CmpStartMenuShortcut", Guid=stable_guid("shortcut:startmenu"))
    ET.SubElement(
        sm_cmp,
        "Shortcut",
        Id="StartMenuShortcut",
        Name="Throne",
        Description="Cross-platform GUI proxy utility powered by sing-box and Rust",
        Target="[INSTALLFOLDER]Throne.exe",
        WorkingDirectory="INSTALLFOLDER",
        Icon="ThroneIcon",
    )
    ET.SubElement(sm_cmp, "RemoveFolder", Id="CleanupMenuFolder", On="uninstall")
    ET.SubElement(
        sm_cmp,
        "RegistryValue",
        Root="HKCU",
        Key="Software\\Throne-Rust\\Shortcuts",
        Name="startmenu",
        Type="integer",
        Value="1",
        KeyPath="yes",
    )
    comp_ids.append("CmpStartMenuShortcut")

    # Desktop shortcut
    dt_dir = ET.SubElement(pkg, "StandardDirectory", Id="DesktopFolder")
    dt_cmp = ET.SubElement(dt_dir, "Component", Id="CmpDesktopShortcut", Guid=stable_guid("shortcut:desktop"))
    ET.SubElement(
        dt_cmp,
        "Shortcut",
        Id="DesktopShortcut",
        Name="Throne",
        Description="Launch Throne-Rust",
        Target="[INSTALLFOLDER]Throne.exe",
        WorkingDirectory="INSTALLFOLDER",
        Icon="ThroneIcon",
    )
    ET.SubElement(
        dt_cmp,
        "RegistryValue",
        Root="HKCU",
        Key="Software\\Throne-Rust\\Shortcuts",
        Name="desktop",
        Type="integer",
        Value="1",
        KeyPath="yes",
    )
    comp_ids.append("CmpDesktopShortcut")

    feat = ET.SubElement(
        pkg,
        "Feature",
        Id="MainFeature",
        Title="Throne-Rust",
        Description="Throne GUI, Core, Rust Acceleration Library, and Dashboard",
        Level="1",
        AllowAbsent="no",
    )
    for cid in comp_ids:
        ET.SubElement(feat, "ComponentRef", Id=cid)

    tree = ET.ElementTree(wix)
    ET.indent(tree, space="  ")
    out_wxs.parent.mkdir(parents=True, exist_ok=True)
    tree.write(out_wxs, encoding="utf-8", xml_declaration=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--version", default="1.4.2.1")
    parser.add_argument("--icon", required=True, type=Path)
    parser.add_argument("--license", required=True, type=Path)
    args = parser.parse_args()

    if not args.stage.is_dir():
        print(f"Stage directory missing: {args.stage}", file=sys.stderr)
        return 1
    if not (args.stage / "Throne.exe").is_file():
        print(f"Throne.exe missing from stage: {args.stage}", file=sys.stderr)
        return 1

    build_wxs(args.stage, args.out, args.version, args.icon, args.license)
    print(f"Generated WiX manifest: {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
