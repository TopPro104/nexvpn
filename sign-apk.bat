@echo off
setlocal
rem Sign the Android release APK: zipalign + apksigner with nexvpn-release.keystore.
rem Build first:  npm run tauri android build -- --target aarch64 --apk
rem Usage:        sign-apk.bat                 (sign with the existing keystore)
rem               sign-apk.bat --new-keystore  (create a keystore if none exists)
cd /d "%~dp0"

set "APK_DIR=src-tauri\gen\android\app\build\outputs\apk\universal\release"
set "UNSIGNED_APK=%APK_DIR%\app-universal-release-unsigned.apk"
set "ALIGNED_APK=%APK_DIR%\app-universal-release-aligned.apk"
set "SIGNED_APK=%APK_DIR%\NexVPN-signed.apk"
set "KEYSTORE=nexvpn-release.keystore"
set "KEY_ALIAS=nexvpn"
set "STORE_PASS=nexvpn123"
set "KEY_PASS=nexvpn123"
set "DNAME=CN=NexVPN, OU=Dev, O=NexVPN, L=Unknown, ST=Unknown, C=US"

echo ============================================
echo   NexVPN APK signing
echo ============================================
echo.

rem --- Android SDK: ANDROID_HOME, ANDROID_SDK_ROOT, then the Android Studio default
set "ANDROID_SDK="
if defined ANDROID_HOME if exist "%ANDROID_HOME%\build-tools" set "ANDROID_SDK=%ANDROID_HOME%"
if not defined ANDROID_SDK if defined ANDROID_SDK_ROOT if exist "%ANDROID_SDK_ROOT%\build-tools" set "ANDROID_SDK=%ANDROID_SDK_ROOT%"
if not defined ANDROID_SDK if exist "%LOCALAPPDATA%\Android\Sdk\build-tools" set "ANDROID_SDK=%LOCALAPPDATA%\Android\Sdk"
if not defined ANDROID_SDK goto :no_sdk

rem --- Newest build-tools version
set "BUILD_TOOLS="
for /f "delims=" %%v in ('dir /b /ad /o-n "%ANDROID_SDK%\build-tools" 2^>nul') do if not defined BUILD_TOOLS set "BUILD_TOOLS=%ANDROID_SDK%\build-tools\%%v"
if not defined BUILD_TOOLS goto :no_sdk
set "ZIPALIGN=%BUILD_TOOLS%\zipalign.exe"
set "APKSIGNER=%BUILD_TOOLS%\apksigner.bat"
if not exist "%ZIPALIGN%" goto :no_sdk
if not exist "%APKSIGNER%" goto :no_sdk

rem --- Java (keytool, and apksigner needs JAVA_HOME): JAVA_HOME, Android Studio's JBR, PATH
set "KEYTOOL="
if defined JAVA_HOME if exist "%JAVA_HOME%\bin\keytool.exe" set "KEYTOOL=%JAVA_HOME%\bin\keytool.exe"
if not defined KEYTOOL if exist "%ProgramFiles%\Android\Android Studio\jbr\bin\keytool.exe" set "KEYTOOL=%ProgramFiles%\Android\Android Studio\jbr\bin\keytool.exe"
if not defined KEYTOOL if exist "%LOCALAPPDATA%\Programs\Android Studio\jbr\bin\keytool.exe" set "KEYTOOL=%LOCALAPPDATA%\Programs\Android Studio\jbr\bin\keytool.exe"
if not defined KEYTOOL for %%k in (keytool.exe) do if not "%%~$PATH:k"=="" set "KEYTOOL=%%~$PATH:k"
if not defined KEYTOOL goto :no_java
for %%k in ("%KEYTOOL%") do set "JAVA_HOME=%%~dpk.."

echo [OK] SDK:         %ANDROID_SDK%
echo [OK] Build tools: %BUILD_TOOLS%
echo [OK] Java:        %JAVA_HOME%
echo.

if not exist "%UNSIGNED_APK%" goto :no_apk

rem --- Keystore: never replace it silently, a different key breaks app updates
if exist "%KEYSTORE%" goto :have_keystore
if /i not "%~1"=="--new-keystore" goto :no_keystore
echo [INFO] Creating a new keystore: %KEYSTORE%
"%KEYTOOL%" -genkeypair -v -keystore "%KEYSTORE%" -alias %KEY_ALIAS% -keyalg RSA -keysize 2048 -validity 10000 -storepass %STORE_PASS% -keypass %KEY_PASS% -dname "%DNAME%"
if errorlevel 1 goto :fail
:have_keystore
echo [OK] Keystore: %KEYSTORE%
echo.

echo [INFO] Zipaligning...
if exist "%ALIGNED_APK%" del "%ALIGNED_APK%"
"%ZIPALIGN%" -p -f 4 "%UNSIGNED_APK%" "%ALIGNED_APK%"
if errorlevel 1 goto :fail

echo [INFO] Signing...
if exist "%SIGNED_APK%" del "%SIGNED_APK%"
call "%APKSIGNER%" sign --ks "%KEYSTORE%" --ks-key-alias %KEY_ALIAS% --ks-pass pass:%STORE_PASS% --key-pass pass:%KEY_PASS% --out "%SIGNED_APK%" "%ALIGNED_APK%"
if errorlevel 1 goto :fail

echo [INFO] Verifying...
call "%APKSIGNER%" verify "%SIGNED_APK%"
if errorlevel 1 goto :fail
if exist "%ALIGNED_APK%" del "%ALIGNED_APK%"

echo.
echo ============================================
echo   Done: %SIGNED_APK%
echo ============================================
goto :end

:no_sdk
echo [ERROR] Android SDK build-tools not found.
echo         Set ANDROID_HOME, or install build-tools via Android Studio (SDK Manager).
goto :fail
:no_java
echo [ERROR] keytool not found. Set JAVA_HOME or install Android Studio.
goto :fail
:no_apk
echo [ERROR] Unsigned APK not found: %UNSIGNED_APK%
echo         Build it first: npm run tauri android build -- --target aarch64 --apk
goto :fail
:no_keystore
echo [ERROR] %KEYSTORE% not found in %CD%
echo         Copy it from the machine that signed the previous releases.
echo         An APK signed with a new key cannot update the installed app:
echo         users would have to uninstall NexVPN first.
echo         To create a new key anyway: sign-apk.bat --new-keystore
goto :fail
:fail
echo.
echo [FAILED]
pause
exit /b 1
:end
pause
