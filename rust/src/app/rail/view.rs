//! Rail controls, thumbnail cards and the non-interactive floating layer.

use super::WIDTH;
use super::layout::{number_position, page_size, paper_bounds};
use super::{FastFill, ImagePicker, Message, Rail, SCROLLBAR_WIDTH};
use crate::app::pages::EditorPage;
use crate::appearance::{self, accent, action_button};
use iced::widget::{canvas, column, container, pick_list, row, text};
use iced::{Color, Element, Fill, Point, Rectangle, Renderer, Size, Theme};
use iced::{Vector, mouse};

pub(in crate::app) fn view(state: &FastFill) -> Element<'_, Message> {
    let enabled = state.image_picker == ImagePicker::Idle
        && state.project.is_idle()
        && !state.confirm_delete_all
        && !state.confirm_delete_page
        && state.dragged_page.is_none();
    let control = |label, message, available| {
        action_button(label, (enabled && available).then_some(message))
            .width(Fill)
    };
    let controls = column![
            row![
                text("Page"),
                pick_list(
                    (1..=state.pages.len()).collect::<Vec<_>>(),
                    Some(
                        state
                            .dragged_page
                            .map_or(state.pages.active_index(), |drag| drag
                                .target,)
                            + 1
                    ),
                    |number| Message::SelectPage(number - 1),
                )
                .width(56),
                text(format!("/ {}", state.pages.len())),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(4),
            control("Add image", Message::AddPage, state.pages.can_add()),
            control(
                "Import PDF",
                Message::ImportPdf,
                state.pages.pdf_capacity() > 0,
            ),
            control("Replace image", Message::AskReplacePage, true),
            control(
                "Edit page",
                Message::PageEdit(crate::app::page_edit::Event::Begin),
                state.image().is_some()
            ),
            control("Delete page", Message::AskDeletePage, true)
                .style(appearance::destructive_action),
            row![
                control(
                    "↑",
                    Message::MovePageLeft,
                    state.pages.active_index() > 0,
                ),
                control(
                    "↓",
                    Message::MovePageRight,
                    state.pages.active_index() + 1 < state.pages.len(),
                ),
            ]
            .spacing(4),
        ];
    #[cfg(all(
        feature = "native-camera",
        any(target_os = "linux", target_os = "windows")
    ))]
    let controls = controls.push(
        control(
            "Camera",
            Message::Camera(crate::app::camera::Event::Begin),
            crate::app::camera::can_open(state),
        )
        .style(appearance::primary_action),
    );
    container(
        controls
            .push(
                // Give the canvas an explicit layer/clip, like the document.
                iced::widget::stack![
                    iced::widget::Space::new().width(Fill).height(Fill),
                    canvas(Rail { app: state }).width(Fill).height(Fill),
                ]
                .width(Fill)
                .height(Fill)
                .clip(true),
            )
            .spacing(6),
    )
    .width(WIDTH)
    .height(Fill)
    .padding(8)
    .style(container::bordered_box)
    .into()
}

pub(super) fn draw(
    rail: &Rail<'_>,
    renderer: &Renderer,
    theme: &Theme,
    bounds: Rectangle,
) -> Vec<canvas::Geometry> {
    let mut frame = canvas::Frame::new(renderer, bounds.size());
    let layout = rail.layout(bounds);
    let motion = rail.app.rail_motion.borrow();
    let local = |card: Rectangle| Rectangle {
        x: card.x - bounds.x,
        y: card.y - bounds.y,
        ..card
    };
    if let Some(drag) = rail.app.dragged_page.filter(|drag| drag.moving) {
        let gap = local(rail.card_bounds(drag.source, bounds));
        let outline = canvas::Path::rounded_rectangle(
            gap.position(),
            gap.size(),
            6.0.into(),
        );
        frame.fill(&outline, accent().scale_alpha(0.12));
        frame.stroke(
            &outline,
            canvas::Stroke::default()
                .with_color(accent())
                .with_width(2.0),
        );
        frame.fill_text(canvas::Text {
            content: "Drop here".into(),
            position: gap.center(),
            align_x: iced::widget::text::Alignment::Center,
            align_y: iced::alignment::Vertical::Center,
            size: 12.0.into(),
            color: theme.palette().text,
            ..canvas::Text::default()
        });
    }
    for (index, page) in rail.app.pages.iter().enumerate() {
        if motion
            .ghost
            .as_ref()
            .is_some_and(|ghost| ghost.id == page.metadata.id)
        {
            continue;
        }
        let Some(slot) = layout.visible_slot(index) else {
            continue;
        };
        let mut card = local(rail.card_bounds(index, bounds));
        let target = layout.top(index);
        card.y += motion.card_y(page.metadata.id, target) - target;
        if card.y + card.height < 0.0 || card.y > bounds.height {
            continue;
        }
        draw_card(
            &mut frame,
            page,
            slot + 1,
            card,
            index == rail.app.pages.active_index(),
            theme,
            1.0,
        );
    }
    let total = layout.total;
    if total > bounds.height && bounds.height > 0.0 {
        let height = rail.scroll_thumb_height(bounds);
        let top = rail.offset(bounds) / (total - bounds.height)
            * (bounds.height - height);
        let width = bounds.width - SCROLLBAR_WIDTH;
        frame.fill_rectangle(
            Point::new(width + 4.0, 0.0),
            Size::new(8.0, bounds.height),
            theme.extended_palette().background.strong.color,
        );
        frame.fill_rectangle(
            Point::new(width + 4.0, top),
            Size::new(8.0, height),
            theme.extended_palette().primary.base.color,
        );
    }
    vec![frame.into_geometry()]
}

fn draw_card(
    frame: &mut canvas::Frame,
    page: &EditorPage,
    number: usize,
    card: Rectangle,
    selected: bool,
    theme: &Theme,
    opacity: f32,
) {
    let palette = theme.extended_palette();
    let pair = if selected {
        palette.primary.weak
    } else {
        palette.background.base
    };
    let outline = canvas::Path::rounded_rectangle(
        card.position(),
        card.size(),
        6.0.into(),
    );
    frame.fill(&outline, pair.color.scale_alpha(opacity));
    frame.stroke(
        &outline,
        canvas::Stroke::default()
            .with_color(
                if selected {
                    accent()
                } else {
                    palette.background.strong.color
                }
                .scale_alpha(opacity),
            )
            .with_width(if selected { 2.0 } else { 1.0 }),
    );
    let paper = paper_bounds(card, page_size(page));
    frame.fill_rectangle(
        paper.position() + Vector::new(1.5, 2.0),
        paper.size(),
        Color::BLACK.scale_alpha(0.18 * opacity),
    );
    frame.fill_rectangle(
        paper.position(),
        paper.size(),
        Color::WHITE.scale_alpha(opacity),
    );
    if let Some(image) = &page.image {
        frame.draw_image(
            paper,
            iced::advanced::image::Image::new(image.display_handle().clone())
                .opacity(opacity),
        );
    }
    frame.fill_text(canvas::Text {
        content: number.to_string(),
        position: number_position(card),
        align_x: iced::widget::text::Alignment::Center,
        align_y: iced::alignment::Vertical::Top,
        size: 13.0.into(),
        color: pair.text.scale_alpha(opacity),
        ..canvas::Text::default()
    });
}

pub(in crate::app) fn overlay(app: &FastFill) -> Option<Element<'_, Message>> {
    if !app.dragged_page.is_some_and(|drag| drag.moving)
        && app.rail_motion.borrow().ghost.is_none()
    {
        return None;
    }
    Some(canvas(Floating { app }).width(Fill).height(Fill).into())
}

struct Floating<'a> {
    app: &'a FastFill,
}

impl canvas::Program<Message> for Floating<'_> {
    type State = ();

    fn draw(
        &self,
        (): &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let motion = self.app.rail_motion.borrow();
        let ghost = self
            .app
            .dragged_page
            .filter(|drag| drag.moving)
            .and_then(|drag| {
                self.app.pages.iter().nth(drag.source).map(|page| {
                    (page.metadata.id, drag.position, drag.origin.size())
                })
            })
            .or_else(|| {
                motion
                    .ghost
                    .as_ref()
                    .map(|ghost| (ghost.id, ghost.position, ghost.size))
            });
        if let Some((id, position, size)) = ghost
            && let Some((index, page)) = self
                .app
                .pages
                .iter()
                .enumerate()
                .find(|(_, page)| page.metadata.id == id)
        {
            let position = position - Vector::new(bounds.x, bounds.y);
            let number = self
                .app
                .dragged_page
                .filter(|drag| drag.moving && drag.source == index)
                .map_or(index + 1, |drag| drag.target + 1);
            draw_card(
                &mut frame,
                page,
                number,
                Rectangle::new(position, size),
                true,
                theme,
                0.82,
            );
        }
        vec![frame.into_geometry()]
    }
}
