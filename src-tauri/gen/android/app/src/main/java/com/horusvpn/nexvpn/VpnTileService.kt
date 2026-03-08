package com.horusvpn.nexvpn

import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.util.Log
import java.io.File

class VpnTileService : TileService() {
    companion object {
        const val TAG = "NexVPN-Tile"
        const val ACTION_TILE = "com.horusvpn.nexvpn.TILE_ACTION"
        const val EXTRA_ACTION = "tile_action"
    }

    override fun onStartListening() {
        super.onStartListening()
        updateTileState()
    }

    override fun onClick() {
        super.onClick()
        val running = isVpnRunning()
        val action = if (running) "disconnect" else "connect"
        Log.i(TAG, "Tile click: $action (running=$running)")

        // Write tile action file — frontend will read it and do the actual work
        writeTileAction(action)
        // Launch app invisibly so Rust backend processes the action
        launchAppInvisibly(action)
    }

    private fun isVpnRunning(): Boolean {
        // Primary: check .vpn_status file (works across processes)
        try {
            val statusFile = File(applicationContext.filesDir, "nexvpn/.vpn_status")
            if (statusFile.exists()) {
                val status = statusFile.readText().trim()
                return status == "running"
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to read vpn status file: $e")
        }
        // Fallback: static var (same process only)
        return NexVpnService.isRunning
    }

    private fun writeTileAction(action: String) {
        try {
            val file = File(applicationContext.filesDir, "nexvpn/.tile_action")
            file.parentFile?.mkdirs()
            file.writeText(action)
            Log.i(TAG, "Wrote tile action: $action")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to write tile action: $e")
        }
    }

    private fun launchAppInvisibly(action: String) {
        try {
            val intent = Intent(applicationContext, MainActivity::class.java).apply {
                addFlags(
                    Intent.FLAG_ACTIVITY_NEW_TASK or
                    Intent.FLAG_ACTIVITY_SINGLE_TOP or
                    Intent.FLAG_ACTIVITY_NO_ANIMATION
                )
                putExtra(EXTRA_ACTION, action)
                putExtra("from_tile", true)
                this.action = ACTION_TILE
            }

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                startActivityAndCollapse(
                    android.app.PendingIntent.getActivity(
                        this, 0, intent,
                        android.app.PendingIntent.FLAG_UPDATE_CURRENT or android.app.PendingIntent.FLAG_IMMUTABLE
                    )
                )
            } else {
                @Suppress("DEPRECATION")
                startActivityAndCollapse(intent)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to launch app: $e")
        }
    }

    fun updateTileState() {
        val tile = qsTile ?: return
        val running = isVpnRunning()

        tile.state = if (running) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.label = "NexVPN"

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            tile.subtitle = if (running) "Connected" else "Tap to connect"
        }

        tile.updateTile()
    }
}
