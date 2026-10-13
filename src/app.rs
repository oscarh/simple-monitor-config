use crate::monitor_canvas;
use crate::sway;
use crate::types::{Mode, OutputInfo};
use iced::widget::{button, column, container, pick_list, radio, row, text};
use iced::{Element, Length, Subscription};

#[derive(Debug, Clone)]
pub enum Message {
    CheckOutputs,
    ChangeResolution(Mode),
    SetMirror(bool),
    Apply,
    Close,
    CanvasMessage(monitor_canvas::CanvasMessage),
}

struct DragInfo {
    name: String,
    origin: (i32, i32),
    last_swapped: Option<String>,
}

pub struct App {
    outputs: Vec<OutputInfo>,
    selected: Option<usize>,
    mirror_mode: bool,
    saved_positions: Vec<(i32, i32)>,
    status: String,
    drag: Option<DragInfo>,
}

impl Default for App {
    fn default() -> Self {
        let (outputs, status) = match sway::get_outputs() {
            Ok(outputs) => (outputs, String::new()),
            Err(e) => (vec![], format!("Error: {}", e)),
        };
        Self {
            selected: if outputs.is_empty() { None } else { Some(0) },
            outputs,
            mirror_mode: false,
            saved_positions: vec![],
            status,
            drag: None,
        }
    }
}

impl App {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::CheckOutputs => {
                if let Ok(new_outputs) = sway::get_outputs() {
                    let current_names: Vec<&str> =
                        self.outputs.iter().map(|o| o.name.as_str()).collect();
                    let new_names: Vec<&str> =
                        new_outputs.iter().map(|o| o.name.as_str()).collect();
                    if current_names != new_names {
                        self.outputs = new_outputs;
                        self.selected = if self.outputs.is_empty() {
                            None
                        } else {
                            Some(0)
                        };
                        self.mirror_mode = false;
                        self.saved_positions.clear();
                        self.status = "Outputs changed.".to_string();
                    }
                }
            }
            Message::CanvasMessage(monitor_canvas::CanvasMessage::SelectOutput(idx)) => {
                if idx < self.outputs.len() {
                    self.selected = Some(idx);
                    let out = &self.outputs[idx];
                    self.drag = Some(DragInfo {
                        name: out.name.clone(),
                        origin: (out.x, out.y),
                        last_swapped: None,
                    });
                }
            }
            Message::CanvasMessage(monitor_canvas::CanvasMessage::DragMove { name, x, y }) => {
                self.handle_drag_move(&name, x, y);
            }
            Message::CanvasMessage(monitor_canvas::CanvasMessage::DragEnd { name, x, y }) => {
                if let Some(idx) = self.outputs.iter().position(|o| o.name == name) {
                    self.outputs[idx].x = x;
                    self.outputs[idx].y = y;
                    self.snap_output(idx);
                    self.selected = Some(idx);
                }
                self.drag = None;
            }
            Message::ChangeResolution(mode) => {
                if let Some(sel) = self.selected {
                    if sel < self.outputs.len() {
                        let old_w = self.outputs[sel].width;
                        let old_h = self.outputs[sel].height;
                        self.outputs[sel].width = mode.width;
                        self.outputs[sel].height = mode.height;
                        self.outputs[sel].current_mode = mode;
                        self.adjust_neighbors(sel, old_w, old_h);
                    }
                }
            }
            Message::SetMirror(mirror) => {
                if mirror && !self.mirror_mode {
                    self.saved_positions = self.outputs.iter().map(|o| (o.x, o.y)).collect();
                    self.mirror_mode = true;
                } else if !mirror && self.mirror_mode {
                    for (i, output) in self.outputs.iter_mut().enumerate() {
                        if let Some(&(x, y)) = self.saved_positions.get(i) {
                            output.x = x;
                            output.y = y;
                        }
                    }
                    self.mirror_mode = false;
                    self.saved_positions.clear();
                }
            }
            Message::Close => {
                std::process::exit(0);
            }
            Message::Apply => match sway::apply_config(&self.outputs, self.mirror_mode) {
                Ok(()) => {
                    self.status = "Configuration applied.".to_string();
                    if let Ok(outputs) = sway::get_outputs() {
                        self.outputs = outputs;
                        if let Some(sel) = self.selected {
                            if sel >= self.outputs.len() {
                                self.selected = if self.outputs.is_empty() {
                                    None
                                } else {
                                    Some(0)
                                };
                            }
                        }
                    }
                }
                Err(e) => self.status = format!("Apply failed: {}", e),
            },
        }
    }

    fn handle_drag_move(&mut self, name: &str, drag_x: i32, drag_y: i32) {
        let drag = match &mut self.drag {
            Some(d) if d.name == name => d,
            _ => return,
        };

        let drag_idx = match self.outputs.iter().position(|o| o.name == name) {
            Some(i) => i,
            None => return,
        };

        let drag_w = self.outputs[drag_idx].width;
        let drag_h = self.outputs[drag_idx].height;
        let drag_cx = drag_x + drag_w / 2;
        let drag_cy = drag_y + drag_h / 2;

        let mut swap_target = None;

        for (i, other) in self.outputs.iter().enumerate() {
            if i == drag_idx {
                continue;
            }
            let other_cx = other.x + other.width / 2;
            let other_cy = other.y + other.height / 2;

            let in_x = drag_cx > other.x && drag_cx < other.x + other.width;
            let in_y = drag_cy > other.y && drag_cy < other.y + other.height;

            if in_x && in_y {
                if drag.last_swapped.as_deref() == Some(other.name.as_str()) {
                    // Check if we've crossed their center (commit to the swap)
                    let origin_cx = drag.origin.0 + drag_w / 2;
                    let origin_cy = drag.origin.1 + drag_h / 2;
                    let crossed_x = (origin_cx < other_cx && drag_cx >= other_cx)
                        || (origin_cx > other_cx && drag_cx <= other_cx);
                    let crossed_y = (origin_cy < other_cy && drag_cy >= other_cy)
                        || (origin_cy > other_cy && drag_cy <= other_cy);
                    if !crossed_x && !crossed_y {
                        continue;
                    }
                }
                swap_target = Some(i);
                break;
            }
        }

        if let Some(target_idx) = swap_target {
            let target_pos = (self.outputs[target_idx].x, self.outputs[target_idx].y);
            let target_name = self.outputs[target_idx].name.clone();

            self.outputs[target_idx].x = drag.origin.0;
            self.outputs[target_idx].y = drag.origin.1;

            drag.origin = target_pos;
            drag.last_swapped = Some(target_name);

            // Update selected to follow dragged monitor
            if let Some(new_idx) = self.outputs.iter().position(|o| o.name == name) {
                self.selected = Some(new_idx);
            }
        }
    }

    fn snap_output(&mut self, idx: usize) {
        let threshold = 300;
        let out = &self.outputs[idx];
        let (mut best_x, mut best_y) = (out.x, out.y);
        let (mut min_dx, mut min_dy) = (threshold + 1, threshold + 1);
        let (ox, oy, ow, oh) = (out.x, out.y, out.width, out.height);

        for (i, other) in self.outputs.iter().enumerate() {
            if i == idx {
                continue;
            }
            let (ax, ay, aw, ah) = (other.x, other.y, other.width, other.height);

            // X: edge-to-edge snapping
            for snap_x in [ax + aw, ax - ow] {
                let d = (ox - snap_x).abs();
                if d < min_dx {
                    min_dx = d;
                    best_x = snap_x;
                }
            }
            // X: center-to-center alignment
            let snap_x = ax + aw / 2 - ow / 2;
            let d = (ox - snap_x).abs();
            if d < min_dx {
                min_dx = d;
                best_x = snap_x;
            }

            // Y: top, center, bottom alignment + above/below stacking
            for snap_y in [
                ay + ah - oh,         // bottom-aligned
                ay,                   // top-aligned
                ay + ah / 2 - oh / 2, // center-aligned
                ay + ah,              // stacked below
                ay - oh,              // stacked above
            ] {
                let d = (oy - snap_y).abs();
                if d < min_dy {
                    min_dy = d;
                    best_y = snap_y;
                }
            }
        }

        self.outputs[idx].x = best_x;
        self.outputs[idx].y = best_y;
    }

    fn adjust_neighbors(&mut self, idx: usize, old_w: i32, old_h: i32) {
        let out = &self.outputs[idx];
        let (ox, oy) = (out.x, out.y);
        let (new_w, new_h) = (out.width, out.height);

        for i in 0..self.outputs.len() {
            if i == idx {
                continue;
            }
            let other = &mut self.outputs[i];

            // Touching right edge: shift horizontally
            if other.x == ox + old_w {
                other.x = ox + new_w;
            }
            // Touching left edge: shift so right edge stays at ox
            if other.x + other.width == ox {
                // no change needed, left neighbor stays put
            }

            // Bottom-aligned: re-align
            if other.y + other.height == oy + old_h {
                other.y = oy + new_h - other.height;
            }
            // Top-aligned: stays the same (both at same y)

            // Touching below: shift vertically
            if other.y == oy + old_h {
                other.y = oy + new_h;
            }
            // Touching above: no change needed
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run(|| {
            iced::stream::channel(
                1,
                |mut sender: iced::futures::channel::mpsc::Sender<Message>| async move {
                    let (tx, mut rx) = iced::futures::channel::mpsc::channel::<()>(1);
                    std::thread::spawn(move || {
                        let conn = match swayipc::Connection::new() {
                            Ok(c) => c,
                            Err(_) => return,
                        };
                        let events = match conn.subscribe([swayipc::EventType::Output]) {
                            Ok(e) => e,
                            Err(_) => return,
                        };
                        let mut tx = tx;
                        for _ in events.flatten() {
                            if tx.try_send(()).is_err() {
                                break;
                            }
                        }
                    });
                    use iced::futures::{SinkExt, StreamExt};
                    while rx.next().await.is_some() {
                        let _ = sender.send(Message::CheckOutputs).await;
                    }
                },
            )
        })
    }

    pub fn view(&self) -> Element<'_, Message> {
        let canvas = monitor_canvas::monitor_canvas(&self.outputs, self.selected)
            .map(Message::CanvasMessage);

        let mirror_controls = row![
            radio("Extend", false, Some(self.mirror_mode), Message::SetMirror).spacing(5),
            radio("Mirror", true, Some(self.mirror_mode), Message::SetMirror).spacing(5),
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center);

        let controls: Element<Message> = if let Some(sel) = self.selected {
            let output = &self.outputs[sel];

            let info = text(format!(
                "{} ({} {})",
                output.name, output.make, output.model
            ))
            .size(16);

            let native = text(match output.native_mode() {
                Some(m) => format!("Native: {}x{} ({})", m.width, m.height, m.aspect_ratio()),
                None => "Native: unknown".to_string(),
            })
            .size(13);

            let modes: Vec<Mode> = if self.mirror_mode {
                common_modes(&self.outputs)
            } else {
                output.modes.clone()
            };

            let resolution_row = row![
                pick_list(
                    modes,
                    Some(output.current_mode.clone()),
                    Message::ChangeResolution,
                )
                .placeholder("Select resolution..."),
                button("Apply").on_press(Message::Apply),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center);

            column![info, native, resolution_row, text(&self.status).size(13)]
                .spacing(10)
                .align_x(iced::Alignment::Center)
                .into()
        } else {
            text("No outputs detected.").size(16).into()
        };

        container(
            column![
                mirror_controls,
                canvas,
                controls,
                button("Close").on_press(Message::Close),
            ]
            .spacing(15)
            .padding(15)
            .align_x(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

fn common_modes(outputs: &[OutputInfo]) -> Vec<Mode> {
    if outputs.is_empty() {
        return vec![];
    }
    outputs[0]
        .modes
        .iter()
        .filter(|mode| {
            outputs[1..].iter().all(|output| {
                output
                    .modes
                    .iter()
                    .any(|m| m.width == mode.width && m.height == mode.height)
            })
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{test_output, Mode};

    fn make_app(outputs: Vec<OutputInfo>) -> App {
        let selected = if outputs.is_empty() { None } else { Some(0) };
        App {
            outputs,
            selected,
            mirror_mode: false,
            saved_positions: vec![],
            status: String::new(),
            drag: None,
        }
    }

    #[test]
    fn snap_edge_to_edge_horizontal() {
        let mut app = make_app(vec![
            test_output("A", 0, 0, 1920, 1080),
            test_output("B", 1950, 0, 1920, 1080),
        ]);
        app.snap_output(1);
        assert_eq!(app.outputs[1].x, 1920);
    }

    #[test]
    fn snap_bottom_aligned() {
        let mut app = make_app(vec![
            test_output("A", 0, 0, 1920, 1080),
            test_output("B", 1920, 50, 2560, 1440),
        ]);
        // B bottom at 50+1440=1490, A bottom at 1080. Closest y snap: top-aligned (0) at dist 50
        app.snap_output(1);
        assert_eq!(app.outputs[1].y, 0);
    }

    #[test]
    fn snap_center_aligned_x() {
        let mut app = make_app(vec![
            test_output("A", 0, 0, 1920, 1080),
            test_output("B", 10, -1080, 1920, 1080),
        ]);
        // B x center should snap to A x center: both 1920 wide, so snap to x=0
        app.snap_output(1);
        assert_eq!(app.outputs[1].x, 0);
    }

    #[test]
    fn snap_no_snap_beyond_threshold() {
        let mut app = make_app(vec![
            test_output("A", 0, 0, 1920, 1080),
            test_output("B", 2500, 500, 1920, 1080),
        ]);
        app.snap_output(1);
        assert_eq!(app.outputs[1].x, 2500);
        assert_eq!(app.outputs[1].y, 500);
    }

    #[test]
    fn live_swap_when_dragged_into_other() {
        let mut app = make_app(vec![
            test_output("A", 0, 0, 1920, 1080),
            test_output("B", 1920, 0, 1920, 1080),
        ]);
        app.drag = Some(DragInfo {
            name: "A".to_string(),
            origin: (0, 0),
            last_swapped: None,
        });

        // Drag A's center into B's bounds
        app.handle_drag_move("A", 1960, 0);

        // B should have moved to A's origin
        assert_eq!(app.outputs[1].x, 0);
        assert_eq!(app.outputs[1].y, 0);
        // drag origin should be B's old position
        assert_eq!(app.drag.as_ref().unwrap().origin, (1920, 0));
    }

    #[test]
    fn common_modes_returns_shared_resolutions() {
        let mode_a = Mode {
            width: 1920,
            height: 1080,
            refresh: 60000,
        };
        let mode_b = Mode {
            width: 2560,
            height: 1440,
            refresh: 60000,
        };

        let mut out1 = test_output("A", 0, 0, 1920, 1080);
        out1.modes = vec![mode_a.clone(), mode_b.clone()];

        let mut out2 = test_output("B", 1920, 0, 2560, 1440);
        out2.modes = vec![mode_a.clone()];

        let common = common_modes(&[out1, out2]);
        assert_eq!(common, vec![mode_a]);
    }

    #[test]
    fn common_modes_empty_outputs() {
        assert_eq!(common_modes(&[]), vec![]);
    }

    #[test]
    fn common_modes_single_output_returns_all() {
        let mut out = test_output("A", 0, 0, 1920, 1080);
        out.modes = vec![
            Mode {
                width: 1920,
                height: 1080,
                refresh: 60000,
            },
            Mode {
                width: 1280,
                height: 720,
                refresh: 60000,
            },
        ];
        let common = common_modes(&[out.clone()]);
        assert_eq!(common, out.modes);
    }

    #[test]
    fn mirror_toggle_saves_and_restores_positions() {
        let mut app = make_app(vec![
            test_output("A", 0, 0, 1920, 1080),
            test_output("B", 1920, 0, 2560, 1080),
        ]);

        app.update(Message::SetMirror(true));
        assert!(app.mirror_mode);
        assert_eq!(app.saved_positions, vec![(0, 0), (1920, 0)]);

        app.update(Message::SetMirror(false));
        assert!(!app.mirror_mode);
        assert_eq!(app.outputs[0].x, 0);
        assert_eq!(app.outputs[1].x, 1920);
    }

    #[test]
    fn change_resolution_adjusts_right_neighbor() {
        // B is snapped to the right edge of A, bottom-aligned
        let mut app = make_app(vec![
            test_output("A", 0, 0, 2560, 1440),
            test_output("B", 2560, 360, 1920, 1080),
        ]);
        app.selected = Some(0);

        let new_mode = Mode {
            width: 1920,
            height: 1080,
            refresh: 60000,
        };
        app.update(Message::ChangeResolution(new_mode));

        // B should shift left to stay touching A's new right edge
        assert_eq!(app.outputs[1].x, 1920);
        // B was bottom-aligned (360+1080=1440=0+1440), should re-align
        assert_eq!(app.outputs[1].y, 0);
    }

    #[test]
    fn change_resolution_preserves_unrelated_neighbor() {
        // B is far away, not touching A
        let mut app = make_app(vec![
            test_output("A", 0, 0, 1920, 1080),
            test_output("B", 3000, 500, 1920, 1080),
        ]);
        app.selected = Some(0);

        let new_mode = Mode {
            width: 2560,
            height: 1440,
            refresh: 60000,
        };
        app.update(Message::ChangeResolution(new_mode));

        // B should not move
        assert_eq!(app.outputs[1].x, 3000);
        assert_eq!(app.outputs[1].y, 500);
    }
}
