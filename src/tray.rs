//! System tray via StatusNotifier (the `system-tray` crate, same host
//! `wlr-trayd` used). A dedicated thread runs a tokio runtime + the tray host,
//! streams item snapshots to GTK, and takes left-click activate requests back.
//! Right-click DBus menus are a planned follow-up.

use std::collections::HashMap;
use system_tray::client::{ActivateRequest, Client, Event, UpdateEvent};
use system_tray::item::{IconPixmap, StatusNotifierItem};

#[derive(Clone, Default)]
pub struct TrayItem {
    pub id: String, // service address
    pub title: String,
    pub icon_name: Option<String>,
    pub icon_theme_path: Option<String>,
    pub pixmap: Option<(i32, i32, Vec<u8>)>, // (w, h, ARGB32)
}

fn best_pixmap(pix: Option<&Vec<IconPixmap>>) -> Option<(i32, i32, Vec<u8>)> {
    let p = pix?.iter().max_by_key(|p| p.width * p.height)?;
    Some((p.width, p.height, p.pixels.clone()))
}

fn to_item(addr: &str, item: &StatusNotifierItem) -> TrayItem {
    TrayItem {
        id: addr.to_string(),
        title: item.title.clone().unwrap_or_default(),
        icon_name: item.icon_name.clone().filter(|s| !s.is_empty()),
        icon_theme_path: item.icon_theme_path.clone(),
        pixmap: best_pixmap(item.icon_pixmap.as_ref()),
    }
}

fn snapshot(items: &HashMap<String, TrayItem>) -> Vec<TrayItem> {
    let mut v: Vec<TrayItem> = items.values().cloned().collect();
    v.sort_by(|a, b| a.id.cmp(&b.id));
    v
}

/// Spawn the tray host thread; returns (snapshot receiver, activate sender).
pub fn spawn() -> (
    async_channel::Receiver<Vec<TrayItem>>,
    async_channel::Sender<String>,
) {
    let (tx, rx) = async_channel::unbounded();
    let (atx, arx) = async_channel::unbounded::<String>();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("xerotop: tray runtime: {e}");
                return;
            }
        };
        rt.block_on(async move {
            if let Err(e) = run(tx, arx).await {
                eprintln!("xerotop: tray host exited: {e}");
            }
        });
    });
    (rx, atx)
}

async fn run(
    tx: async_channel::Sender<Vec<TrayItem>>,
    arx: async_channel::Receiver<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new().await?;
    let mut sub = client.subscribe();
    let mut items: HashMap<String, TrayItem> = HashMap::new();

    // seed with anything already registered
    {
        let initial = client.items();
        if let Ok(guard) = initial.lock() {
            for (addr, (item, _menu)) in guard.iter() {
                items.insert(addr.clone(), to_item(addr, item));
            }
        }
    }
    let _ = tx.send(snapshot(&items)).await;

    loop {
        tokio::select! {
            ev = sub.recv() => {
                match ev {
                    Ok(Event::Add(addr, item)) => {
                        items.insert(addr.clone(), to_item(&addr, &item));
                    }
                    Ok(Event::Update(addr, update)) => {
                        if let Some(it) = items.get_mut(&addr) {
                            match update {
                                UpdateEvent::Icon { icon_name, icon_pixmap } => {
                                    it.icon_name = icon_name.filter(|s| !s.is_empty());
                                    it.pixmap = best_pixmap(icon_pixmap.as_ref());
                                }
                                UpdateEvent::Title(t) => it.title = t.unwrap_or_default(),
                                _ => {}
                            }
                        }
                    }
                    Ok(Event::Remove(addr)) => { items.remove(&addr); }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
                let _ = tx.send(snapshot(&items)).await;
            }
            req = arx.recv() => {
                match req {
                    Ok(address) => {
                        let _ = client
                            .activate(ActivateRequest::Default { address, x: 0, y: 0 })
                            .await;
                    }
                    Err(_) => break,
                }
            }
        }
    }
    Ok(())
}
