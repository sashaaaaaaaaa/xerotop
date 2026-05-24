//! Reusable meters. Both redraw only on data push — never on a frame clock,
//! which is the battery-first core.
//!
//! `Graph` is a filled history chart with a non-linear `gamma` (≈0.5 lifts low
//! values so it stays lively like gkrellm/ewwii). `Bar` is a single-level fill
//! (battery / volume / brightness).

use gtk::DrawingArea;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

pub type Rgba = (f64, f64, f64, f64);

#[derive(Clone)]
pub struct Graph {
    pub area: DrawingArea,
    data: Rc<RefCell<VecDeque<f64>>>,
}

impl Graph {
    /// `fixed_max` None = autoscale to the buffer. `gamma` < 1.0 emphasizes low
    /// values (1.0 = linear).
    pub fn new(len: usize, w: i32, h: i32, fixed_max: Option<f64>, gamma: f64, rgba: Rgba) -> Self {
        let area = DrawingArea::new();
        area.add_css_class("graph");
        area.set_content_width(w);
        area.set_content_height(h);
        let data = Rc::new(RefCell::new(VecDeque::from(vec![0.0; len])));

        let d = data.clone();
        let (r, g, b, a) = rgba;
        area.set_draw_func(move |_, cr, w, h| {
            let (w, h) = (w as f64, h as f64);
            let buf = d.borrow();
            let n = buf.len().max(2) as f64;
            let max = fixed_max
                .unwrap_or_else(|| buf.iter().cloned().fold(1.0_f64, f64::max))
                .max(1e-9);
            let yof = |v: f64| h - (v / max).clamp(0.0, 1.0).powf(gamma) * h;

            cr.move_to(0.0, h);
            for (i, v) in buf.iter().enumerate() {
                cr.line_to(w * (i as f64) / (n - 1.0), yof(*v));
            }
            cr.line_to(w, h);
            cr.close_path();
            cr.set_source_rgba(r, g, b, a * 0.35);
            let _ = cr.fill_preserve();
            cr.set_source_rgba(r, g, b, a);
            cr.set_line_width(1.0);
            let _ = cr.stroke();
        });

        Graph { area, data }
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

#[derive(Clone)]
pub struct Bar {
    pub area: DrawingArea,
    value: Rc<Cell<f64>>,
}

impl Bar {
    pub fn new(w: i32, h: i32, max: f64, rgba: Rgba) -> Self {
        let area = DrawingArea::new();
        area.add_css_class("bar-meter");
        area.set_content_width(w);
        area.set_content_height(h);
        let value = Rc::new(Cell::new(0.0));

        let v = value.clone();
        let (r, g, b, a) = rgba;
        area.set_draw_func(move |_, cr, w, h| {
            let (w, h) = (w as f64, h as f64);
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.10);
            cr.rectangle(0.0, 0.0, w, h);
            let _ = cr.fill();
            let frac = (v.get() / max).clamp(0.0, 1.0);
            cr.set_source_rgba(r, g, b, a);
            cr.rectangle(0.0, 0.0, w * frac, h);
            let _ = cr.fill();
        });

        Bar { area, value }
    }

    pub fn set(&self, v: f64) {
        self.value.set(v);
        self.area.queue_draw();
    }
}
