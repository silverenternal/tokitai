//! Example common utility functions

/// Initialize console UTF-8 encoding (Windows only)
///
/// On Windows, set the console output code page to UTF-8 to ensure proper text display
#[cfg(windows)]
pub fn init_console() {
    use std::io::{self, Write};
    use windows_sys::Win32::System::Console::{SetConsoleCP, SetConsoleOutputCP};

    unsafe {
        // Set input and output code page to UTF-8
        SetConsoleCP(65001);
        SetConsoleOutputCP(65001);
    }

    // Flush stdout to ensure the settings take effect
    let _ = io::stdout().flush();
}

#[cfg(not(windows))]
pub fn init_console() {}
