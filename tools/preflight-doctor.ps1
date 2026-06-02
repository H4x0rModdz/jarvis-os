<#
  Jarvis OS — preflight doctor for the ISO build.

  Why this exists: the ISO build runs inside WSL2, which silently needs a
  whole stack to be true at once — firmware virtualization, the Windows
  hypervisor actually LAUNCHING at boot (hypervisorlaunchtype != Off), the
  VirtualMachinePlatform/WSL features, plus a provisioned Ubuntu user and
  podman. When any layer is off you get the same opaque HYPERV_NOT_INSTALLED
  and a 30-attempt yak shave. This script collapses that into one honest
  report + guided fix.

  Contract (exit codes, consumed by build-iso.bat):
    0  — everything ready, proceed to build
    2  — fixes applied, REBOOT required before building
    3  — BIOS/firmware action required (cannot be fixed from software)
    10 — re-launched elevated in a new window; this run is done
    1  — blocked for another reason (instructions printed)

  Design decisions (see chat 2026-06-02):
    - Fix only AFTER explicit "sim" confirmation. Never silent.
    - UAC (elevation) only when there is actually a software fix to apply —
      the BIOS verdict and the all-green fast path never prompt for admin.
    - Idempotent + fast: when WSL already answers, we skip every heavy check.
#>

param(
    [string]$Distro = "Ubuntu-24.04",
    # Internal: set when we re-launched ourselves elevated, so we know to
    # pause before the (separate) window closes and to avoid relaunch loops.
    [switch]$Elevated
)

$ErrorActionPreference = "Stop"
# WSL emits UTF-16 by default, which turns captured output into garbled
# spaced-out text in PS 5.1. WSL_UTF8=1 makes it clean ASCII so our string
# matches below are reliable.
$env:WSL_UTF8 = "1"

function Say  ($m) { Write-Host $m }
function Head ($m) { Write-Host "`n$m" -ForegroundColor Cyan }
function Ok   ($m) { Write-Host "  [OK]  $m" -ForegroundColor Green }
function Bad  ($m) { Write-Host "  [X]   $m" -ForegroundColor Red }
function Warn ($m) { Write-Host "  [!]   $m" -ForegroundColor Yellow }

function Test-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
}

# Pause only when we own the window (elevated child) so the user can read it.
function Maybe-Pause {
    if ($Elevated) { Read-Host "`nPressione Enter para fechar" | Out-Null }
}

function Finish ($code) { Maybe-Pause; exit $code }

Head "== Jarvis OS — preflight do build da ISO =="
Say  "Distro alvo: $Distro"

# ─────────────────────────────────────────────────────────────────────────
# Layer 0 — fast path. If WSL already answers, the entire Windows stack
# (firmware, hypervisor, features) is provably working. Skip straight to the
# WSL-internal checks. No admin, no UAC, ~2s.
# ─────────────────────────────────────────────────────────────────────────
$wslOk = $false
try {
    $probe = wsl -d $Distro -- sh -lc "echo jarvis_ok" 2>$null
    if ($LASTEXITCODE -eq 0 -and "$probe".Trim() -eq "jarvis_ok") { $wslOk = $true }
} catch { $wslOk = $false }

if ($wslOk) {
    Ok "WSL2 + distro '$Distro' respondendo"

    # The build runs as WSL's DEFAULT user (it calls sudo as that account),
    # so that's the one that must exist and be non-root. `id -un` avoids the
    # fragile $-field quoting that doesn't survive the Windows->WSL boundary
    # (single quotes get stripped, so sh would expand awk's $1/$3 to empty).
    $linuxUser = (wsl -d $Distro -- sh -lc "id -un" 2>$null | Out-String).Trim()
    if ($linuxUser -eq "root") { $linuxUser = "" }   # root == no usable account

    wsl -d $Distro -- sh -lc "command -v podman" *> $null
    $hasPodman = ($LASTEXITCODE -eq 0)

    if ([string]::IsNullOrWhiteSpace($linuxUser) -or -not $hasPodman) {
        if ([string]::IsNullOrWhiteSpace($linuxUser)) { Warn "Nenhum usuario Ubuntu (uid>=1000) ainda" }
        if (-not $hasPodman)                          { Warn "podman nao instalado no WSL" }
        Head "== Provisionando dependencias do WSL =="
        Say  "Chamando setup-iso-deps.bat (cria usuario se preciso + instala podman)..."
        & "$PSScriptRoot\setup-iso-deps.bat"
        # Re-check podman after the helper ran.
        wsl -d $Distro -- sh -lc "command -v podman" *> $null
        if ($LASTEXITCODE -ne 0) {
            Bad "podman ainda ausente apos o setup. Rode tools\setup-iso-deps.bat manualmente."
            Finish 1
        }
        Ok "Dependencias do WSL prontas"
    } else {
        Ok "Usuario Ubuntu: $linuxUser"
        Ok "podman instalado"
    }

    Head "== Tudo verde — seguindo para o build =="
    Finish 0
}

# ─────────────────────────────────────────────────────────────────────────
# WSL did not answer. Diagnose WHY, cheapest-and-no-admin first so the BIOS
# verdict never triggers a UAC prompt.
# ─────────────────────────────────────────────────────────────────────────
Bad "WSL2 nao respondeu (distro '$Distro')"

$cs = Get-CimInstance Win32_ComputerSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
# HypervisorPresent=true means the Windows hypervisor is already running, in
# which case firmware VT is necessarily on (VirtualizationFirmwareEnabled can
# read false precisely BECAUSE Hyper-V owns it — so check both, OR'd).
$hypervisorRunning = [bool]$cs.HypervisorPresent
$firmwareVtOn      = [bool]$cpu.VirtualizationFirmwareEnabled

Head "== Camada de virtualizacao =="
if ($hypervisorRunning) { Ok "Hipervisor do Windows em execucao" }
else                    { Warn "Hipervisor do Windows NAO esta em execucao" }
if ($firmwareVtOn)      { Ok "Virtualizacao habilitada no firmware (VT-x/AMD-V)" }
else                    { Warn "Firmware nao reporta virtualizacao habilitada" }

# Hard stop: nothing in software can flip a BIOS switch.
if (-not $hypervisorRunning -and -not $firmwareVtOn) {
    Head "== ACAO NECESSARIA: BIOS/UEFI =="
    Bad "A virtualizacao por hardware esta DESLIGADA no firmware."
    Say  "Nenhum script resolve isso — precisa ser ativado na BIOS/UEFI:"
    Say  "  1. Reinicie e entre na BIOS (geralmente Del, F2, F10 ou Esc no boot)."
    Say  "  2. Ative 'Intel Virtualization Technology / VT-x' (Intel)"
    Say  "     ou 'SVM Mode / AMD-V' (AMD), normalmente em Advanced > CPU."
    Say  "  3. Salve (F10), volte ao Windows e rode build-iso.bat de novo."
    Finish 3
}

# Software-side problem (hypervisor not launching, or a feature off). To
# inspect features/BCD and apply fixes we need admin — elevate now, since
# from here on there genuinely is something to fix.
if (-not (Test-Admin)) {
    if ($Elevated) { Bad "Sem privilegios de administrador mesmo apos elevar. Abortando."; Finish 1 }
    Head "== Preciso de Administrador para diagnosticar/corrigir =="
    Warn "Vou abrir uma janela elevada (UAC) para inspecionar features e o boot."
    Say  "Siga as instrucoes nessa nova janela; depois rode build-iso.bat de novo."
    $psi = @("-NoProfile","-ExecutionPolicy","Bypass","-File","`"$PSCommandPath`"",
             "-Distro","`"$Distro`"","-Elevated")
    Start-Process -FilePath "powershell.exe" -Verb RunAs -ArgumentList $psi
    exit 10   # parent (non-elevated) stops; elevated child takes over
}

# ── Elevated from here: deep diagnosis ──────────────────────────────────
Head "== Componentes do Windows =="
# Microsoft-Hyper-V-Hypervisor is optional (absent on some Home SKUs); the
# REQUIRED pair for WSL2 is VirtualMachinePlatform + the WSL feature, plus
# the hypervisor being allowed to launch at boot.
$featSpecs = @(
    @{ Name = "VirtualMachinePlatform";            Required = $true  },
    @{ Name = "Microsoft-Windows-Subsystem-Linux"; Required = $true  },
    @{ Name = "Microsoft-Hyper-V-Hypervisor";      Required = $false }
)
$toEnable = @()
foreach ($f in $featSpecs) {
    $state = "Absent"
    try { $state = (Get-WindowsOptionalFeature -Online -FeatureName $f.Name).State } catch {}
    if ($state -eq "Enabled") {
        Ok "$($f.Name): Enabled"
    } elseif ($state -eq "Absent" -and -not $f.Required) {
        Warn "$($f.Name): indisponivel nesta edicao (ok, opcional)"
    } else {
        Warn "$($f.Name): $state"
        $toEnable += $f.Name
    }
}

Head "== Boot do hipervisor =="
# hypervisorlaunchtype Off is THE classic cause of HYPERV_NOT_INSTALLED with
# virtualization enabled. Absent in BCD == default Auto == fine.
$bcd = (bcdedit /enum "{current}" | Out-String)
$hlt = "Auto (padrao)"
if ($bcd -match "hypervisorlaunchtype\s+(\w+)") { $hlt = $Matches[1] }
$fixLaunchType = ($hlt -match "Off")
if ($fixLaunchType) { Bad "hypervisorlaunchtype = Off (impede o hipervisor de subir)" }
else                { Ok  "hypervisorlaunchtype = $hlt" }

# ── Build the fix plan and confirm before touching anything ─────────────
if ($toEnable.Count -eq 0 -and -not $fixLaunchType) {
    Head "== Diagnostico inconclusivo =="
    Warn "Features e boot do hipervisor parecem ok, mas o WSL nao subiu."
    Say  "Tente, nesta janela:  wsl --update   e depois reinicie."
    Say  "Se persistir, suspeite de VirtualBox/VMware antigos, emulador"
    Say  "Android (BlueStacks/Gameloop) ou anti-cheat (Vanguard/Faceit)"
    Say  "segurando o VT-x. Fechar/atualizar esses costuma liberar."
    Finish 1
}

Head "== Plano de correcao =="
foreach ($n in $toEnable) { Say "  - Habilitar feature: $n" }
if ($fixLaunchType)       { Say "  - bcdedit /set hypervisorlaunchtype auto" }
Say ""
$ans = Read-Host "Aplicar essas correcoes agora? (sim/nao)"
if ($ans -notmatch '^(s|sim|y|yes)$') {
    Warn "Nada alterado. Saindo."
    Finish 1
}

Head "== Aplicando =="
foreach ($n in $toEnable) {
    Say "  Habilitando $n ..."
    # Best-effort on the optional Hyper-V feature so Home SKUs don't hard-fail.
    try { Enable-WindowsOptionalFeature -Online -FeatureName $n -All -NoRestart | Out-Null; Ok "$n habilitada" }
    catch { Warn "Falha ao habilitar $n ($($_.Exception.Message)) — seguindo" }
}
if ($fixLaunchType) {
    bcdedit /set hypervisorlaunchtype auto | Out-Null
    Ok "hypervisorlaunchtype -> Auto"
}
# Make sure the WSL kernel is current; harmless and often the missing piece.
Say "  Atualizando kernel do WSL (wsl --update) ..."
wsl --update *> $null

Head "== REINICIE para concluir =="
Warn "As correcoes so valem apos um REBOOT COMPLETO (use Reiniciar, nao Desligar)."
Say  "Depois de reiniciar, rode build-iso.bat de novo — o preflight vai passar"
Say  "direto e o build comeca."
$r = Read-Host "Reiniciar agora? (sim/nao)"
if ($r -match '^(s|sim|y|yes)$') {
    Say "Reiniciando em 5s... (feche o que precisar)"
    Start-Sleep -Seconds 5
    Restart-Computer -Force
}
Finish 2
