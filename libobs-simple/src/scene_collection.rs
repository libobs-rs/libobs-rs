//! Versioned application-level scene collection persistence.
//!
//! This is intentionally not an OBS Studio frontend project-file parser. It persists the subset
//! owned by libobs-rs's managed scene/source model: shared sources, settings/hotkeys, filters,
//! scenes, native groups, item ordering and complete managed scene-item state.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use libobs_wrapper::{
    context::ObsContext,
    data::{object::ObsObjectTrait, ObsData, ObsDataGetters},
    enums::{ObsAlignment, ObsBlendMethod, ObsBlendMode, ObsBoundsType, ObsScaleType},
    graphics::Vec2,
    scenes::{
        ObsSceneItemCrop, ObsSceneItemStateSnapshot, ObsSceneItemTransformSnapshot, ObsSceneRef,
        SceneItemTrait,
    },
    sources::{ObsFilterRef, ObsSourceRef, ObsSourceTrait},
    utils::ObsError,
};
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SCENE_COLLECTION_SCHEMA_VERSION: u32 = 1;
const REDACTED_SECRET: &str = "__LIBOBS_RS_REDACTED_SECRET__";
const REDACTED_PATH: &str = "__LIBOBS_RS_REDACTED_PATH__";

#[derive(Debug)]
pub enum SceneCollectionError {
    Obs(ObsError),
    Io(std::io::Error),
    Json(serde_json::Error),
    UnsupportedVersion { found: u32, supported: u32 },
    InvalidDocument(String),
}

impl fmt::Display for SceneCollectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Obs(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "scene collection I/O error: {error}"),
            Self::Json(error) => write!(f, "scene collection JSON error: {error}"),
            Self::UnsupportedVersion { found, supported } => write!(
                f,
                "scene collection schema version {found} is unsupported (current version {supported})"
            ),
            Self::InvalidDocument(message) => write!(f, "invalid scene collection: {message}"),
        }
    }
}

impl std::error::Error for SceneCollectionError {}

impl From<ObsError> for SceneCollectionError {
    fn from(value: ObsError) -> Self {
        Self::Obs(value)
    }
}

impl From<std::io::Error> for SceneCollectionError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for SceneCollectionError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreMode {
    /// Abort on missing source/filter plugins and on missing machine-local files discovered in
    /// obvious path/file settings.
    Strict,
    /// Restore everything possible and return diagnostics for skipped/degraded objects.
    BestEffort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretHandling {
    Include,
    Redact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachinePathHandling {
    Include,
    RedactAbsolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneCollectionExportPolicy {
    pub secrets: SecretHandling,
    pub machine_paths: MachinePathHandling,
}

impl SceneCollectionExportPolicy {
    pub const fn include_all() -> Self {
        Self {
            secrets: SecretHandling::Include,
            machine_paths: MachinePathHandling::Include,
        }
    }

    pub const fn portable_redacted() -> Self {
        Self {
            secrets: SecretHandling::Redact,
            machine_paths: MachinePathHandling::RedactAbsolute,
        }
    }
}

impl Default for SceneCollectionExportPolicy {
    fn default() -> Self {
        Self::portable_redacted()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportRedaction {
    pub object_id: String,
    pub setting_path: String,
    pub reason: &'static str,
}

#[derive(Debug, Clone)]
pub struct SceneCollectionExport {
    pub json: String,
    pub redactions: Vec<ExportRedaction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreDiagnosticKind {
    MissingSourcePlugin,
    MissingFilterPlugin,
    MissingFile,
    SkippedSceneItem,
    RedactedSetting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreDiagnostic {
    pub kind: RestoreDiagnosticKind,
    pub object_id: String,
    pub message: String,
}

#[derive(Debug)]
pub struct RestoredSceneCollection {
    /// Sources are retained here even when a persisted source is not currently present in a scene.
    pub sources: HashMap<String, ObsSourceRef>,
    pub scenes: HashMap<String, ObsSceneRef>,
    pub diagnostics: Vec<RestoreDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneCollectionDocument {
    pub version: u32,
    #[serde(default)]
    pub name: Option<String>,
    pub sources: Vec<PersistedSource>,
    pub scenes: Vec<PersistedScene>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedSource {
    /// Stable document identity. Captures use libobs's persistent source UUID.
    pub id: String,
    pub plugin_id: String,
    pub name: String,
    pub settings: Value,
    #[serde(default)]
    pub hotkeys: Value,
    #[serde(default)]
    pub filters: Vec<PersistedFilter>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedFilter {
    pub id: String,
    pub plugin_id: String,
    pub name: String,
    pub settings: Value,
    #[serde(default)]
    pub hotkeys: Value,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedScene {
    pub id: String,
    pub name: String,
    /// Native bottom-to-top order.
    pub items: Vec<PersistedSceneNode>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PersistedSceneNode {
    Source(PersistedSourceItem),
    Group(PersistedGroup),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedSourceItem {
    /// Stable document item identity. It is not a native scene-item pointer/ID.
    pub id: String,
    pub source_id: String,
    pub state: PersistedSceneItemState,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedGroup {
    /// Stable group-source UUID at capture time.
    pub id: String,
    pub name: String,
    pub state: PersistedSceneItemState,
    /// OBS's managed group API intentionally supports one group level; nested groups are rejected
    /// on restore instead of being silently flattened.
    pub items: Vec<PersistedSourceItem>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedSceneItemState {
    pub position: [f32; 2],
    pub scale: [f32; 2],
    pub alignment: u32,
    pub rotation: f32,
    pub bounds: [f32; 2],
    pub bounds_type: i64,
    pub bounds_alignment: u32,
    pub crop_to_bounds: bool,
    pub crop: [i32; 4],
    pub scale_filter: i64,
    pub blend_method: i64,
    pub blend_mode: i64,
    pub visible: bool,
    pub locked: bool,
    pub order_position: i32,
}

impl PersistedSceneItemState {
    fn capture(item: &dyn SceneItemTrait) -> Result<Self, ObsError> {
        let snapshot = item.state_snapshot()?;
        Ok(Self::from_snapshot(&snapshot))
    }

    pub fn from_snapshot(snapshot: &ObsSceneItemStateSnapshot) -> Self {
        let transform = &snapshot.transform;
        Self {
            position: [*transform.position.x(), *transform.position.y()],
            scale: [*transform.scale.x(), *transform.scale.y()],
            alignment: transform.alignment.bits(),
            rotation: transform.rotation,
            bounds: [*transform.bounds.x(), *transform.bounds.y()],
            bounds_type: transform.bounds_type as i64,
            bounds_alignment: transform.bounds_alignment.bits(),
            crop_to_bounds: transform.crop_to_bounds,
            crop: [
                transform.crop.left,
                transform.crop.top,
                transform.crop.right,
                transform.crop.bottom,
            ],
            scale_filter: transform.scale_filter as i64,
            blend_method: transform.blend_method as i64,
            blend_mode: transform.blend_mode as i64,
            visible: snapshot.visible,
            locked: snapshot.locked,
            order_position: snapshot.order_position,
        }
    }

    pub fn to_snapshot(&self) -> Result<ObsSceneItemStateSnapshot, SceneCollectionError> {
        let bounds_type = ObsBoundsType::from_i64(self.bounds_type).ok_or_else(|| {
            SceneCollectionError::InvalidDocument(format!(
                "unknown bounds type {}",
                self.bounds_type
            ))
        })?;
        let scale_filter = ObsScaleType::from_i64(self.scale_filter).ok_or_else(|| {
            SceneCollectionError::InvalidDocument(format!(
                "unknown scale filter {}",
                self.scale_filter
            ))
        })?;
        let blend_method = ObsBlendMethod::from_i64(self.blend_method).ok_or_else(|| {
            SceneCollectionError::InvalidDocument(format!(
                "unknown blend method {}",
                self.blend_method
            ))
        })?;
        let blend_mode = ObsBlendMode::from_i64(self.blend_mode).ok_or_else(|| {
            SceneCollectionError::InvalidDocument(format!("unknown blend mode {}", self.blend_mode))
        })?;

        Ok(ObsSceneItemStateSnapshot {
            transform: ObsSceneItemTransformSnapshot {
                position: Vec2::new(self.position[0], self.position[1]),
                scale: Vec2::new(self.scale[0], self.scale[1]),
                alignment: ObsAlignment::from_bits_retain(self.alignment),
                rotation: self.rotation,
                bounds: Vec2::new(self.bounds[0], self.bounds[1]),
                bounds_type,
                bounds_alignment: ObsAlignment::from_bits_retain(self.bounds_alignment),
                crop_to_bounds: self.crop_to_bounds,
                crop: ObsSceneItemCrop::new(self.crop[0], self.crop[1], self.crop[2], self.crop[3]),
                scale_filter,
                blend_method,
                blend_mode,
            },
            visible: self.visible,
            locked: self.locked,
            order_position: self.order_position,
        })
    }
}

#[derive(Debug, Clone)]
struct SceneHistoryEntry {
    label: String,
    before: SceneCollectionDocument,
    after: SceneCollectionDocument,
}

/// Undo/redo editor for the persisted scene-collection model.
///
/// History stores complete versioned document snapshots rather than runtime/native object IDs.
/// Item/source/group IDs therefore remain stable across undo/redo. Group/ungroup operations accept
/// the *post-operation* replacement snapshots, matching libobs's native semantics where ungrouping
/// creates replacement parent-scene items instead of preserving the old native handles.
#[derive(Debug, Clone)]
pub struct SceneCollectionEditor {
    document: SceneCollectionDocument,
    undo: Vec<SceneHistoryEntry>,
    redo: Vec<SceneHistoryEntry>,
}

impl SceneCollectionEditor {
    pub fn new(document: SceneCollectionDocument) -> Result<Self, SceneCollectionError> {
        document.validate_version()?;
        document.validate_references()?;
        Ok(Self {
            document,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    pub fn document(&self) -> &SceneCollectionDocument {
        &self.document
    }

    pub fn into_document(self) -> SceneCollectionDocument {
        self.document
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn next_undo_label(&self) -> Option<&str> {
        self.undo.last().map(|entry| entry.label.as_str())
    }

    pub fn next_redo_label(&self) -> Option<&str> {
        self.redo.last().map(|entry| entry.label.as_str())
    }

    pub fn undo(&mut self) -> bool {
        let Some(entry) = self.undo.pop() else {
            return false;
        };
        self.document = entry.before.clone();
        self.redo.push(entry);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(entry) = self.redo.pop() else {
            return false;
        };
        self.document = entry.after.clone();
        self.undo.push(entry);
        true
    }

    /// Replaces one item's complete persisted transform/visibility/lock/order snapshot.
    pub fn set_item_state(
        &mut self,
        scene_id: &str,
        item_id: &str,
        state: PersistedSceneItemState,
    ) -> Result<(), SceneCollectionError> {
        self.transact("scene item state", |document| {
            let item = find_source_item_mut(document, scene_id, item_id)?;
            item.state = state;
            Ok(())
        })
    }

    /// Reorders one top-level source/group node in the persisted native bottom-to-top order.
    pub fn move_top_level_item(
        &mut self,
        scene_id: &str,
        item_id: &str,
        new_index: usize,
    ) -> Result<(), SceneCollectionError> {
        self.transact("scene item order", |document| {
            let scene = find_scene_mut(document, scene_id)?;
            if new_index >= scene.items.len() {
                return Err(SceneCollectionError::InvalidDocument(format!(
                    "top-level order index {new_index} is out of range"
                )));
            }
            let old_index = scene
                .items
                .iter()
                .position(|node| node_id(node) == item_id)
                .ok_or_else(|| {
                    SceneCollectionError::InvalidDocument(format!(
                        "scene '{scene_id}' has no top-level item '{item_id}'"
                    ))
                })?;
            let node = scene.items.remove(old_index);
            scene.items.insert(new_index, node);
            for (index, node) in scene.items.iter_mut().enumerate() {
                node_state_mut(node).order_position = index as i32;
            }
            Ok(())
        })
    }

    /// Replaces selected top-level source items with the supplied post-grouping snapshot.
    ///
    /// `group.items` must contain exactly the selected persistent item IDs. This lets a live editor
    /// capture libobs's post-reparent child snapshots and record them without pretending native
    /// scene-item handles survived grouping unchanged.
    pub fn replace_items_with_group(
        &mut self,
        scene_id: &str,
        selected_item_ids: &[String],
        group: PersistedGroup,
    ) -> Result<(), SceneCollectionError> {
        self.transact("group scene items", |document| {
            if selected_item_ids.is_empty() {
                return Err(SceneCollectionError::InvalidDocument(
                    "cannot create an empty group edit".into(),
                ));
            }
            let selected = selected_item_ids.iter().cloned().collect::<HashSet<_>>();
            let group_ids = group
                .items
                .iter()
                .map(|item| item.id.clone())
                .collect::<HashSet<_>>();
            if selected != group_ids {
                return Err(SceneCollectionError::InvalidDocument(
                    "group replacement children must exactly match selected item IDs".into(),
                ));
            }
            let scene = find_scene_mut(document, scene_id)?;
            if scene.items.iter().any(|node| node_id(node) == group.id) {
                return Err(SceneCollectionError::InvalidDocument(format!(
                    "group id '{}' already exists in scene",
                    group.id
                )));
            }

            let mut insertion = None;
            let mut found = HashSet::new();
            let mut retained = Vec::with_capacity(scene.items.len());
            for (index, node) in std::mem::take(&mut scene.items).into_iter().enumerate() {
                if selected.contains(node_id(&node)) {
                    if !matches!(node, PersistedSceneNode::Source(_)) {
                        return Err(SceneCollectionError::InvalidDocument(
                            "only top-level source items can be grouped".into(),
                        ));
                    }
                    insertion.get_or_insert(index);
                    found.insert(node_id(&node).to_owned());
                } else {
                    retained.push(node);
                }
            }
            if found != selected {
                return Err(SceneCollectionError::InvalidDocument(
                    "one or more selected item IDs were not found at scene top level".into(),
                ));
            }
            let insertion = insertion.unwrap_or(retained.len()).min(retained.len());
            retained.insert(insertion, PersistedSceneNode::Group(group));
            scene.items = retained;
            for (index, node) in scene.items.iter_mut().enumerate() {
                node_state_mut(node).order_position = index as i32;
            }
            Ok(())
        })
    }

    /// Replaces a group with the post-ungroup replacement item snapshots returned/captured after
    /// libobs created new parent-scene items.
    pub fn ungroup_with_replacements(
        &mut self,
        scene_id: &str,
        group_id: &str,
        replacements: Vec<PersistedSourceItem>,
    ) -> Result<(), SceneCollectionError> {
        self.transact("ungroup scene items", |document| {
            let scene = find_scene_mut(document, scene_id)?;
            let index = scene
                .items
                .iter()
                .position(|node| node_id(node) == group_id)
                .ok_or_else(|| {
                    SceneCollectionError::InvalidDocument(format!(
                        "scene '{scene_id}' has no group '{group_id}'"
                    ))
                })?;
            let PersistedSceneNode::Group(group) = &scene.items[index] else {
                return Err(SceneCollectionError::InvalidDocument(format!(
                    "item '{group_id}' is not a group"
                )));
            };
            let old_ids = group
                .items
                .iter()
                .map(|item| (&item.id, &item.source_id))
                .collect::<HashSet<_>>();
            let replacement_ids = replacements
                .iter()
                .map(|item| (&item.id, &item.source_id))
                .collect::<HashSet<_>>();
            if old_ids != replacement_ids {
                return Err(SceneCollectionError::InvalidDocument(
                    "ungroup replacement IDs/sources must match the group's children".into(),
                ));
            }
            scene.items.remove(index);
            for (offset, replacement) in replacements.into_iter().enumerate() {
                scene
                    .items
                    .insert(index + offset, PersistedSceneNode::Source(replacement));
            }
            for (position, node) in scene.items.iter_mut().enumerate() {
                node_state_mut(node).order_position = position as i32;
            }
            Ok(())
        })
    }

    fn transact(
        &mut self,
        label: &str,
        edit: impl FnOnce(&mut SceneCollectionDocument) -> Result<(), SceneCollectionError>,
    ) -> Result<(), SceneCollectionError> {
        let before = self.document.clone();
        edit(&mut self.document)?;
        self.document.validate_references()?;
        let after = self.document.clone();
        self.undo.push(SceneHistoryEntry {
            label: label.to_owned(),
            before,
            after,
        });
        self.redo.clear();
        Ok(())
    }
}

fn find_scene_mut<'a>(
    document: &'a mut SceneCollectionDocument,
    scene_id: &str,
) -> Result<&'a mut PersistedScene, SceneCollectionError> {
    document
        .scenes
        .iter_mut()
        .find(|scene| scene.id == scene_id)
        .ok_or_else(|| {
            SceneCollectionError::InvalidDocument(format!("unknown scene id '{scene_id}'"))
        })
}

fn find_source_item_mut<'a>(
    document: &'a mut SceneCollectionDocument,
    scene_id: &str,
    item_id: &str,
) -> Result<&'a mut PersistedSourceItem, SceneCollectionError> {
    let scene = find_scene_mut(document, scene_id)?;
    for node in &mut scene.items {
        match node {
            PersistedSceneNode::Source(item) if item.id == item_id => return Ok(item),
            PersistedSceneNode::Group(group) => {
                if let Some(item) = group.items.iter_mut().find(|item| item.id == item_id) {
                    return Ok(item);
                }
            }
            _ => {}
        }
    }
    Err(SceneCollectionError::InvalidDocument(format!(
        "scene '{scene_id}' has no source item '{item_id}'"
    )))
}

fn node_id(node: &PersistedSceneNode) -> &str {
    match node {
        PersistedSceneNode::Source(item) => &item.id,
        PersistedSceneNode::Group(group) => &group.id,
    }
}

fn node_state_mut(node: &mut PersistedSceneNode) -> &mut PersistedSceneItemState {
    match node {
        PersistedSceneNode::Source(item) => &mut item.state,
        PersistedSceneNode::Group(group) => &mut group.state,
    }
}

impl SceneCollectionDocument {
    pub fn new(name: Option<String>) -> Self {
        Self {
            version: SCENE_COLLECTION_SCHEMA_VERSION,
            name,
            sources: Vec::new(),
            scenes: Vec::new(),
            extra: BTreeMap::new(),
        }
    }

    /// Captures every scene managed by `context`.
    pub fn capture_context(context: &ObsContext) -> Result<Self, SceneCollectionError> {
        Self::capture(&context.scenes()?)
    }

    /// Captures the supplied managed scenes and every shared source/filter they reference.
    pub fn capture(scenes: &[ObsSceneRef]) -> Result<Self, SceneCollectionError> {
        let mut document = Self::new(None);
        let mut sources = HashMap::<String, PersistedSource>::new();

        for scene in scenes {
            for source in scene.managed_sources()? {
                let persisted = capture_source(source.as_ref())?;
                sources.entry(persisted.id.clone()).or_insert(persisted);
            }
            document.scenes.push(capture_scene(scene)?);
        }

        document.sources = sources.into_values().collect();
        document.sources.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(document)
    }

    pub fn from_json(json: &str) -> Result<Self, SceneCollectionError> {
        let document: Self = serde_json::from_str(json)?;
        document.validate_version()?;
        document.validate_references()?;
        Ok(document)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, SceneCollectionError> {
        Self::from_json(&fs::read_to_string(path)?)
    }

    pub fn to_json(&self) -> Result<String, SceneCollectionError> {
        self.validate_version()?;
        self.validate_references()?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Serializes using an explicit secret/path policy. Unknown JSON fields captured through
    /// `extra` maps are preserved.
    pub fn export(
        &self,
        policy: SceneCollectionExportPolicy,
    ) -> Result<SceneCollectionExport, SceneCollectionError> {
        self.validate_version()?;
        self.validate_references()?;
        let mut sanitized = self.clone();
        let mut redactions = Vec::new();
        for source in &mut sanitized.sources {
            sanitize_value(
                &source.id,
                "settings",
                &mut source.settings,
                policy,
                &mut redactions,
            );
            sanitize_value(
                &source.id,
                "hotkeys",
                &mut source.hotkeys,
                policy,
                &mut redactions,
            );
            for filter in &mut source.filters {
                sanitize_value(
                    &filter.id,
                    "settings",
                    &mut filter.settings,
                    policy,
                    &mut redactions,
                );
                sanitize_value(
                    &filter.id,
                    "hotkeys",
                    &mut filter.hotkeys,
                    policy,
                    &mut redactions,
                );
            }
        }
        Ok(SceneCollectionExport {
            json: serde_json::to_string_pretty(&sanitized)?,
            redactions,
        })
    }

    /// Writes through a same-directory temporary file so serialization/write failures never
    /// truncate an existing collection. On platforms where rename-over-existing is unavailable,
    /// the old file is first moved to a backup and restored on replacement failure.
    pub fn save_atomic(
        &self,
        path: impl AsRef<Path>,
        policy: SceneCollectionExportPolicy,
    ) -> Result<Vec<ExportRedaction>, SceneCollectionError> {
        let path = path.as_ref();
        let export = self.export(policy)?;
        atomic_write(path, export.json.as_bytes())?;
        Ok(export.redactions)
    }

    pub fn restore(
        &self,
        context: &mut ObsContext,
        mode: RestoreMode,
    ) -> Result<RestoredSceneCollection, SceneCollectionError> {
        self.validate_version()?;
        self.validate_references()?;

        let mut diagnostics = Vec::new();
        let mut sources = HashMap::<String, ObsSourceRef>::new();

        for persisted in &self.sources {
            let Some(source_type) = context.source_type(&persisted.plugin_id)? else {
                let diagnostic = RestoreDiagnostic {
                    kind: RestoreDiagnosticKind::MissingSourcePlugin,
                    object_id: persisted.id.clone(),
                    message: format!(
                        "source '{}' requires missing OBS plugin type '{}'",
                        persisted.name, persisted.plugin_id
                    ),
                };
                if mode == RestoreMode::Strict {
                    return Err(SceneCollectionError::InvalidDocument(diagnostic.message));
                }
                diagnostics.push(diagnostic);
                continue;
            };

            inspect_missing_files(&persisted.id, &persisted.settings, mode, &mut diagnostics)?;
            inspect_redacted_fields(&persisted.id, &persisted.settings, &mut diagnostics);
            let settings = obs_data_from_value(context, &persisted.settings)?.into_immutable();
            let hotkeys = obs_data_from_value(context, &persisted.hotkeys)?.into_immutable();
            let source = ObsSourceRef::new(
                source_type.id(),
                persisted.name.as_str(),
                Some(settings),
                Some(hotkeys),
                context.runtime().clone(),
            )?;

            for filter in &persisted.filters {
                let Some(filter_type) = context.source_type(&filter.plugin_id)? else {
                    let diagnostic = RestoreDiagnostic {
                        kind: RestoreDiagnosticKind::MissingFilterPlugin,
                        object_id: filter.id.clone(),
                        message: format!(
                            "filter '{}' requires missing OBS plugin type '{}'",
                            filter.name, filter.plugin_id
                        ),
                    };
                    if mode == RestoreMode::Strict {
                        return Err(SceneCollectionError::InvalidDocument(diagnostic.message));
                    }
                    diagnostics.push(diagnostic);
                    continue;
                };
                inspect_missing_files(&filter.id, &filter.settings, mode, &mut diagnostics)?;
                inspect_redacted_fields(&filter.id, &filter.settings, &mut diagnostics);
                let settings = obs_data_from_value(context, &filter.settings)?.into_immutable();
                let hotkeys = obs_data_from_value(context, &filter.hotkeys)?.into_immutable();
                let filter_ref = ObsFilterRef::new(
                    filter_type.id(),
                    filter.name.as_str(),
                    Some(settings),
                    Some(hotkeys),
                    context.runtime().clone(),
                )?;
                source.apply_filter(&filter_ref)?;
            }
            sources.insert(persisted.id.clone(), source);
        }

        let mut restored_scenes = HashMap::new();
        for persisted_scene in &self.scenes {
            let mut scene = context.scene(persisted_scene.name.as_str(), None)?;
            restore_scene_items(
                &mut scene,
                &persisted_scene.items,
                &sources,
                mode,
                &mut diagnostics,
            )?;
            restored_scenes.insert(persisted_scene.id.clone(), scene);
        }

        Ok(RestoredSceneCollection {
            sources,
            scenes: restored_scenes,
            diagnostics,
        })
    }

    fn validate_version(&self) -> Result<(), SceneCollectionError> {
        if self.version != SCENE_COLLECTION_SCHEMA_VERSION {
            return Err(SceneCollectionError::UnsupportedVersion {
                found: self.version,
                supported: SCENE_COLLECTION_SCHEMA_VERSION,
            });
        }
        Ok(())
    }

    fn validate_references(&self) -> Result<(), SceneCollectionError> {
        let mut source_ids = HashSet::new();
        for source in &self.sources {
            if !source_ids.insert(source.id.as_str()) {
                return Err(SceneCollectionError::InvalidDocument(format!(
                    "duplicate source id '{}'",
                    source.id
                )));
            }
        }
        let mut scene_ids = HashSet::new();
        let mut item_ids = HashSet::new();
        for scene in &self.scenes {
            if !scene_ids.insert(scene.id.as_str()) {
                return Err(SceneCollectionError::InvalidDocument(format!(
                    "duplicate scene id '{}'",
                    scene.id
                )));
            }
            for node in &scene.items {
                match node {
                    PersistedSceneNode::Source(item) => {
                        validate_item(item, &source_ids, &mut item_ids)?;
                    }
                    PersistedSceneNode::Group(group) => {
                        if !item_ids.insert(group.id.as_str()) {
                            return Err(SceneCollectionError::InvalidDocument(format!(
                                "duplicate scene item/group id '{}'",
                                group.id
                            )));
                        }
                        for item in &group.items {
                            validate_item(item, &source_ids, &mut item_ids)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn capture_source(source: &dyn ObsSourceTrait) -> Result<PersistedSource, SceneCollectionError> {
    let id = source.persistent_uuid()?;
    let settings = serde_json::from_str(&source.settings()?.get_json()?)?;
    let hotkeys = serde_json::from_str(&source.hotkey_data()?.get_json()?)?;
    let mut filters = Vec::new();
    for guard in source.get_active_filters()? {
        let filter = guard.get_inner();
        filters.push(PersistedFilter {
            id: filter.persistent_uuid()?,
            plugin_id: filter.id().to_string(),
            name: filter.name().to_string(),
            settings: serde_json::from_str(&filter.settings()?.get_json()?)?,
            hotkeys: serde_json::from_str(&filter.hotkey_data()?.get_json()?)?,
            extra: BTreeMap::new(),
        });
    }
    Ok(PersistedSource {
        id,
        plugin_id: source.id().to_string(),
        name: source.name().to_string(),
        settings,
        hotkeys,
        filters,
        extra: BTreeMap::new(),
    })
}

fn capture_scene(scene: &ObsSceneRef) -> Result<PersistedScene, SceneCollectionError> {
    let scene_id = scene.persistent_uuid()?;
    let groups = scene.groups()?;
    let groups_by_runtime_id = groups
        .into_iter()
        .map(|group| (group.object_id(), group))
        .collect::<HashMap<_, _>>();
    let mut occurrence = HashMap::<String, usize>::new();
    let mut items = Vec::new();

    for item in scene.items_in_order()? {
        if let Some(group) = groups_by_runtime_id.get(&item.object_id()) {
            let mut children = Vec::new();
            for child in group.items_in_order()? {
                children.push(capture_source_item(
                    child.as_ref(),
                    &scene_id,
                    &mut occurrence,
                )?);
            }
            items.push(PersistedSceneNode::Group(PersistedGroup {
                id: item.source_persistent_uuid()?,
                name: group.name().to_string(),
                state: PersistedSceneItemState::capture(item.as_ref())?,
                items: children,
                extra: BTreeMap::new(),
            }));
        } else {
            items.push(PersistedSceneNode::Source(capture_source_item(
                item.as_ref(),
                &scene_id,
                &mut occurrence,
            )?));
        }
    }

    Ok(PersistedScene {
        id: scene_id,
        name: scene.name().to_string(),
        items,
        extra: BTreeMap::new(),
    })
}

fn capture_source_item(
    item: &dyn SceneItemTrait,
    scene_id: &str,
    occurrence: &mut HashMap<String, usize>,
) -> Result<PersistedSourceItem, SceneCollectionError> {
    let source_id = item.source_persistent_uuid()?;
    let index = occurrence.entry(source_id.clone()).or_default();
    let id = format!("{scene_id}:{source_id}:item:{index}");
    *index += 1;
    Ok(PersistedSourceItem {
        id,
        source_id,
        state: PersistedSceneItemState::capture(item)?,
        extra: BTreeMap::new(),
    })
}

fn restore_scene_items(
    scene: &mut ObsSceneRef,
    nodes: &[PersistedSceneNode],
    sources: &HashMap<String, ObsSourceRef>,
    mode: RestoreMode,
    diagnostics: &mut Vec<RestoreDiagnostic>,
) -> Result<(), SceneCollectionError> {
    for node in nodes {
        match node {
            PersistedSceneNode::Source(item) => {
                let Some(source) = sources.get(&item.source_id) else {
                    skip_missing_item(item, mode, diagnostics)?;
                    continue;
                };
                let runtime_item = scene.add(source.clone())?;
                runtime_item.apply_state_snapshot(&item.state.to_snapshot()?)?;
            }
            PersistedSceneNode::Group(group) => {
                let runtime_group = scene.create_group(group.name.as_str())?;
                for child in &group.items {
                    let Some(source) = sources.get(&child.source_id) else {
                        skip_missing_item(child, mode, diagnostics)?;
                        continue;
                    };
                    let runtime_item = scene.add(source.clone())?;
                    runtime_group.add_item(&runtime_item)?;
                    runtime_item.apply_state_snapshot(&child.state.to_snapshot()?)?;
                }
                runtime_group.apply_state_snapshot(&group.state.to_snapshot()?)?;
            }
        }
    }
    Ok(())
}

fn skip_missing_item(
    item: &PersistedSourceItem,
    mode: RestoreMode,
    diagnostics: &mut Vec<RestoreDiagnostic>,
) -> Result<(), SceneCollectionError> {
    let message = format!(
        "scene item '{}' references source '{}' which could not be restored",
        item.id, item.source_id
    );
    if mode == RestoreMode::Strict {
        return Err(SceneCollectionError::InvalidDocument(message));
    }
    diagnostics.push(RestoreDiagnostic {
        kind: RestoreDiagnosticKind::SkippedSceneItem,
        object_id: item.id.clone(),
        message,
    });
    Ok(())
}

fn validate_item<'a>(
    item: &'a PersistedSourceItem,
    sources: &HashSet<&'a str>,
    item_ids: &mut HashSet<&'a str>,
) -> Result<(), SceneCollectionError> {
    if !item_ids.insert(item.id.as_str()) {
        return Err(SceneCollectionError::InvalidDocument(format!(
            "duplicate scene item id '{}'",
            item.id
        )));
    }
    if !sources.contains(item.source_id.as_str()) {
        return Err(SceneCollectionError::InvalidDocument(format!(
            "scene item '{}' references unknown source '{}'",
            item.id, item.source_id
        )));
    }
    Ok(())
}

fn obs_data_from_value(
    context: &ObsContext,
    value: &Value,
) -> Result<ObsData, SceneCollectionError> {
    Ok(ObsData::from_json(
        &serde_json::to_string(value)?,
        context.runtime().clone(),
    )?)
}

fn inspect_missing_files(
    object_id: &str,
    settings: &Value,
    mode: RestoreMode,
    diagnostics: &mut Vec<RestoreDiagnostic>,
) -> Result<(), SceneCollectionError> {
    let mut missing = Vec::<PathBuf>::new();
    walk_values(settings, "", &mut |key, value| {
        if !looks_like_path_key(key) {
            return;
        }
        if let Some(text) = value.as_str() {
            let path = Path::new(text);
            if path.is_absolute() && !path.exists() {
                missing.push(path.to_path_buf());
            }
        }
    });
    for path in missing {
        let message = format!(
            "persisted setting references missing local path '{}'",
            path.display()
        );
        if mode == RestoreMode::Strict {
            return Err(SceneCollectionError::InvalidDocument(format!(
                "{object_id}: {message}"
            )));
        }
        diagnostics.push(RestoreDiagnostic {
            kind: RestoreDiagnosticKind::MissingFile,
            object_id: object_id.to_owned(),
            message,
        });
    }
    Ok(())
}

fn inspect_redacted_fields(
    object_id: &str,
    settings: &Value,
    diagnostics: &mut Vec<RestoreDiagnostic>,
) {
    walk_values(settings, "", &mut |key, value| {
        let Some(text) = value.as_str() else {
            return;
        };
        if text == REDACTED_SECRET || text == REDACTED_PATH {
            diagnostics.push(RestoreDiagnostic {
                kind: RestoreDiagnosticKind::RedactedSetting,
                object_id: object_id.to_owned(),
                message: format!(
                    "setting '{key}' was redacted during export and must be supplied for this machine"
                ),
            });
        }
    });
}

fn sanitize_value(
    object_id: &str,
    root: &str,
    value: &mut Value,
    policy: SceneCollectionExportPolicy,
    redactions: &mut Vec<ExportRedaction>,
) {
    sanitize_value_inner(object_id, root, "", value, policy, redactions);
}

fn sanitize_value_inner(
    object_id: &str,
    root: &str,
    path: &str,
    value: &mut Value,
    policy: SceneCollectionExportPolicy,
    redactions: &mut Vec<ExportRedaction>,
) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if policy.secrets == SecretHandling::Redact
                    && looks_like_secret_key(key)
                    && !child.is_null()
                {
                    *child = Value::String(REDACTED_SECRET.into());
                    redactions.push(ExportRedaction {
                        object_id: object_id.to_owned(),
                        setting_path: format!("{root}.{child_path}"),
                        reason: "secret",
                    });
                    continue;
                }
                if policy.machine_paths == MachinePathHandling::RedactAbsolute
                    && looks_like_path_key(key)
                    && child
                        .as_str()
                        .is_some_and(|text| Path::new(text).is_absolute())
                {
                    *child = Value::String(REDACTED_PATH.into());
                    redactions.push(ExportRedaction {
                        object_id: object_id.to_owned(),
                        setting_path: format!("{root}.{child_path}"),
                        reason: "machine_path",
                    });
                    continue;
                }
                sanitize_value_inner(object_id, root, &child_path, child, policy, redactions);
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter_mut().enumerate() {
                sanitize_value_inner(
                    object_id,
                    root,
                    &format!("{path}[{index}]"),
                    child,
                    policy,
                    redactions,
                );
            }
        }
        _ => {}
    }
}

fn walk_values(value: &Value, path: &str, callback: &mut impl FnMut(&str, &Value)) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                callback(key, child);
                walk_values(child, &child_path, callback);
            }
        }
        Value::Array(values) => {
            for child in values {
                walk_values(child, path, callback);
            }
        }
        _ => {}
    }
}

fn looks_like_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("password")
        || key.contains("secret")
        || key.contains("token")
        || key.contains("credential")
        || key == "stream_key"
        || key.ends_with("_stream_key")
        || key.ends_with("_api_key")
}

fn looks_like_path_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("path")
        || key == "file"
        || key.ends_with("_file")
        || key.contains("filename")
        || key == "local_file"
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), SceneCollectionError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| SceneCollectionError::InvalidDocument("invalid output filename".into()))?;
    let temp = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let backup = parent.join(format!(".{file_name}.bak-{}", std::process::id()));

    let write_result = (|| -> Result<(), std::io::Error> {
        let mut file = File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }

    match fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(_first_error) if path.exists() => {
            fs::rename(path, &backup)?;
            match fs::rename(&temp, path) {
                Ok(()) => {
                    let _ = fs::remove_file(&backup);
                    Ok(())
                }
                Err(error) => {
                    let _ = fs::rename(&backup, path);
                    let _ = fs::remove_file(&temp);
                    Err(error.into())
                }
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(error.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(order: i32) -> PersistedSceneItemState {
        PersistedSceneItemState {
            position: [1.0, 2.0],
            scale: [1.0, 1.0],
            alignment: ObsAlignment::CENTER.bits(),
            rotation: 0.0,
            bounds: [1920.0, 1080.0],
            bounds_type: ObsBoundsType::None as i64,
            bounds_alignment: ObsAlignment::CENTER.bits(),
            crop_to_bounds: false,
            crop: [0, 0, 0, 0],
            scale_filter: ObsScaleType::Bilinear as i64,
            blend_method: ObsBlendMethod::Default as i64,
            blend_mode: ObsBlendMode::Normal as i64,
            visible: true,
            locked: false,
            order_position: order,
        }
    }

    fn fixture() -> SceneCollectionDocument {
        SceneCollectionDocument {
            version: SCENE_COLLECTION_SCHEMA_VERSION,
            name: Some("test".into()),
            sources: vec![PersistedSource {
                id: "source-a".into(),
                plugin_id: "color_source_v3".into(),
                name: "Color".into(),
                settings: serde_json::json!({"width": 640, "password": "secret"}),
                hotkeys: serde_json::json!({}),
                filters: vec![],
                extra: BTreeMap::new(),
            }],
            scenes: vec![PersistedScene {
                id: "scene-a".into(),
                name: "Scene".into(),
                items: vec![PersistedSceneNode::Source(PersistedSourceItem {
                    id: "item-a".into(),
                    source_id: "source-a".into(),
                    state: state(0),
                    extra: BTreeMap::new(),
                })],
                extra: BTreeMap::new(),
            }],
            extra: BTreeMap::from([("future_field".into(), serde_json::json!({"x": 1}))]),
        }
    }

    #[test]
    fn versioned_json_round_trip_preserves_unknown_fields() {
        let document = fixture();
        let json = document.to_json().unwrap();
        let decoded = SceneCollectionDocument::from_json(&json).unwrap();
        assert_eq!(decoded, document);
        assert!(decoded.extra.contains_key("future_field"));
    }

    #[test]
    fn unsupported_schema_is_rejected() {
        let mut document = fixture();
        document.version += 1;
        assert!(matches!(
            SceneCollectionDocument::from_json(&serde_json::to_string(&document).unwrap()),
            Err(SceneCollectionError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn malformed_or_dangling_references_are_rejected() {
        assert!(SceneCollectionDocument::from_json("{").is_err());
        let mut document = fixture();
        let PersistedSceneNode::Source(item) = &mut document.scenes[0].items[0] else {
            unreachable!()
        };
        item.source_id = "missing".into();
        assert!(document.to_json().is_err());
    }

    #[test]
    fn portable_export_redacts_secrets_and_machine_paths() {
        let mut document = fixture();
        document.sources[0].settings["local_file"] = Value::String("/home/user/private.mp4".into());
        let export = document
            .export(SceneCollectionExportPolicy::portable_redacted())
            .unwrap();
        let decoded: Value = serde_json::from_str(&export.json).unwrap();
        assert_eq!(
            decoded["sources"][0]["settings"]["password"],
            REDACTED_SECRET
        );
        assert_eq!(
            decoded["sources"][0]["settings"]["local_file"],
            REDACTED_PATH
        );
        assert_eq!(export.redactions.len(), 2);
    }

    #[test]
    fn scene_item_state_round_trips_runtime_snapshot_shape() {
        let persisted = state(3);
        let snapshot = persisted.to_snapshot().unwrap();
        assert_eq!(snapshot.order_position, 3);
        assert_eq!(snapshot.transform.position, Vec2::new(1.0, 2.0));
        assert_eq!(PersistedSceneItemState::from_snapshot(&snapshot), persisted);
    }

    #[test]
    fn editor_undo_redo_restores_item_state_by_stable_id() {
        let document = fixture();
        let original = document.scenes[0].items[0].clone();
        let mut editor = SceneCollectionEditor::new(document).unwrap();
        let mut changed = state(0);
        changed.position = [99.0, 123.0];
        editor
            .set_item_state("scene-a", "item-a", changed.clone())
            .unwrap();
        let PersistedSceneNode::Source(item) = &editor.document().scenes[0].items[0] else {
            unreachable!()
        };
        assert_eq!(item.state, changed);
        assert!(editor.undo());
        assert_eq!(editor.document().scenes[0].items[0], original);
        assert!(editor.redo());
        let PersistedSceneNode::Source(item) = &editor.document().scenes[0].items[0] else {
            unreachable!()
        };
        assert_eq!(item.state.position, [99.0, 123.0]);
    }

    #[test]
    fn editor_models_group_and_ungroup_replacement_semantics() {
        let mut document = fixture();
        document.sources.push(PersistedSource {
            id: "source-b".into(),
            plugin_id: "color_source_v3".into(),
            name: "Color B".into(),
            settings: serde_json::json!({}),
            hotkeys: serde_json::json!({}),
            filters: vec![],
            extra: BTreeMap::new(),
        });
        document.scenes[0]
            .items
            .push(PersistedSceneNode::Source(PersistedSourceItem {
                id: "item-b".into(),
                source_id: "source-b".into(),
                state: state(1),
                extra: BTreeMap::new(),
            }));
        let mut editor = SceneCollectionEditor::new(document).unwrap();
        let grouped_children = vec![
            PersistedSourceItem {
                id: "item-a".into(),
                source_id: "source-a".into(),
                state: state(0),
                extra: BTreeMap::new(),
            },
            PersistedSourceItem {
                id: "item-b".into(),
                source_id: "source-b".into(),
                state: state(1),
                extra: BTreeMap::new(),
            },
        ];
        editor
            .replace_items_with_group(
                "scene-a",
                &["item-a".into(), "item-b".into()],
                PersistedGroup {
                    id: "group-a".into(),
                    name: "Group".into(),
                    state: state(0),
                    items: grouped_children.clone(),
                    extra: BTreeMap::new(),
                },
            )
            .unwrap();
        assert!(matches!(
            &editor.document().scenes[0].items[..],
            [PersistedSceneNode::Group(group)] if group.id == "group-a"
        ));

        let mut replacements = grouped_children;
        replacements[0].state.position = [400.0, 200.0];
        replacements[1].state.position = [800.0, 500.0];
        editor
            .ungroup_with_replacements("scene-a", "group-a", replacements.clone())
            .unwrap();
        assert_eq!(editor.document().scenes[0].items.len(), 2);
        let PersistedSceneNode::Source(first) = &editor.document().scenes[0].items[0] else {
            unreachable!()
        };
        assert_eq!(first.id, "item-a");
        assert_eq!(first.state.position, [400.0, 200.0]);

        assert!(editor.undo());
        assert!(matches!(
            &editor.document().scenes[0].items[..],
            [PersistedSceneNode::Group(group)] if group.id == "group-a"
        ));
        assert!(editor.redo());
        assert!(matches!(
            &editor.document().scenes[0].items[..],
            [PersistedSceneNode::Source(_), PersistedSceneNode::Source(_)]
        ));
    }

    #[test]
    fn atomic_save_replaces_without_truncating() {
        let root = std::env::temp_dir().join(format!(
            "libobs-rs-scene-collection-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("collection.json");
        fs::write(&path, "old").unwrap();
        fixture()
            .save_atomic(&path, SceneCollectionExportPolicy::include_all())
            .unwrap();
        let loaded = SceneCollectionDocument::load(&path).unwrap();
        assert_eq!(loaded.name.as_deref(), Some("test"));
        let _ = fs::remove_dir_all(root);
    }
}
