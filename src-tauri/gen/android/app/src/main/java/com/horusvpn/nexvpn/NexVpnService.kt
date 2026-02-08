package com.horusvpn.nexvpn

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
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
    }

    private var vpnInterface: ParcelFileDescriptor? = null
    private var tun2socksPid: Int = -1

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val socksPort = intent?.getIntExtra("socks_port", 10808) ?: 10808

        if (isRunning) {
            Log.w(TAG, "VPN already running")
            return START_STICKY
        }

        createNotificationChannel()
        startForegroundNotification()
        startVpn(socksPort)

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
        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("NexVPN")
            .setContentText("VPN is active")
            .setSmallIcon(android.R.drawable.ic_lock_lock)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()

        startForeground(NOTIFICATION_ID, notification)
    }

    private fun startVpn(socksPort: Int) {
        try {
            val builder = Builder()
                .setSession("NexVPN")
                .addAddress("10.0.0.2", 32)
                .addRoute("0.0.0.0", 0)
                .addDnsServer("8.8.8.8")
                .addDnsServer("8.8.4.4")
                .setMtu(1500)
                .addDisallowedApplication(packageName)

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

            Log.i(TAG, "Starting tun2socks via NativeHelper: fd=$fd proxy=socks5://127.0.0.1:$socksPort")

            // Use NativeHelper (JNI fork+exec) to preserve fd inheritance.
            // Android's ProcessBuilder closes all fds > 2, which would kill the TUN fd.
            tun2socksPid = NativeHelper.startProcess(
                tun2socksPath,
                arrayOf("-device", "fd://$fd", "-proxy", "socks5://127.0.0.1:$socksPort")
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
            writeStatus("running")

            Log.i(TAG, "VPN started, tun2socks pid=$tun2socksPid")

            // Monitor tun2socks process in background
            Thread {
                try {
                    while (isRunning && NativeHelper.isProcessAlive(tun2socksPid)) {
                        Thread.sleep(2000)
                    }
                    if (isRunning) {
                        Log.w(TAG, "tun2socks process died unexpectedly")
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
        if (tun2socksPid > 0) {
            NativeHelper.killProcess(tun2socksPid)
            tun2socksPid = -1
        }
        try { vpnInterface?.close() } catch (_: Exception) {}
        vpnInterface = null
        isRunning = false
        instance = null
    }

    private fun writeStatus(status: String) {
        try {
            val statusFile = File(filesDir, "nexvpn/.vpn_status")
            statusFile.parentFile?.mkdirs()
            statusFile.writeText(status)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to write status: $e")
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
