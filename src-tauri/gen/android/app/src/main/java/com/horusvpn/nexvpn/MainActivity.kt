package com.horusvpn.nexvpn

import android.content.Intent
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.BitmapDrawable
import android.net.Uri
import android.net.VpnService
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.util.Base64
import android.util.Log
import android.view.WindowManager
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import java.io.ByteArrayOutputStream
import java.io.File
import org.json.JSONArray
import org.json.JSONObject

class MainActivity : TauriActivity() {
    companion object {
        const val TAG = "NexVPN-Main"
    }

    private var pendingSocksPort: Int = 10808
    private var pendingAuthUser: String = ""
    private var pendingAuthPass: String = ""
    private var launchedFromTile = false

    private val vpnPermissionLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result ->
        if (result.resultCode == RESULT_OK) {
            Log.i(TAG, "VPN permission granted")
            startVpnService(pendingSocksPort, pendingAuthUser, pendingAuthPass)
        } else {
            Log.w(TAG, "VPN permission denied by user")
            writeVpnStatus("denied")
        }
    }

    private var vpnCommandWatcher: Thread? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        // Detect tile launch BEFORE anything visual happens
        launchedFromTile = intent?.getBooleanExtra("from_tile", false) == true

        if (launchedFromTile) {
            // Make window completely invisible
            setTheme(R.style.Theme_nexvpn_Transparent)
            window.addFlags(WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE)
            window.setDimAmount(0f)
        }

        enableEdgeToEdge()
        super.onCreate(savedInstanceState)

        // If from tile, minimize immediately
        if (launchedFromTile) {
            moveTaskToBack(true)
            overridePendingTransition(0, 0)
        }

        // Handle deep link from cold start intent
        handleDeepLinkIntent(intent)

        // Handle tile action from cold start
        handleTileIntent(intent)

        // Write native library directory path for Rust to read
        val nativeLibDir = applicationInfo.nativeLibraryDir
        val filesDirPath = filesDir.absolutePath
        Log.i(TAG, "Native lib dir: $nativeLibDir")
        Log.i(TAG, "Files dir: $filesDirPath")

        val infoFile = File(filesDirPath, ".android_paths")
        infoFile.writeText("native_lib_dir=$nativeLibDir\nfiles_dir=$filesDirPath\n")

        // List native libs for debugging
        val libFiles = File(nativeLibDir).listFiles()?.map { it.name } ?: listOf("(empty)")
        Log.i(TAG, "Native libs: $libFiles")

        // Request battery optimization exemption (keeps VPN alive in background)
        requestBatteryOptimizationExemption()

        // Generate installed apps list with icons (for per-app VPN UI)
        Thread {
            writeInstalledAppsJson()
        }.start()

        // Start watching for VPN commands from Rust
        startVpnCommandWatcher()
    }

    private fun writeInstalledAppsJson() {
        try {
            val pm = packageManager
            // Get all apps visible to user: combine launcher apps + third-party apps
            val launcherIntent = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
            val launchablePackages = pm.queryIntentActivities(launcherIntent, 0)
                .map { it.activityInfo.packageName }
                .toSet()

            val allApps = pm.getInstalledApplications(PackageManager.GET_META_DATA)
            val apps = allApps
                .filter { app ->
                    // Include if: has launcher icon OR is third-party (user-installed)
                    launchablePackages.contains(app.packageName) ||
                    (app.flags and ApplicationInfo.FLAG_SYSTEM == 0)
                }
                .filter { it.packageName != packageName } // exclude ourselves
                .distinctBy { it.packageName }
                .sortedBy { pm.getApplicationLabel(it).toString().lowercase() }

            Log.i(TAG, "Apps: total=${allApps.size}, launchable=${launchablePackages.size}, filtered=${apps.size}")

            val jsonArray = JSONArray()
            for (app in apps) {
                val obj = JSONObject()
                obj.put("package_name", app.packageName)
                obj.put("label", pm.getApplicationLabel(app).toString())

                // Get icon as base64 PNG (small, 48x48)
                try {
                    val drawable = pm.getApplicationIcon(app)
                    val bitmap = if (drawable is BitmapDrawable) {
                        Bitmap.createScaledBitmap(drawable.bitmap, 48, 48, true)
                    } else {
                        val bmp = Bitmap.createBitmap(48, 48, Bitmap.Config.ARGB_8888)
                        val canvas = Canvas(bmp)
                        drawable.setBounds(0, 0, 48, 48)
                        drawable.draw(canvas)
                        bmp
                    }
                    val stream = ByteArrayOutputStream()
                    bitmap.compress(Bitmap.CompressFormat.PNG, 80, stream)
                    obj.put("icon", Base64.encodeToString(stream.toByteArray(), Base64.NO_WRAP))
                } catch (e: Exception) {
                    obj.put("icon", "")
                }

                jsonArray.put(obj)
            }

            val outFile = File(filesDir, "nexvpn/.installed_apps.json")
            outFile.parentFile?.mkdirs()
            outFile.writeText(jsonArray.toString())
            Log.i(TAG, "Wrote ${apps.size} installed apps to JSON")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to write installed apps: $e")
        }
    }

    private fun requestBatteryOptimizationExemption() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            val pm = getSystemService(POWER_SERVICE) as PowerManager
            if (!pm.isIgnoringBatteryOptimizations(packageName)) {
                try {
                    val intent = Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS).apply {
                        data = Uri.parse("package:$packageName")
                    }
                    startActivity(intent)
                } catch (e: Exception) {
                    Log.w(TAG, "Could not request battery optimization exemption: $e")
                }
            }
        }
    }

    override fun onResume() {
        super.onResume()
        // Double-check: if this was a tile launch, stay in background
        if (launchedFromTile) {
            moveTaskToBack(true)
            // Reset flag so normal app resumes work correctly later
            launchedFromTile = false
        }
    }

    private fun startVpnCommandWatcher() {
        vpnCommandWatcher = Thread {
            val cmdFile = File(filesDir, "nexvpn/.vpn_command")
            cmdFile.parentFile?.mkdirs()

            Log.i(TAG, "VPN command watcher started, watching: ${cmdFile.absolutePath}")

            while (!Thread.currentThread().isInterrupted) {
                try {
                    if (cmdFile.exists()) {
                        val command = cmdFile.readText().trim()
                        if (command.isNotEmpty()) {
                            Log.i(TAG, "VPN command received: $command")
                            cmdFile.delete()

                            runOnUiThread {
                                handleVpnCommand(command)
                            }
                        }
                    }

                    // Check for URL open requests from Rust
                    val urlFile = File(filesDir, "nexvpn/.open_url")
                    if (urlFile.exists()) {
                        val url = urlFile.readText().trim()
                        if (url.isNotEmpty()) {
                            Log.i(TAG, "Open URL request: $url")
                            urlFile.delete()
                            runOnUiThread {
                                try {
                                    val intent = Intent(Intent.ACTION_VIEW, android.net.Uri.parse(url))
                                    startActivity(intent)
                                } catch (e: Exception) {
                                    Log.e(TAG, "Failed to open URL: $e")
                                }
                            }
                        }
                    }

                    Thread.sleep(300)
                } catch (e: InterruptedException) {
                    break
                } catch (e: Exception) {
                    Log.e(TAG, "Watcher error: $e")
                    try { Thread.sleep(1000) } catch (_: InterruptedException) { break }
                }
            }
            Log.i(TAG, "VPN command watcher stopped")
        }.apply {
            isDaemon = true
            start()
        }
    }

    private fun handleVpnCommand(command: String) {
        when {
            command.startsWith("start:") -> {
                // Format: start:port:authUser:authPass
                val parts = command.substringAfter("start:").split(":", limit = 3)
                val port = parts.getOrNull(0)?.toIntOrNull() ?: 10808
                val authUser = parts.getOrNull(1) ?: ""
                val authPass = parts.getOrNull(2) ?: ""
                prepareAndStartVpn(port, authUser, authPass)
            }
            command == "stop" -> {
                stopVpnService()
            }
            else -> {
                Log.w(TAG, "Unknown VPN command: $command")
            }
        }
    }

    private fun prepareAndStartVpn(socksPort: Int, authUser: String = "", authPass: String = "") {
        pendingSocksPort = socksPort
        pendingAuthUser = authUser
        pendingAuthPass = authPass
        val intent = VpnService.prepare(this)
        if (intent != null) {
            Log.i(TAG, "Requesting VPN permission from user")
            vpnPermissionLauncher.launch(intent)
        } else {
            Log.i(TAG, "VPN permission already granted")
            startVpnService(socksPort, authUser, authPass)
        }
    }

    private fun startVpnService(socksPort: Int, authUser: String = "", authPass: String = "") {
        val intent = Intent(this, NexVpnService::class.java)
        intent.putExtra("socks_port", socksPort)
        intent.putExtra("auth_user", authUser)
        intent.putExtra("auth_pass", authPass)
        startService(intent)
        Log.i(TAG, "VPN service started with socks_port=$socksPort auth=${authUser.isNotEmpty()}")
    }

    private fun stopVpnService() {
        NexVpnService.instance?.stopVpn()
        Log.i(TAG, "VPN service stop requested")
    }

    private fun writeVpnStatus(status: String) {
        try {
            val statusFile = File(filesDir, "nexvpn/.vpn_status")
            statusFile.parentFile?.mkdirs()
            statusFile.writeText(status)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to write VPN status: $e")
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)

        // Check if this new intent is from tile
        val fromTile = intent.getBooleanExtra("from_tile", false)
        if (fromTile) {
            // App was already running — just minimize and let frontend poll handle it
            moveTaskToBack(true)
        }

        handleDeepLinkIntent(intent)
        handleTileIntent(intent)
    }

    private fun handleTileIntent(intent: Intent?) {
        if (intent?.action != VpnTileService.ACTION_TILE) return
        val action = intent.getStringExtra(VpnTileService.EXTRA_ACTION) ?: return
        Log.i(TAG, "Tile action received: $action")
        // .tile_action file was already written by TileService.
        // Frontend polls it and handles connect/disconnect via proper Tauri IPC.
    }

    private fun handleDeepLinkIntent(intent: Intent?) {
        val uri = intent?.data ?: return
        val url = uri.toString()
        if (!url.startsWith("nexvpn://")) return
        Log.i(TAG, "Deep link intent: $url")
        try {
            val dlFile = File(filesDir, "nexvpn/.deep_link")
            dlFile.parentFile?.mkdirs()
            dlFile.writeText(url)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to write deep link: $e")
        }
    }

    override fun onDestroy() {
        vpnCommandWatcher?.interrupt()
        super.onDestroy()
    }
}
