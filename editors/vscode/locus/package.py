#!/usr/bin/env python3
"""Builds the .vsix for the Locus extension without npm, and installs it.

    python3 editors/vscode/locus/package.py            writes locus-language-<version>.vsix beside this file
    python3 editors/vscode/locus/package.py --install  also installs it into VS Code (and Cursor if present)

A .vsix is a zip with a manifest, a content-types file, and the extension
folder under extension/. Installing this way is what VS Code expects; a
folder linked into ~/.vscode/extensions is registered once and then treated
as stale, which is how the first attempt failed.
"""
import json
import pathlib
import shutil
import subprocess
import sys
import zipfile

HERE = pathlib.Path(__file__).resolve().parent
FILES = ["package.json", "language-configuration.json", "README.md", "syntaxes/locus.tmLanguage.json", "syntaxes/locus.markdown.tmLanguage.json"]


def build():
    pkg = json.loads((HERE / "package.json").read_text())
    out = HERE / ("%s-%s.vsix" % (pkg["name"], pkg["version"]))
    manifest = """<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011" xmlns:d="http://schemas.microsoft.com/developer/vsx-schema-design/2011">
  <Metadata>
    <Identity Language="en-US" Id="%(name)s" Version="%(version)s" Publisher="%(publisher)s"/>
    <DisplayName>%(displayName)s</DisplayName>
    <Description xml:space="preserve">%(description)s</Description>
    <Tags>locus,lc</Tags>
    <Categories>Programming Languages</Categories>
    <Properties>
      <Property Id="Microsoft.VisualStudio.Code.Engine" Value="%(engine)s"/>
      <Property Id="Microsoft.VisualStudio.Code.ExtensionKind" Value="ui,workspace"/>
    </Properties>
  </Metadata>
  <Installation><InstallationTarget Id="Microsoft.VisualStudio.Code"/></Installation>
  <Dependencies/>
  <Assets><Asset Type="Microsoft.VisualStudio.Code.Manifest" Path="extension/package.json" Addressable="true"/></Assets>
</PackageManifest>
""" % dict(pkg, engine=pkg["engines"]["vscode"])
    types = """<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="json" ContentType="application/json"/>
  <Default Extension="md" ContentType="text/markdown"/>
  <Default Extension="vsixmanifest" ContentType="text/xml"/>
</Types>
"""
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("extension.vsixmanifest", manifest)
        z.writestr("[Content_Types].xml", types)
        for name in FILES:
            z.write(HERE / name, "extension/" + name)
    print("built", out)
    return out


def main():
    out = build()
    if "--install" in sys.argv:
        for editor in ("code", "cursor"):
            if shutil.which(editor):
                subprocess.run([editor, "--install-extension", str(out), "--force"], check=False)
        print("reload the editor window to pick it up")


if __name__ == "__main__":
    main()
