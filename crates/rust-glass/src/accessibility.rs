//! Accessibility tree for screen readers and other assistive technology.
//!
//! Widgets describe themselves with [`Ui::access`](crate::Ui::access) while
//! they are drawn. Nothing is collected until a screen reader asks for the
//! tree, so an app without one pays nothing. The window host passes the tree
//! to the platform through AccessKit and turns requests from the screen
//! reader into [`Event::Access`](crate::input::Event::Access) input.

use std::collections::HashSet;

use crate::context::{Id, Order};
use crate::geometry::Rect;

/// What kind of element a node is. Screen readers announce it with the label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// Something to press.
    Button,
    /// A link to another place.
    Link,
    /// A box that is checked or not.
    CheckBox,
    /// An on and off switch.
    Switch,
    /// One choice of a group.
    RadioButton,
    /// A value picked along a track.
    Slider,
    /// A number changed in steps.
    SpinButton,
    /// A one line text field.
    TextInput,
    /// A text field of several lines.
    MultilineTextInput,
    /// A text field whose content is hidden.
    PasswordInput,
    /// A search field.
    SearchInput,
    /// Text.
    Label,
    /// A title of a page or section.
    Heading,
    /// One tab, segment or navigation entry.
    Tab,
    /// A group of tabs.
    TabList,
    /// An entry of a menu.
    MenuItem,
    /// A button that opens a list of options.
    ComboBox,
    /// Progress of a task.
    ProgressIndicator,
    /// A value within a known range, such as a gauge.
    Meter,
    /// A picture.
    Image,
    /// A color swatch that opens a picker.
    ColorWell,
    /// An area that scrolls.
    ScrollView,
    /// A list of items.
    List,
    /// One item of a list.
    ListItem,
    /// Elements that belong together.
    Group,
}

impl Role {
    /// Returns `true` for roles that react to a click.
    pub fn clickable(self) -> bool {
        matches!(
            self,
            Role::Button
                | Role::Link
                | Role::CheckBox
                | Role::Switch
                | Role::RadioButton
                | Role::Tab
                | Role::MenuItem
                | Role::ComboBox
                | Role::ColorWell
        )
    }
    fn text_input(self) -> bool {
        matches!(self, Role::TextInput | Role::MultilineTextInput | Role::PasswordInput | Role::SearchInput)
    }
}

/// The number of a slider, stepper or progress indicator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Numeric {
    /// Current value.
    pub value: f64,
    /// Smallest value.
    pub min: f64,
    /// Largest value.
    pub max: f64,
    /// Step between values, if it moves in steps.
    pub step: Option<f64>,
}

/// One element as assistive technology sees it.
#[derive(Clone, Debug)]
pub struct Node {
    /// The id of the widget. Requests from the screen reader carry it.
    pub id: Id,
    /// What kind of element it is.
    pub role: Role,
    /// Bounds in points.
    pub rect: Rect,
    /// The name read out first.
    pub label: String,
    /// Text content or a value, read out after the label.
    pub value: Option<String>,
    /// The number of a slider or stepper.
    pub numeric: Option<Numeric>,
    /// Whether a checkbox or switch is on.
    pub toggled: Option<bool>,
    /// Whether a tab, segment or menu item is selected.
    pub selected: Option<bool>,
    /// Whether a menu or section is open.
    pub expanded: Option<bool>,
    /// The element cannot be used right now.
    pub disabled: bool,
    /// Set when the node was registered with [`Context::focusable`](crate::Context::focusable).
    pub focusable: bool,
    pub(crate) layer: usize,
}

impl Node {
    /// Creates a node without a label.
    pub fn new(id: Id, role: Role, rect: Rect) -> Self {
        Self {
            id,
            role,
            rect,
            label: String::new(),
            value: None,
            numeric: None,
            toggled: None,
            selected: None,
            expanded: None,
            disabled: false,
            focusable: false,
            layer: 0,
        }
    }
    /// Sets the label.
    pub fn label(&mut self, label: impl Into<String>) -> &mut Self {
        self.label = label.into();
        self
    }
    /// Sets the value read after the label.
    pub fn value(&mut self, value: impl Into<String>) -> &mut Self {
        self.value = Some(value.into());
        self
    }
    /// Sets the number of a slider, stepper or progress indicator.
    pub fn numeric(&mut self, value: f64, min: f64, max: f64, step: Option<f64>) -> &mut Self {
        self.numeric = Some(Numeric { value, min, max, step });
        self
    }
    /// Sets whether a checkbox or switch is on.
    pub fn toggled(&mut self, on: bool) -> &mut Self {
        self.toggled = Some(on);
        self
    }
    /// Sets whether the element is selected.
    pub fn selected(&mut self, on: bool) -> &mut Self {
        self.selected = Some(on);
        self
    }
    /// Sets whether the element is open.
    pub fn expanded(&mut self, on: bool) -> &mut Self {
        self.expanded = Some(on);
        self
    }
}

/// A request from assistive technology. Arrives as
/// [`Event::Access`](crate::input::Event::Access) and is applied by the widget
/// it targets.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Press the element.
    Click,
    /// Give the element keyboard focus.
    Focus,
    /// Take keyboard focus away from the element.
    Blur,
    /// One step up. Delivered to the focused widget as the right arrow key.
    Increment,
    /// One step down. Delivered as the left arrow key.
    Decrement,
    /// Replace the whole text of a field.
    SetValue(String),
    /// Set the value of a slider or stepper.
    SetNumber(f64),
    /// Insert text at the caret, replacing the selection.
    ReplaceSelection(String),
}

/// Collected nodes and pending requests of one window.
#[derive(Default)]
pub(crate) struct Access {
    pub on: bool,
    pub nodes: Vec<Node>,
    pub requests: Vec<(Id, Action)>,
}

/// Window title, kept as the label of the root node.
pub(crate) const ROOT: Id = Id(0x9e37_79b9_7f4a_7c15);

pub(crate) struct LayerInfo {
    pub index: usize,
    pub id: Id,
    pub order: Order,
}

/// Turn the nodes of a frame into a full AccessKit tree.
pub(crate) fn tree(
    nodes: &[Node],
    layers: &[LayerInfo],
    focus: Option<Id>,
    ppp: f32,
    screen: Rect,
    title: &str,
) -> accesskit::TreeUpdate {
    use accesskit::{
        Action as A, Affine, Node as N, NodeId, Rect as R, Role as AR, Toggled, TreeInfo, TreeUpdate,
    };
    let rect = |r: Rect| R { x0: r.min.x as f64, y0: r.min.y as f64, x1: r.max.x as f64, y1: r.max.y as f64 };
    let mut out: Vec<(NodeId, N)> = Vec::with_capacity(nodes.len() + layers.len() + 1);
    let mut seen = HashSet::new();
    seen.insert(ROOT);
    let mut root_children = Vec::new();
    let mut focus_found = None;
    for l in layers {
        let mut children = Vec::new();
        let mut bounds = Rect::NOTHING;
        for n in nodes.iter().filter(|n| n.layer == l.index) {
            // A widget drawn twice in one frame keeps its first node.
            if !seen.insert(n.id) {
                continue;
            }
            children.push(NodeId(n.id.0));
            bounds = bounds.union(n.rect);
            if focus == Some(n.id) {
                focus_found = Some(n.id);
            }
            let mut a = N::new(match n.role {
                Role::Button => AR::Button,
                Role::Link => AR::Link,
                Role::CheckBox => AR::CheckBox,
                Role::Switch => AR::Switch,
                Role::RadioButton => AR::RadioButton,
                Role::Slider => AR::Slider,
                Role::SpinButton => AR::SpinButton,
                Role::TextInput => AR::TextInput,
                Role::MultilineTextInput => AR::MultilineTextInput,
                Role::PasswordInput => AR::PasswordInput,
                Role::SearchInput => AR::SearchInput,
                Role::Label => AR::Label,
                Role::Heading => AR::Heading,
                Role::Tab => AR::Tab,
                Role::TabList => AR::TabList,
                Role::MenuItem => AR::MenuItem,
                Role::ComboBox => AR::ComboBox,
                Role::ProgressIndicator => AR::ProgressIndicator,
                Role::Meter => AR::Meter,
                Role::Image => AR::Image,
                Role::ColorWell => AR::ColorWell,
                Role::ScrollView => AR::ScrollView,
                Role::List => AR::List,
                Role::ListItem => AR::ListItem,
                Role::Group => AR::Group,
            });
            a.set_bounds(rect(n.rect));
            match (&n.value, n.role) {
                // Static text is read from its value.
                (None, Role::Label) => a.set_value(n.label.as_str()),
                (v, _) => {
                    if !n.label.is_empty() {
                        a.set_label(n.label.as_str());
                    }
                    if let Some(v) = v {
                        a.set_value(v.as_str());
                    }
                }
            }
            if let Some(v) = n.numeric {
                a.set_numeric_value(v.value);
                a.set_min_numeric_value(v.min);
                a.set_max_numeric_value(v.max);
                if let Some(s) = v.step {
                    a.set_numeric_value_step(s);
                }
            }
            if let Some(t) = n.toggled {
                a.set_toggled(if t { Toggled::True } else { Toggled::False });
            }
            if let Some(s) = n.selected {
                a.set_selected(s);
            }
            if let Some(e) = n.expanded {
                a.set_expanded(e);
            }
            if n.disabled {
                a.set_disabled();
            } else {
                if n.role.clickable() {
                    a.add_action(A::Click);
                }
                if n.focusable {
                    a.add_action(A::Focus);
                }
                if n.numeric.is_some() && matches!(n.role, Role::Slider | Role::SpinButton) {
                    a.add_action(A::Increment);
                    a.add_action(A::Decrement);
                    a.add_action(A::SetValue);
                }
                if n.role.text_input() {
                    a.add_action(A::SetValue);
                    a.add_action(A::ReplaceSelectedText);
                }
            }
            out.push((NodeId(n.id.0), a));
        }
        if children.is_empty() {
            continue;
        }
        let group_id = l.id.with("access");
        let mut g = N::new(match l.order {
            Order::Window => AR::Window,
            Order::Modal => AR::Dialog,
            Order::Toast => AR::Alert,
            Order::Tooltip => AR::Tooltip,
            _ => AR::Group,
        });
        g.set_bounds(rect(bounds));
        g.set_children(children);
        root_children.push(NodeId(group_id.0));
        out.push((NodeId(group_id.0), g));
    }
    let mut root = N::new(AR::Window);
    root.set_label(title);
    root.set_bounds(rect(screen));
    root.set_transform(Affine::scale(ppp as f64));
    root.set_children(root_children);
    out.push((NodeId(ROOT.0), root));
    let mut info = TreeInfo::new(NodeId(ROOT.0));
    info.toolkit_name = Some("rust-glass".into());
    info.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
    TreeUpdate {
        nodes: out,
        tree: Some(info),
        tree_id: accesskit::TreeId::ROOT,
        focus: NodeId(focus_found.unwrap_or(ROOT).0),
    }
}

/// Translate a request from AccessKit. Returns `None` for actions no widget
/// handles.
pub(crate) fn from_accesskit(req: &accesskit::ActionRequest) -> Option<(Id, Action)> {
    use accesskit::{Action as A, ActionData as D};
    let id = Id(req.target_node.0);
    let action = match (req.action, &req.data) {
        (A::Click | A::Expand | A::Collapse, _) => Action::Click,
        (A::Focus, _) => Action::Focus,
        (A::Blur, _) => Action::Blur,
        (A::Increment, _) => Action::Increment,
        (A::Decrement, _) => Action::Decrement,
        (A::SetValue, Some(D::Value(v))) => Action::SetValue(v.to_string()),
        (A::SetValue, Some(D::NumericValue(v))) => Action::SetNumber(*v),
        (A::ReplaceSelectedText, Some(D::Value(v))) => Action::ReplaceSelection(v.to_string()),
        _ => return None,
    };
    Some((id, action))
}
