package com.lettuceai.app

import android.content.Context
import java.io.File

object KokoroPhonemizerBridge {
  private const val ASSET_DATA_DIR = "kokoro/espeak-ng-data"

  @JvmStatic
  fun resolve(context: Context): String {
    val appContext = context.applicationContext
    val dataDir = File(appContext.filesDir, ASSET_DATA_DIR)
    ensureAssetDirectory(appContext, ASSET_DATA_DIR, dataDir)
    if (!dataDir.isDirectory || !File(dataDir, "phontab").isFile) {
      throw IllegalStateException(
        "Bundled Kokoro phonemizer data is missing. Package eSpeak NG data under assets/$ASSET_DATA_DIR."
      )
    }

    return dataDir.absolutePath
  }

  private fun ensureAssetDirectory(context: Context, assetPath: String, destination: File) {
    val children = context.assets.list(assetPath) ?: emptyArray()
    if (children.isEmpty()) {
      copyAssetFile(context, assetPath, destination)
      return
    }

    if (!destination.exists() && !destination.mkdirs()) {
      throw IllegalStateException("Failed to create ${destination.absolutePath}")
    }

    for (child in children) {
      ensureAssetDirectory(context, "$assetPath/$child", File(destination, child))
    }
  }

  private fun copyAssetFile(context: Context, assetPath: String, destination: File) {
    destination.parentFile?.mkdirs()
    context.assets.open(assetPath).use { input ->
      destination.outputStream().use { output ->
        input.copyTo(output)
      }
    }
  }
}
