/// Wayland + xrdp workaround: detect if MouseMotion reports absolute positions instead of deltas
pub(crate) struct MouseMotionState {
    absolute_mode: bool,
    absolute_mode_detected: bool,
    last_absolute_x: Option<f64>,
    last_absolute_y: Option<f64>,
}

impl MouseMotionState {
    pub(crate) fn new() -> Self {
        Self {
            absolute_mode: false,
            absolute_mode_detected: false,
            last_absolute_x: None,
            last_absolute_y: None,
        }
    }

    pub(crate) fn process_motion(&mut self, delta: (f64, f64)) -> (f64, f64) {
        if !self.absolute_mode_detected {
            // Detection phase: check if these look like absolute positions
            let looks_absolute = (delta.0 > 100.0 && delta.1 > 100.0)
                || (delta.0 > 0.0
                    && delta.1 > 0.0
                    && delta.0 < 10000.0
                    && delta.1 < 10000.0
                    && (delta.0.abs() > 50.0 || delta.1.abs() > 50.0));

            if looks_absolute {
                self.absolute_mode = true;
                self.absolute_mode_detected = true;
                log::warn!(
                    "Detected absolute mouse positioning bug (Wayland+xrdp) - enabling workaround. \
                    First values: ({:.2}, {:.2})",
                    delta.0,
                    delta.1
                );
            } else {
                self.absolute_mode_detected = true;
            }

            if self.absolute_mode {
                self.last_absolute_x = Some(delta.0);
                self.last_absolute_y = Some(delta.1);
                (0.0, 0.0)
            } else {
                delta
            }
        } else if self.absolute_mode {
            let actual_delta = if let (Some(last_x), Some(last_y)) =
                (self.last_absolute_x, self.last_absolute_y)
            {
                (delta.0 - last_x, delta.1 - last_y)
            } else {
                (0.0, 0.0)
            };

            self.last_absolute_x = Some(delta.0);
            self.last_absolute_y = Some(delta.1);
            actual_delta
        } else {
            delta
        }
    }
}
