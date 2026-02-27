/**
 * Clipboard Helper
 * 
 * T-032: Clipboard helper with fallback for browsers without Clipboard API
 */

/**
 * Result of a clipboard operation
 */
export interface ClipboardResult {
  success: boolean
  method: 'clipboard' | 'execCommand' | 'manual'
  error?: string
}

/**
 * Copy text to clipboard
 * 
 * Uses modern Clipboard API if available, falls back to execCommand,
 * and returns manual copy instructions if both fail.
 * 
 * @param text - Text to copy to clipboard
 * @returns Promise resolving to ClipboardResult
 */
export async function copyToClipboard(text: string): Promise<ClipboardResult> {
  // Try modern Clipboard API first
  if (navigator.clipboard && typeof navigator.clipboard.writeText === 'function') {
    try {
      await navigator.clipboard.writeText(text)
      return { success: true, method: 'clipboard' }
    } catch (error) {
      // Fall through to next method
      console.warn('[clipboard] Clipboard API failed:', error)
    }
  }

  // Fallback: execCommand (deprecated but widely supported)
  try {
    const textArea = document.createElement('textarea')
    textArea.value = text
    
    // Prevent scrolling to bottom
    textArea.style.cssText = `
      position: fixed;
      top: 0;
      left: 0;
      width: 2em;
      height: 2em;
      padding: 0;
      border: none;
      outline: none;
      box-shadow: none;
      background: transparent;
      z-index: -1;
    `
    
    document.body.appendChild(textArea)
    textArea.focus()
    textArea.select()
    
    const successful = document.execCommand('copy')
    document.body.removeChild(textArea)
    
    if (successful) {
      return { success: true, method: 'execCommand' }
    }
  } catch (error) {
    console.warn('[clipboard] execCommand failed:', error)
  }

  // All methods failed
  return { 
    success: false, 
    method: 'manual',
    error: 'clipboard.unavailable'
  }
}

/**
 * Check if clipboard API is available
 */
export function isClipboardSupported(): boolean {
  return !!(navigator.clipboard && typeof navigator.clipboard.writeText === 'function')
}
