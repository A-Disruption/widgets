//! A collapsible container widget with a header and expandable content area.
//! 
//! Supports two modes:
//! 1. Standalone: Self-managing expand/collapse state
//! 2. Group: Wrap multiple collapsibles in `collapsible_group![]` for accordion behavior

use iced::{alignment, Alignment};
use iced::border::{self, Border};
use iced::advanced::Clipboard;
use iced::advanced::layout;
use iced::advanced::Layout;
use iced::advanced::mouse;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::Shell;
use iced::advanced::text;
use iced::advanced::text::Renderer as _;
use iced::time::{Duration, Instant};
use iced::advanced::widget;
use iced::advanced::Widget;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::Text;
use iced::{
    Background, Color, Element, Event, Length, Padding,
    Pixels, Rectangle, Shadow, Size, Vector, Point, window
};

/// Creates a new [`Collapsible`] with the given title and content.
/// 
/// The collapsible will self-manage its expand/collapse state.
/// To create an accordion group, use [`collapsible_group!`].
pub fn collapsible<'a, Message, Theme, Renderer>(
    title: impl Into<String>,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Collapsible<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    Collapsible::new(title, content)
}

/// Macro to create a collapsible group with cleaner syntax.
/// 
/// # Example
/// ```ignore
/// collapsible_group![
///     collapsible("Section 1", content1),
///     collapsible("Section 2", content2),
///     collapsible("Section 3", content3),
/// ]
/// ```
#[macro_export]
macro_rules! collapsible_group {
    ($($item:expr),* $(,)?) => {
        $crate::collapsible::CollapsibleGroup::new(vec![$($item.into()),*])
    };
}

/// The default height of the header.
pub const DEFAULT_HEADER_HEIGHT: f32 = 32.0;

/// A collapsible container with a clickable header and expandable content.
/// 
/// By default, manages its own expand/collapse state internally.
/// Use [`collapsible_group!`] to create accordion behavior.
pub struct Collapsible<
    'a,
    Message,
    Theme = iced::Theme,
    Renderer = iced::Renderer,
> where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    title: String,
    content: Element<'a, Message, Theme, Renderer>,
    on_toggle: Option<Box<dyn Fn(bool) -> Message + 'a>>,
    expand_icon: Option<Element<'a, Message, Theme, Renderer>>,
    collapse_icon: Option<Element<'a, Message, Theme, Renderer>>,
    action_icon: Option<Element<'a, Message, Theme, Renderer>>, // right aligned icon to use as a button
    on_action: Option<Box<dyn Fn() -> Message + 'a>>,   
    width: Length,
    height: Length,
    header_height: f32,
    title_alignment: Alignment,
    header_clickable: bool,
    padding: Padding,
    content_padding: Padding,
    text_size: Option<Pixels>,
    font: Option<Renderer::Font>,
    class: Theme::Class<'a>,
    initially_expanded: bool,
    easing: Easing,
}

impl<'a, Message, Theme, Renderer> Collapsible<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    /// The default padding for the header content.
    pub const DEFAULT_PADDING: Padding = Padding {
        top: 4.0,
        right: 8.0,
        bottom: 4.0,
        left: 8.0,
    };

    /// The default padding for the content area.
    pub const DEFAULT_CONTENT_PADDING: Padding = Padding {
        top: 2.0,
        right: 2.0,
        bottom: 2.0,
        left: 2.0,
    };

    /// The default spacing between icon and title.
    pub const ICON_SPACING: f32 = 8.0;

    /// Creates a new [`Collapsible`] with the given title and content.
    pub fn new(
        title: impl Into<String>,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            on_toggle: None,
            expand_icon: None,
            collapse_icon: None,
            action_icon: None,
            on_action: None,
            width: Length::Fill,
            height: Length::Shrink,
            header_height: DEFAULT_HEADER_HEIGHT,
            title_alignment: Alignment::Start,
            header_clickable: true,
            padding: Self::DEFAULT_PADDING,
            content_padding: Self::DEFAULT_CONTENT_PADDING,
            text_size: None,
            font: None,
            class: Theme::default(),
            initially_expanded: false,
            easing: Easing::Linear,
        }
    }

    /// Sets the message that will be produced when toggled.
    pub fn on_toggle(
        mut self,
        on_toggle: impl Fn(bool) -> Message + 'a,
    ) -> Self {
        self.on_toggle = Some(Box::new(on_toggle));
        self
    }

    /// Sets the callback for when the action icon is clicked.
    pub fn on_action(
        mut self,
        on_action: impl Fn() -> Message + 'a,
    ) -> Self {
        self.on_action = Some(Box::new(on_action));
        self
    }

    /// Sets the width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the header height.
    pub fn header_height(mut self, height: impl Into<Pixels>) -> Self {
        self.header_height = height.into().0;
        self
    }

    /// Sets the title alignment.
    pub fn title_alignment(mut self, alignment: impl Into<Alignment>) -> Self {
        self.title_alignment = alignment.into();
        self
    }

    /// Sets the collapse icon.
    pub fn collapse_icon(
        mut self,
        icon: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.collapse_icon = Some(icon.into());
        self
    }

    /// Sets the expand icon.
    pub fn expand_icon(
        mut self,
        icon: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.expand_icon = Some(icon.into());
        self
    }

    /// Sets the action icon on the right side of the header.
    pub fn action_icon(
        mut self,
        icon: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.action_icon = Some(icon.into());
        self
    }

    /// Sets whether the entire header is clickable.
    pub fn header_clickable(mut self, clickable: bool) -> Self {
        self.header_clickable = clickable;
        self
    }

    /// Sets the padding.
    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the text size.
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// Sets the font.
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Sets the initial expanded state.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.initially_expanded = expanded;
        self
    }

    /// Sets the easing function for animation.
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Sets the style.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class.
    #[cfg(feature = "advanced")]
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    fn child_indices(&self) -> (Option<usize>, Option<usize>, Option<usize>, usize) {
        let mut index = 0;
        let expand_index = if self.expand_icon.is_some() {
            let i = index;
            index += 1;
            Some(i)
        } else {
            None
        };
        
        let collapse_index = if self.collapse_icon.is_some() {
            let i = index;
            index += 1;
            Some(i)
        } else {
            None
        };

        let action_index = if self.action_icon.is_some() {
            let i = index;
            index += 1;
            Some(i)
        } else {
            None
        };
        
        let content_index = index;
        
        (expand_index, collapse_index, action_index, content_index)
    }

}

/// Easing functions for animation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl Easing {
    fn apply(self, t: f32) -> f32 {
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => t * (2.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }
}

/// Internal state for standalone collapsible.
#[derive(Debug, Clone, Copy)]
struct State {
    is_expanded: bool,
    button_is_pressed: bool,
    header_is_hovered: bool,
    raw_animation_progress: f32,
    animation_progress: f32,
    last_update: Option<Instant>,
    header_height: f32,
}

/// Combined state that includes both animation state and text state
struct CombinedState<P> 
where 
    P: iced::advanced::text::Paragraph
{
    animation: State,
    text: widget::text::State<P>,
    icon_text: widget::text::State<P>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            is_expanded: false,
            button_is_pressed: false,
            header_is_hovered: false,
            raw_animation_progress: 0.0,
            animation_progress: 0.0,
            last_update: None,
            header_height: DEFAULT_HEADER_HEIGHT,
        }
    }
}

impl State {
    const ANIMATION_DURATION: f32 = 0.2;

    fn update_animation(&mut self, now: Instant, easing: Easing, target_expanded: bool) -> bool {
        if let Some(last_update) = self.last_update {
            let delta = (now - last_update).as_secs_f32();
            let change = delta / Self::ANIMATION_DURATION;

            if target_expanded {
                self.raw_animation_progress = (self.raw_animation_progress + change).min(1.0);
            } else {
                self.raw_animation_progress = (self.raw_animation_progress - change).max(0.0);
            }

            self.animation_progress = easing.apply(self.raw_animation_progress);
            self.last_update = Some(now);
            
            (target_expanded && self.raw_animation_progress < 1.0)
                || (!target_expanded && self.raw_animation_progress > 0.0)
        } else {
            self.last_update = Some(now);
            self.raw_animation_progress = if target_expanded { 1.0 } else { 0.0 };
            self.animation_progress = easing.apply(self.raw_animation_progress);
            false
        }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Collapsible<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<CombinedState<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        let mut animation_state = State::default();
        animation_state.is_expanded = self.initially_expanded;
        animation_state.raw_animation_progress = if self.initially_expanded { 1.0 } else { 0.0 };
        animation_state.animation_progress = if self.initially_expanded { 1.0 } else { 0.0 };
        animation_state.header_height = self.header_height;
        
        tree::State::new(CombinedState {
            animation: animation_state,
            text: widget::text::State::<Renderer::Paragraph>::default(),
            icon_text: widget::text::State::<Renderer::Paragraph>::default(),
        })
    }

    fn children(&self) -> Vec<Tree> {
        let mut children = vec![];
        
        if let Some(ref expand_icon) = self.expand_icon {
            children.push(Tree::new(expand_icon));
        }
        
        if let Some(ref collapse_icon) = self.collapse_icon {
            children.push(Tree::new(collapse_icon));
        }

        if let Some(ref action_icon) = self.action_icon {
            children.push(Tree::new(action_icon));
        }
        
        children.push(Tree::new(&self.content));
        
        children
    }

    fn diff(&self, tree: &mut Tree) {
        let mut children = vec![];
        
        if let Some(ref expand_icon) = self.expand_icon {
            children.push(expand_icon);
        }
        
        if let Some(ref collapse_icon) = self.collapse_icon {
            children.push(collapse_icon);
        }

        if let Some(ref action_icon) = self.action_icon {
            children.push(action_icon);
        }
        
        children.push(&self.content);
        
        tree.diff_children(&children);
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let combined_state = tree.state.downcast_mut::<CombinedState<Renderer::Paragraph>>();
        let state = &combined_state.animation;
        let limits = limits.width(self.width).height(self.height);

        let icon_node = if self.expand_icon.is_none() && self.collapse_icon.is_none() {
            // Use default text icon
            let arrow = if state.is_expanded { "🠻" } else { "🠺" };
            
            let icon_limits = layout::Limits::new(
                Size::ZERO,
                Size::new(self.header_height, self.header_height),
            );

            widget::text::layout(
                &mut combined_state.icon_text,
                renderer,
                &icon_limits,
                arrow,
                widget::text::Format {
                    width: Length::Shrink,
                    height: Length::Shrink,
                    line_height: text::LineHeight::default(),
                    size: self.text_size,
                    font: self.font,
                    align_x: text::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    shaping: text::Shaping::Advanced,
                    wrapping: text::Wrapping::default(),
                },
            )
        } else {
            // Layout custom icon Element
            let (expand_index, collapse_index, _, _) = self.child_indices();
            
            let (icon_element, icon_tree_index) = if state.is_expanded {
                // When expanded, show collapse icon if available, otherwise expand icon
                if let Some(collapse_idx) = collapse_index {
                    (self.collapse_icon.as_mut().unwrap(), collapse_idx)
                } else {
                    (self.expand_icon.as_mut().unwrap(), expand_index.unwrap())
                }
            } else {
                // When collapsed, show expand icon if available, otherwise collapse icon
                if let Some(expand_idx) = expand_index {
                    (self.expand_icon.as_mut().unwrap(), expand_idx)
                } else {
                    (self.collapse_icon.as_mut().unwrap(), collapse_index.unwrap())
                }
            };
            
            let icon_limits = layout::Limits::new(
                Size::ZERO,
                Size::new(self.header_height, self.header_height),
            );
            
            icon_element.as_widget_mut().layout(
                &mut tree.children[icon_tree_index],
                renderer,
                &icon_limits,
            )
        };

        let icon_size = icon_node.size();

        // Layout action icon if present
        let (_, _, action_index, _) = self.child_indices();
        let action_node_opt = if let Some(ref mut action_icon) = self.action_icon {
            let icon_limits = layout::Limits::new(
                Size::ZERO,
                Size::new(self.header_height, self.header_height),
            );
            
            Some((
                action_icon.as_widget_mut().layout(
                    &mut tree.children[action_index.unwrap()],
                    renderer,
                    &icon_limits,
                ),
                action_index.unwrap(),
            ))
        } else {
            None
        };

        let action_icon_width = action_node_opt
            .as_ref()
            .map(|(node, _)| node.size().width + Self::ICON_SPACING)
            .unwrap_or(0.0);

        // Layout title text after icon
        let title_x = self.header_height + Self::ICON_SPACING;
        let available_title_width = limits.max().width - title_x - self.padding.right - action_icon_width;
        
        let title_limits = layout::Limits::new(
            Size::ZERO,
            Size::new(available_title_width, self.header_height),
        );

        let title_node = widget::text::layout(
            &mut combined_state.text,
            renderer,
            &title_limits,
            &self.title,
            widget::text::Format {
                width: Length::Shrink,
                height: Length::Shrink,
                line_height: text::LineHeight::default(),
                size: self.text_size,
                font: self.font,
                align_x: text::Alignment::Default,
                align_y: alignment::Vertical::Center,
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::default(),
            },
        );

        let title_size = title_node.size();

        // Align icon and title vertically relative to each other
        let (icon_y, title_y) = if icon_size.height > title_size.height {
            (0.0, (icon_size.height - title_size.height) / 2.0)
        } else {
            ((title_size.height - icon_size.height) / 2.0, 0.0)
        };

        // Position icon and title within the header, centering the pair vertically
        let content_height = icon_size.height.max(title_size.height);
        let header_offset = (self.header_height - content_height) / 2.0;

        let positioned_icon = icon_node.move_to(Point::new(
            self.padding.left,
            header_offset + icon_y,
        ));

        // Always create action icon node (zero-sized if not present)
        let positioned_action = if let Some((mut node, _)) = action_node_opt {
            let action_x = limits.max().width - self.padding.right - node.size().width;
            let action_size = node.size();
            let action_y = if action_size.height > title_size.height {
                0.0
            } else {
                (title_size.height - action_size.height) / 2.0
            };
            
            node.move_to(Point::new(action_x, header_offset + action_y))
        } else {
            // Zero-sized placeholder
            layout::Node::new(Size::ZERO)
        };

        let mut positioned_title = if self.title_alignment == Alignment::Center {
            title_node.move_to(Point::new(
                0.0,
                header_offset + title_y,
            ))
        } else {
          title_node.move_to(Point::new(
            title_x,
            header_offset + title_y,
            ))
        };

        // Apply horizontal alignment within available space
        positioned_title.align_mut(
            self.title_alignment,
            Alignment::Center,  // Keep vertical center alignment
            Size::new(available_title_width, content_height),
        );

        // Layout content below header
        let content_limits = limits
            .width(self.width)
            .height(Length::Shrink)
            .shrink(Size::new(
                self.content_padding.horizontal(),
                self.content_padding.vertical(),
            ));

        let (_, _, _, content_index) = self.child_indices();
        let mut content_node = self.content.as_widget_mut().layout(
            &mut tree.children[content_index],
            renderer,
            &content_limits,
        );

        content_node.move_to_mut(Point::new(
            self.content_padding.left,
            self.header_height + self.content_padding.top,
        ));
        
        let full_content_height = content_node.size().height + self.content_padding.vertical();
        let animated_height = full_content_height * state.animation_progress;

        let total_height = self.header_height + animated_height;

        // Return node with icon, title, action_icon, and content as layout children
        layout::Node::with_children(
            Size::new(limits.max().width, total_height),
            vec![positioned_icon, positioned_title, positioned_action, content_node],
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let combined_state = tree.state.downcast_mut::<CombinedState<Renderer::Paragraph>>();
        let state = &mut combined_state.animation;
        let bounds = layout.bounds();
        
        let header_bounds = Rectangle {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: state.header_height,
        };

        // Icon bounds from first layout child
        let mut children = layout.children();
        let icon_layout = children.next().unwrap();
        let _title_layout = children.next();  
        let action_layout = children.next().unwrap();
        let content_layout = children.next();

        let icon_bounds = icon_layout.bounds();
        let action_bounds = action_layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if self.on_action.is_some() && cursor.is_over(action_bounds) {  // ← NEW
                    if let Some(ref on_action) = self.on_action {
                        shell.publish(on_action());
                    }
                } else if (self.header_clickable && cursor.is_over(header_bounds)) 
                    || cursor.is_over(icon_bounds) {
                    // Existing toggle logic
                    state.is_expanded = !state.is_expanded;
                    state.last_update = Some(Instant::now());
                    shell.invalidate_layout();
                    shell.request_redraw();

                    if let Some(ref on_toggle) = self.on_toggle {
                        shell.publish(on_toggle(state.is_expanded));
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let is_over_header = if self.header_clickable {
                    cursor.is_over(header_bounds)
                } else {
                    cursor.is_over(icon_bounds)
                };
                state.header_is_hovered = is_over_header;
                shell.request_redraw();
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                if state.update_animation(*now, self.easing, state.is_expanded) {
                    shell.invalidate_layout();
                    shell.request_redraw();
                }
            }
            _ => {}
        }

        // Forward events to content (third layout child, but content_index tree child)
        let (_, _, _, content_index) = self.child_indices();
        if let Some(content_layout) = content_layout {
            self.content.as_widget_mut().update(
                &mut tree.children[content_index],
                event,
                content_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let combined_state = tree.state.downcast_ref::<CombinedState<Renderer::Paragraph>>();
        let state = &combined_state.animation;
        let bounds = layout.bounds();
        let is_mouse_over = cursor.is_over(bounds);

        let status = if state.button_is_pressed {
            Status::Pressed
        } else if is_mouse_over {
            Status::Hovered
        } else {
            Status::Active
        };

        let style = theme.style(&self.class, status);

        // Draw shadow
        if style.shadow.color.a > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: style.border,
                    shadow: style.shadow,
                    snap: false,
                },
                style.shadow.color,
            );
        }

        let header_bounds = Rectangle {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: state.header_height,
        };

        // Get layout children: icon, title, content
        let mut layout_children = layout.children();
        let icon_layout = layout_children.next().unwrap();
        let title_layout = layout_children.next().unwrap();
        let action_layout = layout_children.next().unwrap();
        let content_layout_opt = layout_children.next();

        let content_bounds = if state.animation_progress > 0.0 {
            content_layout_opt.map(|l| {
                let full_bounds = l.bounds();
                let animated_height = full_bounds.height * state.animation_progress;
                Rectangle {
                    x: bounds.x,
                    y: bounds.y + state.header_height,
                    width: bounds.width,
                    height: animated_height,
                }
            })
        } else {
            None
        };

        let header_border = if state.animation_progress > 0.0 {
            Border {
                radius: border::Radius {
                    top_left: style.border.radius.top_left,
                    top_right: style.border.radius.top_right,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                },
                ..style.border
            }
        } else {
            style.border
        };

        // Draw header background
        if style.header_background.is_some() || header_border.width > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: header_bounds,
                    border: header_border,
                    shadow: Shadow::default(),
                    snap: false,
                },
                style
                    .header_background
                    .unwrap_or(Background::Color(Color::TRANSPARENT)),
            );
        }

        // Draw header shadow
        if state.animation_progress > 0.0 && style.header_shadow.color.a > 0.0 {
            let shadow_bounds = Rectangle {
                x: header_bounds.x,
                y: header_bounds.y + header_bounds.height,
                width: header_bounds.width,
                height: 0.0,
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: shadow_bounds,
                    border: Border::default(),
                    shadow: style.header_shadow,
                    snap: false,
                },
                style.header_shadow.color,
            );
        }

        // Draw content background
        if let Some(content_bounds) = content_bounds {
            let content_border = Border {
                radius: border::Radius {
                    top_left: 0.0,
                    top_right: 0.0,
                    bottom_left: style.border.radius.bottom_left,
                    bottom_right: style.border.radius.bottom_right,
                },
                ..style.border
            };

            if style.content_background.is_some() || content_border.width > 0.0 {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: content_bounds,
                        border: content_border,
                        shadow: Shadow::default(),
                        snap: false,
                    },
                    style
                        .content_background
                        .unwrap_or(Background::Color(Color::TRANSPARENT)),
                );
            }
        }

        let (expand_child, collapse_child, action_child, content_child) = self.child_indices();

        // Draw icon - either default text or custom Element
        if self.expand_icon.is_none() && self.collapse_icon.is_none() {
            // Draw default text icon using icon_text state
            widget::text::draw(
                renderer,
                defaults,
                icon_layout.bounds(),
                combined_state.icon_text.raw(),
                iced::widget::text::Style {
                    color: style.title_text_color,
                },
                viewport,
            );
        } else {
            // Draw custom icon Element
            let (icon_element, icon_tree_index) = if state.is_expanded {
                (self.collapse_icon.as_ref().unwrap(), collapse_child.unwrap())
            } else {
                (self.expand_icon.as_ref().unwrap(), expand_child.unwrap())
            };
            
            icon_element.as_widget().draw(
                &tree.children[icon_tree_index],
                renderer,
                theme,
                defaults,
                icon_layout,
                cursor,
                viewport,
            );
        }

        // Draw title using layout bounds and text state
        let text_color = style.title_text_color.unwrap_or(defaults.text_color);
        
        widget::text::draw(
            renderer,
            defaults,
            title_layout.bounds(),
            combined_state.text.raw(),
            iced::widget::text::Style {
                color: Some(text_color),
            },
            viewport,
        );

        // Draw action icon if present (after drawing title, around line 854)
        if let Some(ref action_icon) = self.action_icon {
            action_icon.as_widget().draw(
                &tree.children[action_child.unwrap()],
                renderer,
                theme,
                defaults,
                action_layout,
                cursor,
                viewport,
            );
        }

        // Draw content
        if state.animation_progress > 0.0 {
            if let Some(content_layout) = content_layout_opt {
                let full_content_height = content_layout.bounds().height;
                let animated_height = full_content_height * state.animation_progress;
                
                let clip_bounds = Rectangle {
                    x: bounds.x,
                    y: bounds.y + self.header_height,
                    width: bounds.width,
                    height: animated_height,
                };

                renderer.with_layer(clip_bounds, |renderer| {
                    self.content.as_widget().draw(
                        &tree.children[content_child],
                        renderer,
                        theme,
                        &renderer::Style {
                            text_color: style
                                .content_text_color
                                .unwrap_or(defaults.text_color),
                        },
                        content_layout,
                        cursor,
                        viewport,
                    );
                });
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
        let combined_state = tree.state.downcast_ref::<CombinedState<Renderer::Paragraph>>();
        let state = &combined_state.animation;
        let bounds = layout.bounds();

        let header_bounds = Rectangle {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: state.header_height,
        };

        // Get icon bounds from first layout child
        let mut children = layout.children();
        let icon_layout = children.next().unwrap();
        let _title_layout = children.next();
        let action_layout = children.next().unwrap();
        let content_layout = children.next();

        let icon_bounds = icon_layout.bounds();
        let action_bounds = action_layout.bounds();

        if self.on_action.is_some() && cursor.is_over(action_bounds) {
            return mouse::Interaction::Pointer;
        }

        let is_over_clickable = if self.header_clickable {
            cursor.is_over(header_bounds)
        } else {
            cursor.is_over(icon_bounds)
        };

        if is_over_clickable && self.on_toggle.is_some() {
            mouse::Interaction::Pointer
        } else if state.animation_progress != 0.0 {
            let (_, _, _, content_index) = self.child_indices();
            if let Some(content_layout) = content_layout {
                self.content.as_widget().mouse_interaction(
                    &tree.children[content_index],
                    content_layout,
                    cursor,
                    viewport,
                    renderer,
                )
            } else {
                mouse::Interaction::default()
            }
        } else {
            mouse::Interaction::default()
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let combined_state = tree.state.downcast_ref::<CombinedState<Renderer::Paragraph>>();
        let state = &combined_state.animation;
        
        if state.animation_progress > 0.0 {
            let (_, _, _, content_index) = self.child_indices();
            let mut children = layout.children();
            let _icon_layout = children.next().unwrap();
            let _title_layout = children.next();  
            let _action_layout = children.next().unwrap();
            let content_layout = children.next();
            
            if let Some(content_layout) = content_layout {
                self.content.as_widget_mut().operate(
                    &mut tree.children[content_index],
                    content_layout,
                    renderer,
                    operation,
                );
            }
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let combined_state = tree.state.downcast_mut::<CombinedState<Renderer::Paragraph>>();
        let state = &mut combined_state.animation;

        if state.animation_progress > 0.0 {
            let (_, _, _, content_index) = self.child_indices();
            let mut children = layout.children();
            let _icon_layout = children.next().unwrap();
            let _title_layout = children.next();  
            let _action_layout = children.next().unwrap();
            let content_layout = children.next();
            
            if let Some(content_layout) = content_layout {
                self.content.as_widget_mut().overlay(
                    &mut tree.children[content_index],
                    content_layout,
                    renderer,
                    viewport,
                    translation,
                )
            } else {
                None
            }
        } else {
            None
        }
    }
}

// ============================================================================
// COLLAPSIBLE GROUP - Accordion Container
// ============================================================================

/// A container that manages multiple collapsibles in accordion mode.
/// Only one collapsible can be expanded at a time.
pub struct CollapsibleGroup<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    items: Vec<Element<'a, Message, Theme, Renderer>>,
    width: Length,
    height: Length,
    spacing: f32,
}

impl<'a, Message, Theme, Renderer> CollapsibleGroup<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    /// Creates a new collapsible group.
    /// 
    /// Use the `collapsible_group!` macro for cleaner syntax.
    pub fn new(items: Vec<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            items,
            width: Length::Fill,
            height: Length::Shrink,
            spacing: 0.0,
        }
    }

    /// Sets the width of the group.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the group.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the spacing between items.
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }
}

/// State for the collapsible group - tracks which item is expanded.
#[derive(Debug, Clone)]
struct GroupState {
    expanded_index: Option<usize>,
}

impl Default for GroupState {
    fn default() -> Self {
        Self {
            expanded_index: None,
        }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for CollapsibleGroup<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<GroupState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(GroupState::default())
    }

    fn children(&self) -> Vec<Tree> {
        self.items.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.items);
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let group_state = tree.state.downcast_mut::<GroupState>();
        let limits = limits.width(self.width).height(self.height);

        let mut nodes = Vec::new();
        let mut y_offset = 0.0;

        let mut header_heights = Vec::new();

        for (index, (item, child_tree)) in self.items.iter_mut()
            .zip(&mut tree.children)
            .enumerate()
        {
            // Access the child's CombinedState and update animation state based on group state
            let child_combined = child_tree.state.downcast_mut::<CombinedState<Renderer::Paragraph>>();
            let child_state = &mut child_combined.animation;
            let should_be_expanded = group_state.expanded_index == Some(index);
            
            // If state changed, trigger animation - always reset timer for simultaneous animations
            if child_state.is_expanded != should_be_expanded {
                child_state.is_expanded = should_be_expanded;
                // Always reset the animation timer so both opening and closing animate together
                child_state.last_update = Some(Instant::now());
            }
            header_heights.push(child_state.header_height);

            let mut node = item.as_widget_mut().layout(
                child_tree,
                renderer,
                &limits,
            );

            node.move_to_mut(Point::new(0.0, y_offset));
            y_offset += node.size().height + self.spacing;
            nodes.push(node);
        }

        let total_height = y_offset - self.spacing.max(0.0);

        layout::Node::with_children(
            Size::new(limits.max().width, total_height.max(0.0)),
            nodes,
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let group_state = tree.state.downcast_mut::<GroupState>();

        // Check if any child was clicked
        for (index, ((item, child_tree), child_layout)) in self.items.iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .enumerate()
        {
            if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
                let child_bounds = child_layout.bounds();
                let child_combined = child_tree.state.downcast_ref::<CombinedState<Renderer::Paragraph>>();
                let child_header_height = child_combined.animation.header_height;
                
                // Check if click is in this child's header area
                if cursor.is_over(child_bounds) {
                    if let Some(pos) = cursor.position() {
                        let relative_y = pos.y - child_bounds.y;
                        
                        // Check if in header area
                        if relative_y < child_header_height {
                            // Toggle: if already expanded, collapse. Otherwise expand this one.
                            if group_state.expanded_index == Some(index) {
                                group_state.expanded_index = None;
                            } else {
                                group_state.expanded_index = Some(index);
                            }
                            
                            // Trigger smooth simultaneous animation for all items
                            shell.invalidate_layout();
                            shell.request_redraw();
                        }
                    }
                }
            }

            // Forward all events to children
            item.as_widget_mut().update(
                child_tree,
                event,
                child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        for ((item, child_tree), child_layout) in self.items.iter()
            .zip(&tree.children)
            .zip(layout.children())
        {
            item.as_widget().draw(
                child_tree,
                renderer,
                theme,
                defaults,
                child_layout,
                cursor,
                viewport,
            );
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
        self.items.iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((item, child_tree), child_layout)| {
                item.as_widget().mouse_interaction(
                    child_tree,
                    child_layout,
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .max()
            .unwrap_or_default()
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        for ((item, child_tree), child_layout) in self.items.iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            item.as_widget_mut().operate(
                child_tree,
                child_layout,
                renderer,
                operation,
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(
            &mut self.items,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Collapsible<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn from(
        collapsible: Collapsible<'a, Message, Theme, Renderer>,
    ) -> Element<'a, Message, Theme, Renderer> {
        Element::new(collapsible)
    }
}

impl<'a, Message, Theme, Renderer> From<CollapsibleGroup<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn from(
        group: CollapsibleGroup<'a, Message, Theme, Renderer>,
    ) -> Element<'a, Message, Theme, Renderer> {
        Element::new(group)
    }
}

/// The possible statuses of a [`Collapsible`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Active,
    Hovered,
    Pressed,
}

/// The appearance of a [`Collapsible`].
#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub title_text_color: Option<Color>,
    pub header_background: Option<Background>,
    pub content_text_color: Option<Color>,
    pub content_background: Option<Background>,
    pub border: Border,
    pub shadow: Shadow,
    pub header_shadow: Shadow,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            title_text_color: None,
            header_background: None,
            content_text_color: None,
            content_background: None,
            border: Border::default(),
            shadow: Shadow::default(),
            header_shadow: Shadow::default(),
        }
    }
}

/// The theme catalog of a [`Collapsible`].
pub trait Catalog {
    type Class<'a>;
    fn default<'a>() -> Self::Class<'a>;
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for iced::Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

pub fn default(theme: &iced::Theme, _status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        title_text_color: Some(palette.background.strong.text),
        header_background: Some(palette.background.strong.color.into()),
        content_text_color: Some(palette.background.weakest.text),
        content_background: Some(palette.background.weakest.color.into()),
        border: border::rounded(4),
        shadow: Shadow::default(),
        header_shadow: Shadow::default(),
    }
}

pub fn primary(theme: &iced::Theme, _status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        title_text_color: Some(palette.primary.weak.text),
        header_background: Some(palette.primary.weak.color.into()),
        content_text_color: Some(palette.primary.base.text),
        content_background: Some(palette.primary.base.color.into()),
        border: iced::border::rounded(8),
        shadow: iced::Shadow::default(),
        header_shadow: iced::Shadow::default(),
    }
}


pub fn success(theme: &iced::Theme, _status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        title_text_color: Some(palette.success.weak.text),
        header_background: Some(palette.success.weak.color.into()),
        content_text_color: Some(palette.success.base.text),
        content_background: Some(palette.success.base.color.into()),
        border: iced::border::rounded(8),
        shadow: iced::Shadow::default(),
        header_shadow: iced::Shadow::default(),
    }
}

pub fn danger(theme: &iced::Theme, _status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        title_text_color: Some(palette.danger.weak.text),
        header_background: Some(palette.danger.weak.color.into()),
        content_text_color: Some(palette.danger.base.text),
        content_background: Some(palette.danger.base.color.into()),
        border: iced::border::rounded(8),
        shadow: iced::Shadow::default(),
        header_shadow: iced::Shadow::default(),
    }
}

pub fn warning(theme: &iced::Theme, _status: Status) -> Style {
    let palette = theme.extended_palette();

    Style {
        title_text_color: Some(palette.warning.weak.text),
        header_background: Some(palette.warning.weak.color.into()),
        content_text_color: Some(palette.warning.base.text),
        content_background: Some(palette.warning.base.color.into()),
        border: iced::border::rounded(8),
        shadow: iced::Shadow::default(),
        header_shadow: iced::Shadow::default(),
    }
}