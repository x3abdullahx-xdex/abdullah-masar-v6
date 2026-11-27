package com.abdullah.masar.timer

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.CountDownTimer
import android.os.IBinder
import android.os.PowerManager

class FocusForegroundService : Service() {
    private var timer: CountDownTimer? = null
    private var wakeLock: PowerManager.WakeLock? = null

    companion object {
        const val CHANNEL_ID = "masar_zen_focus_channel"
        const val NOTIF_ID = 6001
        const val EXTRA_DURATION_SECS = "duration_secs"
        const val EXTRA_LABEL = "label"
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val durationSecs = intent?.getLongExtra(EXTRA_DURATION_SECS, 1500L) ?: 1500L
        val label = intent?.getStringExtra(EXTRA_LABEL) ?: "جلسة تركيز Zen"

        createNotificationChannel()
        acquireWakeLock(durationSecs)
        startForeground(NOTIF_ID, buildNotification(label, formatTime(durationSecs)))

        timer?.cancel()
        timer = object : CountDownTimer(durationSecs * 1000L, 1000L) {
            override fun onTick(millisUntilFinished: Long) {
                val secsLeft = millisUntilFinished / 1000L
                val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
                nm.notify(NOTIF_ID, buildNotification(label, formatTime(secsLeft)))
            }

            override fun onFinish() {
                val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
                nm.notify(NOTIF_ID, buildNotification(label, "اكتملت الجلسة ✓"))
                releaseWakeLock()
                stopSelf()
            }
        }.start()

        return START_STICKY
    }

    private fun acquireWakeLock(durationSecs: Long) {
        val pm = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "MasarV6::ZenFocusWakeLock")
        wakeLock?.acquire((durationSecs + 30) * 1000L)
    }

    private fun releaseWakeLock() {
        try {
            if (wakeLock?.isHeld == true) wakeLock?.release()
        } catch (_: Exception) {}
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "مؤقت التركيز — المسار اليومي",
                NotificationManager.IMPORTANCE_LOW
            )
            val nm = getSystemService(NotificationManager::class.java)
            nm.createNotificationChannel(channel)
        }
    }

    private fun buildNotification(title: String, remaining: String): Notification {
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        return builder
            .setContentTitle(title)
            .setContentText("المتبقي: $remaining")
            .setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
            .setOngoing(true)
            .build()
    }

    private fun formatTime(secs: Long): String {
        val m = secs / 60
        val s = secs % 60
        return String.format("%02d:%02d", m, s)
    }

    override fun onDestroy() {
        timer?.cancel()
        releaseWakeLock()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null
}
