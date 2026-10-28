//! Basic milestone input routing. Source identity is retained end to end. Step 4 can
//! replace contact arbitration here without adding camera math to UIKit views.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{AdapterError, Viewport, error};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputSource {
    Pencil,
    Finger,
    Indirect,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputPhase {
    Began,
    Moved,
    Ended,
    Cancelled,
    Hover,
    Scroll,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputEvent {
    pub source: InputSource,
    pub phase: InputPhase,
    pub id: u64,
    pub x_points: f64,
    pub y_points: f64,
    pub x_pixels: f64,
    pub y_pixels: f64,
    pub scale: f64,
    pub timestamp: f64,
    pub pressure: Option<f64>,
    pub altitude: Option<f64>,
    #[serde(default)]
    pub geometry_revision: Option<u64>,
    #[serde(default)]
    pub consumed: bool,
    #[serde(default)]
    pub shift: bool,
    /// 0 primary, 1 secondary orbit, 2 middle pan.
    #[serde(default)]
    pub button: u8,
    #[serde(default)]
    pub scroll: f64,
}

struct Contact {
    source: InputSource,
    start: [f64; 2],
    current: [f64; 2],
    timestamp: f64,
    moved: bool,
}

#[derive(Default)]
pub(crate) struct Contacts(BTreeMap<u64, Contact>);

impl Contacts {
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl InputEvent {
    pub fn validate(&self) -> Result<(), AdapterError> {
        if ![self.x_points, self.y_points, self.x_pixels, self.y_pixels, self.scale, self.timestamp, self.scroll].iter().all(|v| v.is_finite())
            || !(0.1..=8.0).contains(&self.scale)
            || self.timestamp < 0.0
            || self.x_points.abs() > 1e6
            || self.y_points.abs() > 1e6
            || self.scroll.abs() > 1e4
            || self.pressure.is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(&p))
            || self.altitude.is_some_and(|a| !a.is_finite() || !(0.0..=std::f64::consts::FRAC_PI_2).contains(&a))
            || (self.x_points * self.scale - self.x_pixels).abs() > 0.01
            || (self.y_points * self.scale - self.y_pixels).abs() > 0.01
            || self.button > 2
        {
            return Err(error("input", "invalid input coordinates or device values"));
        }
        Ok(())
    }
}

impl Viewport {
    pub fn input(&mut self, e: InputEvent) -> Result<Value, AdapterError> {
        e.validate()?;
        let source = e.source;
        let phase = e.phase;
        let handled = self.route(e)?;
        let mut reply = self.diagnostic();
        reply["source"] = json!(source);
        reply["phase"] = json!(phase);
        reply["handled"] = json!(handled);
        Ok(reply)
    }

    fn route(&mut self, e: InputEvent) -> Result<bool, AdapterError> {
        let point = [e.x_points, e.y_points];
        if e.consumed || e.source == InputSource::Unknown {
            self.contacts.0.remove(&e.id);
            return Ok(false);
        }
        if (e.scale - self.metrics.scale).abs() > 1e-6 || self.metrics.width_points <= 0.0 || self.metrics.height_points <= 0.0 {
            return Ok(false);
        }
        if e.phase == InputPhase::Cancelled && e.id == 0 {
            self.hover = None;
            self.changed();
            return Ok(true);
        }
        if e.phase == InputPhase::Hover {
            if matches!(e.source, InputSource::Indirect | InputSource::Pencil) {
                self.hover = e.geometry_revision.and_then(|revision| self.pick(point, Some(revision)));
                self.changed();
                return Ok(true);
            }
            return Ok(false);
        }
        if e.phase == InputPhase::Scroll {
            if e.source != InputSource::Indirect || self.metrics.pixel(point).is_none() {
                return Ok(false);
            }
            self.command("view.navigate", &json!({"motion":"zoom","factor":(-e.scroll * 0.01).exp().clamp(0.05,20.0)}))?;
            return Ok(true);
        }
        if e.phase == InputPhase::Began {
            if self.metrics.pixel(point).is_none() || self.contacts.0.len() >= 16 {
                return Ok(false);
            }
            self.contacts.0.insert(e.id, Contact { source: e.source, start: point, current: point, timestamp: e.timestamp, moved: false });
            self.hover = None;
            return Ok(true);
        }
        let Some(contact) = self.contacts.0.get(&e.id) else { return Ok(false) };
        if contact.source != e.source || e.timestamp < contact.timestamp {
            return Err(error("input", "contact source changed or timestamp moved backwards"));
        }
        let old = contact.current;
        let moved = contact.moved || (point[0] - contact.start[0]).hypot(point[1] - contact.start[1]) > 5.0;
        if matches!(e.phase, InputPhase::Ended | InputPhase::Cancelled) {
            self.contacts.0.remove(&e.id);
            if e.phase == InputPhase::Ended && !moved && e.button == 0 && matches!(e.source, InputSource::Pencil | InputSource::Indirect) {
                // Stale display taps do not clear or alter selection either.
                if !self.updating
                    && e.geometry_revision.is_some()
                    && self.snapshot().is_some_and(|s| e.geometry_revision.is_none_or(|r| r == s.geometry_revision))
                {
                    let hit = self.pick(point, e.geometry_revision);
                    self.select(hit, self.multiple || e.shift);
                }
            }
            return Ok(true);
        }
        let fingers: Vec<_> = self.contacts.0.iter().filter(|(_, c)| c.source == InputSource::Finger).map(|(id, c)| (*id, c.current)).collect();
        if let Some(contact) = self.contacts.0.get_mut(&e.id) {
            contact.current = point;
            contact.timestamp = e.timestamp;
            contact.moved = moved;
        }
        match e.source {
            InputSource::Finger if fingers.len() == 1 => {
                self.command("view.navigate", &json!({"motion":"orbit","delta":[point[0]-old[0],point[1]-old[1]]}))?;
            }
            InputSource::Finger if fingers.len() == 2 => {
                let other = fingers.iter().find(|(id, _)| *id != e.id).map(|(_, p)| *p);
                if let Some(other) = other {
                    self.command(
                        "view.navigate",
                        &json!({"motion":"pan","delta":[(point[0]-old[0])*0.5,(point[1]-old[1])*0.5],
                        "viewport":[self.metrics.width_points,self.metrics.height_points]}),
                    )?;
                    let before = (old[0] - other[0]).hypot(old[1] - other[1]);
                    let after = (point[0] - other[0]).hypot(point[1] - other[1]);
                    if before > 4.0 && after > 4.0 {
                        self.command("view.navigate", &json!({"motion":"zoom","factor":(before/after).clamp(0.05,20.0)}))?;
                    }
                }
            }
            InputSource::Indirect if e.button != 0 => {
                let motion = if e.button == 2 || e.shift { "pan" } else { "orbit" };
                self.command(
                    "view.navigate",
                    &json!({"motion":motion,"delta":[point[0]-old[0],point[1]-old[1]],
                    "viewport":[self.metrics.width_points,self.metrics.height_points]}),
                )?;
            }
            _ => {}
        }
        Ok(true)
    }
}
