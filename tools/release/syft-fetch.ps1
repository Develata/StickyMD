[CmdletBinding()]
param([string]$Uri, [string]$OutputPath)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
# Transport only; the caller owns retries, pin checks, publication and cleanup.
Invoke-WebRequest -UseBasicParsing -Uri $Uri -OutFile $OutputPath
