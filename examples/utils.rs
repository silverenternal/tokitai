//! 示例通用工具函数

/// 初始化控制台 UTF-8 编码（仅 Windows）
///
/// 在 Windows 上设置控制台输出代码页为 UTF-8，确保中文正常显示
#[cfg(windows)]
pub fn init_console() {
    use windows_sys::Win32::System::Console::{SetConsoleCP, SetConsoleOutputCP};
    use std::io::{self, Write};

    unsafe {
        // 设置输入和输出代码页为 UTF-8
        SetConsoleCP(65001);
        SetConsoleOutputCP(65001);
    }

    // 刷新 stdout 确保设置生效
    let _ = io::stdout().flush();
}

#[cfg(not(windows))]
pub fn init_console() {}
