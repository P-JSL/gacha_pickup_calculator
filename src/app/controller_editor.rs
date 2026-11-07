use super::*;

// Editor responsibilities are separated without changing egui control IDs or
// business-state update boundaries.
#[path = "controller_editor/preferences.rs"] mod preferences;
#[path = "controller_editor/targets.rs"] mod targets;
#[path = "controller_editor/resources.rs"] mod resources;
