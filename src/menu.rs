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
use iced::time::{Duration, Instant};
use iced::{
    Background, Border, Color, Element, Event, Length, Padding, Pixels, Point, Rectangle, Shadow,
    Size, Theme, Vector, mouse, touch, window,
};

use crate::anchor::{self, Align, Placement, Side};
use crate::animation::{self, Motion, Transition};
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

/// How far the safe triangle is widened at the submenu end.
///
/// The triangle's base is the submenu's facing edge. Widening it a little gives
/// the sweep a more forgiving mouth, so a path that arrives just past a corner
/// still counts as heading in.
const AIM_EXTEND: f32 = 8.0;

/// How long the safe triangle may hold a submenu open once the cursor has
/// stopped making progress towards it.
///
/// Without a deadline, parking the cursor inside the triangle would leave the
/// row underneath it permanently unreachable — the guard has to lose eventually
/// or it stops being a shortcut and becomes a trap.
const AIM_TIMEOUT: Duration = Duration::from_millis(300);

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
fn icon_tree<Message, Theme, Renderer>(icon: &Option<Element<'_, Message, Theme, Renderer>>) -> Tree
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
    motion: Motion,
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
        // So the checkmark and chevron render even if the application never
        // registered `lucide::FONT_BYTES` itself.
        lucide::ensure_loaded();

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
            motion: Motion::QUICK,
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

    /// Sets how the panels animate in and out.
    ///
    /// Defaults to [`Motion::QUICK`].
    pub fn motion(mut self, motion: Motion) -> Self {
        self.motion = motion;
        self
    }

    /// Opens and closes the panels instantly, with no transition.
    ///
    /// A menu set this way never schedules an animation frame.
    pub fn no_animation(mut self) -> Self {
        self.motion = Motion::NONE;
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
    /// The highlighted row, as a panel depth and a row index.
    ///
    /// The mouse and the keyboard share this: hovering a row highlights it,
    /// and the arrow keys move the same highlight.
    highlighted: Option<(usize, usize)>,
    /// Whether the cursor is over the trigger.
    is_trigger_hovered: bool,
    /// A pending request to hand the keyboard over to a neighbouring menu.
    ///
    /// The open panels are an overlay, and an overlay that captures an event
    /// stops the base widget tree from ever seeing it. So a menu that decides
    /// an arrow key means "leave me" records it here and deliberately does not
    /// capture, letting [`MenuBar`] pick it up in the same event pass.
    sibling_request: Option<Sibling>,
    /// The open/close transition of the root panel.
    transition: Transition,
    /// The appear transition of each submenu panel, indexed by depth.
    ///
    /// Index 0 is unused — the root panel uses [`State::transition`], which
    /// also animates *out*. Submenus only animate in: they are torn down by the
    /// path shrinking, and keeping a closed panel alive to fade it would mean
    /// the path no longer describing what is on screen.
    submenu_transitions: Vec<Transition>,
    /// The progress of each panel on screen, refreshed every layout pass.
    ///
    /// Cached because `draw` cannot advance a transition through `&self`, and
    /// layout always runs first in a frame.
    panel_progress: Vec<f32>,
    /// The [`Side`] each panel ended up on, refreshed every layout pass.
    ///
    /// Submenus ask for [`Side::Right`] and flip when they run out of room, so
    /// which edge a panel presents to its parent is only known after placement.
    /// The safe triangle needs it to know which two corners to sweep to.
    panel_sides: Vec<Side>,
    /// The previous cursor position, and the apex of the safe triangle.
    ///
    /// Taking the apex from the last position rather than a fixed point is what
    /// makes the triangle track the direction of travel: the sweep narrows as
    /// the cursor closes on the submenu, so it stops forgiving a path that has
    /// turned away.
    last_cursor: Option<Point>,
    /// When the safe triangle started holding the current submenu open.
    ///
    /// `None` means the guard is not engaged. See [`AIM_TIMEOUT`].
    aim_since: Option<Instant>,
    /// The instant of the frame the menu was last updated in.
    ///
    /// Whether the panels are showing, and how far through its transition the
    /// root one is, are both read from here rather than from the clock. A frame
    /// is not an instant: `UserInterface` lays the overlay out once during
    /// `update`, caches that layout, then rebuilds the overlay tree in `draw`
    /// and hands it the cached one. Deciding visibility from `Instant::now()`
    /// lets a transition finish *between* the two, at which point the panels
    /// leave the tree but not the layout — and `overlay::Group` pairs the two
    /// up by position, so every sibling overlay is then drawn against a layout
    /// node belonging to something else, which panics as soon as that node has
    /// the wrong shape.
    ///
    /// `None` until the first frame, when nothing has opened yet.
    frame: Option<Instant>,
}

/// A neighbouring menu in a [`MenuBar`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sibling {
    /// The menu to the left.
    Previous,
    /// The menu to the right.
    Next,
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
    ///
    /// The menu counts as closed straight away — callbacks fire, the trigger
    /// unlights, and further events are ignored — while the panels stay on
    /// screen until the transition finishes. Deferring the *logical* close to
    /// the end of the animation would make the widget lie about its state for
    /// a tenth of a second.
    fn close(&mut self) {
        let now = Instant::now();

        self.is_open = false;
        self.path.clear();
        self.highlighted = None;
        self.sibling_request = None;
        self.frame = Some(now);
        self.transition.close(now);
        self.submenu_transitions.clear();
        self.forget_aim();
    }

    /// Opens the root panel with nothing highlighted, as a press does.
    fn open(&mut self) {
        let now = Instant::now();

        self.is_open = true;
        self.path.clear();
        self.highlighted = None;
        self.frame = Some(now);
        self.transition.open(now);
        self.submenu_transitions.clear();
        self.forget_aim();
    }

    /// Drops the safe triangle.
    ///
    /// The apex goes with it: a triangle swept from a position the cursor
    /// occupied before the panels changed would be aimed at a submenu that is
    /// no longer there.
    fn forget_aim(&mut self) {
        self.last_cursor = None;
        self.aim_since = None;
    }

    /// Returns `true` when the panels should still be produced and drawn.
    ///
    /// Answered as of the frame the menu was last updated in, so that one frame
    /// gets one answer however many passes it takes. See [`State::frame`].
    fn is_showing(&self) -> bool {
        self.is_open
            || self
                .frame
                .is_some_and(|frame| self.transition.is_visible(frame))
    }

    /// How far open the root panel is, as of that same frame.
    fn progress(&self) -> f32 {
        self.frame
            .map_or(0.0, |frame| self.transition.progress(frame))
    }

    /// Returns `true` while any panel is still moving.
    fn is_animating(&self, now: Instant) -> bool {
        self.transition.is_animating(now)
            || self
                .submenu_transitions
                .iter()
                .any(|transition| transition.is_animating(now))
    }

    /// Collapses the chain to `depth`, fading the panels below it out.
    ///
    /// The path keeps describing the fading panels so they can still be laid
    /// out and drawn; [`State::live_depth`] is what marks them as no longer
    /// interactive, and [`State::retire_finished_panels`] drops them once the
    /// fade completes.
    fn collapse_to(&mut self, depth: usize, now: Instant) {
        for transition in self.submenu_transitions.iter_mut().skip(depth + 1) {
            transition.close(now);
        }
    }

    /// Opens `index` as a submenu of the panel at `depth`, replacing whatever
    /// was open below it.
    ///
    /// Unlike [`State::collapse_to`], this drops the old panels at once. A
    /// replacement arrives in the same place the outgoing panel occupied, so
    /// cross-fading the two would read as a smear rather than a transition.
    fn open_submenu(&mut self, depth: usize, index: usize, now: Instant) {
        // Asking for the submenu that is already open leaves its transition
        // exactly where it is. Rebuilding it would restart the fade from
        // nothing every time the cursor crossed back onto the row that opened
        // it — which reads as the panel flickering shut and open again.
        //
        // A submenu that had begun fading out is resumed rather than restarted,
        // so returning to its row catches it wherever it got to.
        if self.path.len() == depth + 1 && self.path[depth] == index {
            if let Some(transition) = self.submenu_transitions.get_mut(depth + 1) {
                transition.open(now);
            }

            return;
        }

        self.path.truncate(depth);
        self.path.push(index);
        self.submenu_transitions.truncate(depth + 1);

        while self.submenu_transitions.len() <= depth + 1 {
            self.submenu_transitions.push(Transition::new());
        }

        self.submenu_transitions[depth + 1].open(now);
    }

    /// The deepest panel the cursor and keyboard may still reach.
    ///
    /// Panels past this are fading out and must not be hit-tested, or a click
    /// aimed at what replaced them would land on a ghost.
    fn live_depth(&self, now: Instant) -> usize {
        self.submenu_transitions
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, transition)| transition.is_closing(now))
            .map_or(self.path.len(), |(depth, _)| depth - 1)
    }

    /// Drops panels whose fade has finished.
    fn retire_finished_panels(&mut self, now: Instant) {
        let finished = self
            .submenu_transitions
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, transition)| !transition.is_visible(now))
            .map(|(depth, _)| depth);

        if let Some(depth) = finished {
            self.path.truncate(depth - 1);
            self.submenu_transitions.truncate(depth);
        }
    }

    /// Advances the transition of the panel at `depth` and returns its
    /// progress.
    ///
    /// Submenu transitions are created on demand and dropped when the chain
    /// collapses, so reopening a submenu animates it afresh rather than
    /// snapping to the progress it had last time.
    fn advance_panel(&mut self, depth: usize, now: Instant, motion: Motion) -> f32 {
        if depth == 0 {
            return self.transition.progress(now);
        }

        while self.submenu_transitions.len() <= depth {
            let mut transition = Transition::new();
            transition.sync(motion);
            transition.open(now);
            self.submenu_transitions.push(transition);
        }

        let transition = &mut self.submenu_transitions[depth];
        transition.sync(motion);
        transition.progress(now)
    }
}

/// Returns the index of the first row of `items` the keyboard may land on,
/// searching from `from` in the given direction and wrapping around.
///
/// Separators and disabled rows are skipped: they can never be activated, so
/// stopping on one would just cost the user an extra key press.
fn selectable(
    items: &[Item<'_, impl Sized, impl Sized, impl Sized>],
    from: Option<usize>,
    forward: bool,
) -> Option<usize> {
    let count = items.len();

    if count == 0 {
        return None;
    }

    // With nothing highlighted yet, stepping backwards should land on the last
    // row rather than the second one.
    let start = match from {
        Some(index) => index,
        None if forward => count - 1,
        None => 0,
    };

    (1..=count)
        .map(|step| {
            if forward {
                (start + step) % count
            } else {
                (start + count - step % count) % count
            }
        })
        .find(|index| items[*index].is_interactive())
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
            trigger
                .as_widget_mut()
                .layout(trigger_tree, renderer, limits)
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

        {
            let state = tree.state.downcast_mut::<State>();

            // Applied here as well as in `overlay`, because a menu can be
            // opened — by a press, or by the bar handing over — before it has
            // ever produced an overlay to sync in.
            state.transition.sync(self.motion);

            // Drive the transition, and stop asking for frames the moment it
            // settles. A menu sitting there open costs nothing.
            //
            // This is also the one place the frame the rest of the widget reads
            // its visibility from advances, so that a frame gets a single answer
            // however many passes it takes. See [`State::frame`].
            if let Event::Window(window::Event::RedrawRequested(now)) = event {
                let was_showing = state.is_showing();

                state.frame = Some(*now);

                if state.is_animating(*now) {
                    shell.request_redraw();
                }

                // The frame the panels stop showing on is a frame the overlay
                // has to be laid out on again: they are leaving the tree, and
                // the layout cached for them would otherwise be handed to
                // whichever overlay takes their place.
                if was_showing != state.is_showing() {
                    shell.invalidate_layout();
                    shell.request_redraw();
                }
            }
        }

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
                // Must go through `open`, not just set the flag: that is what
                // starts the transition.
                state.open();
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
            Event::Mouse(mouse::Event::CursorMoved { .. })
                | Event::Window(window::Event::Unfocused)
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
        state.transition.sync(self.motion);

        // Panels outlive the logical close so they can animate out.
        //
        // Deliberately the frame's instant and not the clock's: `overlay` is
        // called once to lay the panels out and again to draw them, and the two
        // have to agree. See [`State::frame`].
        if !state.is_showing() {
            return None;
        }

        // Read the transition out before `state` is handed to the overlay.
        let progress = state.progress();
        let is_closing = !state.is_open;

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
            progress,
            motion: self.motion,
            is_closing,
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

/// Creates a [`MenuBar`] from a row of menus.
pub fn menu_bar<'a, Message, Theme, Renderer>(
    menus: Vec<Menu<'a, Message, Theme, Renderer>>,
) -> MenuBar<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font>,
{
    MenuBar::new(menus)
}

/// A row of [`Menu`]s that behave as one menu bar.
///
/// Grouping menus changes two things. Once any menu in the bar is open, moving
/// the cursor onto another menu's trigger opens that one immediately, with no
/// second click — the behaviour every desktop menu bar has. And the left and
/// right arrow keys walk between menus once the keyboard runs out of panels to
/// move within.
///
/// Menus used on their own are unaffected; they simply never hand over.
#[allow(missing_debug_implementations)]
pub struct MenuBar<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: Catalog,
{
    menus: Vec<Menu<'a, Message, Theme, Renderer>>,
    spacing: f32,
    padding: Padding,
}

impl<'a, Message, Theme, Renderer> MenuBar<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font>,
{
    /// Creates a new [`MenuBar`] from a row of menus.
    pub fn new(menus: Vec<Menu<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            menus,
            spacing: 2.0,
            padding: Padding::ZERO,
        }
    }

    /// Sets the space between triggers.
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// Sets the padding around the row of triggers.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Returns the index of the menu that is currently open, if any.
    fn open_index(&self, tree: &Tree) -> Option<usize> {
        tree.children
            .iter()
            .position(|child| child.state.downcast_ref::<State>().is_open)
    }

    /// Closes every menu except the one at `keep`.
    fn close_others(&self, tree: &mut Tree, keep: usize) {
        for (index, child) in tree.children.iter_mut().enumerate() {
            if index != keep {
                child.state.downcast_mut::<State>().close();
            }
        }
    }

    /// Opens the menu at `index`, highlighting its first selectable row.
    ///
    /// Used by keyboard navigation, where landing on a menu with nothing
    /// highlighted would cost an extra key press.
    fn open_for_keyboard(&self, tree: &mut Tree, index: usize) {
        self.close_others(tree, index);

        let Some(child) = tree.children.get_mut(index) else {
            return;
        };

        let state = child.state.downcast_mut::<State>();
        state.open();
        state.highlighted = selectable(&self.menus[index].items, None, true).map(|row| (0, row));
    }

    /// Consumes a pending hand-over request from whichever menu is open.
    fn take_sibling_request(&self, tree: &mut Tree) -> Option<(usize, Sibling)> {
        tree.children
            .iter_mut()
            .enumerate()
            .find_map(|(index, child)| {
                let state = child.state.downcast_mut::<State>();

                state.sibling_request.take().map(|request| (index, request))
            })
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for MenuBar<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font> + 'a,
{
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(
            &mut self
                .menus
                .iter_mut()
                .map(|menu| menu as &mut dyn Widget<Message, Theme, Renderer>)
                .collect::<Vec<_>>(),
        );
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let padding = self.padding;
        let spacing = self.spacing;

        let inner = limits.shrink(padding);

        let mut x = padding.left;
        let mut height: f32 = 0.0;
        let mut triggers = Vec::with_capacity(self.menus.len());

        for (index, (menu, child)) in self
            .menus
            .iter_mut()
            .zip(tree.children.iter_mut())
            .enumerate()
        {
            if index > 0 {
                x += spacing;
            }

            let node = menu.layout(child, renderer, &inner);
            let size = node.size();

            triggers.push(node.move_to(Point::new(x, padding.top)));

            x += size.width;
            height = height.max(size.height);
        }

        Node::with_children(Size::new(x + padding.right, height + padding.y()), triggers)
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
        for ((menu, child), trigger) in self.menus.iter().zip(&tree.children).zip(layout.children())
        {
            menu.draw(child, renderer, theme, style, trigger, cursor, viewport);
        }
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
        // An open menu that ran out of panels to move within asks the bar to
        // hand the keyboard to a neighbour. The request is set during the
        // overlay pass, which runs just before this one.
        if let Some((from, direction)) = self.take_sibling_request(tree) {
            let count = self.menus.len();

            if count > 0 {
                let to = match direction {
                    Sibling::Next => (from + 1) % count,
                    Sibling::Previous => (from + count - 1) % count,
                };

                self.open_for_keyboard(tree, to);

                shell.capture_event();
                shell.invalidate_layout();
                shell.request_redraw();

                return;
            }
        }

        // Once one menu is open, sliding the cursor onto another trigger opens
        // that one instead. This is what makes a bar feel like a bar rather
        // than a row of unrelated buttons.
        if matches!(event, Event::Mouse(mouse::Event::CursorMoved { .. }))
            && let Some(open) = self.open_index(tree)
            && let Some(hovered) = layout
                .children()
                .position(|trigger| cursor.is_over(trigger.bounds()))
            && hovered != open
        {
            self.close_others(tree, hovered);
            tree.children[hovered].state.downcast_mut::<State>().open();

            shell.invalidate_layout();
            shell.request_redraw();
        }

        for ((menu, child), trigger) in self
            .menus
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            menu.update(child, event, trigger, cursor, renderer, shell, viewport);

            if shell.is_event_captured() {
                return;
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
        self.menus
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((menu, child), trigger)| {
                menu.mouse_interaction(child, trigger, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());

        operation.traverse(&mut |operation| {
            for ((menu, child), trigger) in self
                .menus
                .iter_mut()
                .zip(tree.children.iter_mut())
                .zip(layout.children())
            {
                menu.operate(child, trigger, renderer, operation);
            }
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        offset: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let panels: Vec<_> = self
            .menus
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
            .filter_map(|((menu, child), trigger)| {
                menu.overlay(child, trigger, renderer, viewport, offset)
            })
            .collect();

        (!panels.is_empty()).then(|| overlay::Group::with_children(panels).overlay())
    }
}

impl<'a, Message, Theme, Renderer> From<MenuBar<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font> + 'a,
{
    fn from(bar: MenuBar<'a, Message, Theme, Renderer>) -> Self {
        Element::new(bar)
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
    /// How far open the root panel is, from `0.0` to `1.0`.
    progress: f32,
    /// How the panels animate.
    motion: Motion,
    /// Whether the menu is closing, and so should ignore further input.
    is_closing: bool,
    on_toggle: Option<&'a dyn Fn(bool) -> Message>,
    class: &'a Theme::Class<'b>,
}

impl<'a, 'b, Message, Theme, Renderer> Panels<'a, 'b, Message, Theme, Renderer>
where
    Message: Clone,
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

            let row =
                Node::with_children(Size::new(widest, row_height), vec![icon_node, content_node])
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

        // Fading panels are still on screen but are not targets.
        for (depth, panel) in layout
            .children()
            .take(self.state.live_depth(Instant::now()) + 1)
            .enumerate()
        {
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

    /// The depth of the panel under `position`, if any.
    ///
    /// Fading panels are excluded for the same reason [`Panels::row_at`]
    /// excludes them: they are on screen but no longer targets.
    fn panel_at(&self, layout: Layout<'_>, position: Point) -> Option<usize> {
        layout
            .children()
            .take(self.state.live_depth(Instant::now()) + 1)
            .position(|panel| panel.bounds().contains(position))
    }

    /// Returns `true` when the cursor is sweeping towards the submenu that is
    /// already open, and so should not be treated as having left the row that
    /// opened it.
    ///
    /// This is the "safe triangle". A submenu opens beside its parent panel, so
    /// the natural path to its lower rows cuts diagonally across the rows below
    /// the one it came from. Switching on whichever row the cursor happens to
    /// cross would close the panel the user is on their way to — the submenu
    /// would only be reachable by tracing an L, out along its own row and then
    /// down. Treating the triangle swept from the cursor's last position to the
    /// submenu's two facing corners as "still on the parent row" is what makes
    /// the diagonal work.
    ///
    /// The apex moves with the cursor, so the triangle narrows as the submenu
    /// gets closer and a path that turns away falls out of it immediately. The
    /// case that needs [`AIM_TIMEOUT`] is the cursor that stops inside it.
    fn is_aiming_at_submenu(&self, layout: Layout<'_>, position: Point) -> bool {
        let Some(previous) = self.state.last_cursor else {
            return false;
        };

        // Only a cursor still over a panel can be aiming across it. Once it has
        // left the chain entirely there is nothing to protect.
        let Some(depth) = self.panel_at(layout, position) else {
            return false;
        };

        // The submenu at issue is the one this panel opened, not a deeper one:
        // a cursor in panel 1 with 2 and 3 open is heading for 2.
        let submenu = depth + 1;

        if submenu > self.state.path.len() {
            return false;
        }

        let Some(bounds) = layout.children().nth(submenu).map(|panel| panel.bounds()) else {
            return false;
        };

        let side = self
            .state
            .panel_sides
            .get(submenu)
            .copied()
            .unwrap_or(Side::Right);

        anchor::in_safe_corridor(position, previous, bounds, side, AIM_EXTEND)
    }

    /// Moves the highlight within the deepest open panel.
    ///
    /// Stepping onto a row also collapses anything open below it, so the chain
    /// on screen never runs deeper than the row the keyboard is actually on.
    fn step_highlight(&mut self, forward: bool) {
        let depth = self.state.live_depth(Instant::now());

        let Some(items) = panel_items(self.items, &self.state.path, depth) else {
            return;
        };

        // Only a highlight already in this panel is a meaningful starting
        // point; one left behind in a shallower panel is not.
        let from = self
            .state
            .highlighted
            .and_then(|(at, index)| (at == depth).then_some(index));

        if let Some(index) = selectable(items, from, forward) {
            self.state.highlighted = Some((depth, index));
        }
    }

    /// Opens the submenu of the highlighted row, if it has one.
    ///
    /// Returns `true` when a panel was actually opened.
    fn open_highlighted_submenu(&mut self) -> bool {
        let Some((depth, index)) = self.state.highlighted else {
            return false;
        };

        let Some(items) = panel_items(self.items, &self.state.path, depth) else {
            return false;
        };

        let opens = items
            .get(index)
            .is_some_and(|item| item.opens_submenu() && item.is_interactive());

        if !opens {
            return false;
        }

        self.state.open_submenu(depth, index, Instant::now());

        // Land on the first row of the panel that just opened, so the next
        // arrow press continues from somewhere sensible.
        let nested = panel_items(self.items, &self.state.path, depth + 1)
            .and_then(|items| selectable(items, None, true));

        if let Some(first) = nested {
            self.state.highlighted = Some((depth + 1, first));
        }

        true
    }

    /// Activates the highlighted row: opens its submenu, or publishes its
    /// message and closes the menu.
    fn activate_highlighted(&mut self, shell: &mut Shell<'_, Message>) {
        if self.open_highlighted_submenu() {
            return;
        }

        let Some((depth, index)) = self.state.highlighted else {
            return;
        };

        let message = panel_items(self.items, &self.state.path, depth)
            .and_then(|items| items.get(index))
            .filter(|item| item.is_interactive())
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
    }

    /// Handles a key press while the menu is open.
    fn on_key_pressed(&mut self, key: &iced::keyboard::Key, shell: &mut Shell<'_, Message>) {
        use iced::keyboard::{Key, key::Named};

        let Key::Named(named) = key else {
            return;
        };

        match named {
            Named::ArrowDown => self.step_highlight(true),
            Named::ArrowUp => self.step_highlight(false),
            Named::Home => {
                self.state.highlighted = None;
                self.step_highlight(true);
            }
            Named::End => {
                self.state.highlighted = None;
                self.step_highlight(false);
            }
            Named::ArrowRight => {
                if !self.open_highlighted_submenu() {
                    // Nothing to descend into, so the key means "next menu".
                    // Deliberately left uncaptured — see `State`.
                    self.state.sibling_request = Some(Sibling::Next);
                    shell.request_redraw();
                    return;
                }
            }
            Named::ArrowLeft => {
                match self.state.path.last().copied() {
                    Some(index) => {
                        // Step back up onto the row that owned the panel now
                        // fading, rather than losing the highlight entirely.
                        self.state
                            .collapse_to(self.state.path.len() - 1, Instant::now());
                        self.state.highlighted = Some((self.state.path.len() - 1, index));
                    }
                    None => {
                        self.state.sibling_request = Some(Sibling::Previous);
                        shell.request_redraw();
                        return;
                    }
                }
            }
            Named::Enter | Named::Space => self.activate_highlighted(shell),
            Named::Escape => {
                // Escape peels one level off the chain, and closes the menu
                // once only the root panel is left.
                match self.state.path.last().copied() {
                    Some(index) => {
                        self.state
                            .collapse_to(self.state.path.len() - 1, Instant::now());
                        self.state.highlighted = Some((self.state.path.len() - 1, index));
                    }
                    None => {
                        self.state.close();

                        if let Some(on_toggle) = self.on_toggle {
                            shell.publish(on_toggle(false));
                        }
                    }
                }
            }
            _ => return,
        }

        shell.capture_event();
        shell.invalidate_layout();
        shell.request_redraw();
    }

    /// Resolves the panel style with every color scaled by the transition.
    ///
    /// Returned whole rather than faded at each use so that a settled panel
    /// pays nothing beyond the usual style lookup.
    fn faded_style(&self, style: Style, progress: f32) -> Style {
        if progress >= 1.0 {
            return style;
        }

        Style {
            background: animation::fade_background(style.background, progress),
            border: Border {
                color: animation::fade(style.border.color, progress),
                ..style.border
            },
            shadow: Shadow {
                color: animation::fade(style.shadow.color, progress),
                ..style.shadow
            },
            text_color: animation::fade(style.text_color, progress),
            hovered_text_color: animation::fade(style.hovered_text_color, progress),
            disabled_text_color: animation::fade(style.disabled_text_color, progress),
            hovered_item_background: animation::fade_background(
                style.hovered_item_background,
                progress,
            ),
            separator_color: animation::fade(style.separator_color, progress),
            ..style
        }
    }

    /// Returns `true` when the cursor is over any open panel.
    fn is_over_any_panel(&self, layout: Layout<'_>, cursor: mouse::Cursor) -> bool {
        layout
            .children()
            .take(self.state.live_depth(Instant::now()) + 1)
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
        let now = Instant::now();

        self.state.retire_finished_panels(now);
        let mut panels: Vec<Node> = Vec::with_capacity(self.depth());
        let mut progresses: Vec<f32> = Vec::with_capacity(self.depth());
        let mut sides: Vec<Side> = Vec::with_capacity(self.depth());

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

            // Every panel animates on its own clock, so a submenu opened long
            // after its parent still slides in rather than appearing whole.
            // A closing menu drives the whole chain from the root transition
            // instead, so the panels fade out together.
            let progress = if self.is_closing {
                self.progress
            } else {
                self.state.advance_panel(depth, now, self.motion)
            };

            let offset = animation::slide(placed.side, self.motion.slide, progress);

            progresses.push(progress);
            sides.push(placed.side);
            panels.push(panel.move_to(placed.position + offset));
        }

        self.state.panel_progress = progresses;
        self.state.panel_sides = sides;

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
        let style = theme.style(self.class);

        for (depth, panel) in layout.children().take(self.depth()).enumerate() {
            let panel_bounds = panel.bounds();

            // Every color a panel draws is faded by its own transition, so a
            // submenu fades in independently of the parent it opened from.
            // Children that inherit `text_color` come with it; one that sets
            // its own color does not — see `crate::animation`.
            let appearance = self.faded_style(
                style,
                self.state.panel_progress.get(depth).copied().unwrap_or(1.0),
            );

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

                let is_highlighted =
                    self.state.highlighted == Some((depth, index)) || open_row == Some(index);

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

                // The layout can be one frame behind the items — a hover that
                // opens a submenu changes the panel chain before the next
                // layout pass runs. A row whose node does not match the item
                // is simply skipped; the following frame draws it correctly.
                let mut slots = row.children();

                let (Some(icon_layout), Some(content_layout)) = (slots.next(), slots.next()) else {
                    continue;
                };

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
        // A panel on its way out is a picture, not a control: it must not
        // highlight, activate, or swallow the press that follows a dismissal.
        if self.is_closing {
            return;
        }

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Touch(touch::Event::FingerMoved { .. }) => {
                let now = Instant::now();

                // The safe triangle is swept from the previous position, so the
                // apex has to be updated on every move — including the ones
                // that change nothing else.
                let position = cursor.position();
                let aiming = position.is_some_and(|position| {
                    let aiming = self.is_aiming_at_submenu(layout, position);
                    self.state.last_cursor = Some(position);
                    aiming
                });

                if aiming {
                    // Hold the submenu open, but only while the cursor keeps
                    // closing on it. Once the deadline passes, the next move
                    // gives the row underneath the cursor back.
                    //
                    // The deadline is only ever checked on a move, which is the
                    // whole of it: a cursor sitting still is not asking for a
                    // different row, and nothing on screen changes meanwhile.
                    let since = *self.state.aim_since.get_or_insert(now);

                    if now.duration_since(since) < AIM_TIMEOUT {
                        return;
                    }
                } else {
                    self.state.aim_since = None;
                }

                let hovered = self.row_at(layout, cursor);

                if hovered == self.state.highlighted {
                    return;
                }

                self.state.highlighted = hovered;

                if let Some((depth, index)) = hovered {
                    let Some(items) = panel_items(self.items, &self.state.path, depth) else {
                        return;
                    };

                    let opens_submenu = matches!(items.get(index), Some(Item::Submenu { .. }));

                    // Hovering a row closes every panel deeper than it, then
                    // opens its own submenu if it has one. Moving along a chain
                    // of submenus therefore never leaves an orphaned panel
                    // behind.
                    if opens_submenu {
                        self.state.open_submenu(depth, index, now);
                    } else {
                        self.state.collapse_to(depth, now);
                    }

                    // The chain just changed shape, so any deferral was aimed
                    // at a panel that is on its way out.
                    self.state.aim_since = None;
                }

                // The set of panels just changed, so the layout computed for
                // the previous chain no longer describes what is on screen.
                shell.invalidate_layout();
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if !self.is_over_any_panel(layout, cursor) {
                    // A press on the trigger belongs to the trigger, which
                    // toggles. Closing here would hand the widget an
                    // already-closed menu a moment later, and it would dutifully
                    // re-open it — leaving the menu stuck open on every click.
                    if cursor.is_over(self.trigger_bounds) {
                        return;
                    }

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
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                self.on_key_pressed(key, shell);
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
        if !self.is_closing && self.row_at(layout, cursor).is_some() {
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
    use iced::advanced::shell;
    use iced::time::Duration;

    #[derive(Debug, Clone, PartialEq)]
    enum Message {
        Cut,
    }

    fn menu_of<'a>(items: Vec<Item<'a, Message>>) -> Menu<'a, Message> {
        Menu::new(iced::widget::text("Edit"), items)
    }

    /// The bounds every hosted widget is laid out in.
    const HOST_BOUNDS: Size = Size::new(400.0, 400.0);

    /// Drives one menu through frames the way `UserInterface` does.
    ///
    /// Deliberately not a `UserInterface`: what is under test is whether the
    /// widget gives a frame one answer, which is a question about the widget
    /// alone.
    struct Host<'a> {
        element: Element<'a, Message, Theme, ()>,
        tree: Tree,
        node: Node,
    }

    impl<'a> Host<'a> {
        fn new(menu: Menu<'a, Message, Theme, ()>) -> Self {
            let mut element = Element::from(menu);
            let mut tree = Tree::new(element.as_widget());
            element.as_widget_mut().diff(&mut tree);

            let node = element.as_widget_mut().layout(
                &mut tree,
                &(),
                &Limits::new(Size::ZERO, HOST_BOUNDS),
            );

            Self {
                element,
                tree,
                node,
            }
        }

        /// Parked over the trigger, which is where a press has to land to
        /// toggle the menu.
        fn cursor() -> mouse::Cursor {
            mouse::Cursor::Available(Point::new(40.0, 12.0))
        }

        fn drive(&mut self, event: Event) {
            let mut bus = shell::Bus::new();
            let mut shell = Shell::new(&iced::window::Headless, shell::Waker::noop(), &mut bus);

            self.element.as_widget_mut().update(
                &mut self.tree,
                &event,
                Layout::new(&self.node),
                Self::cursor(),
                &(),
                &mut shell,
                &Rectangle::with_size(HOST_BOUNDS),
            );
        }

        fn press(&mut self) {
            self.drive(Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)));
        }

        fn redraw(&mut self) {
            self.drive(Event::Window(window::Event::RedrawRequested(
                Instant::now(),
            )));
        }

        /// Whether the menu would put panels in the overlay tree right now.
        fn has_overlay(&mut self) -> bool {
            self.element
                .as_widget_mut()
                .overlay(
                    &mut self.tree,
                    Layout::new(&self.node),
                    &(),
                    &Rectangle::with_size(HOST_BOUNDS),
                    Vector::ZERO,
                )
                .is_some()
        }
    }

    /// A frame asks for the overlay twice, and the two answers have to match.
    ///
    /// `UserInterface` lays the overlay out once during `update` and caches
    /// that layout; `draw` then rebuilds the overlay tree and hands it the
    /// cached one, and `overlay::Group` pairs the two up by position. A menu
    /// that decided it was showing for the layout and not for the draw takes
    /// its layout node with it, and every sibling overlay — a `tooltip`, say —
    /// is left drawing against a node belonging to something else. So the
    /// answer comes from the frame rather than the clock. See [`State::frame`].
    #[test]
    fn a_closing_menu_answers_the_same_twice_within_one_frame() {
        let mut host = Host::new(Menu::new(
            iced::widget::container(iced::widget::text("Edit"))
                .width(Length::Fixed(80.0))
                .height(Length::Fixed(24.0)),
            vec![item(iced::widget::text("Cut")).on_press(Message::Cut)],
        ));

        // Open it, then close it again. The panels are now animating out, and
        // still showing.
        host.press();
        host.redraw();
        host.press();
        host.redraw();

        let showing_for_the_layout = host.has_overlay();
        assert!(showing_for_the_layout, "still fading out");

        // However long the frame takes to get from its layout to its draw.
        std::thread::sleep(Motion::QUICK.duration + Duration::from_millis(50));

        assert_eq!(
            host.has_overlay(),
            showing_for_the_layout,
            "the panels left the overlay tree without the layout being rebuilt"
        );

        // And the next frame does retire them, rather than pinning them open.
        host.redraw();

        assert!(!host.has_overlay());
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
                    submenu(
                        iced::widget::text("Deeper"),
                        vec![item(iced::widget::text("B"))],
                    ),
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
        let decorated = menu_of(vec![
            item(iced::widget::text("Cut")).icon(iced::widget::text("*")),
        ]);

        assert_eq!(plain.child_trees()[1].children.len(), ITEM_ROWS);
        assert_eq!(decorated.child_trees()[1].children.len(), ITEM_ROWS);
    }

    #[test]
    fn a_toggle_is_a_checkable_entry_and_a_submenu_never_is() {
        let checked: Item<'_, Message> = toggle(iced::widget::text("Wrap"), true);
        let unchecked: Item<'_, Message> = toggle(iced::widget::text("Wrap"), false);
        let plain: Item<'_, Message> = item(iced::widget::text("Cut"));
        let nested: Item<'_, Message> = submenu(
            iced::widget::text("More"),
            vec![item(iced::widget::text("A"))],
        )
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

    /// A collapsing chain keeps its panels on screen to fade them, but they
    /// stop being reachable the moment they start closing.
    #[test]
    fn a_fading_panel_is_still_rendered_but_no_longer_live() {
        let mut state = State::default();
        let now = Instant::now();

        state.open();
        state.open_submenu(0, 1, now);
        state.open_submenu(1, 0, now);

        assert_eq!(state.path.len(), 2);
        assert_eq!(state.live_depth(now), 2);

        state.collapse_to(0, now);

        // Still described by the path, so still laid out and drawn...
        assert_eq!(state.path.len(), 2);
        // ...but no longer hit-tested.
        assert_eq!(state.live_depth(now), 0);
    }

    #[test]
    fn a_finished_fade_drops_the_panel_from_the_chain() {
        let mut state = State::default();
        let now = Instant::now();

        state.open();
        state.open_submenu(0, 1, now);
        state.collapse_to(0, now);

        state.retire_finished_panels(now);
        assert_eq!(state.path.len(), 1, "still fading, so still present");

        state.retire_finished_panels(now + Duration::from_millis(500));
        assert_eq!(state.path.len(), 0, "fade finished, so dropped");
    }

    /// Moving the cursor back onto the row that opened a submenu must not
    /// disturb it. Restarting the transition made the panel flicker.
    #[test]
    fn reopening_the_submenu_already_open_leaves_its_transition_alone() {
        let mut state = State::default();
        let start = Instant::now();

        state.open();
        state.open_submenu(0, 1, start);

        let settled = start + Duration::from_millis(500);
        assert_eq!(state.advance_panel(1, settled, Motion::QUICK), 1.0);

        // The cursor wanders back onto "Level two" while its panel is open.
        state.open_submenu(0, 1, settled);

        assert_eq!(state.path, vec![1]);
        assert_eq!(
            state.advance_panel(1, settled, Motion::QUICK),
            1.0,
            "the panel restarted its fade instead of staying put"
        );
    }

    /// ...but a submenu caught part-way out should come back, not stay closing.
    #[test]
    fn returning_to_a_fading_submenu_reopens_it() {
        let mut state = State::default();
        let start = Instant::now();

        state.open();
        state.open_submenu(0, 1, start);

        let settled = start + Duration::from_millis(500);
        let _ = state.advance_panel(1, settled, Motion::QUICK);

        state.collapse_to(0, settled);
        assert_eq!(state.live_depth(settled), 0);

        state.open_submenu(0, 1, settled);

        assert_eq!(state.path, vec![1]);
        assert_eq!(
            state.live_depth(settled),
            1,
            "the panel stayed closing after the cursor came back"
        );
    }

    /// Replacing a sibling submenu has to be instant: the incoming panel lands
    /// where the outgoing one is, and cross-fading them reads as a smear.
    #[test]
    fn opening_a_sibling_submenu_replaces_it_outright() {
        let mut state = State::default();
        let now = Instant::now();

        state.open();
        state.open_submenu(0, 1, now);
        state.open_submenu(0, 2, now);

        assert_eq!(state.path, vec![2]);
        assert_eq!(state.live_depth(now), 1);
    }

    /// Arrow keys must never park on a row that cannot be activated.
    #[test]
    fn keyboard_stepping_skips_separators_and_disabled_rows() {
        let items: Vec<Item<'_, Message>> = vec![
            item(iced::widget::text("A")),
            separator(),
            item(iced::widget::text("B")).enabled(false),
            item(iced::widget::text("C")),
        ];

        assert_eq!(selectable(&items, Some(0), true), Some(3));
        assert_eq!(selectable(&items, Some(3), false), Some(0));
    }

    #[test]
    fn keyboard_stepping_wraps_around_the_panel() {
        let items: Vec<Item<'_, Message>> =
            vec![item(iced::widget::text("A")), item(iced::widget::text("B"))];

        assert_eq!(selectable(&items, Some(1), true), Some(0));
        assert_eq!(selectable(&items, Some(0), false), Some(1));
    }

    /// With nothing highlighted, Down starts at the top and Up starts at the
    /// bottom — anything else feels like the first key press was swallowed.
    #[test]
    fn keyboard_stepping_from_nothing_enters_at_the_near_end() {
        let items: Vec<Item<'_, Message>> = vec![
            item(iced::widget::text("A")),
            item(iced::widget::text("B")),
            item(iced::widget::text("C")),
        ];

        assert_eq!(selectable(&items, None, true), Some(0));
        assert_eq!(selectable(&items, None, false), Some(2));
    }

    #[test]
    fn keyboard_stepping_gives_up_on_a_panel_with_nothing_to_land_on() {
        let empty: Vec<Item<'_, Message>> = vec![];
        let inert: Vec<Item<'_, Message>> =
            vec![separator(), item(iced::widget::text("A")).enabled(false)];

        assert_eq!(selectable(&empty, None, true), None);
        assert_eq!(selectable(&inert, None, true), None);
    }

    /// The chevron and a trailing checkmark share one gutter, so a row must
    /// never claim both — otherwise they would draw on top of each other.
    #[test]
    fn a_row_never_carries_both_a_chevron_and_a_checkmark() {
        let nested: Item<'_, Message> = submenu(
            iced::widget::text("More"),
            vec![item(iced::widget::text("A"))],
        );
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
                    submenu(
                        iced::widget::text("Deeper"),
                        vec![item(iced::widget::text("B"))],
                    ),
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
