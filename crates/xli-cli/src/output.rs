use serde::Serialize;
use std::io::{self, Write};
use xli_core::ResponseEnvelope;

/// Emit a response envelope in JSON or minimal human-readable text form.
pub fn emit<T>(envelope: &ResponseEnvelope<T>, human: bool) -> anyhow::Result<()>
where
    T: Serialize,
{
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    if human {
        writeln!(handle, "status: {:?}", envelope.status)?;
        writeln!(handle, "command: {}", envelope.command)?;
        if !envelope.errors.is_empty() {
            for error in &envelope.errors {
                writeln!(handle, "error: {error}")?;
            }
        }
        if let Some(output) = &envelope.output {
            serde_json::to_writer_pretty(&mut handle, output)?;
            writeln!(handle)?;
        }
    } else {
        serde_json::to_writer(&mut handle, envelope)?;
        writeln!(handle)?;
    }

    Ok(())
}
