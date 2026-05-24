//! Panels are self-contained meters. Each builder returns a `Panel`: its root
//! widget, a base update interval (seconds), and an `update` closure the central
//! scheduler calls when the panel is due. No per-panel timers.

use crate::config::PanelConfig;
use crate::metrics::{
    Cpu, Disk, Net, battery, brightness, cpu_temp, disk_usage, gpu, mem_percent, volume,
};
use crate::widgets::{Bar, Graph, Rgba};
use gtk::prelude::*;
use gtk::{Box as GtkBox, Label, Orientation};
use std::cell::RefCell;
use std::rc::Rc;

pub struct Panel {
    pub root: gtk::Widget,
    pub interval: u32,
    pub update: Box<dyn Fn()>,
}

const GRAPH_W: i32 = 134;
const GRAPH_H: i32 = 28;
const MINI_H: i32 = 16;
const BAR_H: i32 = 10;
const GAMMA: f64 = 0.5; // lift low values so charts stay lively (ewwii feel)

const GREEN: Rgba = (0.40, 1.0, 0.40, 0.9);
const CYAN: Rgba = (0.40, 0.8, 1.0, 0.9);
const AMBER: Rgba = (1.0, 0.75, 0.30, 0.9);
const RED: Rgba = (1.0, 0.45, 0.40, 0.9);
const VIOLET: Rgba = (0.78, 0.55, 1.0, 0.9);

/// Build a panel from its config, or None for an unknown type.
pub fn build(cfg: &PanelConfig) -> Option<Panel> {
    let iv = cfg.interval.max(1);
    match cfg.kind.as_str() {
        "clock" => Some(clock_panel(iv)),
        "cpu" => Some(metric_panel("CPU", iv, cfg.graph, GREEN, {
            let cpu = Rc::new(RefCell::new(Cpu::new()));
            move || {
                let p = cpu.borrow_mut().sample();
                (format!("{p:.0}%"), p)
            }
        })),
        "mem" => Some(metric_panel("MEM", iv, cfg.graph, CYAN, || {
            let p = mem_percent();
            (format!("{p:.0}%"), p)
        })),
        "temp" => Some(metric_panel("TEMP", iv, cfg.graph, RED, || {
            let t = cpu_temp();
            (format!("{t:.0}\u{00b0}"), t)
        })),
        "gpu" => Some(gpu_panel(iv, cfg.graph)),
        "disk" => Some(disk_panel(iv, cfg.graph)),
        "net" => Some(net_panel(iv, cfg.graph)),
        "bat" | "battery" => Some(bar_panel("BAT", iv, GREEN, || match battery() {
            Some((p, status)) => {
                let arrow = match status.as_str() {
                    "Charging" => " \u{2191}",
                    "Discharging" => " \u{2193}",
                    _ => "",
                };
                (format!("{p:.0}%{arrow}"), p)
            }
            None => ("AC".to_string(), 0.0),
        })),
        "vol" | "volume" => Some(bar_panel("VOL", iv, CYAN, || match volume() {
            Some((_, true)) => ("MUTE".to_string(), 0.0),
            Some((p, false)) => (format!("{p:.0}%"), p),
            None => ("--".to_string(), 0.0),
        })),
        "bri" | "brightness" => Some(bar_panel("BRI", iv, AMBER, || match brightness() {
            Some(p) => (format!("{p:.0}%"), p),
            None => ("--".to_string(), 0.0),
        })),
        other => {
            eprintln!("xerotop: unknown panel type '{other}', skipping");
            None
        }
    }
}

fn panel_box() -> GtkBox {
    let b = GtkBox::new(Orientation::Vertical, 2);
    b.add_css_class("panel");
    b
}

/// "NAME .......... value" header row; returns the value label to update.
fn header(name: &str) -> (GtkBox, Label) {
    let row = GtkBox::new(Orientation::Horizontal, 2);
    let lbl = Label::new(Some(name));
    lbl.add_css_class("label");
    lbl.set_xalign(0.0);
    lbl.set_hexpand(true);
    let val = Label::new(Some("--"));
    val.add_css_class("value");
    val.set_xalign(1.0);
    row.append(&lbl);
    row.append(&val);
    (row, val)
}

fn sub() -> Label {
    let l = Label::new(Some(""));
    l.add_css_class("sub");
    l.set_xalign(0.0);
    l
}

/// 0..100 metric with a filled graph (cpu, mem, temp).
fn metric_panel<F>(name: &str, interval: u32, graph: bool, rgba: Rgba, sampler: F) -> Panel
where
    F: Fn() -> (String, f64) + 'static,
{
    let root = panel_box();
    let (row, val) = header(name);
    root.append(&row);
    let g = graph.then(|| {
        let g = Graph::new(60, GRAPH_W, GRAPH_H, Some(100.0), GAMMA, rgba);
        root.append(&g.area);
        g
    });
    let update = Box::new(move || {
        let (text, pct) = sampler();
        val.set_text(&text);
        if let Some(g) = &g {
            g.push(pct);
        }
    });
    Panel {
        root: root.upcast(),
        interval,
        update,
    }
}

/// Single-level fill bar (battery, volume, brightness).
fn bar_panel<F>(name: &str, interval: u32, rgba: Rgba, sampler: F) -> Panel
where
    F: Fn() -> (String, f64) + 'static,
{
    let root = panel_box();
    let (row, val) = header(name);
    root.append(&row);
    let bar = Bar::new(GRAPH_W, BAR_H, 100.0, rgba);
    root.append(&bar.area);
    let update = Box::new(move || {
        let (text, pct) = sampler();
        val.set_text(&text);
        bar.set(pct);
    });
    Panel {
        root: root.upcast(),
        interval,
        update,
    }
}

fn net_panel(interval: u32, graph: bool) -> Panel {
    let root = panel_box();
    let (row, val) = header("NET");
    root.append(&row);
    let dn = graph.then(|| {
        let g = Graph::new(60, GRAPH_W, MINI_H, None, GAMMA, CYAN);
        root.append(&g.area);
        g
    });
    let up = graph.then(|| {
        let g = Graph::new(60, GRAPH_W, MINI_H, None, GAMMA, AMBER);
        root.append(&g.area);
        g
    });
    let net = Rc::new(RefCell::new(Net::new()));
    let update = Box::new(move || {
        let (down, upl) = net.borrow_mut().sample();
        val.set_text(&format!("\u{2193}{down:.0} \u{2191}{upl:.0}"));
        if let Some(g) = &dn {
            g.push(down);
        }
        if let Some(g) = &up {
            g.push(upl);
        }
    });
    Panel {
        root: root.upcast(),
        interval,
        update,
    }
}

fn gpu_panel(interval: u32, graph: bool) -> Panel {
    let root = panel_box();
    let (row, val) = header("GPU");
    root.append(&row);
    let g = graph.then(|| {
        let g = Graph::new(60, GRAPH_W, GRAPH_H, Some(100.0), GAMMA, VIOLET);
        root.append(&g.area);
        g
    });
    let vram = sub();
    root.append(&vram);
    let update = Box::new(move || match gpu() {
        Some((busy, used, total)) => {
            val.set_text(&format!("{busy:.0}%"));
            vram.set_text(&format!("{used:.1}G/{total:.1}G"));
            if let Some(g) = &g {
                g.push(busy);
            }
        }
        None => val.set_text("--"),
    });
    Panel {
        root: root.upcast(),
        interval,
        update,
    }
}

fn disk_panel(interval: u32, graph: bool) -> Panel {
    let root = panel_box();
    let (row, val) = header("DISK");
    root.append(&row);
    let usage = sub();
    root.append(&usage);
    let rd = graph.then(|| {
        let g = Graph::new(60, GRAPH_W, MINI_H, None, GAMMA, CYAN);
        root.append(&g.area);
        g
    });
    let wr = graph.then(|| {
        let g = Graph::new(60, GRAPH_W, MINI_H, None, GAMMA, AMBER);
        root.append(&g.area);
        g
    });
    let disk = Rc::new(RefCell::new(Disk::new()));
    let update = Box::new(move || {
        if let Some((pct, used, total)) = disk_usage() {
            val.set_text(&format!("{pct:.0}%"));
            usage.set_text(&format!("{used:.0}G/{total:.0}G"));
        }
        let (r, w) = disk.borrow_mut().sample();
        if let Some(g) = &rd {
            g.push(r);
        }
        if let Some(g) = &wr {
            g.push(w);
        }
    });
    Panel {
        root: root.upcast(),
        interval,
        update,
    }
}

fn clock_panel(interval: u32) -> Panel {
    let root = panel_box();
    root.add_css_class("clock");
    let time = Label::new(Some("--:--"));
    time.add_css_class("clock-time");
    let date = Label::new(Some(""));
    date.add_css_class("clock-date");
    root.append(&time);
    root.append(&date);
    let update = Box::new(move || {
        if let Ok(now) = gtk::glib::DateTime::now_local() {
            if let Ok(t) = now.format("%H:%M") {
                time.set_text(&t);
            }
            if let Ok(d) = now.format("%a %d %b") {
                date.set_text(&d);
            }
        }
    });
    Panel {
        root: root.upcast(),
        interval,
        update,
    }
}
