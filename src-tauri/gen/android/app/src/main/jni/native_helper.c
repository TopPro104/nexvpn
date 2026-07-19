#include <jni.h>
#include <unistd.h>
#include <signal.h>
#include <sys/wait.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <fcntl.h>

/*
 * Fork + exec a child process WITHOUT closing inherited file descriptors.
 * This is needed because Android's ProcessBuilder closes all fds > 2,
 * which prevents passing the VPN TUN fd to tun2socks.
 */
JNIEXPORT jint JNICALL
Java_com_horusvpn_nexvpn_NativeHelper_startProcess(
    JNIEnv *env, jclass clazz,
    jstring jpath, jobjectArray jargs, jint unCloexecFd) {

    const char *path = (*env)->GetStringUTFChars(env, jpath, NULL);
    if (!path) return -1;

    int argc = (*env)->GetArrayLength(env, jargs);
    /* argv: [path, arg0, arg1, ..., NULL] */
    char **argv = (char **)calloc(argc + 2, sizeof(char *));
    if (!argv) {
        (*env)->ReleaseStringUTFChars(env, jpath, path);
        return -1;
    }

    argv[0] = (char *)path;
    for (int i = 0; i < argc; i++) {
        jstring arg = (jstring)(*env)->GetObjectArrayElement(env, jargs, i);
        argv[i + 1] = (char *)(*env)->GetStringUTFChars(env, arg, NULL);
    }
    argv[argc + 1] = NULL;

    pid_t pid = fork();
    if (pid == 0) {
        /* Child — all fds are inherited, just exec.
         * The TUN fd carries FD_CLOEXEC (set by VpnService), so it would be
         * closed on execv and never reach tun2socks. Clear it here in native
         * code — Java's android.system.Os.fcntlInt is a hidden non-SDK method
         * that throws NoSuchMethodError on Android 10, crashing the service. */
        if (unCloexecFd >= 0) {
            int flags = fcntl(unCloexecFd, F_GETFD);
            if (flags != -1) {
                fcntl(unCloexecFd, F_SETFD, flags & ~FD_CLOEXEC);
            }
        }
        execv(path, argv);
        /* execv only returns on error */
        _exit(127);
    }

    /* Parent — cleanup JNI strings */
    for (int i = 0; i < argc; i++) {
        jstring arg = (jstring)(*env)->GetObjectArrayElement(env, jargs, i);
        (*env)->ReleaseStringUTFChars(env, arg, argv[i + 1]);
    }
    free(argv);
    (*env)->ReleaseStringUTFChars(env, jpath, path);

    return (jint)pid;
}

/* Send SIGTERM and reap zombie */
JNIEXPORT void JNICALL
Java_com_horusvpn_nexvpn_NativeHelper_killProcess(
    JNIEnv *env, jclass clazz, jint pid) {
    if (pid > 0) {
        kill((pid_t)pid, SIGTERM);
        int status;
        waitpid((pid_t)pid, &status, WNOHANG);
    }
}

/* Check if process is alive */
JNIEXPORT jboolean JNICALL
Java_com_horusvpn_nexvpn_NativeHelper_isProcessAlive(
    JNIEnv *env, jclass clazz, jint pid) {
    if (pid <= 0) return JNI_FALSE;
    int status;
    pid_t ret = waitpid((pid_t)pid, &status, WNOHANG);
    if (ret == 0) return JNI_TRUE;   /* still running */
    return JNI_FALSE;                /* exited or error */
}
