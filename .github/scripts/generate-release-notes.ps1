param(
    [Parameter(Mandatory = $true)]
    [string]$Tag
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$title = "# LoL Mod Fixer $Tag"
$overview = "A portable, standalone native Rust CLI tool for League of Legends mod diagnosis, repair, automatic name beautification, and legacy structure migration, built on the official [LeagueToolkit/ltk-manager](https://github.com/LeagueToolkit/ltk-manager) engine with full [Rose](https://github.com/Alban1911/Rose) integration."

# 1. Check for custom RELEASE_NOTES.md
$customNotes = ""
if (Test-Path "RELEASE_NOTES.md") {
    $raw = [System.IO.File]::ReadAllText((Resolve-Path "RELEASE_NOTES.md"), [System.Text.Encoding]::UTF8)
    if ($raw.Trim().Length -gt 0) {
        $customNotes = $raw.Trim()
    }
}

# 2. If no RELEASE_NOTES.md, generate from git tag annotation or commits
if (-not $customNotes) {
    $tagMsg = git tag -l --format='%(contents)' $Tag
    if ($tagMsg -and $tagMsg.Trim().Length -gt 0 -and $tagMsg.Trim() -ne $Tag) {
        $customNotes = "## What's New`n`n" + $tagMsg.Trim()
    } else {
        # Fallback: get commits since previous tag
        $prevTag = git describe --tags --abbrev=0 "${Tag}^" 2>$null
        if ($prevTag) {
            $commits = git log "$prevTag..$Tag" --pretty=format:"* %s"
        } else {
            $commits = git log -n 5 --pretty=format:"* %s"
        }
        $customNotes = "## What's New`n`n" + $commits
    }
}

# 3. Read sticky sections
$sticky = ""
if (Test-Path ".github/release-sticky.md") {
    $sticky = [System.IO.File]::ReadAllText((Resolve-Path ".github/release-sticky.md"), [System.Text.Encoding]::UTF8).Trim()
}

# 4. Assemble final release body
$sections = @(
    $title,
    $overview,
    "---",
    $customNotes,
    "---",
    $sticky
)

$fullBody = ($sections -join "`n`n").Trim() + "`n"

$outDir = Join-Path (Get-Location) "release-artifacts"
if (-not (Test-Path $outDir)) {
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
}

$outPath = Join-Path $outDir "RELEASE-BODY.md"
[System.IO.File]::WriteAllText($outPath, $fullBody, [System.Text.UTF8Encoding]::new($false))
Write-Host "Successfully generated $outPath for $Tag"
