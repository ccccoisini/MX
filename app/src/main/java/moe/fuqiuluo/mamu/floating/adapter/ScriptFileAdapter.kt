package moe.fuqiuluo.mamu.floating.adapter

import android.content.Context
import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.core.content.ContextCompat
import androidx.recyclerview.widget.RecyclerView
import moe.fuqiuluo.mamu.R
import moe.fuqiuluo.mamu.databinding.ItemScriptFileBinding
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

class ScriptFileAdapter(
    private val context: Context
) : RecyclerView.Adapter<ScriptFileAdapter.FileViewHolder>() {

    companion object {
        val SCRIPT_EXTENSIONS = setOf("lua")
    }

    private val files = mutableListOf<File>()
    private var selectedFile: File? = null

    var onFileClick: ((File) -> Unit)? = null
    var onFolderClick: ((File) -> Unit)? = null

    fun updateFiles(newFiles: List<File>) {
        files.clear()
        files.addAll(newFiles)
        selectedFile = null
        notifyDataSetChanged()
    }

    fun getSelectedFile(): File? = selectedFile

    fun clearSelection() {
        val oldIndex = files.indexOf(selectedFile)
        selectedFile = null
        if (oldIndex >= 0) notifyItemChanged(oldIndex)
    }

    override fun getItemCount(): Int = files.size

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): FileViewHolder {
        val binding = ItemScriptFileBinding.inflate(
            LayoutInflater.from(context), parent, false
        )
        return FileViewHolder(binding)
    }

    override fun onBindViewHolder(holder: FileViewHolder, position: Int) {
        holder.bind(files[position])
    }

    inner class FileViewHolder(
        private val binding: ItemScriptFileBinding
    ) : RecyclerView.ViewHolder(binding.root) {

        init {
            binding.root.setOnClickListener {
                val pos = bindingAdapterPosition
                if (pos == RecyclerView.NO_POSITION) return@setOnClickListener
                val file = files[pos]
                if (file.isDirectory) {
                    onFolderClick?.invoke(file)
                } else {
                    val oldIndex = files.indexOf(selectedFile)
                    selectedFile = file
                    if (oldIndex >= 0) notifyItemChanged(oldIndex)
                    notifyItemChanged(pos)
                    onFileClick?.invoke(file)
                }
            }
        }

        fun bind(file: File) {
            val isDir = file.isDirectory
            val isScript = !isDir && file.extension.lowercase() in SCRIPT_EXTENSIONS
            val isSelected = file == selectedFile

            // Icon
            binding.fileIcon.setImageResource(
                if (isDir) R.drawable.icon_folder_24px
                else R.drawable.icon_description_24px
            )

            // Icon tint
            val iconColor = when {
                isDir -> R.color.floating_text_secondary
                isScript -> R.color.floating_primary
                else -> R.color.floating_text_secondary
            }
            binding.fileIcon.setColorFilter(ContextCompat.getColor(context, iconColor))

            // Name color: script files get primary color
            val nameColor = when {
                isScript -> R.color.floating_primary
                else -> R.color.floating_text_primary
            }
            binding.fileName.setTextColor(ContextCompat.getColor(context, nameColor))
            binding.fileName.text = file.name

            // Info line
            binding.fileInfo.text = if (isDir) {
                val count = file.listFiles()?.size ?: 0
                "${count}项"
            } else {
                val sdf = SimpleDateFormat("yyyy-MM-dd HH:mm", Locale.getDefault())
                val size = formatFileSize(file.length())
                "$size · ${sdf.format(Date(file.lastModified()))}"
            }

            // Selection highlight
            binding.root.isSelected = isSelected
            binding.root.alpha = if (!isDir && !isScript) 0.6f else 1.0f
        }

        private fun formatFileSize(bytes: Long): String {
            return when {
                bytes < 1024 -> "${bytes}B"
                bytes < 1024 * 1024 -> "${bytes / 1024}KB"
                else -> String.format(Locale.US, "%.1fMB", bytes / (1024.0 * 1024.0))
            }
        }
    }
}
