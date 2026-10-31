//! A separate last-working save, independent of the engine's rolling `.bak`.
//! Validate the actual saved document with a cold engine before advancing it:
//! an in-progress sketch or a warm model cache can hide a broken downstream input.

use std::path::Path;

use serde_json::{Value, json};
use solidcraft_engine::{Session, io};

use crate::{AdapterError, SessionAdapter, error};

fn checked_bytes(path: &str) -> Option<Vec<u8>> {
    let bytes = io::vfs::read(path, io::limits::max_design_bytes()).ok()?;
    let document = io::read_design(&bytes).ok()?;
    let mut candidate = SessionAdapter::new();
    // Deliberately ignore the optional model cache and live sketch-editor state.
    // Reopening must work with exactly the document that reached disk.
    candidate.session = Session::opened_at(document, None, Some(path));
    candidate.snapshot().ok()?;
    Some(bytes)
}

fn persist(path: &str, bytes: &[u8]) -> Result<(), AdapterError> {
    io::vfs::check(path).map_err(|failure| error("recovery_write", failure.to_string()))?;
    io::write_atomic(Path::new(path), bytes, false).map_err(|failure| error("recovery_write", format!("{path}: {failure}")))
}

impl SessionAdapter {
    pub(crate) fn save_with_recovery(&mut self, command: &str, params: &Value) -> Result<Value, AdapterError> {
        let path = params
            .get("path")
            .and_then(Value::as_str)
            .filter(|path| !path.trim().is_empty())
            .or(self.session.path.as_deref())
            .ok_or_else(|| error("bad_params", "the design has no save path"))?
            .to_owned();
        let recovery = format!("{path}.recovery");
        // Upgrade existing projects before replacing their only working copy.
        // Never seed from unreadable or unrebuildable content, even a valid ZIP.
        if !io::vfs::exists(&recovery)
            && let Some(bytes) = checked_bytes(&path).or_else(|| checked_bytes(&format!("{path}.bak")))
        {
            persist(&recovery, &bytes)?;
        }
        let mut reply = self.session.execute(command, params)?;
        if let Some(bytes) = checked_bytes(&path) {
            if let Err(failure) = persist(&recovery, &bytes) {
                // The main file may have reached disk, but the save's promised
                // recovery copy did not. Keep retry possible and report failure.
                self.session.mark_unsaved();
                return Err(failure);
            }
            reply["recovery_updated"] = json!(true);
        } else {
            // Preserve unfinished work in the normal file while the independent
            // recovery file retains the last document that really reopened.
            reply["recovery_updated"] = json!(false);
        }
        reply["recovery_path"] = json!(recovery);
        Ok(reply)
    }
}
