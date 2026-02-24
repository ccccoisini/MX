package moe.fuqiuluo.mamu.floating.dialog

import android.content.Context
import android.os.Environment
import android.view.LayoutInflater
import android.view.View
import com.tencent.mmkv.MMKV
import moe.fuqiuluo.mamu.data.settings.getDialogOpacity
import moe.fuqiuluo.mamu.databinding.DialogScriptFilePickerBinding
import moe.fuqiuluo.mamu.floating.adapter.ScriptFileAdapter
import java.io.File

class ScriptFilePickerDialog(
    context: Context,
    private val initialDir: File = File(
        Environment.getExternalStorageDirectory(), "mamu/scripts"
    )
) : BaseDialog(context) {

    var onFileSelected: ((File) -> Unit)? = null

    private lateinit var binding: DialogScriptFilePickerBinding
    private lateinit var fileAdapter: ScriptFileAdapter
    private var currentDir: File = initialDir

    override fun setupDialog() {
        binding = DialogScriptFilePickerBinding.inflate(LayoutInflater.from(dialog.context))
        dialog.setContentView(binding.root)

        // Apply opacity
        val mmkv = MMKV.defaultMMKV()
        val opacity = mmkv.getDialogOpacity()
        binding.rootContainer.background?.alpha = (opacity * 255).toInt()

        // Setup file adapter
        fileAdapter = ScriptFileAdapter(context)

        fileAdapter.onFileClick = { file ->
            onFileSelected?.invoke(file)
            dialog.dismiss()
        }

        fileAdapter.onFolderClick = { folder ->
            navigateTo(folder)
        }

        binding.rvFileList.adapter = fileAdapter

        // Back button
        binding.btnBack.setOnClickListener {
            currentDir.parentFile?.let { parent ->
                navigateTo(parent)
            }
        }

        // Cancel button
        binding.btnCancel.setOnClickListener {
            onCancel?.invoke()
            dialog.dismiss()
        }

        // Ensure initial dir exists
        if (!initialDir.exists()) initialDir.mkdirs()

        // Navigate to initial directory
        navigateTo(currentDir)
    }

    private fun navigateTo(dir: File) {
        currentDir = dir
        binding.tvCurrentPath.text = dir.absolutePath

        // Enable/disable back button
        binding.btnBack.isEnabled = dir.absolutePath != "/"
                && dir.absolutePath != Environment.getExternalStorageDirectory().absolutePath

        // List files: directories first, then sorted by name
        val allFiles = dir.listFiles()?.toList() ?: emptyList()
        val sorted = allFiles.sortedWith(
            compareByDescending<File> { it.isDirectory }
                .thenBy(String.CASE_INSENSITIVE_ORDER) { it.name }
        )

        fileAdapter.updateFiles(sorted)

        // Show/hide empty state
        if (sorted.isEmpty()) {
            binding.rvFileList.visibility = View.GONE
            binding.tvEmpty.visibility = View.VISIBLE
        } else {
            binding.rvFileList.visibility = View.VISIBLE
            binding.tvEmpty.visibility = View.GONE
        }
    }
}
