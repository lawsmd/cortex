//! Windows-specific utilities.

// CORTEX-BEGIN: attach-console-preserve-redirection
/// Attaches the current process to the console of the parent process.
///
/// This is useful for command-line interfaces that need to ensure all standard
/// output gets printed correctly when run from a terminal.
///
/// GUI-subsystem builds (prod `Cortex.exe`) start with no console, so CLI
/// subcommand output would otherwise be discarded. `AttachConsole` fixes the
/// interactive case, but it also resets the process's standard-handle table
/// to the console's buffers — silently discarding any pipe/file redirection
/// the parent set up (`cortex orchestrate ... | foo`, `> out.txt`, or an
/// agent harness capturing output). We snapshot the standard handles before
/// attaching and restore every one that was valid, so redirection survives
/// and only genuinely-unset handles fall through to the parent console.
pub fn attach_to_parent_console() {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Console::{
        AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
        STD_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

    fn existing_handle(id: STD_HANDLE) -> Option<HANDLE> {
        match unsafe { GetStdHandle(id) } {
            Ok(handle) if !handle.is_invalid() => Some(handle),
            _ => None,
        }
    }

    let std_handles = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]
        .map(|id| (id, existing_handle(id)));

    if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) }.is_err() {
        // Either we already have a console (console-subsystem dev builds) or
        // there is no parent console to attach to. Both leave the standard
        // handles untouched, so there is nothing to restore.
        return;
    }

    for (id, handle) in std_handles {
        if let Some(handle) = handle {
            let _ = unsafe { SetStdHandle(id, handle) };
        }
    }
}
// CORTEX-END: attach-console-preserve-redirection
