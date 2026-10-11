//! Passive error-object projection avoids native string conversion on non-strings.
use ottd_script::{Value, Vm, VmError};
use std::io::Write;

/// Consume the explicit trace option before parsing unchanged numeric credits.
pub fn trace_flag(args: &mut std::iter::Peekable<impl Iterator<Item = String>>) -> bool {
    let requested = args.next_if(|argument| argument == "--errors").is_some();
    requested || std::env::var_os("OTTD_SCRIPT_ERROR_TRACE").is_some()
}

/// Print the raw typed object only when the passive error trace is requested.
pub fn snapshot(enabled: bool, error: &Value) {
    if enabled {
        print!("last_error ");
        super::value(error);
    }
}
/// Preserve raw string diagnostics while arbitrary non-string throws have no text.
/// # Errors
/// Returns an error when the output stream cannot be written.
pub fn write_runtime(vm: &Vm<'_>, error: VmError) -> std::io::Result<()> {
    if error == VmError::ThrownValue && vm.native_error().is_none() {
        return Ok(());
    }
    write_diagnostic(vm.native_error(), &error)
}

/// Write exact retained bytes, or the existing typed refusal when none were raised.
/// # Errors
/// Returns an error when the output stream cannot be written.
pub fn write_diagnostic(
    message: Option<&ottd_script::ByteString>,
    error: &impl std::fmt::Display,
) -> std::io::Result<()> {
    if let Some(message) = message {
        std::io::stderr().write_all(message.as_bytes())?;
        std::io::stderr().write_all(b"\n")
    } else {
        writeln!(std::io::stderr(), "{error}")
    }
}
