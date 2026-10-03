# Deterministic WiX authoring for an already verified development stage.
# Dot-sourcing this file performs no filesystem mutation.
Set-StrictMode -Version Latest

function Get-MoWixDigest([string]$Kind, [string]$Name) {
    Assert-MoRelativeName $Name
    $bytes = [Text.Encoding]::UTF8.GetBytes("mo-wix-payload-v1`0$Kind`0$Name")
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes))
}

function Get-MoWixDirectoryId([string]$Name) {
    return 'D_' + (Get-MoWixDigest 'directory' $Name).Substring(0, 24)
}

function Get-MoWixComponentId([string]$Name) {
    switch -CaseSensitive ($Name) {
        'bin/mo-broker.exe' { return 'BrokerX64Component' }
        'bin/mo-settings.exe' { return 'SettingsX64Component' }
        'bin/mo-tip-registrar.exe' { return 'RegistrarX64Component' }
        'tip/x64/mo-tip.dll' { return 'TipX64Component' }
        'tip/x86/mo-tip.dll' { return 'TipX86Component' }
        default { return 'C_' + (Get-MoWixDigest 'component' $Name).Substring(0, 24) }
    }
}

function Get-MoWixFileId([string]$Name) {
    switch -CaseSensitive ($Name) {
        'bin/mo-broker.exe' { return 'BrokerX64File' }
        'bin/mo-settings.exe' { return 'SettingsX64File' }
        'bin/mo-tip-registrar.exe' { return 'RegistrarX64File' }
        'tip/x64/mo-tip.dll' { return 'TipX64File' }
        'tip/x86/mo-tip.dll' { return 'TipX86File' }
        'runtime/librime/rime.dll' { return 'RuntimeRimeFile' }
        default { return 'F_' + (Get-MoWixDigest 'file' $Name).Substring(0, 24) }
    }
}

function Get-MoWixComponentGuid([string]$Name) {
    $characters = (Get-MoWixDigest 'guid' $Name).Substring(0, 32).ToCharArray()
    # Mark the deterministic value as an RFC 4122 version-5/variant-1 UUID.
    $characters[12] = '5'
    $characters[16] = '8'
    $hex = -join $characters
    return ('{{{0}-{1}-{2}-{3}-{4}}}' -f
        $hex.Substring(0, 8), $hex.Substring(8, 4), $hex.Substring(12, 4),
        $hex.Substring(16, 4), $hex.Substring(20, 12)).ToUpperInvariant()
}

function New-MoWixTreeNode {
    return [ordered]@{
        directories = [Collections.Generic.SortedDictionary[string, object]]::new([StringComparer]::Ordinal)
        files = [Collections.Generic.List[string]]::new()
    }
}

function Add-MoWixTreeFile([Collections.IDictionary]$Root, [string]$Name) {
    Assert-MoRelativeName $Name
    $parts = $Name.Split('/')
    $node = $Root
    for ($index = 0; $index -lt $parts.Length - 1; $index++) {
        $part = $parts[$index]
        if (-not $node.directories.ContainsKey($part)) {
            $node.directories.Add($part, (New-MoWixTreeNode))
        }
        $node = $node.directories[$part]
    }
    $node.files.Add($Name)
}

function Write-MoWixComponent([Xml.XmlWriter]$Writer, [string]$Name) {
    $componentId = Get-MoWixComponentId $Name
    $fileId = Get-MoWixFileId $Name
    $Writer.WriteStartElement('Component')
    $Writer.WriteAttributeString('Id', $componentId)
    $Writer.WriteAttributeString('Guid', (Get-MoWixComponentGuid $Name))
    # Every payload file lives below ProgramFiles64Folder, including the x86 DLL.
    # Its separate 32-bit registry-only component is emitted under ProgramFilesFolder.
    $Writer.WriteAttributeString('Bitness', 'always64')
    $Writer.WriteStartElement('File')
    $Writer.WriteAttributeString('Id', $fileId)
    $Writer.WriteAttributeString('Source', ('$(var.StagePayload)\' + $Name.Replace('/', '\')))
    $Writer.WriteAttributeString('Name', $Name.Substring($Name.LastIndexOf('/') + 1))
    $Writer.WriteAttributeString('KeyPath', 'yes')
    if ($Name -ceq 'bin/mo-settings.exe') {
        $Writer.WriteStartElement('Shortcut')
        $Writer.WriteAttributeString('Id', 'MoSettingsStartMenuShortcut')
        $Writer.WriteAttributeString('Directory', 'ProgramMenuFolder')
        $Writer.WriteAttributeString('Name', 'Mo (墨) 输入法设置')
        $Writer.WriteAttributeString('Description', '设置 Mo (墨) 输入法')
        $Writer.WriteAttributeString('Advertise', 'yes')
        $Writer.WriteEndElement()
    }
    $Writer.WriteEndElement()

    if ($Name -ceq 'tip/x64/mo-tip.dll') {
        $Writer.WriteStartElement('RegistryKey')
        $Writer.WriteAttributeString('Root', 'HKLM')
        $Writer.WriteAttributeString('Key', 'Software\Classes\CLSID\{B4911146-2A27-47AA-9D12-109B6AE10A70}\InprocServer32')
        $Writer.WriteStartElement('RegistryValue')
        $Writer.WriteAttributeString('Type', 'string')
        $Writer.WriteAttributeString('Value', "[#$fileId]")
        $Writer.WriteEndElement()
        $Writer.WriteStartElement('RegistryValue')
        $Writer.WriteAttributeString('Name', 'ThreadingModel')
        $Writer.WriteAttributeString('Type', 'string')
        $Writer.WriteAttributeString('Value', 'Apartment')
        $Writer.WriteEndElement()
        $Writer.WriteEndElement()
    }
    $Writer.WriteEndElement()
}

function Write-MoWixDirectoryNode(
    [Xml.XmlWriter]$Writer,
    [Collections.IDictionary]$Node,
    [string]$RelativeDirectory
) {
    foreach ($name in $Node.files) { Write-MoWixComponent $Writer $name }
    foreach ($entry in $Node.directories.GetEnumerator()) {
        $relative = if ($RelativeDirectory) { "$RelativeDirectory/$($entry.Key)" } else { $entry.Key }
        $Writer.WriteStartElement('Directory')
        $Writer.WriteAttributeString('Id', (Get-MoWixDirectoryId $relative))
        $Writer.WriteAttributeString('Name', $entry.Key)
        Write-MoWixDirectoryNode $Writer $entry.Value $relative
        $Writer.WriteEndElement()
    }
}

function New-MoWixPayloadFragment([string]$StageDirectory, [string]$OutputPath) {
    $stage = Assert-MoPlainPath $StageDirectory
    $null = Assert-MoPreparedStage $stage
    $payload = Join-Path $stage 'payload/Mo'
    $names = @(Get-MoStageFiles $payload)
    $output = [IO.Path]::GetFullPath($OutputPath)
    if (-not [IO.Path]::IsPathFullyQualified($OutputPath) -or [IO.Path]::GetExtension($output) -cne '.wxs') {
        throw 'Generated WiX output must be an absolute .wxs path.'
    }
    $parent = Assert-MoPlainPath (Split-Path -Parent $output)
    if ($output.StartsWith($stage + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Generated WiX output must not modify the verified stage.'
    }
    if (Test-Path -LiteralPath $output) {
        $item = Get-Item -LiteralPath $output -Force
        if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw 'Generated WiX output target must be a plain file.'
        }
    }

    $tree = New-MoWixTreeNode
    foreach ($name in $names) { Add-MoWixTreeFile $tree $name }
    $stream = [IO.MemoryStream]::new()
    $settings = [Xml.XmlWriterSettings]::new()
    $settings.Encoding = [Text.UTF8Encoding]::new($false)
    $settings.Indent = $true
    $settings.NewLineChars = "`n"
    $settings.NewLineHandling = [Xml.NewLineHandling]::Replace
    $writer = [Xml.XmlWriter]::Create($stream, $settings)
    try {
        $writer.WriteStartDocument()
        $writer.WriteStartElement('Wix', 'http://wixtoolset.org/schemas/v4/wxs')
        $manifestHash = (Get-FileHash -LiteralPath (Join-Path $stage 'mo-stage.json') -Algorithm SHA256).Hash
        $writer.WriteComment(" Generated from verified development stage $manifestHash; $($names.Count) payload files. ")
        $writer.WriteStartElement('Fragment')
        $writer.WriteStartElement('StandardDirectory')
        $writer.WriteAttributeString('Id', 'ProgramFiles64Folder')
        $writer.WriteStartElement('Directory')
        $writer.WriteAttributeString('Id', 'INSTALLFOLDER')
        $writer.WriteAttributeString('Name', 'Mo')
        Write-MoWixDirectoryNode $writer $tree ''
        $writer.WriteEndElement()
        $writer.WriteEndElement()

        # Advertised Start-menu entry targets the settings file component and
        # requires no per-user registry key path in this per-machine package.
        $writer.WriteStartElement('StandardDirectory')
        $writer.WriteAttributeString('Id', 'ProgramMenuFolder')
        $writer.WriteEndElement()

        # A 32-bit component cannot use ProgramFiles64Folder (ICE80), even when
        # its only payload is registry data. The x86 DLL stays in the 64-bit
        # install tree; use its directory property instead of a cross-component
        # [#file] reference (ICE69, including repair when the file is unchanged).
        $writer.WriteStartElement('StandardDirectory')
        $writer.WriteAttributeString('Id', 'ProgramFilesFolder')
        $writer.WriteStartElement('Component')
        $writer.WriteAttributeString('Id', 'TipX86ComRegistryComponent')
        $writer.WriteAttributeString('Guid', (Get-MoWixComponentGuid 'tip/x86/mo-tip.com-registry'))
        $writer.WriteAttributeString('Bitness', 'always32')
        $writer.WriteStartElement('RegistryKey')
        $writer.WriteAttributeString('Root', 'HKLM')
        $writer.WriteAttributeString('Key', 'Software\Classes\CLSID\{B4911146-2A27-47AA-9D12-109B6AE10A70}\InprocServer32')
        $writer.WriteStartElement('RegistryValue')
        $writer.WriteAttributeString('Type', 'string')
        $writer.WriteAttributeString('Value', '[INSTALLFOLDER]tip\x86\mo-tip.dll')
        $writer.WriteAttributeString('KeyPath', 'yes')
        $writer.WriteEndElement()
        $writer.WriteStartElement('RegistryValue')
        $writer.WriteAttributeString('Name', 'ThreadingModel')
        $writer.WriteAttributeString('Type', 'string')
        $writer.WriteAttributeString('Value', 'Apartment')
        $writer.WriteEndElement()
        $writer.WriteEndElement()
        $writer.WriteEndElement()
        $writer.WriteEndElement()

        $writer.WriteStartElement('ComponentGroup')
        $writer.WriteAttributeString('Id', 'MoPayloadComponents')
        foreach ($name in $names) {
            $writer.WriteStartElement('ComponentRef')
            $writer.WriteAttributeString('Id', (Get-MoWixComponentId $name))
            $writer.WriteEndElement()
        }
        $writer.WriteStartElement('ComponentRef')
        $writer.WriteAttributeString('Id', 'TipX86ComRegistryComponent')
        $writer.WriteEndElement()
        $writer.WriteEndElement()
        $writer.WriteEndElement()
        $writer.WriteEndElement()
        $writer.WriteEndDocument()
    } finally {
        $writer.Dispose()
    }
    $bytes = $stream.ToArray()
    $stream.Dispose()
    [IO.File]::WriteAllBytes((Join-Path $parent (Split-Path -Leaf $output)), $bytes)
    Assert-MoWixPayloadFragment $stage $output
    return $output
}

function Read-MoWixDocument([string]$Path) {
    $full = Assert-MoPlainPath $Path
    $settings = [Xml.XmlReaderSettings]::new()
    $settings.DtdProcessing = [Xml.DtdProcessing]::Prohibit
    $settings.XmlResolver = $null
    $reader = [Xml.XmlReader]::Create($full, $settings)
    $document = [Xml.XmlDocument]::new()
    $document.XmlResolver = $null
    try { $document.Load($reader) } finally { $reader.Dispose() }
    return $document
}

function Assert-MoWixPayloadFragment([string]$StageDirectory, [string]$FragmentPath) {
    $stage = Assert-MoPlainPath $StageDirectory
    $null = Assert-MoPreparedStage $stage
    $expected = @(Get-MoStageFiles (Join-Path $stage 'payload/Mo'))
    $document = Read-MoWixDocument $FragmentPath
    $namespace = [Xml.XmlNamespaceManager]::new($document.NameTable)
    $namespace.AddNamespace('w', 'http://wixtoolset.org/schemas/v4/wxs')
    $files = @($document.SelectNodes('//w:File', $namespace))
    $components = @($document.SelectNodes('//w:Component', $namespace))
    $references = @($document.SelectNodes('//w:ComponentGroup[@Id="MoPayloadComponents"]/w:ComponentRef', $namespace))
    $installFolders = @($document.SelectNodes('//w:Directory[@Id="INSTALLFOLDER"]', $namespace))
    $installRoot = $document.SelectSingleNode(
        '/w:Wix/w:Fragment/w:StandardDirectory[@Id="ProgramFiles64Folder"]/w:Directory[@Id="INSTALLFOLDER" and @Name="Mo"]',
        $namespace)
    $programMenu = $document.SelectSingleNode(
        '/w:Wix/w:Fragment/w:StandardDirectory[@Id="ProgramMenuFolder"]',
        $namespace)
    if ($files.Count -ne $expected.Count -or $components.Count -ne ($expected.Count + 1) -or
        $references.Count -ne ($expected.Count + 1)) {
        throw 'Generated WiX payload count mismatch.'
    }
    if ($installFolders.Count -ne 1 -or $null -eq $installRoot -or $null -eq $programMenu) {
        throw 'Generated WiX install-root contract mismatch.'
    }
    $seenFiles = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seenComponents = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seenReferences = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($reference in $references) {
        if (-not $seenReferences.Add($reference.Id)) { throw 'Duplicate generated WiX component reference.' }
    }
    foreach ($file in $files) {
        $component = $file.ParentNode
        $parts = [Collections.Generic.List[string]]::new()
        $directory = $component.ParentNode
        while ($null -ne $directory -and $directory.LocalName -eq 'Directory' -and
            $directory.GetAttribute('Id') -cne 'INSTALLFOLDER') {
            $parts.Insert(0, $directory.GetAttribute('Name'))
            $directory = $directory.ParentNode
        }
        if ($null -eq $directory -or $directory.GetAttribute('Id') -cne 'INSTALLFOLDER') { throw 'Generated WiX file escaped INSTALLFOLDER.' }
        $parts.Add($file.GetAttribute('Name'))
        $name = $parts -join '/'
        Assert-MoRelativeName $name
        if ($name -cnotin $expected -or -not $seenFiles.Add($name)) { throw 'Unexpected/duplicate generated WiX payload file.' }
        $componentId = Get-MoWixComponentId $name
        if ($component.GetAttribute('Id') -cne $componentId -or
            $file.GetAttribute('Id') -cne (Get-MoWixFileId $name) -or
            $component.GetAttribute('Guid') -cne (Get-MoWixComponentGuid $name) -or
            $file.GetAttribute('KeyPath') -cne 'yes' -or
            $file.GetAttribute('Source') -cne ('$(var.StagePayload)\' + $name.Replace('/', '\')) -or
            $component.GetAttribute('Bitness') -cne 'always64' -or
            -not $seenComponents.Add($componentId) -or -not $seenReferences.Contains($componentId)) {
            throw 'Generated WiX file/component contract mismatch.'
        }
        $registryKeys = @($component.SelectNodes('w:RegistryKey', $namespace))
        if ($name -ceq 'tip/x64/mo-tip.dll') {
            if ($registryKeys.Count -ne 1 -or $registryKeys[0].GetAttribute('Root') -cne 'HKLM' -or
                $registryKeys[0].GetAttribute('Key') -cne 'Software\Classes\CLSID\{B4911146-2A27-47AA-9D12-109B6AE10A70}\InprocServer32') {
                throw 'Generated WiX COM registration contract mismatch.'
            }
            $values = @($registryKeys[0].SelectNodes('w:RegistryValue', $namespace))
            if ($values.Count -ne 2 -or $values[0].GetAttribute('Value') -cne "[#$(Get-MoWixFileId $name)]" -or
                $values[1].GetAttribute('Name') -cne 'ThreadingModel' -or
                $values[1].GetAttribute('Value') -cne 'Apartment') {
                throw 'Generated WiX COM value contract mismatch.'
            }
        } elseif ($registryKeys.Count -ne 0) { throw 'Unexpected generated WiX registry ownership.' }
        $shortcuts = @($file.SelectNodes('w:Shortcut', $namespace))
        if ($name -ceq 'bin/mo-settings.exe') {
            if ($shortcuts.Count -ne 1 -or
                $shortcuts[0].GetAttribute('Id') -cne 'MoSettingsStartMenuShortcut' -or
                $shortcuts[0].GetAttribute('Directory') -cne 'ProgramMenuFolder' -or
                $shortcuts[0].GetAttribute('Name') -cne 'Mo (墨) 输入法设置' -or
                $shortcuts[0].GetAttribute('Advertise') -cne 'yes') {
                throw 'Generated WiX settings shortcut contract mismatch.'
            }
        } elseif ($shortcuts.Count -ne 0) { throw 'Unexpected generated WiX shortcut ownership.' }
    }
    $x86Registry = $document.SelectSingleNode(
        '//w:StandardDirectory[@Id="ProgramFilesFolder"]/w:Component[@Id="TipX86ComRegistryComponent"]',
        $namespace)
    if ($null -eq $x86Registry -or $x86Registry.GetAttribute('Bitness') -cne 'always32' -or
        $x86Registry.GetAttribute('Guid') -cne (Get-MoWixComponentGuid 'tip/x86/mo-tip.com-registry') -or
        -not $seenReferences.Contains('TipX86ComRegistryComponent')) {
        throw 'Generated WiX x86 registry component contract mismatch.'
    }
    $x86Keys = @($x86Registry.SelectNodes('w:RegistryKey', $namespace))
    $x86Values = if ($x86Keys.Count -eq 1) { @($x86Keys[0].SelectNodes('w:RegistryValue', $namespace)) } else { @() }
    if ($x86Keys.Count -ne 1 -or $x86Keys[0].GetAttribute('Root') -cne 'HKLM' -or
        $x86Keys[0].GetAttribute('Key') -cne 'Software\Classes\CLSID\{B4911146-2A27-47AA-9D12-109B6AE10A70}\InprocServer32' -or
        $x86Values.Count -ne 2 -or $x86Values[0].GetAttribute('Value') -cne '[INSTALLFOLDER]tip\x86\mo-tip.dll' -or
        $x86Values[0].GetAttribute('KeyPath') -cne 'yes' -or
        $x86Values[1].GetAttribute('Name') -cne 'ThreadingModel' -or
        $x86Values[1].GetAttribute('Value') -cne 'Apartment') {
        throw 'Generated WiX x86 COM value contract mismatch.'
    }
    if ($seenFiles.Count -ne $expected.Count -or $seenComponents.Count -ne $expected.Count -or
        $seenReferences.Count -ne ($expected.Count + 1)) { throw 'Generated WiX payload coverage mismatch.' }
}
