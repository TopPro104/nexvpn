package com.horusvpn.nexvpn

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.ComponentName
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import android.os.PowerManager
import android.service.quicksettings.TileService
import android.util.Log
import androidx.core.app.NotificationCompat
import java.io.File

class NexVpnService : VpnService() {
    companion object {
        var isRunning = false
        var instance: NexVpnService? = null
        const val TAG = "NexVPN-Vpn"
        const val CHANNEL_ID = "nexvpn_vpn"
        const val NOTIFICATION_ID = 1
        const val MAX_RESTART_ATTEMPTS = 5
        const val RESTART_DELAY_MS = 2000L
    }

    private var vpnInterface: ParcelFileDescriptor? = null
    private var tun2socksPid: Int = -1
    private var wakeLock: PowerManager.WakeLock? = null
    private var lastSocksPort: Int = 10808
    private var lastAuthUser: String = ""
    private var lastAuthPass: String = ""
    private var lastTun2socksPath: String = ""
    private var stealthActive: Boolean = false

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val socksPort = intent?.getIntExtra("socks_port", 10808) ?: 10808
        val authUser = intent?.getStringExtra("auth_user") ?: ""
        val authPass = intent?.getStringExtra("auth_pass") ?: ""

        if (isRunning) {
            Log.w(TAG, "VPN already running")
            return START_STICKY
        }

        createNotificationChannel()
        startForegroundNotification()
        startVpn(socksPort, authUser, authPass)

        return START_STICKY
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "VPN Service",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "NexVPN active connection"
                setShowBadge(false)
            }
            val nm = getSystemService(NotificationManager::class.java)
            nm.createNotificationChannel(channel)
        }
    }

    private fun startForegroundNotification() {
        // Tap notification → open app
        val openIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val pendingIntent = PendingIntent.getActivity(
            this, 0, openIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("NexVPN")
            .setContentText("VPN is active")
            .setSmallIcon(android.R.drawable.ic_lock_lock)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setContentIntent(pendingIntent)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .build()

        startForeground(NOTIFICATION_ID, notification)
    }

    private fun acquireWakeLock() {
        if (wakeLock == null) {
            val pm = getSystemService(POWER_SERVICE) as PowerManager
            wakeLock = pm.newWakeLock(
                PowerManager.PARTIAL_WAKE_LOCK,
                "NexVPN::VpnWakeLock"
            ).apply {
                acquire()
            }
            Log.i(TAG, "WakeLock acquired")
        }
    }

    private fun releaseWakeLock() {
        wakeLock?.let {
            if (it.isHeld) {
                it.release()
                Log.i(TAG, "WakeLock released")
            }
        }
        wakeLock = null
    }

    private fun startVpn(socksPort: Int, authUser: String = "", authPass: String = "") {
        try {
            val builder = Builder()
                .setSession("NexVPN")
                .addAddress("10.0.0.2", 32)
                .addAddress("fd00::2", 128)      // IPv6 TUN address
                .addRoute("0.0.0.0", 0)           // Capture all IPv4
                .addRoute("::", 0)                 // Capture all IPv6 (prevents IPv6 leak)
                .addDnsServer("8.8.8.8")
                .addDnsServer("8.8.4.4")
                .addDnsServer("2001:4860:4860::8888") // Google DNS IPv6
                .setMtu(1500)

            // Always exclude our own app to prevent routing loops
            builder.addDisallowedApplication(packageName)

            // Per-app VPN: read config written by Rust
            applyPerAppConfig(builder)

            vpnInterface = builder.establish()

            if (vpnInterface == null) {
                Log.e(TAG, "Failed to establish VPN interface")
                writeStatus("error:establish_failed")
                stopSelf()
                return
            }

            val fd = vpnInterface!!.fd
            Log.i(TAG, "VPN interface established, fd=$fd")

            // Remove close-on-exec so child process inherits the fd
            try {
                android.system.Os.fcntlInt(
                    vpnInterface!!.fileDescriptor,
                    android.system.OsConstants.F_SETFD,
                    0
                )
                Log.i(TAG, "Cleared CLOEXEC on fd $fd")
            } catch (e: Exception) {
                Log.w(TAG, "fcntlInt CLOEXEC clear failed: $e")
            }

            // Find tun2socks binary
            val nativeLibDir = applicationInfo.nativeLibraryDir
            val tun2socksPath = "$nativeLibDir/libtun2socks.so"

            if (!File(tun2socksPath).exists()) {
                Log.e(TAG, "tun2socks not found at $tun2socksPath")
                val libs = File(nativeLibDir).listFiles()?.map { it.name } ?: listOf("(empty)")
                Log.e(TAG, "Available libs: $libs")
                writeStatus("error:tun2socks_not_found")
                cleanup()
                stopSelf()
                return
            }

            // Ensure executable
            try {
                Runtime.getRuntime().exec(arrayOf("chmod", "755", tun2socksPath)).waitFor()
            } catch (_: Exception) {}

            // Save params for auto-restart
            lastSocksPort = socksPort
            lastAuthUser = authUser
            lastAuthPass = authPass
            lastTun2socksPath = tun2socksPath

            // Build proxy URL with auth if credentials provided
            val proxyUrl = if (authUser.isNotEmpty() && authPass.isNotEmpty()) {
                "socks5://$authUser:$authPass@127.0.0.1:$socksPort"
            } else {
                "socks5://127.0.0.1:$socksPort"
            }

            Log.i(TAG, "Starting tun2socks via NativeHelper: fd=$fd port=$socksPort auth=${authUser.isNotEmpty()}")

            // Use NativeHelper (JNI fork+exec) to preserve fd inheritance.
            // Android's ProcessBuilder closes all fds > 2, which would kill the TUN fd.
            tun2socksPid = NativeHelper.startProcess(
                tun2socksPath,
                arrayOf("-device", "fd://$fd", "-proxy", proxyUrl)
            )

            if (tun2socksPid <= 0) {
                Log.e(TAG, "Failed to start tun2socks (pid=$tun2socksPid)")
                writeStatus("error:tun2socks_start_failed")
                cleanup()
                stopSelf()
                return
            }

            isRunning = true
            instance = this
            acquireWakeLock()
            writeStatus("running")

            // Apply root stealth if enabled (must be after TUN is up)
            applyStealthMode()

            Log.i(TAG, "VPN started, tun2socks pid=$tun2socksPid")

            // Monitor tun2socks + auto-restart (kill switch: TUN stays open = traffic blocked)
            Thread {
                var restartCount = 0
                try {
                    while (isRunning) {
                        if (!NativeHelper.isProcessAlive(tun2socksPid)) {
                            if (!isRunning) break

                            Log.w(TAG, "tun2socks died — TUN still open (kill switch active)")
                            writeStatus("reconnecting")

                            if (restartCount >= MAX_RESTART_ATTEMPTS) {
                                Log.e(TAG, "Max restart attempts reached, giving up")
                                writeStatus("error:tun2socks_crash_loop")
                                break
                            }

                            Thread.sleep(RESTART_DELAY_MS)
                            restartCount++

                            // Restart tun2socks with saved params (TUN fd is still valid)
                            val fd = vpnInterface?.fd ?: break
                            val url = if (lastAuthUser.isNotEmpty() && lastAuthPass.isNotEmpty()) {
                                "socks5://$lastAuthUser:$lastAuthPass@127.0.0.1:$lastSocksPort"
                            } else {
                                "socks5://127.0.0.1:$lastSocksPort"
                            }

                            Log.i(TAG, "Restarting tun2socks (attempt $restartCount/$MAX_RESTART_ATTEMPTS)")
                            tun2socksPid = NativeHelper.startProcess(
                                lastTun2socksPath,
                                arrayOf("-device", "fd://$fd", "-proxy", url)
                            )

                            if (tun2socksPid > 0) {
                                Log.i(TAG, "tun2socks restarted, pid=$tun2socksPid")
                                writeStatus("running")
                                restartCount = 0 // Reset on successful restart
                            } else {
                                Log.e(TAG, "Failed to restart tun2socks")
                            }
                        }
                        Thread.sleep(2000)
                    }
                } catch (_: InterruptedException) {}
                Log.i(TAG, "tun2socks monitor thread ended")
            }.apply {
                isDaemon = true
                start()
            }

        } catch (e: Exception) {
            Log.e(TAG, "Failed to start VPN: $e", e)
            writeStatus("error:${e.message}")
            cleanup()
            stopSelf()
        }
    }

    fun stopVpn() {
        Log.i(TAG, "Stopping VPN")
        cleanup()
        writeStatus("stopped")
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun cleanup() {
        cleanupStealth()
        releaseWakeLock()
        if (tun2socksPid > 0) {
            NativeHelper.killProcess(tun2socksPid)
            tun2socksPid = -1
        }
        try { vpnInterface?.close() } catch (_: Exception) {}
        vpnInterface = null
        isRunning = false
        instance = null
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        // User swiped app from recents — keep VPN alive
        Log.i(TAG, "App removed from recents, VPN stays active")
        super.onTaskRemoved(rootIntent)
    }

    private fun writeStatus(status: String) {
        try {
            val statusFile = File(filesDir, "nexvpn/.vpn_status")
            statusFile.parentFile?.mkdirs()
            statusFile.writeText(status)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to write status: $e")
        }
        // Request Quick Settings Tile to update its state
        try {
            TileService.requestListeningState(
                this,
                ComponentName(this, VpnTileService::class.java)
            )
        } catch (e: Exception) {
            Log.w(TAG, "Failed to request tile update: $e")
        }
        // Update home screen widget
        try {
            VpnWidgetProvider.updateAllWidgets(this)
        } catch (e: Exception) {
            Log.w(TAG, "Failed to update widget: $e")
        }
    }

    // ── Root Stealth Mode ──────────────────────────
    // Hides VPN fingerprints that are only patchable with root access:
    // 1. Fix MTU to 1500 (removes MTU anomaly detection)
    // 2. Block external access to local proxy ports via iptables (per-UID)

    private fun applyStealthMode() {
        try {
            val configFile = File(filesDir, "nexvpn/.stealth_mode")
            if (!configFile.exists() || configFile.readText().trim() != "true") return

            Log.i(TAG, "Stealth mode: activating root-based hiding")

            // Check root availability
            val rootCheck = Runtime.getRuntime().exec(arrayOf("su", "-c", "id"))
            val rootResult = rootCheck.inputStream.bufferedReader().readText()
            rootCheck.waitFor()
            if (!rootResult.contains("uid=0")) {
                Log.w(TAG, "Stealth mode: root not available")
                return
            }

            // Block other apps from scanning our local ports
            // Get our UID
            val myUid = applicationInfo.uid
            // Only allow our own UID to connect to localhost proxy ports
            val ports = listOf(lastSocksPort)
            for (port in ports) {
                // Drop connections from other UIDs to our SOCKS/HTTP ports
                execRoot("iptables -I OUTPUT -p tcp -d 127.0.0.1 --dport $port -m owner ! --uid-owner $myUid -j DROP")
                execRoot("ip6tables -I OUTPUT -p tcp -d ::1 --dport $port -m owner ! --uid-owner $myUid -j DROP")
            }
            Log.i(TAG, "Stealth: iptables rules applied for UID $myUid")

            stealthActive = true
        } catch (e: Exception) {
            Log.e(TAG, "Stealth mode failed: $e")
        }
    }

    private fun cleanupStealth() {
        if (!stealthActive) return
        try {
            val myUid = applicationInfo.uid
            // Remove our iptables rules
            val ports = listOf(lastSocksPort)
            for (port in ports) {
                execRoot("iptables -D OUTPUT -p tcp -d 127.0.0.1 --dport $port -m owner ! --uid-owner $myUid -j DROP")
                execRoot("ip6tables -D OUTPUT -p tcp -d ::1 --dport $port -m owner ! --uid-owner $myUid -j DROP")
            }
            Log.i(TAG, "Stealth: iptables rules cleaned up")
            stealthActive = false
        } catch (e: Exception) {
            Log.w(TAG, "Stealth cleanup failed: $e")
        }
    }

    private fun execRoot(command: String) {
        try {
            val proc = Runtime.getRuntime().exec(arrayOf("su", "-c", command))
            proc.waitFor()
        } catch (e: Exception) {
            Log.w(TAG, "Root exec failed: $command — $e")
        }
    }

    private fun applyPerAppConfig(builder: Builder) {
        try {
            val configFile = File(filesDir, "nexvpn/.per_app_config")
            if (!configFile.exists()) return

            val lines = configFile.readLines()
            if (lines.isEmpty()) return

            val mode = lines[0].trim()
            val apps = lines.drop(1).map { it.trim() }.filter { it.isNotEmpty() }

            if (apps.isEmpty() || mode == "all") return

            when (mode) {
                "include" -> {
                    // Only these apps go through VPN
                    for (pkg in apps) {
                        try {
                            builder.addAllowedApplication(pkg)
                        } catch (e: Exception) {
                            Log.w(TAG, "Cannot add allowed app $pkg: $e")
                        }
                    }
                    // Must also allow our own app's traffic (for sing-box)
                    // Actually we excluded ourselves above, so this is fine
                    Log.i(TAG, "Per-app VPN: include mode, ${apps.size} apps")
                }
                "exclude" -> {
                    // These apps bypass VPN
                    for (pkg in apps) {
                        try {
                            builder.addDisallowedApplication(pkg)
                        } catch (e: Exception) {
                            Log.w(TAG, "Cannot add disallowed app $pkg: $e")
                        }
                    }
                    Log.i(TAG, "Per-app VPN: exclude mode, ${apps.size} apps")
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to read per-app config: $e")
        }
    }

    override fun onDestroy() {
        cleanup()
        super.onDestroy()
    }

    override fun onRevoke() {
        Log.i(TAG, "VPN permission revoked")
        cleanup()
        writeStatus("revoked")
        super.onRevoke()
    }
}
