//! Constraint-first picking proposes existing engine commands; the solver owns validity.
use serde_json::Value;
use solidcraft_engine::{Sel, sketch::CurveKind};

use crate::{AdapterError, Viewport, error, sketching::constraint_actions};

impl Viewport {
    pub(crate) fn constraint_tool(&self) -> Option<&str> {
        self.contacts.sketch.tool.strip_prefix("sketch.constraint.")
    }

    pub(crate) fn start_constraint(&mut self, params: &Value) -> Result<(), AdapterError> {
        let key = params["constraint"]
            .as_str()
            .filter(|key| {
                matches!(
                    *key,
                    "horizontal" | "vertical" | "parallel" | "perpendicular" | "coincident" | "tangent" | "equal" | "concentric" | "midpoint" | "fix"
                )
            })
            .ok_or_else(|| error("bad_params", "ui.ipados.sketch.constraint_invalid"))?;
        if self.updating || self.model.as_ref().is_none_or(|m| m.sketch.is_none()) {
            return Err(error("disabled", "ui.ipados.sketch.constraint_requires_sketch"));
        }
        self.cancel_interactions();
        self.contacts.radius_tool = false;
        self.contacts.sketch.tool = format!("sketch.constraint.{key}");
        self.contacts.previous_sketch_tool = self.contacts.sketch.tool.clone();
        // Preserve a compatible first pick for a two-entity constraint.
        if !self.queue_constraint()? && (self.selection.len() != 1 || !self.selection.first().is_some_and(|s| self.constraint_accepts(s))) {
            self.selection.clear();
        }
        self.changed();
        Ok(())
    }

    pub(crate) fn constraint_accepts(&self, entity: &Sel) -> bool {
        let Some(key) = self.constraint_tool() else { return true };
        let Some(snapshot) = self.model.as_ref().and_then(|m| m.sketch.as_ref()) else { return false };
        match entity {
            Sel::SketchPoint { id } => matches!(key, "coincident" | "midpoint" | "fix") && snapshot.sketch.resolve_point(id).is_some(),
            Sel::SketchCurve { id, sketch } if sketch.is_none_or(|id| id == snapshot.id) => {
                let Some(curve) = snapshot.sketch.curve_index(id).and_then(|i| snapshot.sketch.curves.get(i)) else { return false };
                match key {
                    "horizontal" | "vertical" | "parallel" | "perpendicular" | "midpoint" => matches!(curve.kind, CurveKind::Line { .. }),
                    "concentric" => matches!(curve.kind, CurveKind::Circle { .. } | CurveKind::Arc { .. }),
                    _ => true,
                }
            }
            _ => false,
        }
    }

    pub(crate) fn constraint_pick(&mut self, hit: Option<Sel>) -> Result<(), AdapterError> {
        if self.contacts.sketch.pending() {
            return Ok(());
        }
        let Some(hit) = hit else {
            if self.cancel_tools_on_dismiss {
                self.cancel_interactions();
                self.contacts.sketch.tool = "select".into();
            }
            self.select(None, false);
            return Ok(());
        };
        if !self.constraint_accepts(&hit) {
            return Ok(());
        }
        let previous = self.selection.clone();
        self.select(Some(hit), true);
        if self.selection.len() > 2 || self.selection.len() == 2 && !self.queue_constraint()? {
            // An incompatible second pick must not replace the useful first pick.
            self.selection = previous;
            self.changed();
        } else if self.selection.len() == 1 {
            self.queue_constraint()?;
        }
        Ok(())
    }

    fn queue_constraint(&mut self) -> Result<bool, AdapterError> {
        let Some(key) = self.constraint_tool() else { return Ok(false) };
        let Some(snapshot) = self.model.as_ref().and_then(|m| m.sketch.as_ref()) else { return Ok(false) };
        let action = constraint_actions(&snapshot.sketch, &self.selection).into_iter().find(|action| {
            action["params"]["mode"].as_str().or_else(|| action["command"].as_str().and_then(|id| id.rsplit('.').next())) == Some(key)
        });
        let Some(action) = action else { return Ok(false) };
        let Some(command) = action["command"].as_str() else { return Ok(false) };
        let mut params = action["params"].clone();
        if key == "fix" {
            params["fixed"] = Value::Bool(true);
        }
        self.propose_sketch(command, params, true)?;
        Ok(true)
    }
}
