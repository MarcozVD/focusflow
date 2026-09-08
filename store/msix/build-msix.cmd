@echo off
rem Empaqueta el MSIX de FocusFlow. Uso: build-msix.cmd "CN=XXXXXXXX..."
rem Requiere: Windows SDK (makeappx), payload ya extraido en msix-payload\app\
setlocal
set PUB=%~1
if "%PUB%"=="" (
  echo Falta el Publisher ID de Partner Center.
  exit /b 1
)
set ROOT=C:\Users\mvale\focusflow\store\msix
set PKGDIR=%ROOT%\pkg
set MAKAPPX="C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\makeappx.exe"

rem 1. staging limpio
if exist "%PKGDIR%" rmdir /s /q "%PKGDIR%"
mkdir "%PKGDIR%\assets" "%PKGDIR%\app"

rem 2. manifest con el publisher real
powershell -NoProfile -Command "(Get-Content '%ROOT%\AppxManifest.xml' -Raw) -replace '__PUBLISHER_ID__', '%PUB%' | Set-Content '%PKGDIR%\AppxManifest.xml' -Encoding UTF8"

rem 3. payload + assets
copy /y "%ROOT%\msix-payload\app\focusflow.exe" "%PKGDIR%\app\focusflow.exe" >nul
copy /y "%ROOT%\assets\*.png" "%PKGDIR%\assets\" >nul

rem 4. pack (unsigned - la Store firma al publicar)
%MAKAPPX% pack /d "%PKGDIR%" /p "%ROOT%\MarcozVD.FlowFocus_0.1.4.0_x64.msix" /o
if errorlevel 1 exit /b 1
echo.
echo OK: %ROOT%\MarcozVD.FlowFocus_0.1.4.0_x64.msix
