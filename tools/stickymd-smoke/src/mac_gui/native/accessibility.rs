//! AX inspection/actions for PID-owned status menu and save panel; no AppleScript.
//! Missing/ambiguous nodes fail with a bounded tree dump instead of coordinate guessing.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::cf::{Cf, Owned, kCFBooleanTrue};
use std::time::{Duration, Instant};
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementGetTypeID() -> usize;
    fn AXUIElementCreateApplication(pid: i32) -> Cf;
    fn AXUIElementCreateSystemWide() -> Cf;
    fn AXUIElementCopyAttributeValue(element: Cf, name: Cf, out: *mut Cf) -> i32;
    fn AXUIElementSetAttributeValue(element: Cf, name: Cf, value: Cf) -> i32;
    fn AXUIElementPerformAction(element: Cf, action: Cf) -> i32;
    fn AXUIElementCopyActionNames(element: Cf, out: *mut Cf) -> i32;
    fn AXUIElementGetPid(element: Cf, pid: *mut i32) -> i32;
    fn AXUIElementSetMessagingTimeout(element: Cf, seconds: f32) -> i32;
    fn AXUIElementCopyElementAtPosition(element: Cf, x: f32, y: f32, out: *mut Cf) -> i32;
}
fn app(pid: u32) -> Result<Owned, String> {
    let pid = i32::try_from(pid).map_err(|_| "invalid AX PID")?;
    // SAFETY: positive process ID, Create-rule reference, finite per-request timeout.
    unsafe {
        let root = Owned::take(AXUIElementCreateApplication(pid))?;
        AXUIElementSetMessagingTimeout(root.0, 0.25);
        Ok(root)
    }
}
fn optional_attribute(node: &Owned, name: &str) -> Result<Option<Owned>, String> {
    // SAFETY: the SDK type ID query has no arguments or ownership effects.
    if !node.is_type(unsafe { AXUIElementGetTypeID() }) {
        return Err(format!(
            "{name}: expected AXUIElement; inspection incomplete"
        ));
    }
    let name = Owned::string(name)?;
    let mut out = std::ptr::null();
    // SAFETY: AX object and CFString live through call; initialized out receives Copy-rule value.
    let status = unsafe { AXUIElementCopyAttributeValue(node.0, name.0, &mut out) };
    if !crate::mac_gui::observation::ax_attribute_present(status)? {
        return Ok(None);
    }
    Owned::take(out).map(Some)
}
fn attribute(node: &Owned, name: &str) -> Result<Owned, String> {
    optional_attribute(node, name)?.ok_or_else(|| format!("required AX attribute {name} absent"))
}
fn text(node: &Owned, name: &str) -> Result<String, String> {
    match optional_attribute(node, name)? {
        None => Ok(String::new()),
        Some(v) => v
            .text()
            .ok_or_else(|| format!("{name}: expected readable CFString; inspection incomplete")),
    }
}
fn action_status(node: &Owned, name: &str) -> Result<i32, String> {
    let name = Owned::string(name)?;
    // SAFETY: live AX element and CFString action name; API does not retain borrowed arguments.
    Ok(unsafe { AXUIElementPerformAction(node.0, name.0) })
}
fn action(node: &Owned, name: &str) -> Result<(), String> {
    match action_status(node, name)? {
        0 => Ok(()),
        status => Err(format!("AX {name} error {status}")),
    }
}
/// Performs a menu action whose effect the caller observes afterwards; see
/// `ax_action_confirmed` for why CannotComplete is reported as unconfirmed.
fn menu_action(node: &Owned, name: &str) -> Result<&'static str, String> {
    let status = action_status(node, name)?;
    Ok(
        match crate::mac_gui::observation::ax_action_confirmed(name, status)? {
            true => "confirmed",
            false => "unconfirmed (-25204)",
        },
    )
}
fn set(node: &Owned, name: &str, value: &Owned) -> Result<(), String> {
    let name = Owned::string(name)?;
    // SAFETY: live AX element, attribute string and value for the synchronous set.
    let status = unsafe { AXUIElementSetAttributeValue(node.0, name.0, value.0) };
    if status == 0 {
        Ok(())
    } else {
        Err(format!("AX set error {status}"))
    }
}
struct Node {
    element: Owned,
    role: String,
    title: String,
    description: String,
    identifier: String,
}
impl Node {
    fn describes(&self, label: &str) -> bool {
        self.title == label || self.description == label || self.identifier == label
    }
}
fn tree(root: Owned) -> Result<Vec<Node>, String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut pending = vec![(root, 0)];
    let mut nodes: Vec<Node> = Vec::new();
    while let Some((element, depth)) = pending.pop() {
        if nodes.iter().any(|n| n.element.same(&element)) {
            continue;
        }
        if nodes.len() >= 512 || depth > 12 || Instant::now() > deadline {
            return Err("AX tree budget exceeded; inspection incomplete".into());
        }
        let role = attribute(&element, "AXRole")?
            .text()
            .ok_or("AXRole: expected CFString")?;
        // App extras menu bar is not always in AXChildren. Fetch it explicitly, not all attrs.
        let attrs: &[&str] = if depth == 0 {
            &["AXChildren", "AXExtrasMenuBar"]
        } else {
            &["AXChildren"]
        };
        for attr in attrs {
            if let Some(children) = optional_attribute(&element, attr)? {
                if *attr == "AXExtrasMenuBar" {
                    pending.push((children, depth + 1));
                } else {
                    for child in children.array()? {
                        pending.push((child, depth + 1));
                    }
                }
            }
        }
        nodes.push(Node {
            role,
            title: text(&element, "AXTitle")?,
            description: text(&element, "AXDescription")?,
            identifier: text(&element, "AXIdentifier")?,
            element,
        });
    }
    Ok(nodes)
}
fn dump(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|n| {
            format!(
                "{} title={:?} description={:?} id={:?}",
                n.role, n.title, n.description, n.identifier
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn unique(nodes: &[Node], matches: impl Fn(&Node) -> bool) -> Result<&Node, String> {
    let matches = nodes.iter().filter(|n| matches(n)).collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!(
            "expected one AX node, found {}; tree:\n{}",
            matches.len(),
            dump(nodes)
        ));
    }
    Ok(matches[0])
}
pub(in crate::mac_gui) fn front() -> Result<u32, String> {
    // SAFETY: Create-rule systemwide AX object; no mutations.
    let system = Owned::take(unsafe { AXUIElementCreateSystemWide() })?;
    let focused = attribute(&system, "AXFocusedApplication")?;
    let mut pid = 0;
    // SAFETY: focused AX application is live; writable i32 PID storage.
    if unsafe { AXUIElementGetPid(focused.0, &mut pid) } != 0 || pid <= 0 {
        return Err("AX frontmost PID unavailable".into());
    }
    Ok(pid as u32)
}
/// The process that owns the element under `(x, y)` (global top-left points), found by the
/// system's own hit test: windows that pass clicks through, such as the Dock's full-screen
/// overlay outside its tiles, and the pointer are not hits.
pub(in crate::mac_gui) fn pid_at(x: f64, y: f64) -> Result<u32, String> {
    // SAFETY: Create-rule systemwide AX object, with a finite per-request timeout.
    let system = Owned::take(unsafe { AXUIElementCreateSystemWide() })?;
    // SAFETY: live systemwide element.
    unsafe { AXUIElementSetMessagingTimeout(system.0, 0.5) };
    let mut out = std::ptr::null();
    // SAFETY: live element; the initialized output receives a Copy-rule AX element.
    let status =
        unsafe { AXUIElementCopyElementAtPosition(system.0, x as f32, y as f32, &mut out) };
    if status != 0 {
        return Err(format!("AX hit test at ({x}, {y}) error {status}"));
    }
    let element = Owned::take(out)?;
    let mut pid = 0;
    // SAFETY: the element is live; writable i32 PID storage.
    if unsafe { AXUIElementGetPid(element.0, &mut pid) } != 0 || pid <= 0 {
        return Err(format!("AX hit test at ({x}, {y}) has no owning process"));
    }
    Ok(pid as u32)
}
pub(in crate::mac_gui) fn activate(pid: u32) -> Result<(), String> {
    let root = app(pid)?;
    let name = Owned::string("AXFrontmost")?;
    // SAFETY: process-owned AX root and static CFBoolean true; no ownership is transferred.
    let status = unsafe { AXUIElementSetAttributeValue(root.0, name.0, kCFBooleanTrue) };
    if status != 0 {
        return Err(format!("AX activate error {status}"));
    }
    Ok(())
}
pub(in crate::mac_gui) fn dock_titles(pid: u32) -> Result<String, String> {
    let nodes = tree(app(pid)?)?;
    if !nodes.iter().any(|n| n.role == "AXList") || !nodes.iter().any(|n| n.role == "AXDockItem") {
        return Err(format!("Dock AX list unavailable:\n{}", dump(&nodes)));
    }
    Ok(nodes
        .iter()
        .filter(|n| n.role == "AXDockItem")
        .map(|n| n.title.as_str())
        .collect::<Vec<_>>()
        .join("\n"))
}
pub(in crate::mac_gui) fn menu(pid: u32, label: &str) -> Result<String, String> {
    let root = app(pid)?;
    let extras = attribute(&root, "AXExtrasMenuBar")?;
    let nodes = tree(extras)?;
    let item = unique(&nodes, |n| n.role == "AXMenuBarItem")?;
    let mut actions = std::ptr::null();
    // SAFETY: live status item; initialized output receives a Copy-rule CFArray.
    let status = unsafe { AXUIElementCopyActionNames(item.element.0, &mut actions) };
    if status != 0 {
        return Err(format!("status-item actions unavailable: {status}"));
    }
    let actions = Owned::take(actions)?
        .array()?
        .iter()
        .filter_map(Owned::text)
        .collect::<Vec<_>>();
    let open = if actions.iter().any(|a| a == "AXShowMenu") {
        menu_action(&item.element, "AXShowMenu")?
    } else if actions.iter().any(|a| a == "AXPress") {
        menu_action(&item.element, "AXPress")?
    } else {
        return Err(format!("status item has no menu action: {actions:?}"));
    };
    super::events::wait(150)?;
    let nodes = tree(app(pid)?)?;
    let titles = nodes
        .iter()
        .filter(|n| n.role == "AXMenuItem")
        .map(|n| n.title.as_str())
        .collect::<Vec<_>>();
    if titles.len() != 3
        || !titles.contains(&"置顶")
        || !titles.contains(&"退出")
        || !(titles.contains(&"显示") || titles.contains(&"隐藏"))
    {
        return Err(format!(
            "expected exactly the three product menu items:\n{}",
            dump(&nodes)
        ));
    }
    let item = unique(&nodes, |n| n.role == "AXMenuItem" && n.title == label)?;
    let press = menu_action(&item.element, "AXPress")?;
    Ok(format!(
        "status actions={actions:?}; open {open}; press {label} {press}\n{}",
        dump(&nodes)
    ))
}
fn panel_tree(pid: u32) -> Result<Vec<Node>, String> {
    let start = Instant::now();
    loop {
        let nodes = tree(app(pid)?)?;
        if nodes
            .iter()
            .any(|n| n.role == "AXTextField" || n.role == "AXComboBox")
        {
            return Ok(nodes);
        }
        if start.elapsed() > Duration::from_secs(3) {
            return Err(format!("save panel absent:\n{}", dump(&nodes)));
        }
        super::events::wait(50)?;
    }
}
pub(in crate::mac_gui) fn panel_open(pid: u32) -> Result<bool, String> {
    let nodes = tree(app(pid)?)?;
    Ok(nodes.iter().any(|n| {
        n.role == "AXButton"
            && ["Save", "保存", "Export", "导出"]
                .iter()
                .any(|l| n.describes(l))
    }))
}
pub(in crate::mac_gui) fn panel_button(pid: u32, save: bool) -> Result<String, String> {
    let nodes = panel_tree(pid)?;
    let labels = if save {
        &["Save", "保存", "Export", "导出"][..]
    } else {
        &["Cancel", "取消"][..]
    };
    let button = unique(&nodes, |n| {
        n.role == "AXButton" && labels.iter().any(|l| n.describes(l))
    })?;
    action(&button.element, "AXPress")?;
    Ok(dump(&nodes))
}
pub(in crate::mac_gui) fn filename(pid: u32, name: &str) -> Result<String, String> {
    let nodes = panel_tree(pid)?;
    let field = unique(&nodes, |n| {
        n.role == "AXTextField"
            && (n.identifier == "saveAsNameTextField"
                || ["Save As:", "存储为:", "存储为："]
                    .iter()
                    .any(|l| n.describes(l)))
    })?;
    set(&field.element, "AXValue", &Owned::string(name)?)?;
    Ok(dump(&nodes))
}
pub(in crate::mac_gui) fn folder(pid: u32, path: &str) -> Result<String, String> {
    let nodes = panel_tree(pid)?;
    // Cmd+Shift+G's focused field belongs to the PID's native go-to-folder sheet.
    let root = app(pid)?;
    let focused = attribute(&root, "AXFocusedUIElement")?;
    let role = attribute(&focused, "AXRole")?
        .text()
        .ok_or("AXRole: expected CFString")?;
    if !matches!(role.as_str(), "AXTextField" | "AXComboBox") {
        return Err(format!("Go to Folder focus is {role}; {}", dump(&nodes)));
    }
    set(&focused, "AXValue", &Owned::string(path)?)?;
    Ok(dump(&nodes))
}
