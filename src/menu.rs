//! Nested menus with intrinsic, uniform item widths.
//!
//! A [`Menu`] wraps a trigger element and opens a panel of [`Item`]s anchored
//! to it. Items may nest arbitrarily via [`submenu`], and every level opens on
//! hover of its parent row.
//!
//! # Sizing
//!
//! A menu panel is sized by its contents. Every row in a panel is laid out
//! twice: first loosely, to find the widest intrinsic row, and then pinned to
//! that width. Rows therefore end up identically wide without the caller
//! setting a width on the panel or on any item.
//!
//! Because the measuring pass compresses fluid lengths, a row may use `Fill`
//! to push trailing content to the right edge of the panel and still measure at
//! its natural width:
//!
//! ```rust,ignore
//! use iced::{Fill, widget::{row, space, text}};
//!
//! menu(
//!     text("Edit"),
//!     vec![
//!         item(row![text("Cut"), space().width(Fill), text("Ctrl+X")]).on_press(Message::Cut),
//!         toggle(text("Word Wrap"), self.wrap).on_press(Message::ToggleWrap),
//!         item(text("Preferences")).icon(text("⚙")).on_press(Message::Prefs),
//!         separator(),
//!         submenu(text("Paste Special"), vec![
//!             item(text("Values")).on_press(Message::PasteValues),
//!             item(text("Formatting")).on_press(Message::PasteFormatting),
//!         ]),
//!     ],
//! )
//! ```
//!
//! # Gutters
//!
//! A row is laid out as `[leading gutter] [content] [trailing gutter]`. Both
//! gutters are sized once per panel, so a row without an icon still indents its
//! content past the widest one and every label in the panel starts at the same
//! x. A gutter nobody uses costs nothing.
//!
//! [`Item::icon`] always occupies the leading gutter. A [`toggle`] draws its
//! checkmark in whichever gutter [`Menu::check_side`] names, and reserves that
//! slot even while unchecked so labels do not shift as rows are toggled. A
//! [`submenu`] draws a chevron in the trailing gutter, sharing it with trailing
//! checkmarks — a row is never both.
//!
//! # Icons
//!
//! The checkmark and the chevron come from the Lucide font embedded in
//! [`crate::lucide`], which the application has to register once at startup:
//!
//! ```rust,ignore
//! iced::application(App::new, App::update, App::view)
//!     .font(widgets::lucide::FONT_BYTES)
//!     .run()
//! ```
//!
//! Without it, those two glyphs render as blanks. Everything else still works.
//!
//! # Triggers
//!
//! A menu draws its own trigger surface by default, with idle, hovered and open
//! states — which is what lets a menu-bar entry stay lit while its panel is
//! showing. Pass a bare `text` or icon as the trigger and let the menu style it.
//! Use [`Menu::plain_trigger`] when the trigger is already a `button`, or
//! anything else that paints its own background.
//!
//! A menu claims a press on its trigger before the trigger element sees it, so
//! a trigger that handles presses itself still opens the menu — but its own
//! `on_press` never fires.
//!
//! # Placement
//!
//! Panels are placed by [`crate::anchor`], so a panel that would overflow the
//! viewport flips to the opposite side of its anchor before it is clamped. A
//! submenu near the right edge of the window therefore opens to the left of its
//! parent panel instead of on top of it.

use iced::advanced::widget::{self, Operation, tree::Tree};
use iced::advanced::{
    Layout, Shell, Widget, layout,
    layout::{Limits, Node},
    overlay, renderer, text,
};
use iced::alignment::Vertical;
use iced::{
    Background, Border, Color, Element, Event, Length, Padding, Pixels, Point, Rectangle, Shadow,
    Size, Theme, Vector, mouse, touch, window,
};

use crate::anchor::{self, Align, Placement, Side};
use crate::lucide;

/// The default gap between a root panel and its trigger.
const DEFAULT_GAP: f32 = 4.0;

/// The default gap between a submenu panel and its parent panel.
///
/// Submenus overlap their parent slightly so the two panels read as one
/// connected surface and the cursor never crosses a dead gap between them.
const DEFAULT_SUBMENU_GAP: f32 = -4.0;

/// The default padding inside a panel, around the stack of rows.
const DEFAULT_PANEL_PADDING: f32 = 4.0;

/// The default corner radius of a panel.
const DEFAULT_RADIUS: f32 = 6.0;

/// The default padding around the contents of a menu-drawn trigger.
const DEFAULT_TRIGGER_PADDING: Padding = Padding {
    top: 4.0,
    right: 10.0,
    bottom: 4.0,
    left: 10.0,
};

/// The default space between a gutter and the row contents.
const DEFAULT_GUTTER_SPACING: f32 = 8.0;

/// The size of a gutter glyph, as a fraction of the renderer's default text
/// size.
///
/// Lucide glyphs are drawn as strokes inside a square box, so they need a touch
/// more room than a text cap height to read at the same weight as a label.
const GLYPH_SCALE: f32 = 1.15;

/// The height of a [`separator`] row, including the space around the rule.
const SEPARATOR_HEIGHT: f32 = 9.0;

/// The thickness of the rule drawn inside a [`separator`] row.
const SEPARATOR_THICKNESS: f32 = 1.0;

/// Creates a [`Menu`] that opens below its trigger when pressed.
pub fn menu<'a, Message, Theme, Renderer>(
    trigger: impl Into<Element<'a, Message, Theme, Renderer>>,
    items: Vec<Item<'a, Message, Theme, Renderer>>,
) -> Menu<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font>,
{
    Menu::new(trigger, items)
}

/// Creates a selectable [`Item`].
///
/// An item without an [`Item::on_press`] message is inert: it still highlights
/// on hover but does nothing when pressed.
pub fn item<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Item<'a, Message, Theme, Renderer> {
    Item::Entry {
        content: content.into(),
        icon: None,
        check: None,
        on_press: None,
        enabled: true,
    }
}

/// Creates a checkable [`Item`], drawn with a checkmark while `is_checked`.
///
/// The checkmark sits in the gutter chosen by [`Menu::check_side`]. An
/// unchecked item still reserves its slot, so labels stay aligned down the
/// panel whichever rows happen to be checked.
pub fn toggle<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    is_checked: bool,
) -> Item<'a, Message, Theme, Renderer> {
    item(content).checked(is_checked)
}

/// Creates an [`Item`] that opens a nested panel when hovered.
pub fn submenu<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    items: Vec<Item<'a, Message, Theme, Renderer>>,
) -> Item<'a, Message, Theme, Renderer> {
    Item::Submenu {
        content: content.into(),
        icon: None,
        items,
        enabled: true,
    }
}

/// Creates a horizontal rule between groups of items.
pub fn separator<'a, Message, Theme, Renderer>() -> Item<'a, Message, Theme, Renderer> {
    Item::Separator
}

/// A single row of a menu panel.
#[allow(missing_debug_implementations)]
pub enum Item<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    /// A selectable row.
    Entry {
        /// The contents of the row.
        content: Element<'a, Message, Theme, Renderer>,
        /// An icon drawn in the leading gutter.
        icon: Option<Element<'a, Message, Theme, Renderer>>,
        /// Whether the row draws a checkmark, and whether it is currently set.
        ///
        /// `None` means the row is not checkable at all.
        check: Option<bool>,
        /// The message published when the row is pressed.
        on_press: Option<Message>,
        /// Whether the row may be hovered and pressed.
        enabled: bool,
    },
    /// A row that opens a nested panel on hover.
    Submenu {
        /// The contents of the row.
        content: Element<'a, Message, Theme, Renderer>,
        /// An icon drawn in the leading gutter.
        icon: Option<Element<'a, Message, Theme, Renderer>>,
        /// The rows of the nested panel.
        items: Vec<Item<'a, Message, Theme, Renderer>>,
        /// Whether the row may be hovered to open its panel.
        enabled: bool,
    },
    /// A horizontal rule.
    Separator,
}

impl<'a, Message, Theme, Renderer> Item<'a, Message, Theme, Renderer> {
    /// Sets the message published when this [`Item`] is pressed.
    ///
    /// This has no effect on a [`submenu`] or a [`separator`].
    pub fn on_press(mut self, message: Message) -> Self {
        if let Self::Entry { on_press, .. } = &mut self {
            *on_press = Some(message);
        }

        self
    }

    /// Sets whether this [`Item`] responds to the cursor.
    ///
    /// A disabled row is drawn with the [`Style::disabled_text_color`] and
    /// neither highlights nor opens a submenu.
    pub fn enabled(mut self, is_enabled: bool) -> Self {
        match &mut self {
            Self::Entry { enabled, .. } | Self::Submenu { enabled, .. } => *enabled = is_enabled,
            Self::Separator => {}
        }

        self
    }

    /// Sets an icon drawn in the leading gutter of this [`Item`].
    ///
    /// Every row in a panel indents its contents past the widest icon, so
    /// labels line up whether or not their own row has one.
    pub fn icon(mut self, icon: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        match &mut self {
            Self::Entry { icon: slot, .. } | Self::Submenu { icon: slot, .. } => {
                *slot = Some(icon.into());
            }
            Self::Separator => {}
        }

        self
    }

    /// Makes this [`Item`] checkable, and sets whether it is currently checked.
    ///
    /// This has no effect on a [`submenu`] or a [`separator`].
    pub fn checked(mut self, is_checked: bool) -> Self {
        if let Self::Entry { check, .. } = &mut self {
            *check = Some(is_checked);
        }

        self
    }

    /// Returns the content element of this [`Item`], if it has one.
    fn content(&self) -> Option<&Element<'a, Message, Theme, Renderer>> {
        match self {
            Self::Entry { content, .. } | Self::Submenu { content, .. } => Some(content),
            Self::Separator => None,
        }
    }

    /// Returns the content element of this [`Item`] mutably, if it has one.
    fn content_mut(&mut self) -> Option<&mut Element<'a, Message, Theme, Renderer>> {
        match self {
            Self::Entry { content, .. } | Self::Submenu { content, .. } => Some(content),
            Self::Separator => None,
        }
    }

    /// Returns the icon element of this [`Item`], if it has one.
    fn icon_element(&self) -> Option<&Element<'a, Message, Theme, Renderer>> {
        match self {
            Self::Entry { icon, .. } | Self::Submenu { icon, .. } => icon.as_ref(),
            Self::Separator => None,
        }
    }

    /// Returns the icon element of this [`Item`] mutably, if it has one.
    fn icon_mut(&mut self) -> Option<&mut Element<'a, Message, Theme, Renderer>> {
        match self {
            Self::Entry { icon, .. } | Self::Submenu { icon, .. } => icon.as_mut(),
            Self::Separator => None,
        }
    }

    /// Returns whether this [`Item`] is checkable, and its current state.
    fn check(&self) -> Option<bool> {
        match self {
            Self::Entry { check, .. } => *check,
            Self::Submenu { .. } | Self::Separator => None,
        }
    }

    /// Returns `true` when this [`Item`] opens a nested panel.
    fn opens_submenu(&self) -> bool {
        matches!(self, Self::Submenu { .. })
    }

    /// Returns `true` when the cursor may interact with this [`Item`].
    fn is_interactive(&self) -> bool {
        match self {
            Self::Entry { enabled, .. } | Self::Submenu { enabled, .. } => *enabled,
            Self::Separator => false,
        }
    }

    /// Builds the state [`Tree`] for this [`Item`].
    ///
    /// The first two children are always the content and the icon — the icon
    /// slot stays present but empty when the row has none, so the indices below
    /// are fixed. A [`Item::Submenu`] appends the trees of its own rows after
    /// them, which makes the shape of the state tree mirror the menu.
    fn tree(&self) -> Tree
    where
        Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font>,
    {
        match self {
            Self::Entry { content, icon, .. } => Tree {
                children: vec![Tree::new(content.as_widget()), icon_tree(icon)],
                ..Tree::empty()
            },
            Self::Submenu {
                content,
                icon,
                items,
                ..
            } => Tree {
                children: [Tree::new(content.as_widget()), icon_tree(icon)]
                    .into_iter()
                    .chain(items.iter().map(Item::tree))
                    .collect(),
                ..Tree::empty()
            },
            Self::Separator => Tree::empty(),
        }
    }

    /// Reconciles an existing state [`Tree`] with this [`Item`].
    fn diff(&mut self, tree: &mut Tree)
    where
        Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font>,
    {
        match self {
            Self::Entry { content, icon, .. } => {
                if tree.children.len() != ITEM_ROWS {
                    tree.children = vec![Tree::new(content.as_widget()), icon_tree(icon)];
                }

                tree.children[ITEM_CONTENT].diff(content.as_widget_mut());

                if let Some(icon) = icon {
                    tree.children[ITEM_ICON].diff(icon.as_widget_mut());
                }
            }
            Self::Submenu {
                content,
                icon,
                items,
                ..
            } => {
                if tree.children.len() != items.len() + ITEM_ROWS {
                    tree.children = [Tree::new(content.as_widget()), icon_tree(icon)]
                        .into_iter()
                        .chain(items.iter().map(Item::tree))
                        .collect();
                }

                let (own, item_trees) = tree.children.split_at_mut(ITEM_ROWS);
                own[ITEM_CONTENT].diff(content.as_widget_mut());

                if let Some(icon) = icon {
                    own[ITEM_ICON].diff(icon.as_widget_mut());
                }

                for (item, tree) in items.iter_mut().zip(item_trees) {
                    item.diff(tree);
                }
            }
            Self::Separator => tree.children.clear(),
        }
    }
}

/// The index of an item's content within its state [`Tree`].
const ITEM_CONTENT: usize = 0;

/// The index of an item's icon within its state [`Tree`].
const ITEM_ICON: usize = 1;

/// The index at which a submenu's nested row trees begin.
const ITEM_ROWS: usize = 2;

/// Builds the state [`Tree`] for an optional icon.
///
/// A row without an icon keeps an empty tree in the slot so that [`ITEM_ROWS`]
/// is a constant offset rather than one that depends on the row.
fn icon_tree<Message, Theme, Renderer>(
    icon: &Option<Element<'_, Message, Theme, Renderer>>,
) -> Tree
where
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font>,
{
    match icon {
        Some(icon) => Tree::new(icon.as_widget()),
        None => Tree::empty(),
    }
}

/// A menu of [`Item`]s anchored to a trigger element.
///
/// The trigger is drawn and updated unchanged: a menu adds no padding,
/// background, border or text color of its own. Wrap it in a `button` if you
/// want one.
#[allow(missing_debug_implementations)]
pub struct Menu<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: Catalog,
{
    id: Option<widget::Id>,
    trigger: Element<'a, Message, Theme, Renderer>,
    trigger_padding: Padding,
    plain_trigger: bool,
    items: Vec<Item<'a, Message, Theme, Renderer>>,
    placement: Placement,
    submenu_gap: f32,
    panel_padding: f32,
    item_padding: Padding,
    gutter_spacing: f32,
    check_side: CheckSide,
    radius: f32,
    min_width: f32,
    max_width: Option<f32>,
    on_toggle: Option<Box<dyn Fn(bool) -> Message + 'a>>,
    class: Theme::Class<'a>,
}

/// The gutter a [`toggle`] draws its checkmark in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckSide {
    /// Before the row contents, sharing the gutter with [`Item::icon`].
    #[default]
    Leading,
    /// After the row contents.
    Trailing,
}

impl<'a, Message, Theme, Renderer> Menu<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font>,
{
    /// Creates a new [`Menu`] with the given trigger and rows.
    pub fn new(
        trigger: impl Into<Element<'a, Message, Theme, Renderer>>,
        items: Vec<Item<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            id: None,
            trigger: trigger.into(),
            trigger_padding: DEFAULT_TRIGGER_PADDING,
            plain_trigger: false,
            items,
            placement: Placement::new(Side::Bottom).gap(DEFAULT_GAP),
            submenu_gap: DEFAULT_SUBMENU_GAP,
            panel_padding: DEFAULT_PANEL_PADDING,
            item_padding: Padding::from([4.0, 8.0]),
            gutter_spacing: DEFAULT_GUTTER_SPACING,
            check_side: CheckSide::default(),
            radius: DEFAULT_RADIUS,
            min_width: 0.0,
            max_width: None,
            on_toggle: None,
            class: <Theme as Catalog>::default(),
        }
    }

    /// Sets the [`widget::Id`] of the [`Menu`].
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the padding around the trigger contents.
    ///
    /// This has no effect on a [`Menu::plain_trigger`].
    pub fn trigger_padding(mut self, padding: impl Into<Padding>) -> Self {
        self.trigger_padding = padding.into();
        self
    }

    /// Draws the trigger element with no surface of its own.
    ///
    /// By default a menu draws its own hover and open states behind the
    /// trigger, which is what lets a menu-bar entry stay lit while its panel is
    /// showing. Use this when the trigger is already a `button`, or anything
    /// else that paints its own background.
    ///
    /// This changes only the appearance. A menu claims a press on its trigger
    /// either way, so a `button` used as a trigger will never publish its own
    /// `on_press` — put the message on an [`Item`] instead.
    pub fn plain_trigger(mut self) -> Self {
        self.plain_trigger = true;
        self
    }

    /// Sets which gutter a [`toggle`] draws its checkmark in.
    pub fn check_side(mut self, side: CheckSide) -> Self {
        self.check_side = side;
        self
    }

    /// Sets the space between a gutter and the row contents.
    pub fn gutter_spacing(mut self, spacing: f32) -> Self {
        self.gutter_spacing = spacing;
        self
    }

    /// Sets the [`Side`] of the trigger the root panel opens on.
    ///
    /// A panel that does not fit on this side flips to the opposite one.
    pub fn side(mut self, side: Side) -> Self {
        self.placement.side = side;
        self
    }

    /// Sets how the root panel aligns to its trigger along the cross axis.
    pub fn align(mut self, align: Align) -> Self {
        self.placement.align = align;
        self
    }

    /// Sets the gap between the trigger and the root panel.
    pub fn gap(mut self, gap: f32) -> Self {
        self.placement.gap = gap;
        self
    }

    /// Sets the gap between a submenu panel and its parent panel.
    ///
    /// Negative values overlap the parent, which is the default.
    pub fn submenu_gap(mut self, gap: f32) -> Self {
        self.submenu_gap = gap;
        self
    }

    /// Sets the padding between the edge of a panel and its rows.
    pub fn padding(mut self, padding: f32) -> Self {
        self.panel_padding = padding;
        self
    }

    /// Sets the padding around the contents of each row.
    pub fn item_padding(mut self, padding: impl Into<Padding>) -> Self {
        self.item_padding = padding.into();
        self
    }

    /// Sets the corner radius of a panel.
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Sets a lower bound on the width of a panel.
    ///
    /// Panels are sized by their contents; this only widens a panel whose rows
    /// are all narrower than the given width.
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// Sets an upper bound on the width of a panel.
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Sets a callback for when the menu opens or closes.
    pub fn on_toggle(mut self, callback: impl Fn(bool) -> Message + 'a) -> Self {
        self.on_toggle = Some(Box::new(callback));
        self
    }

    /// Sets the style of the [`Menu`].
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`Menu`].
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    /// Returns the padding to lay the trigger out with.
    ///
    /// A plain trigger draws its own surface, so the menu adds nothing around
    /// it — the element's own padding is the whole of it.
    fn resolved_trigger_padding(&self) -> Padding {
        if self.plain_trigger {
            Padding::ZERO
        } else {
            self.trigger_padding
        }
    }

    /// Builds the state [`Tree`] children of this [`Menu`]: the trigger,
    /// followed by one tree per root row.
    ///
    /// [`Widget`] in this version of iced has no `children` method, so the
    /// whole tree is built here and installed by [`Widget::diff`].
    fn child_trees(&self) -> Vec<Tree> {
        std::iter::once(Tree::new(self.trigger.as_widget()))
            .chain(self.items.iter().map(Item::tree))
            .collect()
    }
}

/// The persistent state of a [`Menu`].
#[derive(Debug, Default)]
struct State {
    /// Whether the root panel is open.
    is_open: bool,
    /// The row index opened at each level, from the root panel downwards.
    ///
    /// An empty path means only the root panel is showing.
    path: Vec<usize>,
    /// The hovered row, as a panel depth and a row index.
    hovered: Option<(usize, usize)>,
    /// Whether the cursor is over the trigger.
    is_trigger_hovered: bool,
}

/// The interaction state of a menu-drawn trigger surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerStatus {
    /// The cursor is elsewhere and the menu is closed.
    Idle,
    /// The cursor is over the trigger.
    Hovered,
    /// The menu is open.
    ///
    /// This outranks [`TriggerStatus::Hovered`], so a menu-bar entry stays lit
    /// while its panel is showing and the cursor has moved down into it.
    Open,
}

impl State {
    /// Closes every panel.
    fn close(&mut self) {
        self.is_open = false;
        self.path.clear();
        self.hovered = None;
    }
}

/// Walks down to the rows of the panel at the given `depth`.
///
/// Returns `None` when the path cannot reach that depth. That happens both when
/// a view rebuild changes the items underneath an open panel, and — routinely —
/// when the cursor collapses the chain: the path shrinks immediately, while the
/// layout still describes the panels that were on screen a moment ago.
fn panel_items<'i, 'a, Message, Theme, Renderer>(
    items: &'i [Item<'a, Message, Theme, Renderer>],
    path: &[usize],
    depth: usize,
) -> Option<&'i [Item<'a, Message, Theme, Renderer>]> {
    let mut items = items;

    for index in path.get(..depth)? {
        match items.get(*index)? {
            Item::Submenu { items: nested, .. } => items = nested,
            _ => return None,
        }
    }

    Some(items)
}

/// The mutable counterpart of [`panel_items`].
fn panel_items_mut<'i, 'a, Message, Theme, Renderer>(
    items: &'i mut [Item<'a, Message, Theme, Renderer>],
    path: &[usize],
    depth: usize,
) -> Option<&'i mut [Item<'a, Message, Theme, Renderer>]> {
    let mut items = items;

    for index in path.get(..depth)? {
        match items.get_mut(*index)? {
            Item::Submenu { items: nested, .. } => items = nested,
            _ => return None,
        }
    }

    Some(items)
}

/// Walks the state tree down to the item trees of the panel at `depth`.
///
/// `trees` is the slice of item trees belonging to the root panel.
fn panel_trees<'t>(trees: &'t mut [Tree], path: &[usize], depth: usize) -> Option<&'t mut [Tree]> {
    let mut trees = trees;

    for index in path.get(..depth)? {
        trees = trees.get_mut(*index)?.children.get_mut(ITEM_ROWS..)?;
    }

    Some(trees)
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Menu<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font> + 'a,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        if tree.children.len() != self.items.len() + 1 {
            tree.children = self.child_trees();

            // The path may now point at rows that no longer exist.
            tree.state.downcast_mut::<State>().close();
        }

        // The skeleton above carries tags and state but no grandchildren, so
        // the recursive diff below has to run on a fresh tree too.
        let (trigger_tree, item_trees) = tree.children.split_at_mut(1);
        trigger_tree[0].diff(self.trigger.as_widget_mut());

        for (item, tree) in self.items.iter_mut().zip(item_trees) {
            item.diff(tree);
        }
    }

    fn size(&self) -> Size<Length> {
        self.trigger.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        // Only the trigger occupies space in the view layer; the panels live
        // entirely in the overlay.
        let size = self.trigger.as_widget().size();
        let padding = self.resolved_trigger_padding();
        let trigger = &mut self.trigger;
        let trigger_tree = &mut tree.children[0];

        layout::padded(limits, size.width, size.height, padding, |limits| {
            trigger.as_widget_mut().layout(trigger_tree, renderer, limits)
        })
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let appearance = theme.style(&self.class);

        let mut style = *style;

        if !self.plain_trigger {
            let status = if state.is_open {
                TriggerStatus::Open
            } else if state.is_trigger_hovered {
                TriggerStatus::Hovered
            } else {
                TriggerStatus::Idle
            };

            if let Some(background) = appearance.trigger_background(status) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: layout.bounds(),
                        border: Border {
                            radius: appearance.trigger_radius.into(),
                            ..Border::default()
                        },
                        ..renderer::Quad::default()
                    },
                    background,
                );
            }

            style.text_color = appearance.trigger_text_color(status);
        }

        self.trigger.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            &style,
            layout.children().next().expect("trigger layout"),
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let trigger_layout = layout.children().next().expect("trigger layout");

        let pressed = matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. })
        );

        // A press on the trigger is claimed before the trigger element sees it.
        // The trigger exists to open the menu, and anything that handles a
        // press itself — a `button` with its own `on_press`, most obviously —
        // would otherwise capture the event and the menu would never open.
        //
        // The whole padded surface is the hit target, not just the contents.
        if pressed && cursor.is_over(layout.bounds()) {
            let state = tree.state.downcast_mut::<State>();
            let was_open = state.is_open;

            if was_open {
                state.close();
            } else {
                state.is_open = true;
            }

            if let Some(on_toggle) = &self.on_toggle {
                shell.publish(on_toggle(!was_open));
            }

            shell.capture_event();
            shell.request_redraw();

            return;
        }

        self.trigger.as_widget_mut().update(
            &mut tree.children[0],
            event,
            trigger_layout,
            cursor,
            renderer,
            shell,
            viewport,
        );

        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();

        if matches!(
            event,
            Event::Mouse(mouse::Event::CursorMoved { .. }) | Event::Window(window::Event::Unfocused)
        ) {
            let is_hovered = cursor.is_over(layout.bounds());

            if is_hovered != state.is_trigger_hovered {
                state.is_trigger_hovered = is_hovered;
                shell.request_redraw();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.trigger.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().expect("trigger layout"),
            cursor,
            viewport,
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(self.id.as_ref(), layout.bounds());

        operation.traverse(&mut |operation| {
            self.trigger.as_widget_mut().operate(
                &mut tree.children[0],
                layout.children().next().expect("trigger layout"),
                renderer,
                operation,
            );
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        offset: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<State>();

        if !state.is_open {
            return None;
        }

        let mut trigger_bounds = layout.bounds();
        trigger_bounds.x += offset.x;
        trigger_bounds.y += offset.y;

        let (_, item_trees) = tree.children.split_at_mut(1);

        Some(overlay::Element::new(Box::new(Panels {
            state,
            items: &mut self.items,
            trees: item_trees,
            trigger_bounds,
            placement: self.placement,
            submenu_gap: self.submenu_gap,
            panel_padding: self.panel_padding,
            item_padding: self.item_padding,
            gutter_spacing: self.gutter_spacing,
            check_side: self.check_side,
            radius: self.radius,
            min_width: self.min_width,
            max_width: self.max_width,
            on_toggle: self.on_toggle.as_deref(),
            class: &self.class,
        })))
    }
}

impl<'a, Message, Theme, Renderer> From<Menu<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font> + 'a,
{
    fn from(menu: Menu<'a, Message, Theme, Renderer>) -> Self {
        Element::new(menu)
    }
}

/// The stack of open panels, drawn as a single overlay.
///
/// Holding every level in one overlay — rather than nesting an overlay per
/// submenu — keeps the placement of a panel a plain function of its parent's
/// layout, which is what makes deep submenu chains position correctly.
struct Panels<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
{
    state: &'a mut State,
    items: &'a mut Vec<Item<'b, Message, Theme, Renderer>>,
    trees: &'a mut [Tree],
    trigger_bounds: Rectangle,
    placement: Placement,
    submenu_gap: f32,
    panel_padding: f32,
    item_padding: Padding,
    gutter_spacing: f32,
    check_side: CheckSide,
    radius: f32,
    min_width: f32,
    max_width: Option<f32>,
    on_toggle: Option<&'a dyn Fn(bool) -> Message>,
    class: &'a Theme::Class<'b>,
}

impl<'a, 'b, Message, Theme, Renderer> Panels<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font>,
{
    /// The number of panels currently on screen.
    fn depth(&self) -> usize {
        self.state.path.len() + 1
    }

    /// Lays out one panel, sizing every row to the widest of them.
    ///
    /// Each row is `[leading gutter] [content] [trailing gutter]`. The gutters
    /// are sized once for the whole panel, so a row with no icon still indents
    /// its content past the widest one and every label starts at the same x.
    ///
    /// The returned [`Node`] is positioned at the origin; the caller moves it
    /// into place once it knows how large it is.
    fn layout_panel(&mut self, renderer: &Renderer, viewport: Size, depth: usize) -> Option<Node> {
        let item_padding = self.item_padding;
        let panel_padding = self.panel_padding;
        let gutter_spacing = self.gutter_spacing;
        let check_side = self.check_side;
        let glyph = glyph_size(renderer);
        let min_width = self.min_width;
        let max_width = self.max_width;

        let available = (viewport.width - 2.0 * panel_padding).max(0.0);

        let items = panel_items_mut(self.items, &self.state.path, depth)?;
        let trees = panel_trees(self.trees, &self.state.path, depth)?;

        // Pass 1 — measure. Constraining the width to `Shrink` sets the
        // compression flag on the limits, which makes fluid children resolve to
        // their intrinsic size instead of swallowing the available space. A row
        // built as `row![label, space().width(Fill), shortcut]` therefore
        // measures at the width of its text, not at `available`.
        let measure =
            Limits::new(Size::ZERO, Size::new(available, f32::INFINITY)).width(Length::Shrink);

        let mut leading_gutter: f32 = 0.0;
        let mut trailing_gutter: f32 = 0.0;
        let mut widest_content: f32 = 0.0;

        for (item, tree) in items.iter_mut().zip(trees.iter_mut()) {
            if item.check().is_some() {
                match check_side {
                    CheckSide::Leading => leading_gutter = leading_gutter.max(glyph),
                    CheckSide::Trailing => trailing_gutter = trailing_gutter.max(glyph),
                }
            }

            // A submenu chevron always sits in the trailing gutter, sharing it
            // with trailing checkmarks. A row is never both, so they never
            // collide — they only agree on how wide the gutter has to be.
            if item.opens_submenu() {
                trailing_gutter = trailing_gutter.max(glyph);
            }

            if let Some(icon) = item.icon_mut() {
                let width = icon
                    .as_widget_mut()
                    .layout(&mut tree.children[ITEM_ICON], renderer, &measure)
                    .size()
                    .width;

                leading_gutter = leading_gutter.max(width);
            }

            if let Some(content) = item.content_mut() {
                let width = content
                    .as_widget_mut()
                    .layout(&mut tree.children[ITEM_CONTENT], renderer, &measure)
                    .size()
                    .width;

                widest_content = widest_content.max(width);
            }
        }

        // A gutter only claims space, and only pushes the contents along, when
        // some row in the panel actually has something to put in it.
        let leading = gutter_offset(leading_gutter, gutter_spacing);
        let trailing = gutter_offset(trailing_gutter, gutter_spacing);

        let chrome = item_padding.x() + leading + trailing;
        let mut widest = (widest_content + chrome).max(min_width);

        if let Some(max) = max_width {
            widest = widest.min(max);
        }

        widest = widest.min(available);

        // Pass 2 — pin. `min == max` on the width makes every row resolve to
        // exactly `content_width`: intrinsic rows are clamped up to it, and
        // fluid rows are filled down to it.
        let content_width = (widest - chrome).max(0.0);
        let pinned = Limits::new(
            Size::new(content_width, 0.0),
            Size::new(content_width, f32::INFINITY),
        );

        let mut rows = Vec::with_capacity(items.len());
        let mut y = panel_padding;

        for (item, tree) in items.iter_mut().zip(trees.iter_mut()) {
            if item.content().is_none() {
                rows.push(
                    Node::new(Size::new(widest, SEPARATOR_HEIGHT))
                        .move_to(Point::new(panel_padding, y)),
                );

                y += SEPARATOR_HEIGHT;

                continue;
            }

            let icon_node = match item.icon_mut() {
                Some(icon) => {
                    icon.as_widget_mut()
                        .layout(&mut tree.children[ITEM_ICON], renderer, &measure)
                }
                None => Node::new(Size::ZERO),
            };

            let content_node = item
                .content_mut()
                .expect("checked above")
                .as_widget_mut()
                .layout(&mut tree.children[ITEM_CONTENT], renderer, &pinned);

            let row_height = content_node
                .size()
                .height
                .max(icon_node.size().height)
                .max(glyph)
                + item_padding.y();

            // Both slots are centered against the row so a tall content element
            // does not drag its icon to the top.
            let icon_node = icon_node.clone().move_to(Point::new(
                item_padding.left,
                (row_height - icon_node.size().height) / 2.0,
            ));

            let content_node = content_node.clone().move_to(Point::new(
                item_padding.left + leading,
                (row_height - content_node.size().height) / 2.0,
            ));

            let row = Node::with_children(
                Size::new(widest, row_height),
                vec![icon_node, content_node],
            )
            .move_to(Point::new(panel_padding, y));

            rows.push(row);

            y += row_height;
        }

        Some(Node::with_children(
            Size::new(widest + 2.0 * panel_padding, y + panel_padding),
            rows,
        ))
    }

    /// Returns the bounds of the row a panel at `depth` is anchored to.
    ///
    /// The root panel anchors to the trigger. A submenu anchors to the *panel*
    /// its parent row lives in, not to the row itself: taking the width from
    /// the panel is what keeps a submenu flush against the panel edge instead
    /// of against a row that may be narrower.
    fn anchor_for(&self, depth: usize, panels: &[Node]) -> Rectangle {
        if depth == 0 {
            return self.trigger_bounds;
        }

        let parent = &panels[depth - 1];
        let parent_bounds = Rectangle::new(parent.bounds().position(), parent.size());

        let row = parent
            .children()
            .get(self.state.path[depth - 1])
            .map(Node::bounds);

        match row {
            Some(row) => Rectangle {
                x: parent_bounds.x,
                y: parent_bounds.y + row.y,
                width: parent_bounds.width,
                height: row.height,
            },
            None => parent_bounds,
        }
    }

    /// Returns the panel depth and row index under the cursor, if any.
    fn row_at(&self, layout: Layout<'_>, cursor: mouse::Cursor) -> Option<(usize, usize)> {
        let position = cursor.position()?;

        // The layout may still hold panels the path has since dropped.
        for (depth, panel) in layout.children().take(self.depth()).enumerate() {
            if !panel.bounds().contains(position) {
                continue;
            }

            let items = panel_items(self.items, &self.state.path, depth)?;

            for (index, row) in panel.children().enumerate() {
                if row.bounds().contains(position)
                    && items.get(index).is_some_and(Item::is_interactive)
                {
                    return Some((depth, index));
                }
            }

            // The cursor is on the panel but between rows, on its padding.
            return None;
        }

        None
    }

    /// Returns `true` when the cursor is over any open panel.
    fn is_over_any_panel(&self, layout: Layout<'_>, cursor: mouse::Cursor) -> bool {
        layout
            .children()
            .take(self.depth())
            .any(|panel| cursor.is_over(panel.bounds()))
    }
}

impl<'a, 'b, Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for Panels<'a, 'b, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font>,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
        let viewport = Rectangle::with_size(bounds);
        let mut panels: Vec<Node> = Vec::with_capacity(self.depth());

        for depth in 0..self.depth() {
            let Some(panel) = self.layout_panel(renderer, bounds, depth) else {
                // The path went stale mid-layout; drop the rest of the chain.
                self.state.path.truncate(depth.saturating_sub(1));
                break;
            };

            let anchor = self.anchor_for(depth, &panels);

            let placement = if depth == 0 {
                self.placement
            } else {
                // Submenus prefer to continue rightwards, and flip as a group
                // when they run out of room.
                Placement::new(Side::Right).gap(self.submenu_gap)
            };

            let placed = anchor::place(anchor, panel.size(), viewport, placement);

            panels.push(panel.move_to(placed.position));
        }

        Node::with_children(bounds, panels)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        // A panel resolves the text color of every row from its own `Style`,
        // so the inherited one is deliberately dropped.
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let appearance = theme.style(self.class);

        for (depth, panel) in layout.children().take(self.depth()).enumerate() {
            let panel_bounds = panel.bounds();

            let Some(items) = panel_items(self.items, &self.state.path, depth) else {
                continue;
            };

            let Some(trees) = panel_trees_ref(self.trees, &self.state.path, depth) else {
                continue;
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: panel_bounds,
                    border: Border {
                        radius: self.radius.into(),
                        ..appearance.border
                    },
                    shadow: appearance.shadow,
                    ..renderer::Quad::default()
                },
                appearance.background,
            );

            // A submenu stays highlighted while its own panel is open, so the
            // chain of open rows reads as a single connected path.
            let open_row = self.state.path.get(depth).copied();

            for ((index, row), (item, tree)) in panel
                .children()
                .enumerate()
                .zip(items.iter().zip(trees.iter()))
            {
                let row_bounds = row.bounds();

                let Some(content) = item.content() else {
                    let rule = Rectangle {
                        x: row_bounds.x,
                        y: row_bounds.y + (row_bounds.height - SEPARATOR_THICKNESS) / 2.0,
                        width: row_bounds.width,
                        height: SEPARATOR_THICKNESS,
                    };

                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: rule,
                            ..renderer::Quad::default()
                        },
                        appearance.separator_color,
                    );

                    continue;
                };

                let is_highlighted = self.state.hovered == Some((depth, index))
                    || open_row == Some(index);

                if is_highlighted && item.is_interactive() {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: row_bounds,
                            border: Border {
                                radius: appearance.item_radius.into(),
                                ..Border::default()
                            },
                            ..renderer::Quad::default()
                        },
                        appearance.hovered_item_background,
                    );
                }

                let text_color = if !item.is_interactive() {
                    appearance.disabled_text_color
                } else if is_highlighted {
                    appearance.hovered_text_color
                } else {
                    appearance.text_color
                };

                let mut slots = row.children();
                let icon_layout = slots.next().expect("row icon layout");
                let content_layout = slots.next().expect("row content layout");

                if let Some(icon) = item.icon_element() {
                    icon.as_widget().draw(
                        &tree.children[ITEM_ICON],
                        renderer,
                        theme,
                        &renderer::Style { text_color },
                        icon_layout,
                        cursor,
                        &row_bounds,
                    );
                }

                // The gutters are recovered from where pass 2 placed the
                // content, which keeps the panel geometry in one place rather
                // than duplicated into the widget state.
                let content_bounds = content_layout.bounds();
                let glyph = glyph_size(renderer);

                let leading_slot = || Rectangle {
                    x: row_bounds.x + self.item_padding.left,
                    width: content_bounds.x
                        - self.gutter_spacing
                        - (row_bounds.x + self.item_padding.left),
                    ..row_bounds
                };

                let trailing_slot = || Rectangle {
                    x: content_bounds.x + content_bounds.width + self.gutter_spacing,
                    width: (row_bounds.x + row_bounds.width - self.item_padding.right)
                        - (content_bounds.x + content_bounds.width + self.gutter_spacing),
                    ..row_bounds
                };

                if item.check() == Some(true) {
                    let slot = match self.check_side {
                        CheckSide::Leading => leading_slot(),
                        CheckSide::Trailing => trailing_slot(),
                    };

                    draw_glyph(
                        renderer,
                        lucide::Icon::Check,
                        slot,
                        glyph,
                        text_color,
                        row_bounds,
                    );
                }

                if item.opens_submenu() {
                    draw_glyph(
                        renderer,
                        lucide::Icon::ChevronRight,
                        trailing_slot(),
                        glyph,
                        text_color,
                        row_bounds,
                    );
                }

                content.as_widget().draw(
                    &tree.children[ITEM_CONTENT],
                    renderer,
                    theme,
                    &renderer::Style { text_color },
                    content_layout,
                    cursor,
                    &row_bounds,
                );
            }
        }
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Touch(touch::Event::FingerMoved { .. }) => {
                let hovered = self.row_at(layout, cursor);

                if hovered == self.state.hovered {
                    return;
                }

                self.state.hovered = hovered;

                if let Some((depth, index)) = hovered {
                    let Some(items) = panel_items(self.items, &self.state.path, depth) else {
                        return;
                    };

                    let opens_submenu = matches!(items.get(index), Some(Item::Submenu { .. }));

                    // Hovering a row closes every panel deeper than it, then
                    // opens its own submenu if it has one. Moving along a chain
                    // of submenus therefore never leaves an orphaned panel
                    // behind.
                    self.state.path.truncate(depth);

                    if opens_submenu {
                        self.state.path.push(index);
                    }
                }

                // The set of panels just changed, so the layout computed for
                // the previous chain no longer describes what is on screen.
                shell.invalidate_layout();
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if !self.is_over_any_panel(layout, cursor) {
                    self.state.close();

                    if let Some(on_toggle) = self.on_toggle {
                        shell.publish(on_toggle(false));
                    }

                    shell.request_redraw();

                    return;
                }

                let Some((depth, index)) = self.row_at(layout, cursor) else {
                    // A press on panel padding is swallowed rather than
                    // treated as a press outside.
                    shell.capture_event();
                    return;
                };

                let message = panel_items(self.items, &self.state.path, depth)
                    .and_then(|items| items.get(index))
                    .and_then(|item| match item {
                        Item::Entry { on_press, .. } => on_press.clone(),
                        _ => None,
                    });

                if let Some(message) = message {
                    shell.publish(message);

                    self.state.close();

                    if let Some(on_toggle) = self.on_toggle {
                        shell.publish(on_toggle(false));
                    }
                }

                shell.capture_event();
                shell.request_redraw();
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            }) => {
                // Escape peels one level off the chain, and closes the menu
                // once only the root panel is left.
                if self.state.path.pop().is_none() {
                    self.state.close();

                    if let Some(on_toggle) = self.on_toggle {
                        shell.publish(on_toggle(false));
                    }
                }

                self.state.hovered = None;

                shell.capture_event();
                shell.request_redraw();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.row_at(layout, cursor).is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn operate(
        &mut self,
        _layout: Layout<'_>,
        _renderer: &Renderer,
        _operation: &mut dyn Operation,
    ) {
    }

    fn index(&self) -> f32 {
        1.0
    }
}

/// Returns the side of a gutter glyph, in logical pixels.
///
/// Tying the size to the renderer's default text size keeps checkmarks and
/// chevrons in proportion with row labels at any text scale.
fn glyph_size<Renderer>(renderer: &Renderer) -> f32
where
    Renderer: text::Renderer,
{
    renderer.default_size().0 * GLYPH_SCALE
}

/// Draws a Lucide glyph centered in `slot`.
///
/// Nothing is drawn if the application never registered
/// [`crate::lucide::FONT_BYTES`] — iced falls back to a blank rather than
/// failing, so a missing font shows up as a missing checkmark.
fn draw_glyph<Renderer>(
    renderer: &mut Renderer,
    icon: lucide::Icon,
    slot: Rectangle,
    size: f32,
    color: Color,
    clip: Rectangle,
) where
    Renderer: text::Renderer<Font = iced::Font>,
{
    renderer.fill_text(
        text::Text {
            content: icon.character().to_string(),
            font: lucide::FONT,
            size: Pixels(size),
            line_height: text::LineHeight::default(),
            bounds: slot.size(),
            align_x: text::Alignment::Center,
            align_y: Vertical::Center,
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::default(),
            hint_factor: None,
        },
        slot.center(),
        color,
        clip,
    );
}

/// Returns how far a gutter of the given width pushes the row contents along.
///
/// An empty gutter contributes nothing at all — no width and no spacing — so a
/// panel with no icons and no checkable rows looks exactly as it did before
/// either feature existed.
fn gutter_offset(width: f32, spacing: f32) -> f32 {
    if width > 0.0 { width + spacing } else { 0.0 }
}

/// The immutable counterpart of [`panel_trees`].
fn panel_trees_ref<'t>(trees: &'t [Tree], path: &[usize], depth: usize) -> Option<&'t [Tree]> {
    let mut trees = trees;

    for index in path.get(..depth)? {
        trees = trees.get(*index)?.children.get(ITEM_ROWS..)?;
    }

    Some(trees)
}

/// The appearance of a [`Menu`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The [`Background`] of a panel.
    pub background: Background,
    /// The [`Border`] of a panel. Its radius is overridden by
    /// [`Menu::radius`].
    pub border: Border,
    /// The [`Shadow`] of a panel.
    pub shadow: Shadow,
    /// The text [`Color`] of an idle row.
    pub text_color: Color,
    /// The text [`Color`] of a hovered row, or one whose submenu is open.
    pub hovered_text_color: Color,
    /// The text [`Color`] of a disabled row.
    pub disabled_text_color: Color,
    /// The [`Background`] drawn behind a hovered row.
    pub hovered_item_background: Background,
    /// The corner radius of the highlight behind a hovered row.
    pub item_radius: f32,
    /// The [`Color`] of a [`separator`] rule.
    pub separator_color: Color,
    /// The [`Background`] behind an idle trigger, if any.
    pub trigger_background: Option<Background>,
    /// The [`Background`] behind a hovered trigger, if any.
    pub trigger_hovered_background: Option<Background>,
    /// The [`Background`] behind a trigger whose menu is open, if any.
    pub trigger_open_background: Option<Background>,
    /// The text [`Color`] of an idle trigger.
    pub trigger_text_color: Color,
    /// The text [`Color`] of a hovered or open trigger.
    pub trigger_active_text_color: Color,
    /// The corner radius of the trigger surface.
    pub trigger_radius: f32,
}

impl Style {
    /// Returns the [`Background`] to draw behind a trigger in the given state.
    fn trigger_background(&self, status: TriggerStatus) -> Option<Background> {
        match status {
            TriggerStatus::Idle => self.trigger_background,
            TriggerStatus::Hovered => self.trigger_hovered_background,
            TriggerStatus::Open => self.trigger_open_background,
        }
    }

    /// Returns the text [`Color`] of a trigger in the given state.
    fn trigger_text_color(&self, status: TriggerStatus) -> Color {
        match status {
            TriggerStatus::Idle => self.trigger_text_color,
            TriggerStatus::Hovered | TriggerStatus::Open => self.trigger_active_text_color,
        }
    }
}

/// A boxed [`Menu`] style function.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

/// The theme catalog of a [`Menu`].
pub trait Catalog {
    /// The style class of this [`Catalog`].
    type Class<'a>;

    /// The default class produced by this [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class into a [`Style`].
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// The default style of a [`Menu`], drawn from the palette of the given theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Background::Color(palette.background.weak.color),
        border: Border {
            width: 1.0,
            color: palette.background.strong.color,
            radius: DEFAULT_RADIUS.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        text_color: palette.background.base.text,
        hovered_text_color: palette.primary.base.text,
        disabled_text_color: Color {
            a: 0.4,
            ..palette.background.base.text
        },
        hovered_item_background: Background::Color(palette.primary.base.color),
        item_radius: 4.0,
        separator_color: palette.background.strong.color,
        trigger_background: None,
        trigger_hovered_background: Background::Color(palette.background.weak.color).into(),
        trigger_open_background: Background::Color(palette.background.strong.color).into(),
        trigger_text_color: palette.background.base.text,
        trigger_active_text_color: palette.background.base.text,
        trigger_radius: 4.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    enum Message {
        Cut,
    }

    fn menu_of<'a>(
        items: Vec<Item<'a, Message>>,
    ) -> Menu<'a, Message> {
        Menu::new(iced::widget::text("Edit"), items)
    }

    #[test]
    fn the_state_tree_mirrors_the_shape_of_the_menu() {
        let menu = menu_of(vec![
            item(iced::widget::text("Cut")).on_press(Message::Cut),
            separator(),
            submenu(
                iced::widget::text("More"),
                vec![
                    item(iced::widget::text("A")),
                    submenu(iced::widget::text("Deeper"), vec![item(iced::widget::text("B"))]),
                ],
            ),
        ]);

        let children = menu.child_trees();

        // trigger + three rows
        assert_eq!(children.len(), 4);
        // an entry owns its content and an icon slot
        assert_eq!(children[1].children.len(), ITEM_ROWS);
        // a separator owns nothing
        assert_eq!(children[2].children.len(), 0);
        // a submenu owns both slots plus a tree per nested row
        assert_eq!(children[3].children.len(), ITEM_ROWS + 2);
        // and nesting continues to the bottom
        assert_eq!(
            children[3].children[ITEM_ROWS + 1].children.len(),
            ITEM_ROWS + 1
        );
    }

    /// The icon slot is present whether or not the row has an icon, which is
    /// what lets `ITEM_ROWS` be a constant offset into a submenu's children.
    #[test]
    fn the_icon_slot_is_reserved_even_on_rows_without_one() {
        let plain = menu_of(vec![item(iced::widget::text("Cut"))]);
        let decorated =
            menu_of(vec![item(iced::widget::text("Cut")).icon(iced::widget::text("*"))]);

        assert_eq!(plain.child_trees()[1].children.len(), ITEM_ROWS);
        assert_eq!(decorated.child_trees()[1].children.len(), ITEM_ROWS);
    }

    #[test]
    fn a_toggle_is_a_checkable_entry_and_a_submenu_never_is() {
        let checked: Item<'_, Message> = toggle(iced::widget::text("Wrap"), true);
        let unchecked: Item<'_, Message> = toggle(iced::widget::text("Wrap"), false);
        let plain: Item<'_, Message> = item(iced::widget::text("Cut"));
        let nested: Item<'_, Message> =
            submenu(iced::widget::text("More"), vec![item(iced::widget::text("A"))])
                .checked(true);

        assert_eq!(checked.check(), Some(true));
        // An unchecked toggle still reserves its gutter slot, so it reports
        // `Some(false)` rather than `None`.
        assert_eq!(unchecked.check(), Some(false));
        assert_eq!(plain.check(), None);
        assert_eq!(nested.check(), None);
    }

    /// A gutter nobody uses must not indent the rows, or every existing menu
    /// would gain a phantom margin.
    #[test]
    fn an_empty_gutter_costs_nothing() {
        assert_eq!(gutter_offset(0.0, 8.0), 0.0);
        assert_eq!(gutter_offset(16.0, 8.0), 24.0);
    }

    /// The chevron and a trailing checkmark share one gutter, so a row must
    /// never claim both — otherwise they would draw on top of each other.
    #[test]
    fn a_row_never_carries_both_a_chevron_and_a_checkmark() {
        let nested: Item<'_, Message> =
            submenu(iced::widget::text("More"), vec![item(iced::widget::text("A"))]);
        let checked: Item<'_, Message> = toggle(iced::widget::text("Wrap"), true);

        assert!(nested.opens_submenu());
        assert_eq!(nested.check(), None);

        assert!(!checked.opens_submenu());
        assert_eq!(checked.check(), Some(true));
    }

    #[test]
    fn panel_items_walks_the_open_path() {
        let items: Vec<Item<'_, Message>> = vec![
            item(iced::widget::text("Cut")),
            submenu(
                iced::widget::text("More"),
                vec![
                    item(iced::widget::text("A")),
                    submenu(iced::widget::text("Deeper"), vec![item(iced::widget::text("B"))]),
                ],
            ),
        ];

        let path = vec![1, 1];

        assert_eq!(panel_items(&items, &path, 0).unwrap().len(), 2);
        assert_eq!(panel_items(&items, &path, 1).unwrap().len(), 2);
        assert_eq!(panel_items(&items, &path, 2).unwrap().len(), 1);
    }

    /// Collapsing a submenu chain shortens the path immediately, while the
    /// layout still describes the panels drawn a frame ago. Every walk down a
    /// depth the path can no longer reach has to fail softly.
    #[test]
    fn walking_deeper_than_the_path_yields_nothing_rather_than_panicking() {
        let mut items: Vec<Item<'_, Message>> = vec![item(iced::widget::text("Cut"))];
        let mut trees = vec![Tree::empty()];

        assert!(panel_items(&items, &[], 1).is_none());
        assert!(panel_items_mut(&mut items, &[], 1).is_none());
        assert!(panel_trees(&mut trees, &[], 2).is_none());
        assert!(panel_trees_ref(&trees, &[], 2).is_none());
    }

    #[test]
    fn panel_items_rejects_a_path_that_no_longer_matches() {
        let items: Vec<Item<'_, Message>> = vec![item(iced::widget::text("Cut"))];

        // Index 0 is an entry, not a submenu, so there is no panel below it.
        assert!(panel_items(&items, &[0], 1).is_none());
        // And an index past the end resolves to nothing at all.
        assert!(panel_items(&items, &[7], 1).is_none());
    }

    #[test]
    fn item_trees_and_panel_trees_stay_in_step() {
        let menu = menu_of(vec![
            item(iced::widget::text("Cut")),
            submenu(
                iced::widget::text("More"),
                vec![item(iced::widget::text("A")), item(iced::widget::text("B"))],
            ),
        ]);

        let mut children = menu.child_trees();
        let (_, item_trees) = children.split_at_mut(1);

        let path = vec![1];

        assert_eq!(panel_trees(item_trees, &path, 0).unwrap().len(), 2);
        assert_eq!(panel_trees(item_trees, &path, 1).unwrap().len(), 2);
    }
}
