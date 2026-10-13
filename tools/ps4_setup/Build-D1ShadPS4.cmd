@echo off
rem Build shadPS4 v.0.19.0 (c7e065d1) with shadps4-0.19.0-local-ip.patch applied, using the
rem official LLVM 23.1.3 clang-cl and the MSVC/Windows SDK environment. Discord RPC and the
rem updater are off: a contained lab binary must not reach the network on its own.
rem HAVE_ENDIAN_H=OFF: LLVM 23 ships an endian.h without the Windows byte-order helpers, which
rem would otherwise make the vendored LibreSSL skip its own Windows definitions.
rem Usage: Build-D1ShadPS4.cmd [source-dir] [build-dir]
setlocal
set "SRC=%~1"
if "%SRC%"=="" set "SRC=F:\d1\tools\shadPS4-src\v0.19.0-d1"
set "OUT=%~2"
if "%OUT%"=="" set "OUT=F:\d1\tools\shadPS4-src\build-d1"
set "LLVM=F:\d1\tools\llvm-23.1.3\bin"
set "NINJA=%LOCALAPPDATA%\Microsoft\WinGet\Packages\Ninja-build.Ninja_Microsoft.Winget.Source_8wekyb3d8bbwe"
call "F:\VS\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul || exit /b 1
set "PATH=%LLVM%;%NINJA%;C:\Program Files\CMake\bin;%PATH%"
cmake -S "%SRC%" -B "%OUT%" -G Ninja -DCMAKE_BUILD_TYPE=Release ^
  -DCMAKE_C_COMPILER=clang-cl -DCMAKE_CXX_COMPILER=clang-cl ^
  -DENABLE_DISCORD_RPC=OFF -DENABLE_UPDATER=OFF ^
  -DHAVE_ENDIAN_H=OFF || exit /b 2
cmake --build "%OUT%" --parallel || exit /b 3
echo built: %OUT%\shadPS4.exe
