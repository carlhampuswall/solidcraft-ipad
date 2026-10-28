//! Exact joint-reference checks over the shared native naming/resolution APIs.
//! No stored snap is repaired or replaced while a proposal is being validated.

use solidcraft_engine::{
    doc::{
        Document, ModelBody, ModelState, apply_point,
        joints::{self, JointOrigin, Snap, XRef},
        mat_inverse, naming,
    },
    geom::Vec3,
    kernel::{self, FaceSurface},
};

use crate::{AdapterError, SessionAdapter, error};

fn body_pick(doc: &Document, body: &ModelBody, point: Vec3) -> Result<Vec3, String> {
    match doc.model_body_offset(body) {
        Some(offset) => {
            mat_inverse(offset).map(|inverse| apply_point(&inverse, point)).ok_or_else(|| "the referenced body's placement is singular".into())
        }
        None => Ok(point),
    }
}

fn face_on<'a>(doc: &Document, bodies: impl Iterator<Item = &'a ModelBody>, name: &str, point: Vec3) -> Result<(&'a ModelBody, usize), String> {
    if name.is_empty() {
        return Err("a face has no saved native identity".into());
    }
    let mut found = None;
    for body in bodies {
        let point = body_pick(doc, body, point)?;
        // A singleton state retains the native lookup's exact/split/legacy
        // diagnostics while supplying the body's own coordinate frame.
        let mut source = ModelState::default();
        source.bodies.push(body.clone());
        let Some(hit) = naming::locate_face(&source, name, point) else { continue };
        if hit.note(name).is_some() || (hit.legacy && !hit.on_point) {
            continue;
        }
        if found.is_some() {
            return Err(format!("face `{name}` is ambiguous on several bodies"));
        }
        found = Some((body, hit.face));
    }
    found.ok_or_else(|| format!("face `{name}` no longer resolves exactly"))
}

fn exact_face<'a>(doc: &Document, state: &'a ModelState, component: u64, name: &str, point: Vec3) -> Result<(&'a ModelBody, usize), String> {
    let own: Vec<_> = state.bodies.iter().filter(|body| doc.model_body_component(body) == component).collect();
    let own_answers = own.iter().any(|body| body_pick(doc, body, point).ok().is_some_and(|point| naming::face_on_body(body, name, point).is_some()));
    if own_answers {
        return face_on(doc, own.into_iter(), name, point);
    }
    // This is the native named-face ownership-transfer path: retain the exact
    // lineage when its body moved components; never substitute nearby geometry.
    face_on(doc, state.bodies.iter(), name, point)
}

fn exact_edge<'a>(doc: &Document, state: &'a ModelState, component: u64, name: &str, point: Vec3) -> Result<&'a ModelBody, String> {
    if name.is_empty() {
        return Err("an edge has no saved native identity".into());
    }
    let mut found = None;
    for body in state.bodies.iter().filter(|body| doc.model_body_component(body) == component) {
        let point = body_pick(doc, body, point)?;
        let Ok(hit) = naming::locate_edge(body, name, point) else { continue };
        if hit.note(name).is_some() || (hit.legacy && !hit.on_point) {
            continue;
        }
        if found.is_some() {
            return Err(format!("edge `{name}` is ambiguous on several bodies"));
        }
        found = Some(body);
    }
    found.ok_or_else(|| format!("edge `{name}` no longer resolves exactly"))
}

fn saved_name<'a>(name: &'a Option<String>, role: &str) -> Result<&'a str, String> {
    name.as_deref().filter(|name| !name.is_empty()).ok_or_else(|| format!("the {role} has no saved native identity"))
}

fn check_x(doc: &Document, state: &ModelState, component: u64, x: &Option<XRef>, fallback: Vec3) -> Result<(), String> {
    if let Some(XRef::Face { face, pick }) = x {
        exact_face(doc, state, component, face, pick.unwrap_or(fallback))?;
    }
    Ok(())
}

fn check_origin(doc: &Document, state: &ModelState, origin: &JointOrigin) -> Result<(), String> {
    let component = if origin.occurrence == 0 {
        0
    } else {
        doc.occurrences
            .iter()
            .find(|occurrence| occurrence.id == origin.occurrence)
            .map(|occurrence| occurrence.component)
            .ok_or_else(|| format!("occurrence {} no longer exists", origin.occurrence))?
    };
    let frame = joints::resolve_origin(doc, state, origin).map_err(|failure| failure.to_string())?;
    let centre = Vec3::new(frame[3][0], frame[3][1], frame[3][2]);
    match &origin.snap {
        Snap::Point { .. } | Snap::Frame { .. } => {}
        Snap::FaceCenter { pick, face, x } => {
            let name = saved_name(face, "planar face")?;
            let (body, index) = exact_face(doc, state, component, name, *pick)?;
            if !matches!(kernel::face_surface(&body.body, index), Some(FaceSurface::Plane { .. })) {
                return Err(format!("face `{name}` is no longer the selected planar face"));
            }
            check_x(doc, state, component, x, centre)?;
        }
        Snap::CircleCenter { pick, face, end, x } => {
            let name = saved_name(face, "cylindrical face")?;
            let (body, index) = exact_face(doc, state, component, name, *pick)?;
            if kernel::cylinder_face(&body.body, &body.mesh(), index).is_none() {
                return Err(format!("face `{name}` is no longer the selected cylindrical face"));
            }
            if let Some(end) = end {
                exact_face(doc, state, component, end, *pick)?;
                if joints::end_faces(doc, state, component, &origin.snap).iter().filter(|candidate| *candidate == end).count() != 1 {
                    return Err(format!("end face `{end}` no longer uniquely meets the selected cylinder"));
                }
            }
            check_x(doc, state, component, x, centre)?;
        }
        Snap::Edge { pick, edge, face, corner, .. } => {
            let edge = saved_name(edge, "edge")?;
            let source = exact_edge(doc, state, component, edge, *pick)?;
            let face = saved_name(face, "edge support face")?;
            let (_, index) = face_on(doc, std::iter::once(source), face, *pick)?;
            if !matches!(kernel::face_surface(&source.body, index), Some(FaceSurface::Plane { .. })) {
                return Err(format!("edge support face `{face}` is no longer planar"));
            }
            if let Some(corner) = corner {
                let corner_body = exact_edge(doc, state, component, corner, *pick)?;
                if corner_body.name != source.name {
                    return Err(format!("corner edge `{corner}` belongs to a different body"));
                }
            }
        }
    }
    Ok(())
}

impl SessionAdapter {
    /// Validate active native joint inputs without changing the saved assembly.
    pub(crate) fn validate_joint_references(&self) -> Result<(), AdapterError> {
        let state = self.session.model.state();
        for joint in self.session.doc.assembly.joints.iter().filter(|joint| !joint.suppressed) {
            for (side, origin) in [("A", &joint.a), ("B", &joint.b)] {
                check_origin(&self.session.doc, &state, origin).map_err(|failure| {
                    error("joint_reference", format!("{} · {side}: {failure}. Explicitly reselect this joint origin to repair it.", joint.name))
                })?;
            }
        }
        Ok(())
    }
}
