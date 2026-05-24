//! Reusable graph widget: a ring buffer drawn on a DrawingArea. Redraws only
//! when `push` is called (on new data) — never on a frame clock. That decoupling
//! of sample-rate from frame-rate is the battery-first core.

use gtk::prelude::*;
use gtk::DrawingArea;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Clone)]
pub struct Graph {
    pub area: DrawingArea,
    data: Rc<RefCell<VecDeque<f64>>>,
    /// Fixed Y scale (e.g. 100.0 for percentages); None = autoscale to the buffer.
    fixed_max: Option<f64>,
    rgba: (f64, f64, f64, f64),
}

impl Graph {
    pub fn new(len: usize, w: i32, h: i32, fixed_max: Option<f64>, rgba: (f64, f64, f64, f64)) -> Self {
        let area = DrawingArea::new();
        area.add_css_class("graph");
        area.set_content_width(w);
        area.set_content_height(h);

        let data = Rc::new(RefCell::new(VecDeque::from(vec![0.0; len])));

        let g = Graph {
            area,
            data,
            fixed_max,
            rgba,
        };

        let draw_data = g.data.clone();
        let draw_max = g.fixed_max;
        let (r, gr, b, a) = g.rgba;
        g.area.set_draw_func(move |_, cr, w, h| {
            let (w, h) = (w as f64, h as f64);
            let buf = draw_data.borrow();
            let n = buf.len().max(2) as f64;
            let max = draw_max
                .unwrap_or_else(|| buf.iter().cloned().fold(1.0_f64, f64::max))
                .max(1.0);
            cr.set_source_rgba(r, gr, b, a);
            cr.set_line_width(1.0);
            for (i, v) in buf.iter().enumerate() {
                let x = w * (i as f64) / (n - 1.0);
                let y = h - (v / max).clamp(0.0, 1.0) * h;
                if i == 0 {
                    cr.move_to(x, y);
                } else {
                    cr.line_to(x, y);
                }
            }
            let _ = cr.stroke();
        });

        g
    }

    pub fn push(&self, v: f64) {
        {
            let mut buf = self.data.borrow_mut();
            buf.pop_front();
            buf.push_back(v.max(0.0));
        }
        self.area.queue_draw();
    }
}
