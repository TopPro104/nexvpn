package com.horusvpn.nexvpn

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.util.Log
import android.widget.RemoteViews
import java.io.File

class VpnWidgetProvider : AppWidgetProvider() {
    companion object {
        const val TAG = "NexVPN-Widget"
        const val ACTION_TOGGLE = "com.horusvpn.nexvpn.WIDGET_TOGGLE"

        fun updateAllWidgets(context: Context) {
            val intent = Intent(context, VpnWidgetProvider::class.java).apply {
                action = AppWidgetManager.ACTION_APPWIDGET_UPDATE
            }
            val ids = AppWidgetManager.getInstance(context)
                .getAppWidgetIds(ComponentName(context, VpnWidgetProvider::class.java))
            intent.putExtra(AppWidgetManager.EXTRA_APPWIDGET_IDS, ids)
            context.sendBroadcast(intent)
        }
    }

    override fun onUpdate(context: Context, appWidgetManager: AppWidgetManager, appWidgetIds: IntArray) {
        for (widgetId in appWidgetIds) {
            updateWidget(context, appWidgetManager, widgetId)
        }
    }

    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)

        if (intent.action == ACTION_TOGGLE) {
            Log.i(TAG, "Widget toggle tapped")
            val isRunning = readVpnStatus(context)

            // Write tile_action for the frontend to pick up (same mechanism as Quick Tile)
            val action = if (isRunning) "disconnect" else "connect"
            writeTileAction(context, action)

            // Launch MainActivity invisibly to process the action
            val launchIntent = Intent(context, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
                putExtra("from_tile", true)
                this.action = VpnTileService.ACTION_TILE
                putExtra(VpnTileService.EXTRA_ACTION, action)
            }
            context.startActivity(launchIntent)

            // Update widget immediately with transitional state
            val mgr = AppWidgetManager.getInstance(context)
            val ids = mgr.getAppWidgetIds(ComponentName(context, VpnWidgetProvider::class.java))
            for (id in ids) {
                updateWidget(context, mgr, id)
            }
        }
    }

    private fun updateWidget(context: Context, manager: AppWidgetManager, widgetId: Int) {
        val isRunning = readVpnStatus(context)

        val views = RemoteViews(context.packageName, R.layout.widget_vpn)

        views.setTextViewText(R.id.widget_status,
            if (isRunning) "Connected" else "Disconnected"
        )
        views.setTextColor(R.id.widget_status,
            if (isRunning) 0xFF4CAF50.toInt() else 0xFFAAAAAA.toInt()
        )
        views.setImageViewResource(R.id.widget_toggle,
            if (isRunning) android.R.drawable.ic_media_pause else android.R.drawable.ic_media_play
        )

        // Toggle intent on tap
        val toggleIntent = Intent(context, VpnWidgetProvider::class.java).apply {
            action = ACTION_TOGGLE
        }
        val pendingIntent = PendingIntent.getBroadcast(
            context, 0, toggleIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        views.setOnClickPendingIntent(R.id.widget_toggle, pendingIntent)

        // Tap title → open app
        val openIntent = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
        }
        val openPending = PendingIntent.getActivity(
            context, 1, openIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        views.setOnClickPendingIntent(R.id.widget_title, openPending)

        manager.updateAppWidget(widgetId, views)
    }

    private fun readVpnStatus(context: Context): Boolean {
        for (base in arrayOf(
            "/data/data/com.horusvpn.nexvpn/files",
            "/data/user/0/com.horusvpn.nexvpn/files"
        )) {
            val file = File(base, "nexvpn/.vpn_status")
            if (file.exists()) {
                return file.readText().trim() == "running"
            }
        }
        return NexVpnService.isRunning
    }

    private fun writeTileAction(context: Context, action: String) {
        for (base in arrayOf(
            "/data/data/com.horusvpn.nexvpn/files",
            "/data/user/0/com.horusvpn.nexvpn/files"
        )) {
            val dir = File(base, "nexvpn")
            if (dir.exists() || dir.mkdirs()) {
                File(dir, ".tile_action").writeText(action)
                return
            }
        }
    }
}
