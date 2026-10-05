# Policy regression: these checks never launch an emulator or post input.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Final124RunHelpers.ps1"
function New-LoginState {
    [pscustomobject]@{
        current = 23; target = 23; name = 'character:signin'; stage_unchanged_during_sample = $true
        character_signin = [pscustomobject]@{
            vtable = '0x013e8d58'; current = 5; target = 5; unchanged_during_sample = $true
        }
    }
}
function Assert-Retry($State, [bool]$Expected, [int]$Attempts = 0,
    [double]$Elapsed = 6, [bool]$Answered = $false) {
    $actual = Test-D1CharacterLoginRetry -State $State -Attempts $Attempts `
        -SecondsSincePress $Elapsed -SelectionAnswered $Answered
    if ($actual -ne $Expected) { throw "Login retry policy: expected $Expected, got $actual" }
}
Assert-Retry (New-LoginState) $true
Assert-Retry (New-LoginState) $true -Attempts 2
Assert-Retry (New-LoginState) $false -Attempts 3
Assert-Retry (New-LoginState) $false -Elapsed 5.99
Assert-Retry (New-LoginState) $false -Answered $true
foreach ($phase in @(19, 21, 22, 24, 36)) {
    $state = New-LoginState; $state.current = $phase; $state.target = $phase
    Assert-Retry $state $false
}
$state = New-LoginState; $state.target = 24; Assert-Retry $state $false
$state = New-LoginState; $state.name = 'character:create'; Assert-Retry $state $false
$state = New-LoginState; $state.stage_unchanged_during_sample = $false; Assert-Retry $state $false
$state = New-LoginState; $state.character_signin = $null; Assert-Retry $state $false
$state = New-LoginState; $state.character_signin.vtable = '0x013e8b74'; Assert-Retry $state $false
$state = New-LoginState; $state.character_signin.current = 2; Assert-Retry $state $false
$state = New-LoginState; $state.character_signin.target = 7; Assert-Retry $state $false
$state = New-LoginState; $state.character_signin.unchanged_during_sample = $false; Assert-Retry $state $false
'18 character-login policy checks passed'
