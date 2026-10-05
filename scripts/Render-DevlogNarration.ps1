# Optional Windows desktop TTS for a review cut; use recorded narration for a final edit.
param([Parameter(Mandatory)][string]$JobsFile, [string]$Voice = 'Microsoft Hazel Desktop')
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Speech
$jobs = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $JobsFile)) | ConvertFrom-Json
$speaker = [System.Speech.Synthesis.SpeechSynthesizer]::new()
try {
    $speaker.SelectVoice($Voice)
    foreach ($job in $jobs) {
        if (Test-Path -LiteralPath $job.destination) { throw "Narration already exists: $($job.destination)" }
        $speaker.SetOutputToWaveFile($job.destination)
        $speaker.Speak([string]$job.text)
        $speaker.SetOutputToNull()
    }
} finally { $speaker.Dispose() }
'Narration rendered'
