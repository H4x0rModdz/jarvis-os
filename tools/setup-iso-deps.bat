@echo off
REM Jarvis OS - ISO build dependencies setup
REM Initializes Ubuntu-24.04 in WSL2 and installs podman.
REM Run this ONCE before build-iso.bat.

setlocal enabledelayedexpansion

set "DISTRO=Ubuntu-24.04"

echo.
echo === Jarvis OS - ISO build deps setup ===
echo   Distro: %DISTRO%
echo.

REM Sanity: WSL must be available
where wsl >nul 2>&1
if errorlevel 1 (
    echo [ERRO] wsl.exe nao encontrado. Instale o WSL primeiro.
    pause
    exit /b 1
)

REM Check if distro exists
wsl -d %DISTRO% -u root echo ok >nul 2>&1
if errorlevel 1 (
    echo [ERRO] Distro "%DISTRO%" nao encontrada.
    echo        Execute: wsl --install Ubuntu-24.04 --no-launch
    pause
    exit /b 1
)

REM First-time Ubuntu initialization
REM If no non-root user exists yet, launch normally so WSL runs the
REM first-time setup wizard (choose username + password).
for /f "usebackq delims=" %%u in (
    `wsl -d %DISTRO% -u root bash -c "awk -F: '$3>=1000 && $3<65534{print $1}' /etc/passwd | head -1" 2^>nul`
) do set "LINUX_USER=%%u"

if "!LINUX_USER!"=="" (
    echo [INFO] Nenhum usuario encontrado - abrindo Ubuntu para criar conta.
    echo        Escolha um username e senha quando solicitado.
    echo        Depois feche a janela do Ubuntu para continuar.
    echo.
    wsl -d %DISTRO%
    echo.
    for /f "usebackq delims=" %%u in (
        `wsl -d %DISTRO% -u root bash -c "awk -F: '$3>=1000 && $3<65534{print $1}' /etc/passwd | head -1" 2^>nul`
    ) do set "LINUX_USER=%%u"
)

if "!LINUX_USER!"=="" (
    echo [ERRO] Ainda nao ha usuario no Ubuntu. Execute manualmente: wsl -d %DISTRO%
    pause
    exit /b 1
)

echo [OK] Usuario Linux: !LINUX_USER!
echo.

REM Install podman
echo Instalando podman (pode demorar ~1-2 min)...
wsl -d %DISTRO% -u root bash -lc "apt-get update -qq && apt-get install -y podman uidmap slirp4netns"
if errorlevel 1 (
    echo [ERRO] Falha ao instalar podman.
    pause
    exit /b 1
)

REM Verify podman version
for /f "usebackq delims=" %%v in (
    `wsl -d %DISTRO% -u !LINUX_USER! bash -lc "podman --version 2>/dev/null"`
) do set "PODMAN_VER=%%v"

echo.
echo [OK] %PODMAN_VER%

REM Rootful podman: ensure /etc/subuid and /etc/subgid for root
REM bootc-image-builder runs under sudo (rootful). Root needs uid/gid mappings.
wsl -d %DISTRO% -u root bash -lc "grep -q '^root:' /etc/subuid || echo 'root:100000:65536' >> /etc/subuid; grep -q '^root:' /etc/subgid || echo 'root:100000:65536' >> /etc/subgid"
echo [OK] /etc/subuid e /etc/subgid configurados para root

echo.
echo === Setup concluido! ===
echo Agora execute: build-iso.bat
echo.
pause
endlocal
