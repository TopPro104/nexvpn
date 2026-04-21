package com.horusvpn.nexvpn.xposed

import de.robv.android.xposed.IXposedHookLoadPackage
import de.robv.android.xposed.XC_MethodHook
import de.robv.android.xposed.XposedBridge
import de.robv.android.xposed.XposedHelpers
import de.robv.android.xposed.callbacks.XC_LoadPackage

/**
 * Xposed module that hides VPN presence from other apps.
 *
 * Enable in LSPosed → Modules → NexVPN, then select target apps in scope
 * (e.g. banking apps, government apps, detection checkers).
 *
 * Hooks:
 * 1. NetworkCapabilities — hide TRANSPORT_VPN, add NOT_VPN, hide VpnTransportInfo
 * 2. NetworkInterface — filter out tun/tap/ppp interfaces, fix MTU
 * 3. PackageManager — hide known VPN packages
 */
class VpnHideModule : IXposedHookLoadPackage {

    companion object {
        const val TAG = "NexVPN-Xposed"

        const val TRANSPORT_VPN = 4
        const val NET_CAPABILITY_NOT_VPN = 15

        val VPN_PACKAGES = setOf(
            "com.horusvpn.nexvpn",
            "com.v2ray.ang",
            "moe.nb4a",
            "app.hiddify.com",
            "com.happproxy",
            "fr.niclas.music",
            "com.github.clash",
            "org.amnezia.vpn",
            "com.wireguard.android",
            "de.blinkt.openvpn",
            "net.openvpn.openvpn",
            "org.torproject.torbrowser",
            "com.psiphon3",
            "com.psiphon3.subscription",
        )

        val VPN_INTERFACE_PREFIXES = arrayOf("tun", "tap", "ppp", "wg", "ipsec", "utun")
    }

    override fun handleLoadPackage(lpparam: XC_LoadPackage.LoadPackageParam) {
        // Don't hook ourselves or system WebView
        if (lpparam.packageName == "com.horusvpn.nexvpn") return
        if (lpparam.packageName.contains("webview", ignoreCase = true)) return
        if (lpparam.packageName == "com.google.android.trichromelibrary") return

        try {
            XposedBridge.log("$TAG: hooking ${lpparam.packageName}")
            hookNetworkCapabilities(lpparam)
            hookNetworkInterface(lpparam)
            hookPackageManager(lpparam)
            writeModuleStatus(lpparam.packageName)
            XposedBridge.log("$TAG: all hooks installed for ${lpparam.packageName}")
        } catch (t: Throwable) {
            XposedBridge.log("$TAG: error hooking ${lpparam.packageName}: $t")
        }
    }

    private fun writeModuleStatus(hookedPackage: String) {
        try {
            // Write status to NexVPN's data dir so the app can read it
            for (base in arrayOf(
                "/data/data/com.horusvpn.nexvpn/files",
                "/data/user/0/com.horusvpn.nexvpn/files"
            )) {
                val dir = java.io.File(base, "nexvpn")
                if (dir.exists() || dir.mkdirs()) {
                    val file = java.io.File(dir, ".xposed_status")
                    val timestamp = System.currentTimeMillis()
                    // Append hooked package with timestamp
                    val existing = if (file.exists()) file.readText() else ""
                    val lines = existing.lines().filter { it.isNotBlank() }.toMutableList()
                    lines.removeAll { it.startsWith("$hookedPackage:") }
                    lines.add("$hookedPackage:$timestamp")
                    // Keep last 50 entries
                    val trimmed = lines.takeLast(50)
                    file.writeText(trimmed.joinToString("\n"))
                    return
                }
            }
        } catch (_: Throwable) {
            // Writing status is best-effort, don't crash
        }
    }

    private fun hookNetworkCapabilities(lpparam: XC_LoadPackage.LoadPackageParam) {
        // NetworkCapabilities is a framework class loaded by boot classloader — use null
        val ncClass = XposedHelpers.findClass("android.net.NetworkCapabilities", null)

        // hasTransport(TRANSPORT_VPN) → false
        try {
            XposedHelpers.findAndHookMethod(ncClass, "hasTransport", Int::class.javaPrimitiveType,
                object : XC_MethodHook() {
                    override fun afterHookedMethod(param: MethodHookParam) {
                        if (param.args[0] as Int == TRANSPORT_VPN) {
                            param.result = false
                        }
                    }
                }
            )
            XposedBridge.log("$TAG: hasTransport hook OK")
        } catch (t: Throwable) {
            XposedBridge.log("$TAG: hasTransport hook failed: $t")
        }

        // hasCapability(NET_CAPABILITY_NOT_VPN) → true
        try {
            XposedHelpers.findAndHookMethod(ncClass, "hasCapability", Int::class.javaPrimitiveType,
                object : XC_MethodHook() {
                    override fun afterHookedMethod(param: MethodHookParam) {
                        if (param.args[0] as Int == NET_CAPABILITY_NOT_VPN) {
                            param.result = true
                        }
                    }
                }
            )
            XposedBridge.log("$TAG: hasCapability hook OK")
        } catch (t: Throwable) {
            XposedBridge.log("$TAG: hasCapability hook failed: $t")
        }

        // getTransportInfo() → null (hide VpnTransportInfo)
        try {
            XposedHelpers.findAndHookMethod(ncClass, "getTransportInfo",
                object : XC_MethodHook() {
                    override fun afterHookedMethod(param: MethodHookParam) {
                        val result = param.result ?: return
                        if (result.javaClass.name.contains("Vpn")) {
                            param.result = null
                        }
                    }
                }
            )
            XposedBridge.log("$TAG: getTransportInfo hook OK")
        } catch (_: Throwable) {}

        // toString() → remove VPN references from string representation
        try {
            XposedHelpers.findAndHookMethod(ncClass, "toString",
                object : XC_MethodHook() {
                    override fun afterHookedMethod(param: MethodHookParam) {
                        var str = param.result as? String ?: return
                        str = str.replace("VPN", "WIFI")
                            .replace("NOT_VPN", "")
                        param.result = str
                    }
                }
            )
        } catch (_: Throwable) {}
    }

    private fun hookNetworkInterface(lpparam: XC_LoadPackage.LoadPackageParam) {
        val niClass = XposedHelpers.findClass("java.net.NetworkInterface", null)

        // getNetworkInterfaces() → filter out VPN interfaces
        try {
            XposedHelpers.findAndHookMethod(niClass, "getNetworkInterfaces",
                object : XC_MethodHook() {
                    @Suppress("UNCHECKED_CAST")
                    override fun afterHookedMethod(param: MethodHookParam) {
                        try {
                            val result = param.result as? java.util.Enumeration<java.net.NetworkInterface> ?: return
                            val list = java.util.Collections.list(result)
                            val filtered = list.filter { iface ->
                                val name = try { iface.name } catch (_: Throwable) { "" }
                                !VPN_INTERFACE_PREFIXES.any { name.startsWith(it) }
                            }
                            param.result = java.util.Collections.enumeration(filtered)
                        } catch (_: Throwable) {}
                    }
                }
            )
        } catch (t: Throwable) {
            XposedBridge.log("$TAG: getNetworkInterfaces hook failed: $t")
        }

        // getName() → rename VPN interfaces
        try {
            XposedHelpers.findAndHookMethod(niClass, "getName",
                object : XC_MethodHook() {
                    override fun afterHookedMethod(param: MethodHookParam) {
                        val name = param.result as? String ?: return
                        if (VPN_INTERFACE_PREFIXES.any { name.startsWith(it) }) {
                            param.result = "rmnet_data3"
                        }
                    }
                }
            )
        } catch (_: Throwable) {}

        // getMTU() → normalize VPN MTU to 1500
        try {
            XposedHelpers.findAndHookMethod(niClass, "getMTU",
                object : XC_MethodHook() {
                    override fun afterHookedMethod(param: MethodHookParam) {
                        val mtu = param.result as? Int ?: return
                        if (mtu in 1200..1499) {
                            param.result = 1500
                        }
                    }
                }
            )
        } catch (_: Throwable) {}
    }

    private fun hookPackageManager(lpparam: XC_LoadPackage.LoadPackageParam) {
        // Hook all PM methods via XposedBridge.hookAllMethods for compatibility
        // across Android versions (avoids signature mismatch on Android 13+)

        try {
            val pmClass = XposedHelpers.findClass(
                "android.app.ApplicationPackageManager", lpparam.classLoader
            )

            // getInstalledApplications — all overloads
            for (method in pmClass.declaredMethods) {
                if (method.name == "getInstalledApplications") {
                    XposedBridge.hookMethod(method, object : XC_MethodHook() {
                        @Suppress("UNCHECKED_CAST")
                        override fun afterHookedMethod(param: MethodHookParam) {
                            try {
                                val result = param.result as? MutableList<*> ?: return
                                result.removeAll { item ->
                                    val pkg = (item as? android.content.pm.ApplicationInfo)?.packageName
                                    pkg != null && VPN_PACKAGES.contains(pkg)
                                }
                            } catch (_: Throwable) {}
                        }
                    })
                }
            }

            // getInstalledPackages — all overloads
            for (method in pmClass.declaredMethods) {
                if (method.name == "getInstalledPackages") {
                    XposedBridge.hookMethod(method, object : XC_MethodHook() {
                        @Suppress("UNCHECKED_CAST")
                        override fun afterHookedMethod(param: MethodHookParam) {
                            try {
                                val result = param.result as? MutableList<*> ?: return
                                result.removeAll { item ->
                                    val pkg = (item as? android.content.pm.PackageInfo)?.packageName
                                    pkg != null && VPN_PACKAGES.contains(pkg)
                                }
                            } catch (_: Throwable) {}
                        }
                    })
                }
            }

            // queryIntentActivities — all overloads
            for (method in pmClass.declaredMethods) {
                if (method.name == "queryIntentActivities") {
                    XposedBridge.hookMethod(method, object : XC_MethodHook() {
                        @Suppress("UNCHECKED_CAST")
                        override fun afterHookedMethod(param: MethodHookParam) {
                            try {
                                val result = param.result as? MutableList<*> ?: return
                                result.removeAll { item ->
                                    val pkg = (item as? android.content.pm.ResolveInfo)?.activityInfo?.packageName
                                    pkg != null && VPN_PACKAGES.contains(pkg)
                                }
                            } catch (_: Throwable) {}
                        }
                    })
                }
            }

            // NOTE: getApplicationInfo and getPackageInfo hooks removed —
            // throwing NameNotFoundException breaks WebView (Chromium) initialization.
            // List-filtering hooks above are sufficient to hide VPN apps from enumeration.

        } catch (t: Throwable) {
            XposedBridge.log("$TAG: PackageManager hooks failed: $t")
        }
    }
}
