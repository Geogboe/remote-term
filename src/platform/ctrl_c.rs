#[cfg(windows)]
pub fn protect_child_helper() -> anyhow::Result<()> {
    use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;

    // SAFETY: the callback has the required system ABI and remains valid for
    // the process lifetime.
    let registered = unsafe { SetConsoleCtrlHandler(Some(child_helper_handler), 1) };
    if registered == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn protect_child_helper() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(windows)]
unsafe extern "system" fn child_helper_handler(control_type: u32) -> i32 {
    use windows_sys::Win32::System::Console::CTRL_C_EVENT;

    i32::from(control_type == CTRL_C_EVENT)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT};

    #[test]
    fn helper_consumes_ctrl_c_but_not_shutdown_signals() {
        // SAFETY: the callback is a pure discriminator over the event value.
        assert_eq!(unsafe { child_helper_handler(CTRL_C_EVENT) }, 1);
        // SAFETY: the callback is a pure discriminator over the event value.
        assert_eq!(unsafe { child_helper_handler(CTRL_BREAK_EVENT) }, 0);
    }
}
