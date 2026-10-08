package com.scratchnote.app

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.Settings
import androidx.activity.result.ActivityResult
import app.tauri.PermissionState
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/**
 * Chooses a shared folder for the notes and relaunches the app, for
 * src/android.rs. The backend reads and writes the notes as plain files, so a
 * folder outside the app's own storage needs "All files access" (Android 11
 * and later) or the storage permission (before).
 */
@TauriPlugin(
  permissions = [
    Permission(strings = [Manifest.permission.WRITE_EXTERNAL_STORAGE], alias = "storage")
  ]
)
class StoragePlugin(private val activity: Activity) : Plugin(activity) {
  /** The folder chosen, while the app asks for access to it. */
  private var picked: String? = null

  @Command
  fun pickFolder(invoke: Invoke) {
    startActivityForResult(invoke, Intent(Intent.ACTION_OPEN_DOCUMENT_TREE), "folderPicked")
  }

  @ActivityCallback
  fun folderPicked(invoke: Invoke, result: ActivityResult) {
    val uri = result.data?.data
    if (result.resultCode != Activity.RESULT_OK || uri == null) {
      answer(invoke, null)
      return
    }
    val path = pathOf(uri)
    if (path == null) {
      invoke.reject("Choose a folder on the device's storage or SD card")
      return
    }
    picked = path
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
      if (Environment.isExternalStorageManager()) {
        answer(invoke, path)
      } else {
        val ask = Intent(
          Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
          Uri.parse("package:${activity.packageName}")
        )
        startActivityForResult(invoke, ask, "accessAnswered")
      }
    } else if (getPermissionState("storage") == PermissionState.GRANTED) {
      answer(invoke, path)
    } else {
      requestPermissionForAlias("storage", invoke, "permissionAnswered")
    }
  }

  @ActivityCallback
  fun accessAnswered(invoke: Invoke, result: ActivityResult) {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && Environment.isExternalStorageManager()) {
      answer(invoke, picked)
    } else {
      invoke.reject("Scratchnote needs \"All files access\" to keep notes in that folder")
    }
  }

  @PermissionCallback
  fun permissionAnswered(invoke: Invoke) {
    if (getPermissionState("storage") == PermissionState.GRANTED) {
      answer(invoke, picked)
    } else {
      invoke.reject("Scratchnote needs storage access to keep notes in that folder")
    }
  }

  @Command
  fun restart(invoke: Invoke) {
    val launch = activity.packageManager.getLaunchIntentForPackage(activity.packageName)
    if (launch?.component == null) {
      invoke.reject("no launcher activity to start again")
      return
    }
    activity.startActivity(Intent.makeRestartActivityTask(launch.component))
    invoke.resolve()
    Runtime.getRuntime().exit(0)
  }

  private fun answer(invoke: Invoke, path: String?) {
    val result = JSObject()
    result.put("path", path)
    invoke.resolve(result)
  }

  /**
   * The file path behind a folder the system chooser returns, such as
   * primary:Documents/Notes for /storage/emulated/0/Documents/Notes. Null
   * for providers that are not a disk, such as a cloud drive.
   */
  private fun pathOf(tree: Uri): String? {
    if (tree.authority != "com.android.externalstorage.documents") return null
    val id = DocumentsContract.getTreeDocumentId(tree)
    val volume = id.substringBefore(':')
    val inside = id.substringAfter(':', "")
    val base = if (volume.equals("primary", ignoreCase = true)) {
      Environment.getExternalStorageDirectory().path
    } else {
      "/storage/$volume"
    }
    return if (inside.isEmpty()) base else "$base/$inside"
  }
}
