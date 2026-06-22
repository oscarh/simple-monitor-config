use crate::types::OutputInfo;
use iced::mouse;
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Program, Text};
use iced::widget::Action;
use iced::{Color, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme};

pub struct MonitorCanvas<'a> {
    pub outputs: &'a [OutputInfo],
    pub selected: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum CanvasMessage {
    SelectOutput(usize),
    DragMove { name: String, x: i32, y: i32 },
    DragEnd { name: String, x: i32, y: i32 },
}

#[derive(Debug, Default)]
pub struct CanvasState {
    drag: Option<DragState>,
}

#[derive(Debug)]
struct DragState {
    name: String,
    grab_offset_x: f32,
    grab_offset_y: f32,
    current_x: f32,
    current_y: f32,
    start: Point,
    moved: bool,
}

const DRAG_THRESHOLD: f32 = 4.0;

struct ScaleInfo {
    min_x: i32,
    min_y: i32,
    scale: f32,
    offset_x: f32,
    offset_y: f32,
}

fn compute_scale(outputs: &[OutputInfo], bounds: Rectangle) -> ScaleInfo {
    let min_x = outputs.iter().map(|o| o.x).min().unwrap_or(0);
    let min_y = outputs.iter().map(|o| o.y).min().unwrap_or(0);
    let max_x = outputs.iter().map(|o| o.x + o.width).max().unwrap_or(1);
    let max_y = outputs.iter().map(|o| o.y + o.height).max().unwrap_or(1);

    let total_w = (max_x - min_x) as f32;
    let total_h = (max_y - min_y) as f32;

    let padding = 20.0;
    let available_w = bounds.width - padding * 2.0;
    let available_h = bounds.height - padding * 2.0;

    let scale = (available_w / total_w).min(available_h / total_h);

    let scaled_w = total_w * scale;
    let scaled_h = total_h * scale;

    ScaleInfo {
        min_x,
        min_y,
        scale,
        offset_x: (bounds.width - scaled_w) / 2.0,
        offset_y: (bounds.height - scaled_h) / 2.0,
    }
}

fn output_rect(output: &OutputInfo, si: &ScaleInfo) -> Rectangle {
    let x = si.offset_x + (output.x - si.min_x) as f32 * si.scale;
    let y = si.offset_y + (output.y - si.min_y) as f32 * si.scale;
    let w = output.width as f32 * si.scale;
    let h = output.height as f32 * si.scale;
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

fn draw_monitor(frame: &mut Frame, rect: Rectangle, output: &OutputInfo, is_selected: bool) {
    let fill_color = if is_selected {
        Color::from_rgb(0.2, 0.4, 0.7)
    } else {
        Color::from_rgb(0.3, 0.3, 0.35)
    };

    frame.fill_rectangle(rect.position(), rect.size(), fill_color);

    let border_color = if is_selected {
        Color::from_rgb(0.4, 0.7, 1.0)
    } else {
        Color::from_rgb(0.5, 0.5, 0.55)
    };
    frame.stroke(
        &Path::rectangle(rect.position(), rect.size()),
        canvas::Stroke::default()
            .with_color(border_color)
            .with_width(2.0),
    );

    let cx = rect.x + rect.width / 2.0;
    let cy = rect.y + rect.height / 2.0;

    frame.fill_text(Text {
        content: output.name.clone(),
        position: Point::new(cx, cy - 16.0),
        color: Color::WHITE,
        size: 14.0.into(),
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center,
        ..Text::default()
    });

    if !output.model.is_empty() {
        frame.fill_text(Text {
            content: output.model.clone(),
            position: Point::new(cx, cy),
            color: Color::from_rgb(0.8, 0.8, 0.8),
            size: 11.0.into(),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            ..Text::default()
        });
    }

    frame.fill_text(Text {
        content: format!(
            "{}x{}",
            output.current_mode.width, output.current_mode.height
        ),
        position: Point::new(cx, cy + 16.0),
        color: Color::from_rgb(0.8, 0.8, 0.8),
        size: 11.0.into(),
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center,
        ..Text::default()
    });
}

impl<'a> Program<CanvasMessage> for MonitorCanvas<'a> {
    type State = CanvasState;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        if self.outputs.is_empty() {
            return vec![];
        }

        let mut frame = Frame::new(renderer, bounds.size());
        let si = compute_scale(self.outputs, bounds);
        let dragging_name = state
            .drag
            .as_ref()
            .filter(|d| d.moved)
            .map(|d| d.name.as_str());

        for (i, output) in self.outputs.iter().enumerate() {
            if dragging_name == Some(output.name.as_str()) {
                continue;
            }
            let rect = output_rect(output, &si);
            let is_selected = self.selected == Some(i);
            draw_monitor(&mut frame, rect, output, is_selected);
        }

        if let Some(drag) = state.drag.as_ref().filter(|d| d.moved) {
            if let Some(output) = self.outputs.iter().find(|o| o.name == drag.name) {
                let base_rect = output_rect(output, &si);
                let rect =
                    Rectangle::new(Point::new(drag.current_x, drag.current_y), base_rect.size());
                draw_monitor(&mut frame, rect, output, true);
            }
        }

        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<CanvasMessage>> {
        let cursor_pos = cursor.position_in(bounds);

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor_pos {
                    if !self.outputs.is_empty() {
                        let si = compute_scale(self.outputs, bounds);
                        let start_drag = |i: usize, output: &OutputInfo, pos: Point| {
                            let rect = output_rect(output, &si);
                            (
                                DragState {
                                    name: output.name.clone(),
                                    grab_offset_x: pos.x - rect.x,
                                    grab_offset_y: pos.y - rect.y,
                                    current_x: rect.x,
                                    current_y: rect.y,
                                    start: pos,
                                    moved: false,
                                },
                                i,
                            )
                        };
                        // Prefer the selected monitor when overlapping
                        let hit = self
                            .selected
                            .filter(|&s| s < self.outputs.len())
                            .and_then(|s| {
                                let rect = output_rect(&self.outputs[s], &si);
                                rect.contains(pos)
                                    .then(|| start_drag(s, &self.outputs[s], pos))
                            })
                            .or_else(|| {
                                self.outputs.iter().enumerate().find_map(|(i, output)| {
                                    let rect = output_rect(output, &si);
                                    rect.contains(pos).then(|| start_drag(i, output, pos))
                                })
                            });
                        if let Some((drag_state, idx)) = hit {
                            state.drag = Some(drag_state);
                            return Some(Action::publish(CanvasMessage::SelectOutput(idx)));
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let (Some(drag), Some(pos)) = (&mut state.drag, cursor_pos) {
                    let dx = pos.x - drag.start.x;
                    let dy = pos.y - drag.start.y;
                    if !drag.moved && (dx * dx + dy * dy) > DRAG_THRESHOLD * DRAG_THRESHOLD {
                        drag.moved = true;
                    }
                    if drag.moved {
                        drag.current_x = pos.x - drag.grab_offset_x;
                        drag.current_y = pos.y - drag.grab_offset_y;
                        let si = compute_scale(self.outputs, bounds);
                        let real_x = ((drag.current_x - si.offset_x) / si.scale) as i32 + si.min_x;
                        let real_y = ((drag.current_y - si.offset_y) / si.scale) as i32 + si.min_y;
                        return Some(Action::publish(CanvasMessage::DragMove {
                            name: drag.name.clone(),
                            x: real_x,
                            y: real_y,
                        }));
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(drag) = state.drag.take() {
                    if drag.moved {
                        let si = compute_scale(self.outputs, bounds);
                        let real_x = ((drag.current_x - si.offset_x) / si.scale) as i32 + si.min_x;
                        let real_y = ((drag.current_y - si.offset_y) / si.scale) as i32 + si.min_y;
                        return Some(Action::publish(CanvasMessage::DragEnd {
                            name: drag.name,
                            x: real_x,
                            y: real_y,
                        }));
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag.as_ref().is_some_and(|d| d.moved) {
            return mouse::Interaction::Grabbing;
        }
        if let Some(pos) = cursor.position_in(bounds) {
            if !self.outputs.is_empty() {
                let si = compute_scale(self.outputs, bounds);
                for output in self.outputs {
                    let rect = output_rect(output, &si);
                    if rect.contains(pos) {
                        return mouse::Interaction::Grab;
                    }
                }
            }
        }
        mouse::Interaction::default()
    }
}

pub fn monitor_canvas<'a>(
    outputs: &'a [OutputInfo],
    selected: Option<usize>,
) -> Element<'a, CanvasMessage> {
    Canvas::new(MonitorCanvas { outputs, selected })
        .width(Length::Fill)
        .height(250.0)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::test_output;

    fn bounds(w: f32, h: f32) -> Rectangle {
        Rectangle::new(Point::new(0.0, 0.0), Size::new(w, h))
    }

    #[test]
    fn compute_scale_single_output() {
        let outputs = [test_output("A", 0, 0, 1920, 1080)];
        let si = compute_scale(&outputs, bounds(600.0, 300.0));

        assert_eq!(si.min_x, 0);
        assert_eq!(si.min_y, 0);
        assert!(si.scale > 0.0);
        let scaled_w = 1920.0 * si.scale;
        let scaled_h = 1080.0 * si.scale;
        assert!(scaled_w <= 600.0);
        assert!(scaled_h <= 300.0);
    }

    #[test]
    fn compute_scale_centers_horizontally() {
        let outputs = [test_output("A", 0, 0, 100, 300)];
        let si = compute_scale(&outputs, bounds(600.0, 300.0));

        let scaled_w = 100.0 * si.scale;
        let expected_offset_x = (600.0 - scaled_w) / 2.0;
        assert!((si.offset_x - expected_offset_x).abs() < 0.01);
    }

    #[test]
    fn compute_scale_centers_vertically() {
        let outputs = [test_output("A", 0, 0, 600, 100)];
        let si = compute_scale(&outputs, bounds(600.0, 300.0));

        let scaled_h = 100.0 * si.scale;
        let expected_offset_y = (300.0 - scaled_h) / 2.0;
        assert!((si.offset_y - expected_offset_y).abs() < 0.01);
    }

    #[test]
    fn output_rect_positions_correctly() {
        let outputs = [
            test_output("A", 0, 0, 1920, 1080),
            test_output("B", 1920, 0, 2560, 1080),
        ];
        let si = compute_scale(&outputs, bounds(600.0, 300.0));

        let rect_a = output_rect(&outputs[0], &si);
        let rect_b = output_rect(&outputs[1], &si);

        assert!(rect_a.x < rect_b.x);
        assert!((rect_a.x + rect_a.width - rect_b.x).abs() < 0.01);
        assert!((rect_a.y - rect_b.y).abs() < 0.01);
    }

    #[test]
    fn output_rect_fits_within_bounds() {
        let outputs = [
            test_output("A", 0, 0, 3840, 2160),
            test_output("B", 3840, 0, 1920, 1080),
        ];
        let b = bounds(600.0, 300.0);
        let si = compute_scale(&outputs, b);

        for output in &outputs {
            let rect = output_rect(output, &si);
            assert!(rect.x >= 0.0);
            assert!(rect.y >= 0.0);
            assert!(rect.x + rect.width <= b.width + 0.01);
            assert!(rect.y + rect.height <= b.height + 0.01);
        }
    }

    #[test]
    fn coordinate_roundtrip() {
        let outputs = [test_output("A", 500, 200, 1920, 1080)];
        let b = bounds(600.0, 300.0);
        let si = compute_scale(&outputs, b);

        let rect = output_rect(&outputs[0], &si);
        let real_x = ((rect.x - si.offset_x) / si.scale) as i32 + si.min_x;
        let real_y = ((rect.y - si.offset_y) / si.scale) as i32 + si.min_y;

        assert_eq!(real_x, 500);
        assert_eq!(real_y, 200);
    }
}
