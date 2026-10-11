::@echo off

::------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
:: updates cargo once (for Cargo.lock)
:: updates local rust components in Windows and WSL.
:: clean and test on Windows for native Windows (x86_64), and WSL for native Linux (x86_64).
:: **** if a test fails the result of cargo test effects the shell's exit code, which will quit before building
:: it builds on Windows for Windows, and on WSL for Android NDK, Linux. (Android recently moved toolchain from Windows to WSL, keeping just windows-msvc on Windows).
::------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------


chcp 65001 1>nul 2>nul
set "LANG=en_US.UTF-8"
set "LANGUAGE=en_US"
set "LC_CTYPE=en_US.UTF-8"
set "LC_NUMERIC=en_US.UTF-8"
set "LC_TIME=en_US.UTF-8"
set "LC_COLLATE=en_US.UTF-8"
set "LC_MONETARY=en_US.UTF-8"
set "LC_MESSAGES=en_US.UTF-8"
set "LC_PAPER=en_US.UTF-8"
set "LC_NAME=en_US.UTF-8"
set "LC_ADDRESS=en_US.UTF-8"
set "LC_TELEPHONE=en_US.UTF-8"
set "LC_MEASUREMENT=en_US.UTF-8"
set "LC_IDENTIFICATION=en_US.UTF-8"
set "LC_ALL=en_US.UTF-8"
set "TZ=UTC"

pushd "%~sdp0"


::------------------ run once, on Windows for all builds/tests.
cargo update



::------------------ all tests were successful, meaning you can continue to building.


::  ███████████     █████  █████    █████    █████          ██████████  
:: ░░███░░░░░███   ░░███  ░░███    ░░███    ░░███          ░░███░░░░███ 
::  ░███    ░███    ░███   ░███     ░███     ░███           ░███   ░░███
::  ░██████████     ░███   ░███     ░███     ░███           ░███    ░███
::  ░███░░░░░███    ░███   ░███     ░███     ░███           ░███    ░███
::  ░███    ░███    ░███   ░███     ░███     ░███      █    ░███    ███ 
::  ███████████     ░░████████      █████    ███████████    ██████████  
:: ░░░░░░░░░░░       ░░░░░░░░      ░░░░░    ░░░░░░░░░░░    ░░░░░░░░░░   


::------------------ this should be done once, here, for both building on Windows and WSL.
cargo clean


::------------------ update toolchains and components (Windows)
rustup update

::---------------------------------------------------------------- build for Windows (on Windows)
rustup target add   x86_64-pc-windows-msvc   i686-pc-windows-msvc  aarch64-pc-windows-msvc
title x86_64-pc-windows-msvc
cargo build  --release  --target   x86_64-pc-windows-msvc
title i686-pc-windows-msvc
cargo build  --release  --target   i686-pc-windows-msvc
title aarch64-pc-windows-msvc
cargo build  --release  --target   aarch64-pc-windows-msvc



::------------------------------------------- on Linux (WSL). set +o pipefail not fail on pipe. set +o errexit no exit on error.
set "ARGS="
::------------------ continue (do not exit) even if pipeline command fails.
set "ARGS=%ARGS% set +o pipefail ;"
::------------------ continue (do not exit) even if there is an error (exit code != 0).
set "ARGS=%ARGS% set +o errexit ;"
::------------------ print each command before executing (debug mode).
set "ARGS=%ARGS% set -o xtrace ;"
::------------------ allow WSL to load.
set "ARGS=%ARGS% sleep 5 ;"
::------------------ apply profile/user custom stuff and path (`bash -lc` does that already. if missing rustup won't be found in PATH)
set "ARGS=%ARGS% source ~/.profile ;"
set "ARGS=%ARGS% source ~/.bashrc ;"
::------------------ update toolchains and components (WSL - Linux)
set "ARGS=%ARGS% rustup update ;"
::------------------ Android NDK 30.0.16248370 SDK v21
set "ARGS=%ARGS% rustup target add   aarch64-linux-android   i686-linux-android  x86_64-linux-android  armv7-linux-androideabi ;"
set "ARGS=%ARGS% cargo build  --release  --target   aarch64-linux-android ;"
set "ARGS=%ARGS% cargo build  --release  --target   i686-linux-android ;"
set "ARGS=%ARGS% cargo build  --release  --target   x86_64-linux-android ;"
set "ARGS=%ARGS% cargo build  --release  --target   armv7-linux-androideabi ;"
::------------------ Linux (gnu)
set "ARGS=%ARGS% rustup target add  aarch64-unknown-linux-gnu  i686-unknown-linux-gnu  x86_64-unknown-linux-gnu  armv7-unknown-linux-gnueabi  armv7-unknown-linux-gnueabihf ;"
set "ARGS=%ARGS% cargo build  --release  --target   aarch64-unknown-linux-gnu ;"
set "ARGS=%ARGS% cargo build  --release  --target   i686-unknown-linux-gnu ;"
set "ARGS=%ARGS% cargo build  --release  --target   x86_64-unknown-linux-gnu ;"
set "ARGS=%ARGS% cargo build  --release  --target   armv7-unknown-linux-gnueabi ;"
set "ARGS=%ARGS% cargo build  --release  --target   armv7-unknown-linux-gnueabihf ;"
::------------------ Linux (musl)
set "ARGS=%ARGS% rustup target add  aarch64-unknown-linux-musl  i686-unknown-linux-musl  x86_64-unknown-linux-musl  armv7-unknown-linux-musleabi  armv7-unknown-linux-musleabihf ;"
set "ARGS=%ARGS% cargo build  --release  --target   aarch64-unknown-linux-musl ;"
set "ARGS=%ARGS% cargo build  --release  --target   i686-unknown-linux-musl ;"
set "ARGS=%ARGS% cargo build  --release  --target   x86_64-unknown-linux-musl ;"
set "ARGS=%ARGS% cargo build  --release  --target   armv7-unknown-linux-musleabi ;"
set "ARGS=%ARGS% cargo build  --release  --target   armv7-unknown-linux-musleabihf ;"

title linux
call wsl bash -lc "%ARGS%"


pause
exit /b 0
