package com.horusvpn.nexvpn

import android.content.Intent
import android.net.VpnService
import android.os.Bundle
import android.util.Log
import android.view.WindowManager
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import java.io.File

class MainActivity : TauriActivity() {
    companion object {
        const val TAG = "NexVPN-Main"
    }

    private var pendingSocksPort: Int = 10808
    private var launchedFromTile = false

    private val vpnPermissionLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result ->
        if (result.resultCode == RESULT_OK) {
            Log.i(TAG, "VPN permission granted")
            startVpnService(pendingSocksPort)
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

        // Start watching for VPN commands from Rust
        startVpnCommandWatcher()
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
                val port = command.substringAfter("start:").toIntOrNull() ?: 10808
                prepareAndStartVpn(port)
            }
            command == "stop" -> {
                stopVpnService()
            }
            else -> {
                Log.w(TAG, "Unknown VPN command: $command")
            }
        }
    }

    private fun prepareAndStartVpn(socksPort: Int) {
        pendingSocksPort = socksPort
        val intent = VpnService.prepare(this)
        if (intent != null) {
            Log.i(TAG, "Requesting VPN permission from user")
            vpnPermissionLauncher.launch(intent)
        } else {
            Log.i(TAG, "VPN permission already granted")
            startVpnService(socksPort)
        }
    }

    private fun startVpnService(socksPort: Int) {
        val intent = Intent(this, NexVpnService::class.java)
        intent.putExtra("socks_port", socksPort)
        startService(intent)
        Log.i(TAG, "VPN service started with socks_port=$socksPort")
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
