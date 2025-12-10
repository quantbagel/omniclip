//! Cross-platform clipboard abstraction

use std::time::Duration;
use tokio::sync::mpsc;
use arboard::Clipboard as ArboardClipboard;

use crate::protocol::{ClipboardContent, ContentHash};
use crate::{Error, Result};

/// Clipboard manager for reading, writing, and monitoring changes
pub struct ClipboardManager {
    /// Last known content hash (for change detection)
    last_hash: Option<ContentHash>,
}

impl ClipboardManager {
    pub fn new() -> Self {
        Self { last_hash: None }
    }

    /// Read current clipboard content
    pub fn read(&self) -> Result<Option<ClipboardContent>> {
        let mut clipboard = ArboardClipboard::new()
            .map_err(|e| Error::Clipboard(e.to_string()))?;

        // Try to get text content
        match clipboard.get_text() {
            Ok(text) if !text.is_empty() => Ok(Some(ClipboardContent::Text(text))),
            Ok(_) => Ok(None),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(Error::Clipboard(e.to_string())),
        }
    }

    /// Write content to clipboard
    pub fn write(&self, content: &ClipboardContent) -> Result<()> {
        let mut clipboard = ArboardClipboard::new()
            .map_err(|e| Error::Clipboard(e.to_string()))?;

        match content {
            ClipboardContent::Text(text) => {
                clipboard.set_text(text)
                    .map_err(|e| Error::Clipboard(e.to_string()))
            }
            ClipboardContent::RichText { plain, .. } => {
                // For now, just set plain text (rich text support varies by platform)
                clipboard.set_text(plain)
                    .map_err(|e| Error::Clipboard(e.to_string()))
            }
        }
    }

    /// Check if clipboard content has changed since last check
    pub fn check_change(&mut self) -> Result<Option<ClipboardContent>> {
        let content = self.read()?;

        match &content {
            Some(c) => {
                let hash = c.hash();
                if self.last_hash.as_ref() != Some(&hash) {
                    self.last_hash = Some(hash);
                    Ok(content)
                } else {
                    Ok(None)
                }
            }
            None => {
                self.last_hash = None;
                Ok(None)
            }
        }
    }

    /// Update the last hash without triggering a change event
    /// (used when we write content ourselves)
    pub fn update_hash(&mut self, content: &ClipboardContent) {
        self.last_hash = Some(content.hash());
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Clipboard change event
#[derive(Debug, Clone)]
pub struct ClipboardChange {
    pub content: ClipboardContent,
    pub hash: ContentHash,
}

/// Start a clipboard monitoring task that sends changes to a channel
pub fn start_monitor(
    poll_interval: Duration,
) -> (mpsc::Receiver<ClipboardChange>, tokio::task::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel(16);

    let handle = tokio::spawn(async move {
        let mut manager = ClipboardManager::new();

        loop {
            tokio::time::sleep(poll_interval).await;

            match manager.check_change() {
                Ok(Some(content)) => {
                    let hash = content.hash();
                    if tx.send(ClipboardChange { content, hash }).await.is_err() {
                        // Receiver dropped, stop monitoring
                        break;
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!("clipboard read error: {}", e);
                }
            }
        }
    });

    (rx, handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_roundtrip() {
        let manager = ClipboardManager::new();
        let content = ClipboardContent::Text("omniclip test".to_string());

        // This test may fail if run in headless environment
        if manager.write(&content).is_ok() {
            let read = manager.read().unwrap();
            assert!(read.is_some());
            if let Some(ClipboardContent::Text(text)) = read {
                assert_eq!(text, "omniclip test");
            }
        }
    }

    #[test]
    fn test_change_detection() {
        let mut manager = ClipboardManager::new();

        // First read captures the initial state
        let _ = manager.check_change();

        // Same content should not trigger change
        let _ = manager.check_change();
        // (Can't really test this without changing clipboard)
    }
}
