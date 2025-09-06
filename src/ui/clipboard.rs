//! Clipboard integration for copy/paste functionality

use crate::utils::errors::KaiError;
use crate::Result;

/// Clipboard manager for handling copy/paste operations
pub struct ClipboardManager {
    /// Internal clipboard storage (fallback when system clipboard is unavailable)
    internal_clipboard: String,
}

impl ClipboardManager {
    /// Create a new clipboard manager
    pub fn new() -> Self {
        Self {
            internal_clipboard: String::new(),
        }
    }

    /// Copy text to clipboard
    pub async fn copy(&mut self, text: &str) -> Result<()> {
        // Try to use system clipboard first, fall back to internal storage
        match self.copy_to_system_clipboard(text).await {
            Ok(()) => Ok(()),
            Err(_) => {
                // Fallback to internal clipboard
                self.internal_clipboard = text.to_string();
                Ok(())
            }
        }
    }

    /// Paste text from clipboard
    pub async fn paste(&self) -> Result<String> {
        // Try to get from system clipboard first, fall back to internal storage
        match self.paste_from_system_clipboard().await {
            Ok(text) => Ok(text),
            Err(_) => {
                // Fallback to internal clipboard
                Ok(self.internal_clipboard.clone())
            }
        }
    }

    /// Check if clipboard has content
    pub fn has_content(&self) -> bool {
        !self.internal_clipboard.is_empty() || self.system_clipboard_has_content()
    }

    /// Clear clipboard content
    pub async fn clear(&mut self) -> Result<()> {
        self.internal_clipboard.clear();
        // Note: We don't clear system clipboard as that would be intrusive
        Ok(())
    }

    /// Copy to system clipboard (platform-specific implementation)
    async fn copy_to_system_clipboard(&self, text: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.copy_macos(text).await
        }
        
        #[cfg(target_os = "linux")]
        {
            self.copy_linux(text).await
        }
        
        #[cfg(target_os = "windows")]
        {
            self.copy_windows(text).await
        }
        
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            Err(KaiError::ui("Clipboard not supported on this platform".to_string()))
        }
    }

    /// Paste from system clipboard (platform-specific implementation)
    async fn paste_from_system_clipboard(&self) -> Result<String> {
        #[cfg(target_os = "macos")]
        {
            self.paste_macos().await
        }
        
        #[cfg(target_os = "linux")]
        {
            self.paste_linux().await
        }
        
        #[cfg(target_os = "windows")]
        {
            self.paste_windows().await
        }
        
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            Err(KaiError::ui("Clipboard not supported on this platform".to_string()))
        }
    }

    /// Check if system clipboard has content
    fn system_clipboard_has_content(&self) -> bool {
        // This is a simplified check - in a real implementation,
        // we'd query the system clipboard without reading the content
        true
    }

    /// macOS clipboard implementation using pbcopy/pbpaste
    #[cfg(target_os = "macos")]
    async fn copy_macos(&self, text: &str) -> Result<()> {
        use std::process::Stdio;
        use tokio::io::AsyncWriteExt;
        use tokio::process::Command;

        let mut child = Command::new("pbcopy")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| KaiError::ui(format!("Failed to spawn pbcopy: {}", e)))?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(text.as_bytes()).await
            .map_err(|e| KaiError::ui(format!("Failed to write to pbcopy: {}", e)))?;
        stdin.flush().await
            .map_err(|e| KaiError::ui(format!("Failed to flush pbcopy: {}", e)))?;

        drop(stdin); // Close stdin to signal end of input

        let status = child.wait().await
            .map_err(|e| KaiError::ui(format!("Failed to wait for pbcopy: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(KaiError::ui("pbcopy command failed".to_string()))
        }
    }

    #[cfg(target_os = "macos")]
    async fn paste_macos(&self) -> Result<String> {
        use tokio::process::Command;

        let output = Command::new("pbpaste")
            .output()
            .await
            .map_err(|e| KaiError::ui(format!("Failed to run pbpaste: {}", e)))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(KaiError::ui("pbpaste command failed".to_string()))
        }
    }

    /// Linux clipboard implementation using xclip/xsel
    #[cfg(target_os = "linux")]
    async fn copy_linux(&self, text: &str) -> Result<()> {
        use std::process::Stdio;
        use tokio::io::AsyncWriteExt;
        use tokio::process::Command;

        // Try xclip first, then xsel
        let commands = ["xclip -selection clipboard", "xsel --clipboard --input"];
        
        for cmd_str in &commands {
            let cmd_parts: Vec<&str> = cmd_str.split_whitespace().collect();
            if cmd_parts.is_empty() {
                continue;
            }

            let mut command = Command::new(cmd_parts[0]);
            for arg in &cmd_parts[1..] {
                command.arg(arg);
            }

            if let Ok(mut child) = command
                .stdin(Stdio::piped())
                .spawn()
            {
                if let Some(stdin) = child.stdin.as_mut() {
                    if stdin.write_all(text.as_bytes()).await.is_ok() {
                        if let Ok(status) = child.wait().await {
                            if status.success() {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        Err(KaiError::ui("No working clipboard command found (tried xclip, xsel)".to_string()))
    }

    #[cfg(target_os = "linux")]
    async fn paste_linux(&self) -> Result<String> {
        use tokio::process::Command;

        // Try xclip first, then xsel
        let commands = ["xclip -selection clipboard -o", "xsel --clipboard --output"];
        
        for cmd_str in &commands {
            let cmd_parts: Vec<&str> = cmd_str.split_whitespace().collect();
            if cmd_parts.is_empty() {
                continue;
            }

            let mut command = Command::new(cmd_parts[0]);
            for arg in &cmd_parts[1..] {
                command.arg(arg);
            }

            if let Ok(output) = command.output().await {
                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).to_string());
                }
            }
        }

        Err(KaiError::ui("No working clipboard command found (tried xclip, xsel)".to_string()))
    }

    /// Windows clipboard implementation using clip.exe and powershell
    #[cfg(target_os = "windows")]
    async fn copy_windows(&self, text: &str) -> Result<()> {
        use std::process::Stdio;
        use tokio::io::AsyncWriteExt;
        use tokio::process::Command;

        let mut child = Command::new("cmd")
            .args(&["/C", "clip"])
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| KaiError::ui(format!("Failed to spawn clip: {}", e)))?;

        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(text.as_bytes()).await
            .map_err(|e| KaiError::ui(format!("Failed to write to clip: {}", e)))?;
        stdin.flush().await
            .map_err(|e| KaiError::ui(format!("Failed to flush clip: {}", e)))?;

        drop(stdin);

        let status = child.wait().await
            .map_err(|e| KaiError::ui(format!("Failed to wait for clip: {}", e)))?;

        if status.success() {
            Ok(())
        } else {
            Err(KaiError::ui("clip command failed".to_string()))
        }
    }

    #[cfg(target_os = "windows")]
    async fn paste_windows(&self) -> Result<String> {
        use tokio::process::Command;

        let output = Command::new("powershell")
            .args(&["-Command", "Get-Clipboard"])
            .output()
            .await
            .map_err(|e| KaiError::ui(format!("Failed to run Get-Clipboard: {}", e)))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
        } else {
            Err(KaiError::ui("Get-Clipboard command failed".to_string()))
        }
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global clipboard instance for easy access
static mut CLIPBOARD_INSTANCE: Option<ClipboardManager> = None;
static CLIPBOARD_INIT: std::sync::Once = std::sync::Once::new();

/// Get the global clipboard instance
#[allow(static_mut_refs)]
pub fn get_clipboard() -> &'static mut ClipboardManager {
    unsafe {
        CLIPBOARD_INIT.call_once(|| {
            CLIPBOARD_INSTANCE = Some(ClipboardManager::new());
        });
        CLIPBOARD_INSTANCE.as_mut().unwrap()
    }
}

/// Convenience function to copy text to clipboard
pub async fn copy_to_clipboard(text: &str) -> Result<()> {
    get_clipboard().copy(text).await
}

/// Convenience function to paste text from clipboard
pub async fn paste_from_clipboard() -> Result<String> {
    get_clipboard().paste().await
}

/// Convenience function to check if clipboard has content
pub fn clipboard_has_content() -> bool {
    get_clipboard().has_content()
}