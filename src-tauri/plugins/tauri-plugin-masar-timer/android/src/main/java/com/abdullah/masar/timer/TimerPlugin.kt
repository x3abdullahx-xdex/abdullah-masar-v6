package com.abdullah.masar.timer

import android.app.Activity
import android.content.Intent
import android.os.Build
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@InvokeArg
class StartTimerArgs {
    var durationSecs: Long = 1500L
    var label: String = "جلسة تركيز Zen"
}

@TauriPlugin
class TimerPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun startForegroundTimer(invoke: Invoke) {
        val args = invoke.parseArgs(StartTimerArgs::class.java)
        val intent = Intent(activity, FocusForegroundService::class.java).apply {
            putExtra(FocusForegroundService.EXTRA_DURATION_SECS, args.durationSecs)
            putExtra(FocusForegroundService.EXTRA_LABEL, args.label)
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            activity.startForegroundService(intent)
        } else {
            activity.startService(intent)
        }
        invoke.resolve()
    }

    @Command
    fun stopForegroundTimer(invoke: Invoke) {
        val intent = Intent(activity, FocusForegroundService::class.java)
        activity.stopService(intent)
        invoke.resolve()
    }
}
