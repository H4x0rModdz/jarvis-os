@echo off
REM ============================================================
REM  LilithOS — full ISO build from Windows (drives WSL).
REM
REM  Builds the prebuilt builder base (once, ADR 0021 P2), the
REM  OCI image, then the bootable ISO via bootc-image-builder.
REM  Everything runs inside WSL where podman + the Linux
REM  toolchain live.
REM
REM  Usage (from this folder, in a terminal):
REM      build-iso.bat            normal build (reuses builder base)
REM      build-iso.bat rebuild    force a fresh builder base too
REM
REM  Output:
REM      iso\output\bootiso\install.iso   (Windows-visible — the
REM      repo lives on the Windows filesystem)
REM
REM  Notes:
REM   - Runs under sudo inside WSL so the OCI image + bootc-image-
REM     builder share rootful podman storage (rootless splits them
REM     and the ISO step fails with "image not known"). You'll be
REM     prompted for your WSL sudo password in the console.
REM   - First run builds the builder base (~10 min). Later runs
REM     reuse it; only your crates recompile (~2 min) thanks to the
REM     cargo cache mounts.
REM   - bootc-image-builder needs loop devices + --privileged. WSL2
REM     usually allows this; if the ISO step fails here, the GitHub
REM     "Build ISO" workflow is the reliable fallback.
REM ============================================================

setlocal enabledelayedexpansion

set "DISTRO=Ubuntu-24.04"

REM Force a fresh builder base when the first argument is "rebuild".
set "REBUILD=0"
if /i "%~1"=="rebuild" set "REBUILD=1"

REM ── Preflight doctor ────────────────────────────────────────────────
REM Runs BEFORE any wsl call (the wslpath below already needs a live WSL).
REM Idempotent: when the environment is healthy it returns in ~2s and we
REM fall straight through. When something is off it reports, fixes (only
REM after you confirm), and tells you to reboot. See tools\preflight-doctor.ps1.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\preflight-doctor.ps1" -Distro "%DISTRO%"
if errorlevel 1 (
    echo.
    echo [PREFLIGHT] Ambiente ainda nao esta pronto para o build.
    echo            Siga as instrucoes acima ^(corrigir / reiniciar^) e
    echo            rode build-iso.bat de novo.
    echo.
    pause
    exit /b 1
)

REM Convert this .bat's own directory to a WSL path so the script is
REM portable regardless of where the repo sits. %~dp0 ends with a
REM trailing backslash; wslpath handles it.
for /f "usebackq delims=" %%i in (`wsl -d %DISTRO% wslpath "%~dp0."`) do set "REPO=%%i"

if "%REPO%"=="" (
    echo [ERRO] Nao consegui resolver o caminho WSL do repo.
    echo        Confirme que a distro "%DISTRO%" existe: wsl -l -v
    exit /b 1
)

echo.
echo === LilithOS ISO build ===
echo   Distro : %DISTRO%
echo   Repo   : %REPO%
echo   Rebuild builder base: %REBUILD%
echo.
echo Voce sera solicitado a senha do sudo do WSL.
echo.

REM Run the whole pipeline under sudo inside a login shell (login
REM shell so cargo/podman are on PATH). REBUILD_BUILDER is read by
REM tools/build-iso.sh.
wsl -d %DISTRO% bash -lc "cd '%REPO%' && sudo REBUILD_BUILDER=%REBUILD% bash tools/build-iso.sh"
set "RC=%errorlevel%"

echo.
echo Log completo salvo em: %~dp0build-iso.log
echo.

if not "%RC%"=="0" (
    echo [FALHOU] O build retornou erro ^(codigo %RC%^).
    echo   Abra build-iso.log para ver onde quebrou.
    echo   Se foi no passo bootc-image-builder, o workflow
    echo   "Build ISO" no GitHub Actions e o fallback.
    echo.
    pause
    exit /b %RC%
)

echo === Build concluido ===
echo ISO: %~dp0iso\output\bootiso\install.iso
echo.
pause
endlocal
