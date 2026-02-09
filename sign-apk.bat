@echo off
chcp 65001 >nul 2>&1
setlocal enabledelayedexpansion

:: === Настройки ===
set "ANDROID_SDK=C:\Users\mmmat\AppData\Local\Android\Sdk"
set "BUILD_TOOLS=%ANDROID_SDK%\build-tools\36.1.0"
set "JAVA_HOME=C:\Program Files\Android\Android Studio\jbr"
set "KEYTOOL=%JAVA_HOME%\bin\keytool.exe"
set "APKSIGNER=%BUILD_TOOLS%\apksigner.bat"
set "ZIPALIGN=%BUILD_TOOLS%\zipalign.exe"

set "APK_DIR=src-tauri\gen\android\app\build\outputs\apk\universal\release"
set "UNSIGNED_APK=%APK_DIR%\app-universal-release-unsigned.apk"
set "ALIGNED_APK=%APK_DIR%\app-universal-release-aligned.apk"
set "SIGNED_APK=%APK_DIR%\NexVPN-signed.apk"

set "KEYSTORE=nexvpn-release.keystore"
set "KEY_ALIAS=nexvpn"
set "STORE_PASS=nexvpn123"
set "KEY_PASS=nexvpn123"
set "DNAME=CN=NexVPN, OU=Dev, O=NexVPN, L=Unknown, ST=Unknown, C=US"
set "VALIDITY=10000"

echo ============================================
echo   NexVPN APK Auto-Sign
echo ============================================
echo.

:: Проверка наличия unsigned APK
if not exist "%UNSIGNED_APK%" (
    echo [ERROR] Unsigned APK not found: %UNSIGNED_APK%
    echo Run "npx tauri android build --apk" first.
    pause
    exit /b 1
)

:: Генерация keystore если не существует
if not exist "%KEYSTORE%" (
    echo [INFO] Keystore not found. Generating new keystore...
    "%KEYTOOL%" -genkeypair -v ^
        -keystore "%KEYSTORE%" ^
        -alias %KEY_ALIAS% ^
        -keyalg RSA ^
        -keysize 2048 ^
        -validity %VALIDITY% ^
        -storepass %STORE_PASS% ^
        -keypass %KEY_PASS% ^
        -dname "%DNAME%"
    if errorlevel 1 (
        echo [ERROR] Failed to generate keystore.
        pause
        exit /b 1
    )
    echo [OK] Keystore created: %KEYSTORE%
) else (
    echo [OK] Using existing keystore: %KEYSTORE%
)
echo.

:: Zipalign
echo [INFO] Zipaligning APK...
if exist "%ALIGNED_APK%" del "%ALIGNED_APK%"
"%ZIPALIGN%" -v 4 "%UNSIGNED_APK%" "%ALIGNED_APK%"
if errorlevel 1 (
    echo [ERROR] Zipalign failed.
    pause
    exit /b 1
)
echo [OK] Zipalign complete.
echo.

:: Sign APK
echo [INFO] Signing APK...
if exist "%SIGNED_APK%" del "%SIGNED_APK%"
call "%APKSIGNER%" sign ^
    --ks "%KEYSTORE%" ^
    --ks-key-alias %KEY_ALIAS% ^
    --ks-pass pass:%STORE_PASS% ^
    --key-pass pass:%KEY_PASS% ^
    --out "%SIGNED_APK%" ^
    "%ALIGNED_APK%"
if errorlevel 1 (
    echo [ERROR] Signing failed.
    pause
    exit /b 1
)
echo [OK] APK signed successfully!
echo.

:: Verify
echo [INFO] Verifying signature...
call "%APKSIGNER%" verify --verbose "%SIGNED_APK%"
echo.

:: Cleanup aligned APK
if exist "%ALIGNED_APK%" del "%ALIGNED_APK%"

echo ============================================
echo   Done! Signed APK:
echo   %SIGNED_APK%
echo ============================================
pause
