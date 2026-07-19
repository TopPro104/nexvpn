package com.horusvpn.nexvpn

/**
 * JNI helper that fork+exec's a child process WITHOUT closing
 * inherited file descriptors (unlike Android's ProcessBuilder).
 * This is needed to pass the VPN TUN fd to tun2socks.
 */
object NativeHelper {
    init {
        System.loadLibrary("nativehelper")
    }

    /**
     * Fork and exec a process. All open file descriptors are inherited.
     * @param unCloexecFd fd to clear FD_CLOEXEC on in the child before exec
     *   (so the TUN fd survives execv). Pass -1 to skip.
     * @return child PID, or -1 on error
     */
    @JvmStatic
    external fun startProcess(path: String, args: Array<String>, unCloexecFd: Int): Int

    /** Send SIGTERM to process */
    @JvmStatic
    external fun killProcess(pid: Int)

    /** Check if process is still running */
    @JvmStatic
    external fun isProcessAlive(pid: Int): Boolean
}
